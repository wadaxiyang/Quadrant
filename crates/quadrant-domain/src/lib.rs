//! Pure domain model and rules for Quadrant.

#![forbid(unsafe_code)]

mod recurrence;
mod task;
mod time;

pub use recurrence::{
    RecurrenceAdvanceError, RecurrencePattern, RecurrenceRule, RecurrenceRuleError,
};
pub use task::{
    CompletionSnapshot, NewTask, Quadrant, SortKey, Task, TaskDetailsUpdate, TaskDomainError,
    TaskId, TaskPlacement, TaskRecord, TaskStatus, TaskTitle,
};
pub use time::{LocalDate, ScheduledInstant, TimeValueError, TimeZoneId, UtcTimestamp};
