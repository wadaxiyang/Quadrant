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
    let endpoint = quadrant_platform::AgentEndpoint::for_current_user()?;
    // One transport worker; no application services, timers or blocking SQL work.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .thread_name("quadrant-ipc")
        .enable_all()
        .build()?;
    let Some((client, handle)) = runtime.block_on(quadrant_app::ipc::GuiClient::connect(
        endpoint,
        quadrant_protocol::GuiLaunchMode::Main,
    ))?
    else {
        return Ok(());
    };
    quadrant_platform::initialize_application_identity()?;
    let shell = quadrant_ui::UiShell::new(
        client.snapshot(),
        env!("CARGO_PKG_VERSION"),
        move |intent| handle.submit(intent.into()).is_ok(),
    )?;
    let sink = shell.update_sink();
    let (shutdown, stopped) = tokio::sync::oneshot::channel();
    let worker = runtime.spawn(client.run(sink, stopped));
    let result = shell.run();
    // Independent of retained callback handles and valid even after Agent full exit.
    let _ = shutdown.send(());
    runtime.block_on(worker)?;
    result?;
    Ok(())
}
