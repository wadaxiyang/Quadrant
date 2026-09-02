//! Application-owned persistence and clock ports.

use std::{error::Error, fmt, path::Path, time::SystemTime};

use quadrant_domain::{
    FocusSession, FocusSessionId, FocusStatus, LocalDate, NewTask, PomodoroSettings, Task,
    TaskDetailsUpdate, TaskId, TaskPlacement, UtcTimestamp,
};
use uuid::Uuid;

use crate::{
    BackupInfo, DesktopSettings, FocusDaySummary, MaintenanceState, ReorderDirection, ThemeMode,
    TodayContext,
};
use crate::{ReviewQuery, ReviewQueryData};

/// Semantic repository operation used for error classification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RepositoryOperation {
    /// Opening/configuring storage.
    Open,
    /// Applying schema migrations.
    Migrate,
    /// Reading task state.
    ReadTasks,
    /// Reading active reminder state.
    ReadReminders,
    /// Creating a task.
    CreateTask,
    /// Updating or moving a task.
    UpdateTask,
    /// Consuming a delivered reminder.
    UpdateReminder,
    /// Completing/reopening a task and its history.
    TransitionTask,
    /// Permanently deleting a task.
    DeleteTask,
    /// Reading settings.
    ReadSettings,
    /// Writing settings.
    WriteSettings,
    /// Reading current or historical focus state.
    ReadFocus,
    /// Creating or transitioning a focus session.
    WriteFocus,
    /// Reading Review aggregates or completed-task history.
    ReadHistory,
    /// Creating, validating, or staging a local database backup.
    MaintainData,
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

    /// Moves an active task one position within its current placement.
    ///
    /// # Errors
    ///
    /// Returns an update failure or a missing/invalid task error.
    fn reorder_task(
        &self,
        id: TaskId,
        direction: ReorderDirection,
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
    fn complete_task(
        &self,
        id: TaskId,
        next_occurrence_id: TaskId,
        now: UtcTimestamp,
        completed_local_date: LocalDate,
    ) -> Result<Task, RepositoryError>;

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

/// Read model source for the derived Today projection.
pub trait TodayRepository: Send + Sync {
    /// Lists active Today candidates: any due task or a task planned no later than today.
    ///
    /// # Errors
    ///
    /// Returns a task-read repository failure.
    fn list_today_candidates(&self, local_today: LocalDate) -> Result<Vec<Task>, RepositoryError>;
}

/// Persistence capability used by the nearest-deadline reminder scheduler.
pub trait ReminderRepository: Send + Sync {
    /// Lists active tasks with reminders in ascending deadline order.
    ///
    /// # Errors
    ///
    /// Returns a reminder-read repository failure.
    fn list_pending_reminders(&self) -> Result<Vec<Task>, RepositoryError>;

    /// Clears a reminder only when its persisted deadline still matches the delivered value.
    ///
    /// # Errors
    ///
    /// Returns an update failure. A stale or missing reminder returns `Ok(false)`.
    fn clear_reminder_if_matches(
        &self,
        id: TaskId,
        scheduled_for: UtcTimestamp,
        now: UtcTimestamp,
    ) -> Result<bool, RepositoryError>;
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

    /// Loads desktop lifecycle settings, applying product defaults for missing keys.
    ///
    /// # Errors
    ///
    /// Returns a settings read or validation failure.
    fn load_desktop_settings(&self) -> Result<DesktopSettings, RepositoryError>;

    /// Stores the coherent desktop lifecycle settings group.
    ///
    /// # Errors
    ///
    /// Returns a settings write failure; partial settings must be rolled back.
    fn save_desktop_settings(
        &self,
        settings: DesktopSettings,
        now: UtcTimestamp,
    ) -> Result<(), RepositoryError>;

    /// Loads validated Pomodoro defaults, using product defaults for missing keys.
    ///
    /// # Errors
    ///
    /// Returns a settings read or validation failure.
    fn load_pomodoro_settings(&self) -> Result<PomodoroSettings, RepositoryError>;

    /// Stores the coherent Pomodoro settings group.
    ///
    /// # Errors
    ///
    /// Returns a settings write failure; partial settings must be rolled back.
    fn save_pomodoro_settings(
        &self,
        settings: PomodoroSettings,
        now: UtcTimestamp,
    ) -> Result<(), RepositoryError>;
}

/// Focus-session persistence capability.
pub trait FocusRepository: Send + Sync {
    /// Loads the only running/paused session, if any.
    ///
    /// # Errors
    ///
    /// Returns a focus read or mapping failure.
    fn get_current_focus_session(&self) -> Result<Option<FocusSession>, RepositoryError>;

    /// Inserts a new running session while enforcing one-current-session globally.
    ///
    /// # Errors
    ///
    /// Returns a focus write failure when another current session exists or storage rejects it.
    fn create_focus_session(&self, session: FocusSession) -> Result<FocusSession, RepositoryError>;

    /// Persists one optimistic lifecycle transition.
    ///
    /// # Errors
    ///
    /// Returns a focus write failure if the stored status no longer matches `expected`.
    fn transition_focus_session(
        &self,
        session: FocusSession,
        expected: FocusStatus,
    ) -> Result<FocusSession, RepositoryError>;

    /// Aggregates productive completed focus for one host-local creation date.
    ///
    /// # Errors
    ///
    /// Returns a focus query failure.
    fn productive_focus_summary(
        &self,
        local_date: LocalDate,
    ) -> Result<FocusDaySummary, RepositoryError>;

    /// Counts completed productive Pomodoro focus intervals for break cadence.
    ///
    /// # Errors
    ///
    /// Returns a focus query failure.
    fn completed_pomodoro_focus_count(&self) -> Result<u64, RepositoryError>;

    /// Returns the most recent Pomodoro focus task association, if it still exists.
    ///
    /// # Errors
    ///
    /// Returns a focus query failure.
    fn latest_pomodoro_focus_task_id(&self) -> Result<Option<TaskId>, RepositoryError>;
}

/// Read-optimized Review aggregation capability.
pub trait ReviewRepository: Send + Sync {
    /// Loads range totals, trends, quadrant metrics, focus highlights, and recent completions.
    ///
    /// # Errors
    ///
    /// Returns a history query or mapping failure.
    fn load_review(&self, query: ReviewQuery) -> Result<ReviewQueryData, RepositoryError>;
}

/// Bounded completed-task history capability.
pub trait CompletedRepository: Send + Sync {
    /// Lists the most recently completed tasks, newest first, up to `limit` rows.
    ///
    /// # Errors
    ///
    /// Returns a history query or task mapping failure.
    fn list_completed_tasks(&self, limit: u32) -> Result<Vec<Task>, RepositoryError>;
}

/// SQLite-consistent backup and staged-restore capability.
pub trait MaintenanceRepository: Send + Sync {
    /// Returns the current backup directory, latest valid-looking file, and pending state.
    ///
    /// # Errors
    ///
    /// Returns an I/O or metadata query failure.
    fn maintenance_state(&self) -> Result<MaintenanceState, RepositoryError>;

    /// Creates and validates a new immutable backup snapshot.
    ///
    /// # Errors
    ///
    /// Returns a snapshot, filesystem, schema, or integrity-check failure.
    fn create_backup(&self, now: UtcTimestamp) -> Result<BackupInfo, RepositoryError>;

    /// Validates the newest backup and stages it for the next process startup.
    ///
    /// # Errors
    ///
    /// Returns a missing-backup, validation, or filesystem failure.
    fn stage_latest_restore(&self) -> Result<BackupInfo, RepositoryError>;
}

/// Platform-owned opening of local paths and external URLs.
pub trait ExternalOpener: Send + Sync {
    /// Opens a directory in the native file manager.
    ///
    /// # Errors
    ///
    /// Returns a normalized platform failure.
    fn open_path(&self, path: &Path) -> Result<(), PlatformActionError>;

    /// Opens an HTTPS URL in the user's default browser.
    ///
    /// # Errors
    ///
    /// Returns a normalized platform failure.
    fn open_url(&self, url: &str) -> Result<(), PlatformActionError>;
}

/// Normalized external-action failure without OS details in application code.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("{detail}")]
pub struct PlatformActionError {
    detail: String,
}

impl PlatformActionError {
    /// Wraps platform diagnostic context.
    #[must_use]
    pub fn new(detail: impl fmt::Display) -> Self {
        Self {
            detail: detail.to_string(),
        }
    }
}

/// Platform capability for registering login/startup launch behavior.
pub trait AutostartService: Send + Sync {
    /// Returns whether this target can configure autostart.
    fn is_supported(&self) -> bool;

    /// Applies or removes the current executable's startup registration.
    ///
    /// # Errors
    ///
    /// Returns a normalized platform error when the external registration fails.
    fn set_enabled(&self, enabled: bool, start_hidden: bool) -> Result<(), AutostartError>;
}

/// Normalized autostart failure without leaking registry/package types.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AutostartError {
    detail: String,
}

impl AutostartError {
    /// Wraps diagnostic platform context.
    #[must_use]
    pub fn new(detail: impl fmt::Display) -> Self {
        Self {
            detail: detail.to_string(),
        }
    }
}

impl fmt::Display for AutostartError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.detail)
    }
}

impl Error for AutostartError {}

/// Deterministic application clock boundary.
pub trait Clock: Send + Sync {
    /// Returns the current UTC instant.
    fn now(&self) -> UtcTimestamp;
}

/// Failure to derive local calendar boundaries from the host platform.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CalendarError {
    detail: String,
}

impl CalendarError {
    /// Creates a calendar-boundary error without exposing platform types.
    #[must_use]
    pub fn new(detail: impl fmt::Display) -> Self {
        Self {
            detail: detail.to_string(),
        }
    }
}

impl fmt::Display for CalendarError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.detail)
    }
}

impl Error for CalendarError {}

/// Platform calendar boundary used by Today selection logic.
pub trait TodayContextSource: Send + Sync {
    /// Derives the local date and DST-safe UTC boundaries containing `now`.
    ///
    /// # Errors
    ///
    /// Returns a normalized platform calendar error.
    fn today_context(&self, now: UtcTimestamp) -> Result<TodayContext, CalendarError>;
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

/// Focus-session identity generation boundary.
pub trait FocusSessionIdGenerator: Send + Sync {
    /// Generates a fresh session identity.
    fn generate(&self) -> FocusSessionId;
}

/// Production `UUIDv7` focus-session identity generator.
#[derive(Clone, Copy, Debug, Default)]
pub struct UuidFocusSessionIdGenerator;

impl FocusSessionIdGenerator for UuidFocusSessionIdGenerator {
    fn generate(&self) -> FocusSessionId {
        FocusSessionId::from_uuid(Uuid::now_v7())
    }
}

/// Production `UUIDv7` task identity generator.
#[derive(Clone, Copy, Debug, Default)]
pub struct UuidTaskIdGenerator;

impl TaskIdGenerator for UuidTaskIdGenerator {
    fn generate(&self) -> TaskId {
        TaskId::from_uuid(Uuid::now_v7())
    }
}
