//! Task use cases and projection refresh orchestration.

use std::sync::Arc;

use crate::{
    ApplicationEvent, Clock, NewTask, QuadrantsViewState, RepositoryError, SettingsRepository,
    TaskIdGenerator, TaskRepository, UiIntent, UserFacingError,
};

/// Synchronous application use cases designed to run on the app-owned runtime's
/// blocking pool rather than the Slint event loop.
#[derive(Clone)]
pub struct TaskApplication {
    tasks: Arc<dyn TaskRepository>,
    settings: Arc<dyn SettingsRepository>,
    clock: Arc<dyn Clock>,
    ids: Arc<dyn TaskIdGenerator>,
}

impl TaskApplication {
    /// Assembles task use cases from application-owned ports.
    #[must_use]
    pub fn new(
        tasks: Arc<dyn TaskRepository>,
        settings: Arc<dyn SettingsRepository>,
        clock: Arc<dyn Clock>,
        ids: Arc<dyn TaskIdGenerator>,
    ) -> Self {
        Self {
            tasks,
            settings,
            clock,
            ids,
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

    /// Handles a typed UI intent and produces zero or more UI-safe events.
    #[must_use]
    pub fn handle(&self, intent: UiIntent) -> Vec<ApplicationEvent> {
        match intent {
            UiIntent::Navigate(_) | UiIntent::OpenQuickAdd => Vec::new(),
            UiIntent::SetTheme(mode) => match self.settings.save_theme_mode(mode, self.clock.now())
            {
                Ok(()) => Vec::new(),
                Err(error) => vec![failure_event(&error)],
            },
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
                    Ok(_) => match self.load_quadrants() {
                        Ok(state) => vec![
                            ApplicationEvent::QuadrantsChanged(state),
                            ApplicationEvent::TaskEditorSaved,
                            ApplicationEvent::OperationSucceeded("Task updated.".to_owned()),
                        ],
                        Err(error) => vec![failure_event(&error)],
                    },
                    Err(error) => vec![failure_event(&error)],
                }
            }
            UiIntent::CompleteTask(task_id) => {
                match self.tasks.complete_task(task_id, self.clock.now()) {
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
        match self.load_quadrants() {
            Ok(state) => vec![
                ApplicationEvent::QuadrantsChanged(state),
                ApplicationEvent::OperationSucceeded(message.to_owned()),
            ],
            Err(error) => vec![failure_event(&error)],
        }
    }
}

fn failure_event(error: &RepositoryError) -> ApplicationEvent {
    let message = match error.operation() {
        crate::RepositoryOperation::ReadTasks => "Tasks could not be loaded.",
        crate::RepositoryOperation::CreateTask => "The task could not be added.",
        crate::RepositoryOperation::UpdateTask => "The task could not be updated.",
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
