//! Mapping between `SQLite` scalar values and validated domain aggregates.

use std::{error::Error, str::FromStr};

use quadrant_domain::{
    LocalDate, Quadrant, RecurrenceRule, ScheduledInstant, SortKey, Task, TaskId, TaskPlacement,
    TaskRecord, TaskStatus, TaskTitle, TimeZoneId, UtcTimestamp,
};
use rusqlite::{Row, types::Type};

pub(crate) fn task_from_row(row: &Row<'_>) -> rusqlite::Result<Task> {
    let id = TaskId::from_str(row.get_ref(0)?.as_str()?)
        .map_err(|error| conversion_error(0, Type::Text, error))?;
    let title = TaskTitle::new(row.get::<_, String>(1)?)
        .map_err(|error| conversion_error(1, Type::Text, error))?;
    let quadrant = placement_from_db(row.get(3)?)
        .map_err(|error| conversion_error(3, Type::Integer, error))?;
    let status =
        status_from_db(row.get(4)?).map_err(|error| conversion_error(4, Type::Integer, error))?;
    let planned_on = row
        .get::<_, Option<String>>(5)?
        .map(|value| LocalDate::parse_iso(&value))
        .transpose()
        .map_err(|error| conversion_error(5, Type::Text, error))?;
    let due = scheduled_from_db(row.get(6)?, row.get(7)?, 6)?;
    let reminder = scheduled_from_db(row.get(8)?, row.get(9)?, 8)?;
    let recurrence = row
        .get::<_, Option<String>>(10)?
        .map(|value| serde_json::from_str::<RecurrenceRule>(&value))
        .transpose()
        .map_err(|error| conversion_error(10, Type::Text, error))?;

    Task::restore(TaskRecord {
        id,
        title,
        notes: row.get(2)?,
        placement: quadrant,
        status,
        planned_on,
        due,
        reminder,
        recurrence,
        sort_key: SortKey::from_i64(row.get(11)?),
        created_at: UtcTimestamp::from_unix_seconds(row.get(12)?),
        updated_at: UtcTimestamp::from_unix_seconds(row.get(13)?),
        completed_at: row
            .get::<_, Option<i64>>(14)?
            .map(UtcTimestamp::from_unix_seconds),
    })
    .map_err(|error| conversion_error(14, Type::Integer, error))
}

pub(crate) const fn placement_to_db(placement: TaskPlacement) -> Option<i64> {
    match placement {
        TaskPlacement::Inbox => None,
        TaskPlacement::Quadrant(Quadrant::Q1) => Some(1),
        TaskPlacement::Quadrant(Quadrant::Q2) => Some(2),
        TaskPlacement::Quadrant(Quadrant::Q3) => Some(3),
        TaskPlacement::Quadrant(Quadrant::Q4) => Some(4),
    }
}

fn placement_from_db(value: Option<i64>) -> Result<TaskPlacement, MappingError> {
    match value {
        None => Ok(TaskPlacement::Inbox),
        Some(1) => Ok(TaskPlacement::Quadrant(Quadrant::Q1)),
        Some(2) => Ok(TaskPlacement::Quadrant(Quadrant::Q2)),
        Some(3) => Ok(TaskPlacement::Quadrant(Quadrant::Q3)),
        Some(4) => Ok(TaskPlacement::Quadrant(Quadrant::Q4)),
        Some(_) => Err(MappingError("invalid quadrant value")),
    }
}

pub(crate) const fn status_to_db(status: TaskStatus) -> i64 {
    match status {
        TaskStatus::Active => 0,
        TaskStatus::Completed => 1,
    }
}

fn status_from_db(value: i64) -> Result<TaskStatus, MappingError> {
    match value {
        0 => Ok(TaskStatus::Active),
        1 => Ok(TaskStatus::Completed),
        _ => Err(MappingError("invalid task status value")),
    }
}

fn scheduled_from_db(
    timestamp: Option<i64>,
    timezone: Option<String>,
    column: usize,
) -> rusqlite::Result<Option<ScheduledInstant>> {
    match (timestamp, timezone) {
        (None, None) => Ok(None),
        (Some(timestamp), Some(timezone)) => Ok(Some(ScheduledInstant {
            at_utc: UtcTimestamp::from_unix_seconds(timestamp),
            time_zone: TimeZoneId::new(timezone)
                .map_err(|error| conversion_error(column + 1, Type::Text, error))?,
        })),
        _ => Err(conversion_error(
            column,
            Type::Integer,
            MappingError("scheduled timestamp/timezone pair is incomplete"),
        )),
    }
}

fn conversion_error(
    column: usize,
    value_type: Type,
    error: impl Error + Send + Sync + 'static,
) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(column, value_type, Box::new(error))
}

#[derive(Debug)]
struct MappingError(&'static str);

impl std::fmt::Display for MappingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.0)
    }
}

impl Error for MappingError {}
