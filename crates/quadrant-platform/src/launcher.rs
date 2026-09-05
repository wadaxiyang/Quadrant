// SPDX-License-Identifier: GPL-3.0-only
//! Sibling executable discovery and owned, console-free child processes.

use std::{
    future::Future,
    io,
    path::{Path, PathBuf},
    pin::Pin,
    process::Stdio,
};

/// An owned GUI child. Dropping its completion cancels an unresponsive child.
pub struct GuiProcess {
    /// Kernel process identifier, used only for launch supervision.
    pub id: u32,
    /// Waits and reaps the child on the caller's runtime.
    pub completion: Pin<Box<dyn Future<Output = io::Result<()>> + Send>>,
}

/// Injectable process boundary; IPC sessions remain the activation authority.
pub trait GuiLauncher: Send + Sync {
    /// Starts a Main GUI beside this installation's Agent.
    /// # Errors
    /// Returns a missing executable or process creation failure.
    fn launch_main(&self) -> io::Result<GuiProcess>;
    /// Starts only the capture surface, without a main window.
    /// # Errors
    /// Returns a missing executable or process creation failure.
    fn launch_quick_add(&self) -> io::Result<GuiProcess>;
}

/// Production sibling executable launcher.
pub struct PlatformGuiLauncher;

impl GuiLauncher for PlatformGuiLauncher {
    fn launch_main(&self) -> io::Result<GuiProcess> {
        launch_gui(false)
    }

    fn launch_quick_add(&self) -> io::Result<GuiProcess> {
        launch_gui(true)
    }
}

fn launch_gui(quick_add: bool) -> io::Result<GuiProcess> {
    let path = sibling(&std::env::current_exe()?, &["quadrant", "quadrant-app"])?;
    owned_gui(gui_command(&path, quick_add).spawn()?)
}

fn gui_command(path: &Path, quick_add: bool) -> tokio::process::Command {
    let mut process = command(path);
    // A child must never restart its parent during a concurrent full Exit.
    process.arg("--agent-launched").kill_on_drop(true);
    if quick_add {
        process.arg("--quick-add");
    }
    process
}

fn owned_gui(mut child: tokio::process::Child) -> io::Result<GuiProcess> {
    let id = child
        .id()
        .ok_or_else(|| io::Error::other("GUI has no process ID"))?;
    Ok(GuiProcess {
        id,
        completion: Box::pin(async move {
            let status = child.wait().await?;
            if status.success() {
                Ok(())
            } else {
                Err(io::Error::other("GUI process failed"))
            }
        }),
    })
}

/// Starts the Agent for a user-launched GUI, without requesting another GUI.
/// The caller retains and reaps the handle; dropping it leaves the Agent running.
/// # Errors
/// Returns a missing sibling or OS process creation error.
pub fn launch_agent() -> io::Result<tokio::process::Child> {
    let path = sibling(&std::env::current_exe()?, &["quadrant-agent"])?;
    command(&path).arg("--gui-bootstrap").spawn()
}

fn command(path: &Path) -> tokio::process::Command {
    let mut command = tokio::process::Command::new(path);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(target_os = "windows")]
    command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW; Slint windows remain visible.
    command
}

fn sibling(executable: &Path, names: &[&str]) -> io::Result<PathBuf> {
    let directory = executable
        .parent()
        .ok_or_else(|| io::Error::other("missing executable directory"))?;
    for name in names {
        let path = directory.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
        if path.is_file() {
            return Ok(path);
        }
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!(
            "Quadrant companion executable is missing from {}. Extract the complete package.",
            directory.display()
        ),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_launch_passes_mode_and_parent_protection_as_separate_arguments() {
        let path = Path::new("installation with spaces/quadrant.exe");
        for (quick, expected) in [
            (false, vec!["--agent-launched"]),
            (true, vec!["--agent-launched", "--quick-add"]),
        ] {
            let command = gui_command(path, quick);
            assert_eq!(command.as_std().get_program(), path.as_os_str());
            assert_eq!(command.as_std().get_args().collect::<Vec<_>>(), expected);
        }
    }

    #[test]
    fn child_process_fixture() {
        if std::env::var_os("QUADRANT_LAUNCHER_TEST_CHILD").is_some() {
            std::thread::sleep(std::time::Duration::from_secs(30));
        }
    }

    #[tokio::test]
    async fn native_child_is_reaped_on_completion() {
        let child = command(&std::env::current_exe().unwrap())
            .args(["--exact", "launcher::tests::child_process_fixture"])
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let child = owned_gui(child).unwrap();
        assert!(child.id > 0);
        tokio::time::timeout(std::time::Duration::from_secs(5), child.completion)
            .await
            .unwrap()
            .unwrap();
    }

    #[cfg(target_os = "windows")]
    #[tokio::test]
    async fn cancelling_owned_gui_terminates_only_that_native_child() {
        use windows::Win32::{
            Foundation::{CloseHandle, WAIT_OBJECT_0},
            System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject},
        };
        let child = command(&std::env::current_exe().unwrap())
            .args(["--exact", "launcher::tests::child_process_fixture"])
            .env("QUADRANT_LAUNCHER_TEST_CHILD", "1")
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let child = owned_gui(child).unwrap();
        // SAFETY: request a wait-only handle to the child we just created.
        let handle = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, child.id) }.unwrap();
        drop(child.completion);
        // SAFETY: handle remains owned and valid until the single close below.
        let status = unsafe { WaitForSingleObject(handle, 5_000) };
        // SAFETY: releases this test's sole owned process handle.
        unsafe { CloseHandle(handle) }.unwrap();
        assert_eq!(status, WAIT_OBJECT_0);
    }

    #[test]
    fn discovery_uses_installation_directory_and_packaged_name_first() {
        let directory =
            std::env::temp_dir().join(format!("quadrant-launcher-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let agent = directory.join("quadrant-agent");
        assert!(sibling(&agent, &["missing"]).is_err());
        let development = directory.join(format!("quadrant-app{}", std::env::consts::EXE_SUFFIX));
        let packaged = directory.join(format!("quadrant{}", std::env::consts::EXE_SUFFIX));
        std::fs::write(&development, []).unwrap();
        assert_eq!(
            sibling(&agent, &["quadrant", "quadrant-app"]).unwrap(),
            development
        );
        std::fs::write(&packaged, []).unwrap();
        assert_eq!(
            sibling(&agent, &["quadrant", "quadrant-app"]).unwrap(),
            packaged
        );
        std::fs::remove_dir_all(directory).unwrap();
    }
}
