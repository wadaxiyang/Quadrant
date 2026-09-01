//! Quadrant composition root.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::sync::Arc;

    use quadrant_application::{
        ApplicationEvent, Clock, SettingsRepository, SystemClock, SystemThemeSource,
        TaskApplication, TaskIdGenerator, TaskRepository, UiIntent, UserFacingError,
        UuidTaskIdGenerator,
    };

    let database_path = quadrant_platform::PlatformPaths.database_path()?;
    let store = Arc::new(quadrant_storage::SqliteStore::open(database_path)?);
    let tasks: Arc<dyn TaskRepository> = store.clone();
    let settings: Arc<dyn SettingsRepository> = store.clone();
    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let ids: Arc<dyn TaskIdGenerator> = Arc::new(UuidTaskIdGenerator);
    let application = TaskApplication::new(tasks, settings, clock, ids);
    let initial_quadrants = application.load_quadrants()?;
    let theme_mode = store.load_theme_mode()?.unwrap_or_default();

    let theme_source = quadrant_platform::PlatformThemeSource;
    let config = quadrant_ui::UiShellConfig {
        theme_mode,
        system_theme: theme_source.current_theme(),
        quadrants: initial_quadrants,
    };

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .thread_name("quadrant-application")
        .enable_all()
        .build()?;
    let (intent_sender, mut intent_receiver) = tokio::sync::mpsc::unbounded_channel::<UiIntent>();
    let shell = quadrant_ui::UiShell::new(&config, move |intent| {
        drop(intent_sender.send(intent));
    })?;
    let event_sink = shell.event_sink();
    let worker = runtime.spawn(async move {
        while let Some(intent) = intent_receiver.recv().await {
            let application = application.clone();
            let events = tokio::task::spawn_blocking(move || application.handle(intent)).await;
            match events {
                Ok(events) => {
                    for event in events {
                        event_sink(event);
                    }
                }
                Err(_) => event_sink(ApplicationEvent::OperationFailed(UserFacingError {
                    message: "The background task stopped unexpectedly.".to_owned(),
                })),
            }
        }
    });

    let ui_result = shell.run();
    runtime.block_on(worker)?;
    ui_result?;
    Ok(())
}
