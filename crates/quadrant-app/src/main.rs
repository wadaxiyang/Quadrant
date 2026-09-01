//! Quadrant composition root.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::sync::Arc;

    use quadrant_application::{
        ApplicationEvent, Clock, ReminderAlert, ReminderDelivery, ReminderDeliveryError,
        ReminderRepository, ReminderScheduler, SettingsRepository, SystemClock, SystemThemeSource,
        TaskApplication, TaskIdGenerator, TaskRepository, TodayContextSource, TodayRepository,
        UiIntent, UserFacingError, UuidTaskIdGenerator,
    };

    let database_path = quadrant_platform::PlatformPaths.database_path()?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .thread_name("quadrant-application")
        .enable_all()
        .build()?;
    let single_instance =
        quadrant_platform::SingleInstanceCoordinator::claim(database_path.as_path())?;
    if !single_instance.is_primary() {
        single_instance.notify_primary()?;
        return Ok(());
    }
    let activation_listener = {
        let _runtime_context = runtime.enter();
        single_instance.bind_activation_listener()?
    };

    let store = Arc::new(quadrant_storage::SqliteStore::open(database_path)?);
    let tasks: Arc<dyn TaskRepository> = store.clone();
    let today_tasks: Arc<dyn TodayRepository> = store.clone();
    let reminders: Arc<dyn ReminderRepository> = store.clone();
    let settings: Arc<dyn SettingsRepository> = store.clone();
    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let ids: Arc<dyn TaskIdGenerator> = Arc::new(UuidTaskIdGenerator);
    let today_context: Arc<dyn TodayContextSource> =
        Arc::new(quadrant_platform::PlatformTodayContextSource);
    let application = TaskApplication::new(
        Arc::clone(&tasks),
        today_tasks,
        settings,
        Arc::clone(&clock),
        ids,
        today_context,
    );
    let initial_quadrants = application.load_quadrants()?;
    let initial_today = application.load_today()?;
    let theme_mode = store.load_theme_mode()?.unwrap_or_default();

    let theme_source = quadrant_platform::PlatformThemeSource;
    let config = quadrant_ui::UiShellConfig {
        theme_mode,
        system_theme: theme_source.current_theme(),
        quadrants: initial_quadrants,
        today: initial_today,
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
        match quadrant_platform::DesktopIntegration::start(Arc::clone(&desktop_event_sink)) {
            Ok(integration) => Some(integration),
            Err(_) => {
                event_sink(ApplicationEvent::OperationFailed(UserFacingError {
                    message: "Desktop shortcut and tray integration are unavailable.".to_owned(),
                }));
                None
            }
        };
    let reminder_event_sink = Arc::clone(&event_sink);
    let native_notifications = quadrant_platform::PlatformNotificationDelivery;
    let reminder_delivery: Arc<dyn ReminderDelivery> = Arc::new(move |alert: ReminderAlert| {
        if native_notifications.deliver(alert.clone()).is_err() {
            reminder_event_sink(ApplicationEvent::ReminderDue(alert));
        }
        Ok::<(), ReminderDeliveryError>(())
    });
    let (reminder_scheduler, reminder_handle) =
        ReminderScheduler::new(reminders, clock, reminder_delivery);
    let scheduler_worker = runtime.spawn(reminder_scheduler.run());
    let application_event_sink = Arc::clone(&event_sink);
    let worker_reminder_handle = reminder_handle.clone();
    let worker = runtime.spawn(async move {
        while let Some(intent) = intent_receiver.recv().await {
            let affects_reminders = intent.affects_reminder_schedule();
            let application = application.clone();
            let events = tokio::task::spawn_blocking(move || application.handle(intent)).await;
            match events {
                Ok(events) => {
                    for event in events {
                        application_event_sink(event);
                    }
                    if affects_reminders {
                        worker_reminder_handle.schedule_changed();
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

    let ui_result = shell.run();
    if let Some(integration) = desktop_integration {
        integration.shutdown();
    }
    let _ = activation_shutdown.send(());
    reminder_handle.shutdown();
    runtime.block_on(worker)?;
    runtime.block_on(scheduler_worker)?;
    runtime.block_on(activation_worker)?;
    ui_result?;
    Ok(())
}
