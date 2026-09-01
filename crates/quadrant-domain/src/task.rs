//! Task aggregate and capture/classification rules.

use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::{LocalDate, RecurrenceRule, ScheduledInstant, UtcTimestamp};

/// Identifies the four architectural quadrants without UI or storage coupling.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Quadrant {
    /// Important and urgent.
    Q1,
    /// Important and not urgent.
    Q2,
    /// Not important and urgent.
    Q3,
    /// Not important and not urgent.
    Q4,
}

/// A task's placement in the capture and classification workflow.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum TaskPlacement {
    /// The task is captured but not classified.
    #[default]
    Inbox,
    /// The task has been classified into a quadrant.
    Quadrant(Quadrant),
}

/// Opaque task identity backed by a `UUIDv7` value.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TaskId(Uuid);

impl TaskId {
    /// Generates a time-ordered `UUIDv7` identity.
    #[must_use]
    pub fn generate() -> Self {
        Self(Uuid::now_v7())
    }

    /// Wraps an already validated UUID.
    #[must_use]
    pub const fn from_uuid(value: Uuid) -> Self {
        Self(value)
    }

    /// Returns the UUID value.
    #[must_use]
    pub const fn as_uuid(self) -> Uuid {
        self.0
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for TaskId {
    type Err = uuid::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(value).map(Self)
    }
}

/// A validated non-empty task title.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskTitle(String);

impl TaskTitle {
    /// Trims and validates a user-entered title.
    ///
    /// # Errors
    ///
    /// Returns [`TaskDomainError::EmptyTitle`] when no content remains, or
    /// [`TaskDomainError::TitleTooLong`] above 500 Unicode scalar values.
    pub fn new(value: impl Into<String>) -> Result<Self, TaskDomainError> {
        let value = value.into();
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(TaskDomainError::EmptyTitle);
        }
        if trimmed.chars().count() > 500 {
            return Err(TaskDomainError::TitleTooLong);
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// Returns the normalized title.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Active/completed task lifecycle state.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TaskStatus {
    /// The task remains actionable.
    #[default]
    Active,
    /// The task has a current completion timestamp.
    Completed,
}

/// Stable manual ordering value. Gaps allow local reorder operations.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SortKey(i64);

impl SortKey {
    /// First key used in an empty placement.
    pub const INITIAL: Self = Self(1_024);
    /// Normal gap between appended tasks.
    pub const STEP: i64 = 1_024;

    /// Restores a persisted sort key.
    #[must_use]
    pub const fn from_i64(value: i64) -> Self {
        Self(value)
    }

    /// Returns the persisted integer.
    #[must_use]
    pub const fn value(self) -> i64 {
        self.0
    }

    /// Returns the next gapped append key when it fits.
    #[must_use]
    pub const fn checked_next(self) -> Option<Self> {
        match self.0.checked_add(Self::STEP) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }
}

/// Input for creating a validated task aggregate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewTask {
    /// Validated title.
    pub title: TaskTitle,
    /// Optional long-form notes.
    pub notes: String,
    /// Inbox or quadrant destination.
    pub placement: TaskPlacement,
    /// Optional intentional planning date.
    pub planned_on: Option<LocalDate>,
    /// Optional deadline with timezone semantics.
    pub due: Option<ScheduledInstant>,
    /// Optional reminder with timezone semantics.
    pub reminder: Option<ScheduledInstant>,
    /// Optional validated recurrence.
    pub recurrence: Option<RecurrenceRule>,
}

impl NewTask {
    /// Creates the minimal capture used by Quick Add.
    ///
    /// # Errors
    ///
    /// Returns a title validation error.
    pub fn quick_capture(
        title: impl Into<String>,
        placement: TaskPlacement,
    ) -> Result<Self, TaskDomainError> {
        Ok(Self {
            title: TaskTitle::new(title)?,
            notes: String::new(),
            placement,
            planned_on: None,
            due: None,
            reminder: None,
            recurrence: None,
        })
    }

    fn validate(&self) -> Result<(), TaskDomainError> {
        if let (Some(reminder), Some(due)) = (&self.reminder, &self.due)
            && reminder.at_utc > due.at_utc
        {
            return Err(TaskDomainError::ReminderAfterDue);
        }
        Ok(())
    }
}

/// Editable task details, separate from identity and lifecycle state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskDetailsUpdate {
    /// Replacement title.
    pub title: TaskTitle,
    /// Replacement notes.
    pub notes: String,
    /// Replacement placement.
    pub placement: TaskPlacement,
    /// Replacement planning date.
    pub planned_on: Option<LocalDate>,
    /// Replacement deadline.
    pub due: Option<ScheduledInstant>,
    /// Replacement reminder.
    pub reminder: Option<ScheduledInstant>,
    /// Replacement recurrence rule.
    pub recurrence: Option<RecurrenceRule>,
}

/// Persistence-shaped data used only to restore a validated aggregate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskRecord {
    /// Task identity.
    pub id: TaskId,
    /// Validated title.
    pub title: TaskTitle,
    /// Notes.
    pub notes: String,
    /// Inbox/quadrant placement.
    pub placement: TaskPlacement,
    /// Lifecycle state.
    pub status: TaskStatus,
    /// Optional plan date.
    pub planned_on: Option<LocalDate>,
    /// Optional due instant.
    pub due: Option<ScheduledInstant>,
    /// Optional reminder instant.
    pub reminder: Option<ScheduledInstant>,
    /// Optional recurrence.
    pub recurrence: Option<RecurrenceRule>,
    /// Manual order key.
    pub sort_key: SortKey,
    /// Creation audit time.
    pub created_at: UtcTimestamp,
    /// Last update audit time.
    pub updated_at: UtcTimestamp,
    /// Current completion time.
    pub completed_at: Option<UtcTimestamp>,
}

/// The validated task aggregate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Task(TaskRecord);

impl Task {
    /// Creates an active task.
    ///
    /// # Errors
    ///
    /// Returns a scheduling invariant error when reminder/due values conflict.
    pub fn create(
        id: TaskId,
        draft: NewTask,
        sort_key: SortKey,
        now: UtcTimestamp,
    ) -> Result<Self, TaskDomainError> {
        draft.validate()?;
        Ok(Self(TaskRecord {
            id,
            title: draft.title,
            notes: draft.notes,
            placement: draft.placement,
            status: TaskStatus::Active,
            planned_on: draft.planned_on,
            due: draft.due,
            reminder: draft.reminder,
            recurrence: draft.recurrence,
            sort_key,
            created_at: now,
            updated_at: now,
            completed_at: None,
        }))
    }

    /// Restores and validates persisted state.
    ///
    /// # Errors
    ///
    /// Returns an invariant error for inconsistent lifecycle or schedule data.
    pub fn restore(record: TaskRecord) -> Result<Self, TaskDomainError> {
        match (record.status, record.completed_at) {
            (TaskStatus::Active, Some(_)) => return Err(TaskDomainError::ActiveHasCompletion),
            (TaskStatus::Completed, None) => {
                return Err(TaskDomainError::CompletedWithoutTimestamp);
            }
            _ => {}
        }
        if let (Some(reminder), Some(due)) = (&record.reminder, &record.due)
            && reminder.at_utc > due.at_utc
        {
            return Err(TaskDomainError::ReminderAfterDue);
        }
        Ok(Self(record))
    }

    /// Returns the aggregate data for persistence/projection.
    #[must_use]
    pub const fn record(&self) -> &TaskRecord {
        &self.0
    }

    /// Moves the task and assigns its new placement order key.
    pub fn move_to(&mut self, placement: TaskPlacement, sort_key: SortKey, now: UtcTimestamp) {
        self.0.placement = placement;
        self.0.sort_key = sort_key;
        self.0.updated_at = now;
    }

    /// Replaces editable details while preserving identity/audit/lifecycle fields.
    ///
    /// # Errors
    ///
    /// Returns a schedule validation error.
    pub fn update_details(
        &mut self,
        update: TaskDetailsUpdate,
        now: UtcTimestamp,
    ) -> Result<(), TaskDomainError> {
        let candidate = NewTask {
            title: update.title.clone(),
            notes: update.notes.clone(),
            placement: update.placement,
            planned_on: update.planned_on,
            due: update.due.clone(),
            reminder: update.reminder.clone(),
            recurrence: update.recurrence,
        };
        candidate.validate()?;
        self.0.title = update.title;
        self.0.notes = update.notes;
        self.0.placement = update.placement;
        self.0.planned_on = update.planned_on;
        self.0.due = update.due;
        self.0.reminder = update.reminder;
        self.0.recurrence = update.recurrence;
        self.0.updated_at = now;
        Ok(())
    }

    /// Completes an active task and returns the immutable history snapshot.
    ///
    /// # Errors
    ///
    /// Returns [`TaskDomainError::AlreadyCompleted`] for a duplicate transition.
    pub fn complete(&mut self, now: UtcTimestamp) -> Result<CompletionSnapshot, TaskDomainError> {
        if self.0.status == TaskStatus::Completed {
            return Err(TaskDomainError::AlreadyCompleted);
        }
        self.0.status = TaskStatus::Completed;
        self.0.completed_at = Some(now);
        self.0.updated_at = now;
        Ok(CompletionSnapshot {
            task_id: self.0.id,
            title: self.0.title.clone(),
            placement: self.0.placement,
            completed_at: now,
        })
    }

    /// Reopens a completed task.
    ///
    /// # Errors
    ///
    /// Returns [`TaskDomainError::AlreadyActive`] for a duplicate transition.
    pub fn reopen(&mut self, now: UtcTimestamp) -> Result<(), TaskDomainError> {
        if self.0.status == TaskStatus::Active {
            return Err(TaskDomainError::AlreadyActive);
        }
        self.0.status = TaskStatus::Active;
        self.0.completed_at = None;
        self.0.updated_at = now;
        Ok(())
    }
}

/// Immutable fields recorded for completion/review history.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompletionSnapshot {
    /// Source task identity.
    pub task_id: TaskId,
    /// Title at completion time.
    pub title: TaskTitle,
    /// Placement at completion time.
    pub placement: TaskPlacement,
    /// Completion instant.
    pub completed_at: UtcTimestamp,
}

/// Domain validation/state-transition failures.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum TaskDomainError {
    /// Title was blank after trimming.
    #[error("task title is required")]
    EmptyTitle,
    /// Title exceeded the supported user-facing limit.
    #[error("task title cannot exceed 500 characters")]
    TitleTooLong,
    /// Reminder was scheduled after its due instant.
    #[error("task reminder cannot be after its due time")]
    ReminderAfterDue,
    /// Persisted active state carried a completion timestamp.
    #[error("active task cannot have a completion timestamp")]
    ActiveHasCompletion,
    /// Persisted completed state lacked a completion timestamp.
    #[error("completed task requires a completion timestamp")]
    CompletedWithoutTimestamp,
    /// Completion was requested twice.
    #[error("task is already completed")]
    AlreadyCompleted,
    /// Reopen was requested for an active task.
    #[error("task is already active")]
    AlreadyActive,
}

#[cfg(test)]
mod tests {
    use super::{NewTask, Quadrant, SortKey, Task, TaskId, TaskPlacement, TaskStatus};
    use crate::UtcTimestamp;

    #[test]
    fn tasks_start_active_in_inbox_by_default() {
        let task = Task::create(
            TaskId::generate(),
            NewTask::quick_capture("  Capture this  ", TaskPlacement::default())
                .expect("valid capture"),
            SortKey::INITIAL,
            UtcTimestamp::from_unix_seconds(10),
        )
        .expect("valid task");

        assert_eq!(task.record().title.as_str(), "Capture this");
        assert_eq!(task.record().placement, TaskPlacement::Inbox);
        assert_eq!(task.record().status, TaskStatus::Active);
    }

    #[test]
    fn placement_and_completion_transitions_preserve_invariants() {
        let now = UtcTimestamp::from_unix_seconds(10);
        let mut task = Task::create(
            TaskId::generate(),
            NewTask::quick_capture("Test", TaskPlacement::Inbox).expect("valid capture"),
            SortKey::INITIAL,
            now,
        )
        .expect("valid task");

        task.move_to(
            TaskPlacement::Quadrant(Quadrant::Q2),
            SortKey::from_i64(2_048),
            UtcTimestamp::from_unix_seconds(11),
        );
        let snapshot = task
            .complete(UtcTimestamp::from_unix_seconds(12))
            .expect("active task completes");

        assert_eq!(snapshot.placement, TaskPlacement::Quadrant(Quadrant::Q2));
        assert_eq!(task.record().status, TaskStatus::Completed);
        assert!(task.complete(UtcTimestamp::from_unix_seconds(13)).is_err());
        task.reopen(UtcTimestamp::from_unix_seconds(14))
            .expect("completed task reopens");
        assert_eq!(task.record().status, TaskStatus::Active);
        assert_eq!(task.record().completed_at, None);
    }

    #[test]
    fn titles_are_trimmed_and_required() {
        assert!(NewTask::quick_capture("  ", TaskPlacement::Inbox).is_err());
        let draft = NewTask::quick_capture(" title ", TaskPlacement::Inbox).expect("valid title");
        assert_eq!(draft.title.as_str(), "title");
    }
}
