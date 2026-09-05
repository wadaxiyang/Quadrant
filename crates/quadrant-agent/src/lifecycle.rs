// SPDX-License-Identifier: GPL-3.0-only
//! Event-driven child supervision. Failed launches never cause a respawn loop.

use quadrant_platform::GuiLauncher;
use quadrant_protocol::GuiLaunchMode;
use std::{sync::Arc, time::Duration};
use tokio::{
    task::{AbortHandle, JoinSet},
    time::Instant,
};

pub(crate) struct Lifecycle {
    launcher: Arc<dyn GuiLauncher>,
    children: JoinSet<std::io::Result<()>>,
    starting: Vec<(GuiLaunchMode, Instant, AbortHandle)>,
}

pub(crate) struct LifecycleChange {
    pub event: &'static str,
    pub startup_failed: Option<GuiLaunchMode>,
}

impl Lifecycle {
    pub fn new(launcher: Arc<dyn GuiLauncher>) -> Self {
        Self {
            launcher,
            children: JoinSet::new(),
            starting: Vec::new(),
        }
    }

    pub async fn launch(&mut self, mode: GuiLaunchMode) -> Result<bool, crate::AgentError> {
        if self.starting.iter().any(|(pending, _, _)| {
            *pending == mode || (mode == GuiLaunchMode::QuickAdd && *pending == GuiLaunchMode::Main)
        }) {
            return Ok(false);
        }
        let launcher = self.launcher.clone();
        let process = tokio::task::spawn_blocking(move || match mode {
            GuiLaunchMode::Main => launcher.launch_main(),
            GuiLaunchMode::QuickAdd => launcher.launch_quick_add(),
        })
        .await??;
        let abort = self.children.spawn(process.completion);
        self.starting
            .push((mode, Instant::now() + Duration::from_secs(30), abort));
        Ok(true)
    }

    pub fn connected(&mut self, mode: GuiLaunchMode) {
        // An external GUI may win the handshake. Session negotiation redirects
        // the launched child; its completion is still reaped independently.
        self.starting.retain(|(pending, _, _)| *pending != mode);
    }

    pub async fn changed(&mut self) -> LifecycleChange {
        let deadline = self.starting.iter().map(|(_, deadline, _)| *deadline).min();
        tokio::select! {
            completion = self.children.join_next_with_id(), if !self.children.is_empty() => {
                if let Some(Ok((id, result))) = completion {
                    let startup_failed = self.starting.iter().find(|(_, _, abort)| abort.id() == id).map(|(mode, _, _)| *mode);
                    self.starting.retain(|(_, _, abort)| abort.id() != id);
                    LifecycleChange { event: if result.is_err() { "gui_process_failed" } else { "gui_process_exited" }, startup_failed }
                } else { LifecycleChange { event: "gui_process_cancelled", startup_failed: None } }
            },
            () = async {
                if let Some(deadline) = deadline { tokio::time::sleep_until(deadline).await; }
                else { std::future::pending::<()>().await; }
            } => {
                let expired = self.starting.iter().position(|(_, at, _)| *at <= Instant::now());
                let startup_failed = expired.map(|index| {
                    let (mode, _, abort) = self.starting.remove(index);
                    abort.abort();
                    mode
                });
                LifecycleChange { event: "gui_startup_timed_out", startup_failed }
            }
        }
    }

    pub async fn shutdown(&mut self) {
        if tokio::time::timeout(Duration::from_secs(3), async {
            while self.children.join_next().await.is_some() {}
        })
        .await
        .is_err()
        {
            // ExitGui was already sent. Only our own unresponsive children are
            // terminated; unrelated processes are never found/killed by name.
            self.children.shutdown().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use tokio::sync::{mpsc, oneshot};

    struct Launcher {
        calls: AtomicU32,
        children: mpsc::UnboundedSender<oneshot::Sender<()>>,
    }
    impl GuiLauncher for Launcher {
        fn launch_quick_add(&self) -> std::io::Result<quadrant_platform::GuiProcess> {
            self.launch_main()
        }
        fn launch_main(&self) -> std::io::Result<quadrant_platform::GuiProcess> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let (exit, exited) = oneshot::channel();
            self.children.send(exit).unwrap();
            Ok(quadrant_platform::GuiProcess {
                id: 1, // Deliberately reuse the PID; supervision must use task generations.
                completion: Box::pin(async move {
                    let _ = exited.await;
                    Ok(())
                }),
            })
        }
    }

    #[tokio::test]
    async fn startup_deadlines_are_independent_for_main_and_capture() {
        let (children, mut completions) = mpsc::unbounded_channel();
        let mut lifecycle = Lifecycle::new(Arc::new(Launcher {
            calls: AtomicU32::new(0),
            children,
        }));
        assert!(lifecycle.launch(GuiLaunchMode::QuickAdd).await.unwrap());
        let mut quick = completions.recv().await.unwrap();
        assert!(lifecycle.launch(GuiLaunchMode::Main).await.unwrap());
        let main = completions.recv().await.unwrap();
        lifecycle.connected(GuiLaunchMode::Main);
        assert_eq!(lifecycle.starting.len(), 1);
        lifecycle.starting[0].1 = Instant::now();
        assert_eq!(
            lifecycle.changed().await.startup_failed,
            Some(GuiLaunchMode::QuickAdd)
        );
        quick.closed().await;
        main.send(()).unwrap();
        lifecycle.shutdown().await;
    }

    #[tokio::test]
    async fn launch_coalesces_reaps_crashes_and_allows_explicit_reopen() {
        let (children, mut completions) = mpsc::unbounded_channel();
        let launcher = Arc::new(Launcher {
            calls: AtomicU32::new(0),
            children,
        });
        let mut lifecycle = Lifecycle::new(launcher.clone());
        assert!(lifecycle.launch(GuiLaunchMode::Main).await.unwrap());
        let first = completions.recv().await.unwrap();
        assert!(!lifecycle.launch(GuiLaunchMode::Main).await.unwrap());
        first.send(()).unwrap();
        assert_eq!(lifecycle.changed().await.event, "gui_process_exited");
        assert_eq!(launcher.calls.load(Ordering::SeqCst), 1);
        assert!(lifecycle.launch(GuiLaunchMode::Main).await.unwrap());
        lifecycle.connected(GuiLaunchMode::Main);
        completions.recv().await.unwrap().send(()).unwrap();
        lifecycle.shutdown().await;
        assert!(lifecycle.children.is_empty());
    }

    #[tokio::test]
    async fn late_exit_with_reused_pid_cannot_clear_a_new_pending_launch() {
        let (children, mut completions) = mpsc::unbounded_channel();
        let mut lifecycle = Lifecycle::new(Arc::new(Launcher {
            calls: AtomicU32::new(0),
            children,
        }));
        lifecycle.launch(GuiLaunchMode::Main).await.unwrap();
        let first = completions.recv().await.unwrap();
        lifecycle.connected(GuiLaunchMode::Main);
        assert!(lifecycle.launch(GuiLaunchMode::Main).await.unwrap());
        let second = completions.recv().await.unwrap();
        first.send(()).unwrap();
        assert!(lifecycle.changed().await.startup_failed.is_none());
        assert!(!lifecycle.launch(GuiLaunchMode::Main).await.unwrap());
        second.send(()).unwrap();
        lifecycle.shutdown().await;
    }

    #[tokio::test]
    async fn expired_startup_cancels_child_and_next_request_can_retry() {
        let (children, mut completions) = mpsc::unbounded_channel();
        let mut lifecycle = Lifecycle::new(Arc::new(Launcher {
            calls: AtomicU32::new(0),
            children,
        }));
        lifecycle.launch(GuiLaunchMode::Main).await.unwrap();
        let mut first = completions.recv().await.unwrap();
        lifecycle.starting[0].1 = Instant::now();
        let change = lifecycle.changed().await;
        assert_eq!(change.event, "gui_startup_timed_out");
        assert_eq!(change.startup_failed, Some(GuiLaunchMode::Main));
        first.closed().await;
        lifecycle.changed().await;
        assert!(lifecycle.launch(GuiLaunchMode::Main).await.unwrap());
        completions.recv().await.unwrap().send(()).unwrap();
        lifecycle.shutdown().await;
    }
}
