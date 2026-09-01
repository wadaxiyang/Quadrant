//! Task use cases and projection refresh orchestration.

use std::sync::Arc;

use crate::{
    ApplicationEvent, AutostartService, CalendarError, Clock, DesktopSettings, NewTask,
    QuadrantsViewState, RepositoryError, SettingsRepository, TaskIdGenerator, TaskRepository,
    TodayContextSource, TodayRepository, TodayViewState, UiIntent, UserFacingError,
};

/// Initial/refresh projection load failure.
#[derive(Debug, thiserror::Error)]
pub enum ApplicationLoadError {
    /// Repository query failed.
    #[error(transparent)]
    Repository(#[from] RepositoryError),
    /// Platform local-calendar derivation failed.
    #[error(transparent)]
    Calendar(#[from] CalendarError),
}

/// Synchronous application use cases designed to run on the app-owned runtime's
/// blocking pool rather than the Slint event loop.
#[derive(Clone)]
pub struct TaskApplication {
    tasks: Arc<dyn TaskRepository>,
    today_tasks: Arc<dyn TodayRepository>,
    settings: Arc<dyn SettingsRepository>,
    autostart: Arc<dyn AutostartService>,
    clock: Arc<dyn Clock>,
    ids: Arc<dyn TaskIdGenerator>,
    today_context: Arc<dyn TodayContextSource>,
}

impl TaskApplication {
    /// Assembles task use cases from application-owned ports.
    #[must_use]
    pub fn new(
        tasks: Arc<dyn TaskRepository>,
        today_tasks: Arc<dyn TodayRepository>,
        settings: Arc<dyn SettingsRepository>,
        autostart: Arc<dyn AutostartService>,
        clock: Arc<dyn Clock>,
        ids: Arc<dyn TaskIdGenerator>,
        today_context: Arc<dyn TodayContextSource>,
    ) -> Self {
        Self {
            tasks,
            today_tasks,
            settings,
            autostart,
            clock,
            ids,
            today_context,
        }
    }

    /// Loads the initial active-task projection.
    ///
    /// # Errors
    ///
    /// Returns repository failures with operation context.
    pub fn load_quadrants(&self) -> Result<QuadrantsViewState, RepositoryError> {
        self.tasks
            .list_active_tasks()
            .map(|tasks| QuadrantsViewState::from_tasks(&tasks))
    }

    /// Loads the deterministic Today projection using current platform calendar boundaries.
    ///
    /// # Errors
    ///
    /// Returns a repository or platform calendar failure.
    pub fn load_today(&self) -> Result<TodayViewState, ApplicationLoadError> {
        let now = self.clock.now();
        let context = self.today_context.today_context(now)?;
        let tasks = self.today_tasks.list_today_candidates(context.local_date)?;
        Ok(TodayViewState::from_tasks(&tasks, now, context))
    }

    /// Handles a typed UI intent and produces zero or more UI-safe events.
    #[must_use]
    pub fn handle(&self, intent: UiIntent) -> Vec<ApplicationEvent> {
        match intent {
            UiIntent::Navigate(crate::NavigationRoute::Today) => match self.load_today() {
                Ok(state) => vec![ApplicationEvent::TodayChanged(state)],
                Err(error) => vec![load_failure_event(&error)],
            },
            UiIntent::Navigate(_) | UiIntent::OpenQuickAdd => Vec::new(),
            UiIntent::SetTheme(mode) => match self.settings.save_theme_mode(mode, self.clock.now())
            {
                Ok(()) => Vec::new(),
                Err(error) => vec![failure_event(&error)],
            },
            UiIntent::SetDesktopSettings(settings) => self.apply_desktop_settings(settings),
            UiIntent::SubmitQuickAdd(submission) => {
                let draft = match NewTask::quick_capture(submission.title, submission.placement) {
                    Ok(draft) => draft,
                    Err(error) => {
                        return vec![ApplicationEvent::OperationFailed(UserFacingError {
                            message: error.to_string(),
                        })];
                    }
                };
                match self
                    .tasks
                    .create_task(self.ids.generate(), draft, self.clock.now())
                {
                    Ok(_) => self.refresh_after_success("Task added."),
                    Err(error) => vec![failure_event(&error)],
                }
            }
            UiIntent::MoveTask { task_id, placement } => {
                match self.tasks.move_task(task_id, placement, self.clock.now()) {
                    Ok(_) => self.refresh_after_success("Task moved."),
                    Err(error) => vec![failure_event(&error)],
                }
            }
            UiIntent::ReorderTask { task_id, direction } => {
                match self
                    .tasks
                    .reorder_task(task_id, direction, self.clock.now())
                {
                    Ok(_) => self.refresh_after_success("Task reordered."),
                    Err(error) => vec![failure_event(&error)],
                }
            }
            UiIntent::OpenTaskEditor(task_id) => match self.tasks.get_task(task_id) {
                Ok(Some(task)) => vec![ApplicationEvent::TaskEditorLoaded((&task).into())],
                Ok(None) => vec![ApplicationEvent::OperationFailed(UserFacingError {
                    message: "That task no longer exists.".to_owned(),
                })],
                Err(error) => vec![failure_event(&error)],
            },
            UiIntent::SubmitTaskEditor(submission) => {
                let task_id = submission.task_id;
                let update = match submission.into_update() {
                    Ok(update) => update,
                    Err(error) => {
                        return vec![ApplicationEvent::TaskEditorValidationFailed(
                            error.to_string(),
                        )];
                    }
                };
                match self.tasks.update_task(task_id, update, self.clock.now()) {
                    Ok(_) => match self.load_states() {
                        Ok((quadrants, today)) => vec![
                            ApplicationEvent::QuadrantsChanged(quadrants),
                            ApplicationEvent::TodayChanged(today),
                            ApplicationEvent::TaskEditorSaved,
                            ApplicationEvent::OperationSucceeded("Task updated.".to_owned()),
                        ],
                        Err(error) => vec![load_failure_event(&error)],
                    },
                    Err(error) => vec![failure_event(&error)],
                }
            }
            UiIntent::CompleteTask(task_id) => {
                match self
                    .tasks
                    .complete_task(task_id, self.ids.generate(), self.clock.now())
                {
                    Ok(_) => self.refresh_after_success("Task completed."),
                    Err(error) => vec![failure_event(&error)],
                }
            }
            UiIntent::DeleteTask(task_id) => match self.tasks.delete_task(task_id) {
                Ok(()) => self.refresh_after_success("Task deleted."),
                Err(error) => vec![failure_event(&error)],
            },
            UiIntent::UpdateTask { task_id, update } => {
                match self.tasks.update_task(task_id, update, self.clock.now()) {
                    Ok(_) => self.refresh_after_success("Task updated."),
                    Err(error) => vec![failure_event(&error)],
                }
            }
        }
    }

    fn refresh_after_success(&self, message: &str) -> Vec<ApplicationEvent> {
        match self.load_states() {
            Ok((quadrants, today)) => vec![
                ApplicationEvent::QuadrantsChanged(quadrants),
                ApplicationEvent::TodayChanged(today),
                ApplicationEvent::OperationSucceeded(message.to_owned()),
            ],
            Err(error) => vec![load_failure_event(&error)],
        }
    }

    fn apply_desktop_settings(&self, settings: DesktopSettings) -> Vec<ApplicationEvent> {
        let previous = match self.settings.load_desktop_settings() {
            Ok(previous) => previous,
            Err(error) => return vec![failure_event(&error)],
        };
        if settings.launch_at_startup && !self.autostart.is_supported() {
            return vec![
                ApplicationEvent::DesktopSettingsChanged(previous),
                ApplicationEvent::OperationFailed(UserFacingError {
                    message: "Launch at startup is not supported on this platform.".to_owned(),
                }),
            ];
        }
        if self
            .autostart
            .set_enabled(settings.launch_at_startup, settings.start_hidden)
            .is_err()
        {
            return vec![
                ApplicationEvent::DesktopSettingsChanged(previous),
                ApplicationEvent::OperationFailed(UserFacingError {
                    message: "The startup registration could not be changed.".to_owned(),
                }),
            ];
        }
        if let Err(error) = self
            .settings
            .save_desktop_settings(settings, self.clock.now())
        {
            let _ = self
                .autostart
                .set_enabled(previous.launch_at_startup, previous.start_hidden);
            return vec![
                ApplicationEvent::DesktopSettingsChanged(previous),
                failure_event(&error),
            ];
        }
        vec![
            ApplicationEvent::DesktopSettingsChanged(settings),
            ApplicationEvent::OperationSucceeded("Desktop settings saved.".to_owned()),
        ]
    }

    fn load_states(&self) -> Result<(QuadrantsViewState, TodayViewState), ApplicationLoadError> {
        Ok((self.load_quadrants()?, self.load_today()?))
    }
}

fn failure_event(error: &RepositoryError) -> ApplicationEvent {
    let message = match error.operation() {
        crate::RepositoryOperation::ReadTasks => "Tasks could not be loaded.",
        crate::RepositoryOperation::ReadReminders => "Reminders could not be loaded.",
        crate::RepositoryOperation::CreateTask => "The task could not be added.",
        crate::RepositoryOperation::UpdateTask => "The task could not be updated.",
        crate::RepositoryOperation::UpdateReminder => "The reminder could not be updated.",
        crate::RepositoryOperation::TransitionTask => "The task state could not be changed.",
        crate::RepositoryOperation::DeleteTask => "The task could not be deleted.",
        crate::RepositoryOperation::ReadSettings => "Settings could not be loaded.",
        crate::RepositoryOperation::WriteSettings => "Settings could not be saved.",
        crate::RepositoryOperation::Open | crate::RepositoryOperation::Migrate => {
            "Quadrant storage is unavailable."
        }
    };
    ApplicationEvent::OperationFailed(UserFacingError {
        message: message.to_owned(),
    })
}

fn load_failure_event(error: &ApplicationLoadError) -> ApplicationEvent {
    match error {
        ApplicationLoadError::Repository(error) => failure_event(error),
        ApplicationLoadError::Calendar(_) => ApplicationEvent::OperationFailed(UserFacingError {
            message: "The local Today calendar could not be determined.".to_owned(),
        }),
    }
}
