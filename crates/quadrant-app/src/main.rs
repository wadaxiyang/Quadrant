// SPDX-License-Identifier: GPL-3.0-only
#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]
//! GUI bootstrap: negotiate IPC first, then own only Slint and its transport task.

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            quadrant_platform::report_startup_error(error.as_ref());
            std::process::ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut agent_launched = false;
    for argument in std::env::args().skip(1) {
        match argument.as_str() {
            "--agent-launched" => agent_launched = true,
            _ => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "Unknown GUI argument",
                )
                .into());
            }
        }
    }
    let endpoint = quadrant_platform::AgentEndpoint::for_current_user()?;
    // One transport worker; no application services, timers or blocking SQL work.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .thread_name("quadrant-ipc")
        .enable_all()
        .build()?;
    let mut agent_child = None;
    let connection = runtime.block_on(quadrant_app::ipc::GuiClient::connect_or_start(
        endpoint,
        quadrant_protocol::GuiLaunchMode::Main,
        !agent_launched,
        || {
            agent_child = Some(quadrant_platform::launch_agent()?);
            Ok(())
        },
    ))?;
    let agent_waiter =
        agent_child.map(|mut child| runtime.spawn(async move { child.wait().await }));
    let Some((client, handle)) = connection else {
        return Ok(());
    };
    quadrant_platform::initialize_application_identity()?;
    let shell = quadrant_ui::UiShell::new(
        client.snapshot(),
        env!("CARGO_PKG_VERSION"),
        move |command| handle.submit(command).is_ok(),
    )?;
    let sink = shell.update_sink();
    let (shutdown, stopped) = tokio::sync::oneshot::channel();
    let worker = runtime.spawn(client.run(sink, stopped));
    let result = shell.run();
    // Independent of retained callback handles and valid even after Agent full exit.
    let _ = shutdown.send(());
    runtime.block_on(worker)?;
    if let Some(waiter) = agent_waiter {
        waiter.abort();
        let _ = runtime.block_on(waiter); // Drop the wait, never kill the resident Agent.
    }
    result?;
    Ok(())
}
