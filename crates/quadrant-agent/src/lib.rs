// SPDX-License-Identifier: GPL-3.0-only
//! Resident application composition, secure local IPC, and ordered shutdown.
//!
//! Phase 2 provides the Agent server. The old GUI entry point is retained until
//! Phase 3 and shares the profile ownership guard, so both cannot own one store.

#![forbid(unsafe_code)]

mod broker;
mod log;
mod services;
mod transport;

use quadrant_application::{
    ApplicationEvent, AutostartService, Clock, DesktopEvent, ExternalOpener, FocusScheduler,
    ReminderDelivery, ReminderScheduler, SettingsRepository, SystemClock,
};
use quadrant_platform::{
    AgentEndpoint, AgentListener, DesktopIntegration, PlatformIntegrationError,
    SingleInstanceCoordinator,
};
use quadrant_protocol::PlatformCapabilities;
use std::{error::Error, path::Path, sync::Arc};
use tokio::sync::{mpsc, oneshot};

/// Errors crossing the Agent startup/runtime boundary without string-based control flow.
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    /// Filesystem/endpoint initialization failure.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// Native identity, ownership, or desktop initialization failure.
    #[error(transparent)]
    Platform(#[from] PlatformIntegrationError),
    /// Storage operation failed.
    #[error(transparent)]
    Repository(#[from] quadrant_application::RepositoryError),
    /// Coherent application execution is unavailable after a panic.
    #[error(transparent)]
    Execution(#[from] quadrant_application::ExecutionGateError),
    /// A required application projection could not be constructed.
    #[error("application projection could not be loaded: {0}")]
    Projection(Box<dyn Error + Send + Sync>),
    /// An owned worker failed while being joined.
    #[error(transparent)]
    Worker(#[from] tokio::task::JoinError),
    /// IPC listener unexpectedly stopped.
    #[error("the local Agent listener stopped unexpectedly")]
    Listener,
}

impl AgentError {
    fn projection(error: impl Error + Send + Sync + 'static) -> Self {
        Self::Projection(Box::new(error))
    }
}

/// Host ports injected at the composition boundary; tests use deterministic adapters.
#[derive(Clone)]
pub struct HostServices {
    /// Authoritative UTC clock.
    pub clock: Arc<dyn Clock>,
    /// Startup registration, reconciled before clients can mutate settings.
    pub autostart: Arc<dyn AutostartService>,
    /// Native reminder delivery, independent of any GUI connection.
    pub reminders: Arc<dyn ReminderDelivery>,
    /// File-manager/browser actions owned by the Agent.
    pub opener: Arc<dyn ExternalOpener>,
    /// Native Focus completion adapter, invoked on a blocking worker.
    pub focus_completed: Arc<dyn Fn() -> Result<(), PlatformIntegrationError> + Send + Sync>,
    /// Enable the native tray/hotkey adapter (false for isolated service tests).
    pub desktop_integration: bool,
}

impl HostServices {
    /// Selects production platform adapters, with no Slint dependency.
    #[must_use]
    pub fn native() -> Self {
        Self {
            clock: Arc::new(SystemClock),
            autostart: Arc::new(quadrant_platform::PlatformAutostartService),
            reminders: Arc::new(quadrant_platform::PlatformNotificationDelivery),
            opener: Arc::new(quadrant_platform::PlatformExternalOpener),
            focus_completed: Arc::new(
                quadrant_platform::PlatformNotificationDelivery::focus_completed,
            ),
            desktop_integration: true,
        }
    }
}

/// A primary Agent with exclusive profile ownership and no presentation resources.
pub struct Agent {
    services: services::Services,
    listener: AgentListener,
    _instance: SingleInstanceCoordinator,
    host: HostServices,
    log: Arc<log::AgentLog>,
    startup_notices: Vec<ApplicationEvent>,
}

impl Agent {
    /// Claims a profile, applies staged restore, opens `SQLite`, and binds secure IPC.
    ///
    /// Call on a blocking startup worker entered into the application runtime.
    /// A secondary invocation returns `None` without opening the database.
    /// # Errors
    /// Returns path, ownership, identity, restore, storage, or endpoint failures.
    pub fn open(database_path: &Path, host: HostServices) -> Result<Option<Self>, AgentError> {
        let instance = SingleInstanceCoordinator::claim(database_path)?;
        if !instance.is_primary() {
            return Ok(None);
        }
        quadrant_platform::initialize_application_identity()?;
        let profile = database_path
            .parent()
            .ok_or_else(|| std::io::Error::other("missing profile directory"))?;
        let log = Arc::new(log::AgentLog::open(profile)?);
        log.event("agent_started");
        let restored = quadrant_storage::apply_pending_restore(database_path)?;
        let services = services::Services::open(database_path, &host)?;
        let mut startup_notices = Vec::new();
        if let Some(restored) = restored {
            log.event("staged_restore_applied");
            let message = restored.recovery_directory.map_or_else(
                || "The staged backup was restored.".to_owned(),
                |path| {
                    format!(
                        "The staged backup was restored. Previous data is in {}.",
                        path.display()
                    )
                },
            );
            startup_notices.push(ApplicationEvent::OperationSucceeded(message));
        }
        let settings = services.store.load_desktop_settings()?;
        if host
            .autostart
            .set_enabled(settings.launch_at_startup, settings.start_hidden)
            .is_err()
        {
            log.event("autostart_reconcile_failed");
            startup_notices.push(services::failure(
                "The saved startup registration could not be refreshed.",
            ));
        }
        services.snapshot(PlatformCapabilities::default())?;
        let endpoint = AgentEndpoint::for_database(database_path)?;
        let listener = instance.bind_agent_listener(&endpoint)?;
        log.event("ipc_listening");
        Ok(Some(Self {
            services,
            listener,
            _instance: instance,
            host,
            log,
            startup_notices,
        }))
    }

    /// Runs the Agent until explicit shutdown or `ExitApplication`, even with no GUI.
    ///
    /// All worker tasks are signaled and joined; caller shutdown-sender destruction
    /// is also a shutdown request. The profile guard outlives every worker/store.
    /// # Errors
    /// Returns unexpected listener or owned worker failures after cleanup.
    pub async fn run(mut self, shutdown: oneshot::Receiver<()>) -> Result<(), AgentError> {
        let (desktop_sender, desktop_receiver) = mpsc::unbounded_channel::<DesktopEvent>();
        let desktop_sink: quadrant_platform::DesktopEventSink = Arc::new(move |event| {
            let _ = desktop_sender.send(event);
        });
        let desktop = if self.host.desktop_integration {
            let sink = desktop_sink.clone();
            if let Ok(desktop) =
                tokio::task::spawn_blocking(move || DesktopIntegration::start(sink)).await?
            {
                Some(desktop)
            } else {
                self.log.event("desktop_integration_unavailable");
                None
            }
        } else {
            None
        };
        // Keep the desktop sender alive even on unsupported/disabled hosts.
        let _desktop_sink = desktop_sink;
        let native = desktop
            .as_ref()
            .map(DesktopIntegration::capabilities)
            .unwrap_or_default();
        let capabilities = PlatformCapabilities {
            autostart: self.host.autostart.is_supported(),
            tray: native.tray,
            global_hotkey: native.global_hotkey,
            native_notifications: native.native_notifications,
            single_instance: true,
        };
        let (background_sender, background_receiver) = mpsc::unbounded_channel();
        let focus_sender = background_sender.clone();
        let events: Arc<dyn Fn(ApplicationEvent) + Send + Sync> = Arc::new(move |event| {
            let _ = focus_sender.send(event);
        });
        let native_reminders = self.host.reminders.clone();
        let delivery: Arc<dyn ReminderDelivery> =
            Arc::new(move |alert: quadrant_application::ReminderAlert| {
                let result = native_reminders.deliver(alert.clone());
                if result.is_ok() {
                    let _ = background_sender.send(ApplicationEvent::ReminderDue(alert));
                }
                // With no GUI, a failed native delivery must never be marked delivered.
                result
            });
        let (reminders, reminder_handle) = ReminderScheduler::new(
            self.services.store.clone(),
            self.host.clock.clone(),
            delivery,
        );
        let reminder_worker = tokio::spawn(
            reminders
                .with_execution_gate(self.services.gate.clone())
                .run(),
        );
        let (focus, focus_handle) = FocusScheduler::new(self.services.focus.clone(), events);
        let focus_worker =
            tokio::spawn(focus.with_execution_gate(self.services.gate.clone()).run());
        let (input_sender, input_receiver) = mpsc::channel(32);
        let (stop_transport, stopped_transport) = oneshot::channel();
        let transport_worker = tokio::spawn(transport::run(
            self.listener,
            input_sender,
            stopped_transport,
        ));
        let broker = broker::Broker::new(
            self.services.clone(),
            capabilities,
            reminder_handle.clone(),
            focus_handle.clone(),
            self.log.clone(),
        )
        .with_startup_notices(std::mem::take(&mut self.startup_notices))
        .with_focus_notification(self.host.focus_completed.clone());
        let result = broker
            .run(
                input_receiver,
                background_receiver,
                desktop_receiver,
                shutdown,
            )
            .await;
        let _ = stop_transport.send(());
        reminder_handle.shutdown();
        focus_handle.shutdown();
        let desktop_result = if let Some(desktop) = desktop {
            tokio::task::spawn_blocking(move || desktop.shutdown()).await
        } else {
            Ok(())
        };
        // Join every owned worker even if another one failed.
        let (transport_result, reminder_result, focus_result) =
            tokio::join!(transport_worker, reminder_worker, focus_worker);
        desktop_result?;
        transport_result?;
        reminder_result?;
        focus_result?;
        self.log.event("agent_stopped");
        result
    }
}
