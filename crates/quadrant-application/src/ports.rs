//! Application-owned persistence and clock ports.

use std::{error::Error, fmt, time::SystemTime};

use quadrant_domain::{NewTask, Task, TaskDetailsUpdate, TaskId, TaskPlacement, UtcTimestamp};
use uuid::Uuid;

use crate::ThemeMode;

/// Semantic repository operation used for error classification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RepositoryOperation {
    /// Opening/configuring storage.
    Open,
    /// Applying schema migrations.
    Migrate,
    /// Reading task state.
    ReadTasks,
    /// Creating a task.
    CreateTask,
    /// Updating or moving a task.
    UpdateTask,
    /// Completing/reopening a task and its history.
    TransitionTask,
    /// Permanently deleting a task.
    DeleteTask,
    /// Reading settings.
    ReadSettings,
    /// Writing settings.
    WriteSettings,
}

/// Storage adapter failure with typed operation context and diagnostic detail.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositoryError {
    operation: RepositoryOperation,
    detail: String,
}

impl RepositoryError {
    /// Creates an adapter error at a specific semantic operation.
    #[must_use]
    pub fn new(operation: RepositoryOperation, detail: impl fmt::Display) -> Self {
        Self {
            operation,
            detail: detail.to_string(),
        }
    }

    /// Returns the failed semantic operation.
    #[must_use]
    pub const fn operation(&self) -> RepositoryOperation {
        self.operation
    }

    /// Returns adapter diagnostic detail for logs/startup reporting.
    #[must_use]
    pub fn detail(&self) -> &str {
        &self.detail
    }
}

impl fmt::Display for RepositoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?}: {}", self.operation, self.detail)
    }
}

impl Error for RepositoryError {}

/// Task mutation/query port implemented by the storage crate.
pub trait TaskRepository: Send + Sync {
    /// Creates a task and atomically assigns the next placement sort key.
    ///
    /// # Errors
    ///
    /// Returns an operation-classified repository failure.
    fn create_task(
        &self,
        id: TaskId,
        draft: NewTask,
        now: UtcTimestamp,
    ) -> Result<Task, RepositoryError>;

    /// Lists active tasks in placement/sort order.
    ///
    /// # Errors
    ///
    /// Returns a task-read repository failure.
    fn list_active_tasks(&self) -> Result<Vec<Task>, RepositoryError>;

    /// Loads one task by identity.
    ///
    /// # Errors
    ///
    /// Returns a task-read repository failure.
    fn get_task(&self, id: TaskId) -> Result<Option<Task>, RepositoryError>;

    /// Moves an active task and appends it to the destination order.
    ///
    /// # Errors
    ///
    /// Returns an update failure or a missing/invalid task error.
    fn move_task(
        &self,
        id: TaskId,
        placement: TaskPlacement,
        now: UtcTimestamp,
    ) -> Result<Task, RepositoryError>;

    /// Replaces editable task details.
    ///
    /// # Errors
    ///
    /// Returns an update failure or a domain validation error.
    fn update_task(
        &self,
        id: TaskId,
        update: TaskDetailsUpdate,
        now: UtcTimestamp,
    ) -> Result<Task, RepositoryError>;

    /// Completes a task and records immutable history atomically.
    ///
    /// # Errors
    ///
    /// Returns a transition failure; partial state must be rolled back.
    fn complete_task(&self, id: TaskId, now: UtcTimestamp) -> Result<Task, RepositoryError>;

    /// Reopens a completed task and reconciles its latest completion event.
    ///
    /// # Errors
    ///
    /// Returns a transition failure; partial state must be rolled back.
    fn reopen_task(&self, id: TaskId, now: UtcTimestamp) -> Result<Task, RepositoryError>;

    /// Hard-deletes the task row. Completion snapshots remain detached history.
    ///
    /// # Errors
    ///
    /// Returns a delete failure or a missing-task error.
    fn delete_task(&self, id: TaskId) -> Result<(), RepositoryError>;
}

/// Typed settings port; heterogeneous JSON remains behind the adapter.
pub trait SettingsRepository: Send + Sync {
    /// Loads the persisted theme preference.
    ///
    /// # Errors
    ///
    /// Returns a settings read or validation failure.
    fn load_theme_mode(&self) -> Result<Option<ThemeMode>, RepositoryError>;

    /// Stores the validated theme preference.
    ///
    /// # Errors
    ///
    /// Returns a settings write failure.
    fn save_theme_mode(
        &self,
        theme_mode: ThemeMode,
        now: UtcTimestamp,
    ) -> Result<(), RepositoryError>;
}

/// Deterministic application clock boundary.
pub trait Clock: Send + Sync {
    /// Returns the current UTC instant.
    fn now(&self) -> UtcTimestamp;
}

/// Production clock based on the standard system clock.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> UtcTimestamp {
        let seconds = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        UtcTimestamp::from_unix_seconds(i64::try_from(seconds).unwrap_or(i64::MAX))
    }
}

/// Task identity generation boundary.
pub trait TaskIdGenerator: Send + Sync {
    /// Generates a fresh task identity.
    fn generate(&self) -> TaskId;
}

/// Production `UUIDv7` task identity generator.
#[derive(Clone, Copy, Debug, Default)]
pub struct UuidTaskIdGenerator;

impl TaskIdGenerator for UuidTaskIdGenerator {
    fn generate(&self) -> TaskId {
        TaskId::from_uuid(Uuid::now_v7())
    }
}
