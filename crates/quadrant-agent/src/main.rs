// SPDX-License-Identifier: GPL-3.0-only
#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]
//! Resident Quadrant background process.
fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            quadrant_platform::report_startup_error(&error);
            std::process::ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), quadrant_agent::AgentError> {
    let mut bootstrap = false;
    for argument in std::env::args().skip(1) {
        match argument.as_str() {
            "--gui-bootstrap" => bootstrap = true,
            "--background" => {} // Login startup: use the persisted start_hidden setting.
            _ => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "Unknown Agent argument",
                )
                .into());
            }
        }
    }
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .thread_name("quadrant-agent")
        .enable_all()
        .build()?;
    let database = quadrant_platform::PlatformPaths.database_path()?;
    runtime.block_on(async move {
        let agent = tokio::task::spawn_blocking(move || {
            quadrant_agent::Agent::open(&database, quadrant_agent::HostServices::native())
        })
        .await??;
        if let Some(agent) = agent {
            let (_shutdown_sender, shutdown) = tokio::sync::oneshot::channel();
            agent.run_with_startup(shutdown, !bootstrap).await?;
        }
        Ok(())
    })
}
