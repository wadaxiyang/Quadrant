// SPDX-License-Identifier: GPL-3.0-only
//! Event-driven child supervision. Failed launches never cause a respawn loop.

use quadrant_platform::GuiLauncher;
use std::{sync::Arc, time::Duration};
use tokio::{
    task::{AbortHandle, JoinSet},
    time::Instant,
};

pub(crate) struct Lifecycle {
    launcher: Arc<dyn GuiLauncher>,
    children: JoinSet<std::io::Result<()>>,
    starting: Option<(Instant, AbortHandle)>,
}

pub(crate) struct LifecycleChange {
    pub event: &'static str,
    pub startup_failed: bool,
}

impl Lifecycle {
    pub fn new(launcher: Arc<dyn GuiLauncher>) -> Self {
        Self {
            launcher,
            children: JoinSet::new(),
            starting: None,
        }
    }

    pub async fn launch(&mut self) -> Result<bool, crate::AgentError> {
        if self.starting.is_some() {
            return Ok(false);
        }
        let launcher = self.launcher.clone();
        let process = tokio::task::spawn_blocking(move || launcher.launch_main()).await??;
        let abort = self.children.spawn(process.completion);
        self.starting = Some((Instant::now() + Duration::from_secs(30), abort));
        Ok(true)
    }

    pub fn connected(&mut self) {
        // An external GUI may win the handshake. Session negotiation redirects
        // the launched child; its completion is still reaped independently.
        self.starting = None;
    }

    pub async fn changed(&mut self) -> LifecycleChange {
        let deadline = self.starting.as_ref().map(|(deadline, _)| *deadline);
        tokio::select! {
            completion = self.children.join_next_with_id(), if !self.children.is_empty() => {
                if let Some(Ok((id, result))) = completion {
                    let startup_failed = self.starting.as_ref().is_some_and(|(_, abort)| abort.id() == id);
                    if startup_failed {
                        self.starting = None;
                    }
                    LifecycleChange { event: if result.is_err() { "gui_process_failed" } else { "gui_process_exited" }, startup_failed }
                } else { LifecycleChange { event: "gui_process_cancelled", startup_failed: false } }
            },
            () = async {
                if let Some(deadline) = deadline { tokio::time::sleep_until(deadline).await; }
                else { std::future::pending::<()>().await; }
            } => {
                if let Some((_, abort)) = self.starting.take() { abort.abort(); }
                LifecycleChange { event: "gui_startup_timed_out", startup_failed: true }
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
    async fn launch_coalesces_reaps_crashes_and_allows_explicit_reopen() {
        let (children, mut completions) = mpsc::unbounded_channel();
        let launcher = Arc::new(Launcher {
            calls: AtomicU32::new(0),
            children,
        });
        let mut lifecycle = Lifecycle::new(launcher.clone());
        assert!(lifecycle.launch().await.unwrap());
        let first = completions.recv().await.unwrap();
        assert!(!lifecycle.launch().await.unwrap());
        first.send(()).unwrap();
        assert_eq!(lifecycle.changed().await.event, "gui_process_exited");
        assert_eq!(launcher.calls.load(Ordering::SeqCst), 1);
        assert!(lifecycle.launch().await.unwrap());
        lifecycle.connected();
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
        lifecycle.launch().await.unwrap();
        let first = completions.recv().await.unwrap();
        lifecycle.connected();
        assert!(lifecycle.launch().await.unwrap());
        let second = completions.recv().await.unwrap();
        first.send(()).unwrap();
        assert!(!lifecycle.changed().await.startup_failed);
        assert!(!lifecycle.launch().await.unwrap());
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
        lifecycle.launch().await.unwrap();
        let mut first = completions.recv().await.unwrap();
        lifecycle.starting.as_mut().unwrap().0 = Instant::now();
        let change = lifecycle.changed().await;
        assert_eq!(change.event, "gui_startup_timed_out");
        assert!(change.startup_failed);
        first.closed().await;
        lifecycle.changed().await;
        assert!(lifecycle.launch().await.unwrap());
        completions.recv().await.unwrap().send(()).unwrap();
        lifecycle.shutdown().await;
    }
}
