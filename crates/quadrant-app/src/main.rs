#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

//! Quadrant composition root.

use std::sync::Arc;

use quadrant_application::{
    ApplicationEvent, AutostartService, Clock, CompletedRepository, FocusApplication,
    FocusRepository, FocusScheduler, FocusSessionIdGenerator, HistoryApplication,
    MaintenanceApplication, MaintenanceRepository, ReminderAlert, ReminderDelivery,
    ReminderDeliveryError, ReminderRepository, ReminderScheduler, ReviewRepository,
    SettingsRepository, SystemClock, SystemThemeSource, TaskApplication, TaskIdGenerator,
    TaskRepository, TodayContextSource, TodayRepository, UiIntent, UserFacingError,
    UuidFocusSessionIdGenerator, UuidTaskIdGenerator,
};

fn main() {
    if let Err(error) = run() {
        quadrant_platform::report_startup_error(error.as_ref());
        std::process::exit(1);
    }
}

#[allow(clippy::too_many_lines)] // Composition root keeps all concrete wiring visible in one place.
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let database_path = quadrant_platform::PlatformPaths.database_path()?;
    let runtime = application_runtime()?;
    let single_instance =
        quadrant_platform::SingleInstanceCoordinator::claim(database_path.as_path())?;
    if !single_instance.is_primary() {
        single_instance.notify_primary()?;
        return Ok(());
    }
    let applied_restore = quadrant_storage::apply_pending_restore(&database_path)?;
    let activation_listener = {
        let _runtime_context = runtime.enter();
        single_instance.bind_activation_listener()?
    };

    let store = Arc::new(quadrant_storage::SqliteStore::open(&database_path)?);
    let tasks: Arc<dyn TaskRepository> = store.clone();
    let today_tasks: Arc<dyn TodayRepository> = store.clone();
    let reminders: Arc<dyn ReminderRepository> = store.clone();
    let focus_repository: Arc<dyn FocusRepository> = store.clone();
    let review_repository: Arc<dyn ReviewRepository> = store.clone();
    let completed_repository: Arc<dyn CompletedRepository> = store.clone();
    let maintenance_repository: Arc<dyn MaintenanceRepository> = store.clone();
    let settings: Arc<dyn SettingsRepository> = store.clone();
    let autostart: Arc<dyn AutostartService> =
        Arc::new(quadrant_platform::PlatformAutostartService);
    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let ids: Arc<dyn TaskIdGenerator> = Arc::new(UuidTaskIdGenerator);
    let focus_ids: Arc<dyn FocusSessionIdGenerator> = Arc::new(UuidFocusSessionIdGenerator);
    let today_context: Arc<dyn TodayContextSource> =
        Arc::new(quadrant_platform::PlatformTodayContextSource);
    let application = TaskApplication::new(
        Arc::clone(&tasks),
        today_tasks,
        Arc::clone(&settings),
        Arc::clone(&autostart),
        Arc::clone(&clock),
        ids,
        Arc::clone(&today_context),
    );
    let focus_application = FocusApplication::new(
        focus_repository,
        Arc::clone(&tasks),
        Arc::clone(&settings),
        Arc::clone(&clock),
        focus_ids,
        today_context,
    );
    let history_application = HistoryApplication::new(
        review_repository,
        completed_repository,
        Arc::clone(&clock),
        Arc::new(quadrant_platform::PlatformTodayContextSource),
    );
    let maintenance_application = MaintenanceApplication::new(
        maintenance_repository,
        Arc::new(quadrant_platform::PlatformExternalOpener),
        Arc::clone(&clock),
    );
    let initial_quadrants = application.load_quadrants()?;
    let initial_today = application.load_today()?;
    let initial_focus = focus_application.load_state()?;
    let initial_review = history_application.load_review()?;
    let initial_completed = history_application.load_completed()?;
    let initial_maintenance = maintenance_application.load_state()?;
    let theme_mode = store.load_theme_mode()?.unwrap_or_default();
    let desktop_settings = settings.load_desktop_settings()?;
    let autostart_reconcile_failed = reconcile_autostart(&*autostart, desktop_settings);

    let theme_source = quadrant_platform::PlatformThemeSource;
    let config = quadrant_ui::UiShellConfig {
        application_version: env!("CARGO_PKG_VERSION").to_owned(),
        updates: quadrant_application::UpdateViewState::from_build(
            env!("CARGO_PKG_VERSION"),
            option_env!("QUADRANT_DISTRIBUTION_CHANNEL"),
        ),
        theme_mode,
        system_theme: theme_source.current_theme(),
        quadrants: initial_quadrants,
        today: initial_today,
        focus: initial_focus,
        review: initial_review,
        completed: initial_completed,
        maintenance: initial_maintenance,
        desktop_settings,
    };

    let (intent_sender, mut intent_receiver) = tokio::sync::mpsc::unbounded_channel::<UiIntent>();
    let shell = quadrant_ui::UiShell::new(&config, move |intent| {
        drop(intent_sender.send(intent));
    })?;
    let event_sink = shell.event_sink();
    let desktop_event_sink = shell.desktop_event_sink();
    let (activation_shutdown, activation_shutdown_receiver) = tokio::sync::oneshot::channel();
    let activation_worker = runtime.spawn(activation_listener.run(
        Arc::clone(&desktop_event_sink),
        activation_shutdown_receiver,
    ));
    let desktop_integration =
        start_desktop_integration(Arc::clone(&desktop_event_sink), &event_sink);
    shell.set_platform_capabilities(ui_capabilities(desktop_integration.as_ref(), &*autostart));
    report_autostart_reconcile_failure(autostart_reconcile_failed, &event_sink);
    report_applied_restore(applied_restore.as_ref(), &event_sink);
    let reminder_delivery = native_reminder_delivery(Arc::clone(&event_sink));
    let (reminder_scheduler, reminder_handle) =
        ReminderScheduler::new(reminders, clock, reminder_delivery);
    let scheduler_worker = runtime.spawn(reminder_scheduler.run());
    let (focus_scheduler, focus_handle) =
        FocusScheduler::new(focus_application.clone(), Arc::clone(&event_sink));
    let focus_scheduler_worker = runtime.spawn(focus_scheduler.run());
    let application_event_sink = Arc::clone(&event_sink);
    let worker_reminder_handle = reminder_handle.clone();
    let worker_focus_handle = focus_handle.clone();
    let (intent_shutdown, mut intent_shutdown_receiver) = tokio::sync::oneshot::channel();
    let worker = runtime.spawn(async move {
        while let Some(intent) =
            next_intent_or_shutdown(&mut intent_receiver, &mut intent_shutdown_receiver).await
        {
            let affects_reminders = intent.affects_reminder_schedule();
            let affects_focus = intent.affects_focus_schedule();
            let refreshes_focus = intent.affects_focus_projection();
            let is_focus = intent.is_focus_intent();
            let is_history = intent.is_history_intent();
            let is_maintenance = intent.is_maintenance_intent();
            let refreshes_history = intent.affects_history_projection();
            let application = application.clone();
            let focus_application = focus_application.clone();
            let history_application = history_application.clone();
            let maintenance_application = maintenance_application.clone();
            let events = tokio::task::spawn_blocking(move || {
                let mut events = if is_maintenance {
                    maintenance_application.handle(&intent)
                } else if is_history {
                    history_application.handle(&intent)
                } else if is_focus {
                    focus_application.handle(&intent)
                } else {
                    let mut events = application.handle(intent);
                    if refreshes_focus && let Ok(state) = focus_application.load_state() {
                        events.push(ApplicationEvent::FocusChanged(state));
                    }
                    events
                };
                if refreshes_history {
                    events.extend(history_application.refresh_after_mutation());
                }
                events
            })
            .await;
            match events {
                Ok(events) => {
                    for event in events {
                        application_event_sink(event);
                    }
                    if affects_reminders {
                        worker_reminder_handle.schedule_changed();
                    }
                    if affects_focus {
                        worker_focus_handle.schedule_changed();
                    }
                }
                Err(_) => {
                    application_event_sink(ApplicationEvent::OperationFailed(UserFacingError {
                        message: "The background task stopped unexpectedly.".to_owned(),
                    }));
                }
            }
        }
    });

    let ui_result = shell.run(background_requested());
    let _ = intent_shutdown.send(());
    if let Some(integration) = desktop_integration {
        integration.shutdown();
    }
    let _ = activation_shutdown.send(());
    reminder_handle.shutdown();
    focus_handle.shutdown();
    runtime.block_on(worker)?;
    runtime.block_on(scheduler_worker)?;
    runtime.block_on(focus_scheduler_worker)?;
    runtime.block_on(activation_worker)?;
    ui_result?;
    Ok(())
}

async fn next_intent_or_shutdown(
    intent_receiver: &mut tokio::sync::mpsc::UnboundedReceiver<UiIntent>,
    shutdown: &mut tokio::sync::oneshot::Receiver<()>,
) -> Option<UiIntent> {
    tokio::select! {
        biased;
        _ = shutdown => None,
        intent = intent_receiver.recv() => intent,
    }
}

fn report_applied_restore(
    restore: Option<&quadrant_storage::AppliedRestore>,
    event_sink: &quadrant_ui::ApplicationEventSink,
) {
    let Some(restore) = restore else {
        return;
    };
    let message = restore.recovery_directory.as_ref().map_or_else(
        || "The staged backup was restored.".to_owned(),
        |directory| {
            format!(
                "The staged backup was restored. Previous data is in {}.",
                directory.display()
            )
        },
    );
    event_sink(ApplicationEvent::OperationSucceeded(message));
}

fn application_runtime() -> std::io::Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_multi_thread()
        .thread_name("quadrant-application")
        .enable_all()
        .build()
}

fn reconcile_autostart(
    autostart: &dyn AutostartService,
    settings: quadrant_application::DesktopSettings,
) -> bool {
    autostart
        .set_enabled(settings.launch_at_startup, settings.start_hidden)
        .is_err()
}

fn ui_capabilities(
    desktop: Option<&quadrant_platform::DesktopIntegration>,
    autostart: &dyn AutostartService,
) -> quadrant_ui::UiPlatformCapabilities {
    let capabilities = desktop
        .map(quadrant_platform::DesktopIntegration::capabilities)
        .unwrap_or_default();
    quadrant_ui::UiPlatformCapabilities {
        autostart: autostart.is_supported(),
        tray: capabilities.tray,
        global_hotkey: capabilities.global_hotkey,
        native_notifications: capabilities.native_notifications,
        single_instance: capabilities.single_instance,
    }
}

fn report_autostart_reconcile_failure(
    failed: bool,
    event_sink: &quadrant_ui::ApplicationEventSink,
) {
    if failed {
        event_sink(ApplicationEvent::OperationFailed(UserFacingError {
            message: "The saved startup registration could not be refreshed.".to_owned(),
        }));
    }
}

fn background_requested() -> bool {
    std::env::args_os()
        .skip(1)
        .any(|argument| argument == "--background")
}

fn start_desktop_integration(
    desktop_event_sink: quadrant_platform::DesktopEventSink,
    event_sink: &quadrant_ui::ApplicationEventSink,
) -> Option<quadrant_platform::DesktopIntegration> {
    let Ok(integration) = quadrant_platform::DesktopIntegration::start(desktop_event_sink) else {
        event_sink(quadrant_application::ApplicationEvent::OperationFailed(
            quadrant_application::UserFacingError {
                message: "Desktop shortcut and tray integration are unavailable.".to_owned(),
            },
        ));
        return None;
    };
    Some(integration)
}

fn native_reminder_delivery(
    event_sink: quadrant_ui::ApplicationEventSink,
) -> Arc<dyn ReminderDelivery> {
    let native_notifications = quadrant_platform::PlatformNotificationDelivery;
    Arc::new(move |alert: ReminderAlert| {
        if native_notifications.deliver(alert.clone()).is_err() {
            event_sink(ApplicationEvent::ReminderDue(alert));
        }
        Ok::<(), ReminderDeliveryError>(())
    })
}

#[cfg(test)]
mod tests {
    use super::next_intent_or_shutdown;

    #[tokio::test]
    async fn shutdown_stops_the_intent_worker_while_a_sender_is_still_retained() {
        let (_intent_sender, mut intent_receiver) = tokio::sync::mpsc::unbounded_channel();
        let (shutdown_sender, mut shutdown_receiver) = tokio::sync::oneshot::channel();

        assert!(shutdown_sender.send(()).is_ok());

        assert!(
            next_intent_or_shutdown(&mut intent_receiver, &mut shutdown_receiver)
                .await
                .is_none()
        );
    }
}
