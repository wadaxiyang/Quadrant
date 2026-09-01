//! Read-optimized Review aggregates and bounded Completed history.

use std::{collections::BTreeMap, error::Error};

use quadrant_application::{
    CompletedRepository, RepositoryError, RepositoryOperation, ReviewActivityPoint,
    ReviewDateRange, ReviewFocusHighlights, ReviewQuadrantValue, ReviewQuery, ReviewQueryData,
    ReviewRecentCompletion, ReviewRepository, ReviewTotals,
};
use quadrant_domain::{LocalDate, Quadrant, Task, UtcTimestamp};
use rusqlite::{Connection, OptionalExtension, Row, params, types::Type};

use crate::{SqliteStore, mapping};

const TASK_COLUMNS: &str = "id, title, notes, quadrant, status, planned_on,
    due_at_utc, due_tz, reminder_at_utc, reminder_tz, recurrence_json,
    sort_key, created_at_utc, updated_at_utc, completed_at_utc";
const PRODUCTIVE_FOCUS: &str = "status = 2 AND (mode = 0 OR (mode = 1 AND pomodoro_kind = 0))";

impl CompletedRepository for SqliteStore {
    fn list_completed_tasks(&self, limit: u32) -> Result<Vec<Task>, RepositoryError> {
        let operation = RepositoryOperation::ReadHistory;
        let connection = self.lock(operation)?;
        let sql = format!(
            "SELECT {TASK_COLUMNS} FROM tasks
             WHERE status = 1
             ORDER BY completed_at_utc DESC, id DESC LIMIT ?1"
        );
        let mut statement = connection
            .prepare(&sql)
            .map_err(|error| RepositoryError::new(operation, error))?;
        let rows = statement
            .query_map([i64::from(limit)], mapping::task_from_row)
            .map_err(|error| RepositoryError::new(operation, error))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|error| RepositoryError::new(operation, error))
    }
}

impl ReviewRepository for SqliteStore {
    fn load_review(&self, query: ReviewQuery) -> Result<ReviewQueryData, RepositoryError> {
        let operation = RepositoryOperation::ReadHistory;
        if query.recent_limit == 0 || query.recent_limit > 50 {
            return Err(RepositoryError::new(
                operation,
                "recent completion limit must be between 1 and 50",
            ));
        }
        let connection = self.lock(operation)?;
        let current = read_totals(&connection, query.current, operation)?;
        let previous = query
            .previous
            .map(|range| read_totals(&connection, range, operation))
            .transpose()?;
        let daily_activity = read_daily_activity(&connection, query.current, operation)?;
        let quadrants = read_quadrants(&connection, query.current, operation)?;
        let focus = read_focus_highlights(&connection, query.current, operation)?;
        let recent_completed = read_recent(&connection, query.recent_limit, operation)?;
        let current_inbox_count = read_count(
            &connection,
            "SELECT COUNT(*) FROM tasks WHERE status = 0 AND quadrant IS NULL",
            [],
            operation,
        )?;
        let current_overdue_count = read_count(
            &connection,
            "SELECT COUNT(*) FROM tasks
             WHERE status = 0 AND due_at_utc IS NOT NULL AND due_at_utc < ?1",
            [query.now.unix_seconds()],
            operation,
        )?;
        Ok(ReviewQueryData {
            current,
            previous,
            daily_activity,
            quadrants,
            focus,
            recent_completed,
            current_inbox_count,
            current_overdue_count,
        })
    }
}

fn read_totals(
    connection: &Connection,
    range: ReviewDateRange,
    operation: RepositoryOperation,
) -> Result<ReviewTotals, RepositoryError> {
    let (lower, upper) = range_params(range);
    let completed = connection
        .query_row(
            "SELECT COUNT(*) FROM task_completion_events
             WHERE reverted_at_utc IS NULL
               AND (?1 IS NULL OR completed_local_date >= ?1)
               AND completed_local_date < ?2",
            params![lower, upper],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|error| RepositoryError::new(operation, error))?;
    let (lower, upper) = range_params(range);
    let (sessions, seconds) = connection
        .query_row(
            &format!(
                "SELECT COUNT(*), COALESCE(SUM(duration_seconds), 0)
                 FROM focus_sessions WHERE {PRODUCTIVE_FOCUS}
                   AND (?1 IS NULL OR created_local_date >= ?1)
                   AND created_local_date < ?2"
            ),
            params![lower, upper],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )
        .map_err(|error| RepositoryError::new(operation, error))?;
    Ok(ReviewTotals {
        completed_tasks: nonnegative_u64(completed),
        focus_sessions: nonnegative_u64(sessions),
        focus_seconds: nonnegative_u64(seconds),
    })
}

fn read_daily_activity(
    connection: &Connection,
    range: ReviewDateRange,
    operation: RepositoryOperation,
) -> Result<Vec<ReviewActivityPoint>, RepositoryError> {
    let mut values = BTreeMap::<LocalDate, (u64, u64)>::new();
    let (lower, upper) = range_params(range);
    let mut statement = connection
        .prepare(
            "SELECT completed_local_date, COUNT(*) FROM task_completion_events
             WHERE reverted_at_utc IS NULL
               AND (?1 IS NULL OR completed_local_date >= ?1)
               AND completed_local_date < ?2
             GROUP BY completed_local_date ORDER BY completed_local_date",
        )
        .map_err(|error| RepositoryError::new(operation, error))?;
    let rows = statement
        .query_map(params![lower, upper], |row| {
            Ok((parse_date(row, 0)?, nonnegative_u64(row.get(1)?)))
        })
        .map_err(|error| RepositoryError::new(operation, error))?;
    for row in rows {
        let (date, completed) = row.map_err(|error| RepositoryError::new(operation, error))?;
        values.entry(date).or_default().0 = completed;
    }

    let (lower, upper) = range_params(range);
    let mut statement = connection
        .prepare(&format!(
            "SELECT created_local_date, COALESCE(SUM(duration_seconds), 0)
             FROM focus_sessions WHERE {PRODUCTIVE_FOCUS}
               AND (?1 IS NULL OR created_local_date >= ?1)
               AND created_local_date < ?2
             GROUP BY created_local_date ORDER BY created_local_date"
        ))
        .map_err(|error| RepositoryError::new(operation, error))?;
    let rows = statement
        .query_map(params![lower, upper], |row| {
            Ok((parse_date(row, 0)?, nonnegative_u64(row.get(1)?)))
        })
        .map_err(|error| RepositoryError::new(operation, error))?;
    for row in rows {
        let (date, seconds) = row.map_err(|error| RepositoryError::new(operation, error))?;
        values.entry(date).or_default().1 = seconds;
    }
    Ok(values
        .into_iter()
        .map(|(date, (completed, focus_seconds))| ReviewActivityPoint {
            date,
            completed,
            focus_seconds,
        })
        .collect())
}

fn read_quadrants(
    connection: &Connection,
    range: ReviewDateRange,
    operation: RepositoryOperation,
) -> Result<Vec<ReviewQuadrantValue>, RepositoryError> {
    let mut values = BTreeMap::<Option<i64>, (u64, u64)>::new();
    let (lower, upper) = range_params(range);
    let mut statement = connection
        .prepare(
            "SELECT quadrant_snapshot, COUNT(*) FROM task_completion_events
             WHERE reverted_at_utc IS NULL
               AND (?1 IS NULL OR completed_local_date >= ?1)
               AND completed_local_date < ?2
             GROUP BY quadrant_snapshot",
        )
        .map_err(|error| RepositoryError::new(operation, error))?;
    let rows = statement
        .query_map(params![lower, upper], |row| {
            Ok((row.get::<_, Option<i64>>(0)?, nonnegative_u64(row.get(1)?)))
        })
        .map_err(|error| RepositoryError::new(operation, error))?;
    for row in rows {
        let (quadrant, count) = row.map_err(|error| RepositoryError::new(operation, error))?;
        values.entry(quadrant).or_default().0 = count;
    }

    let (lower, upper) = range_params(range);
    let mut statement = connection
        .prepare(&format!(
            "SELECT quadrant_snapshot, COALESCE(SUM(duration_seconds), 0)
             FROM focus_sessions WHERE {PRODUCTIVE_FOCUS}
               AND (?1 IS NULL OR created_local_date >= ?1)
               AND created_local_date < ?2
             GROUP BY quadrant_snapshot"
        ))
        .map_err(|error| RepositoryError::new(operation, error))?;
    let rows = statement
        .query_map(params![lower, upper], |row| {
            Ok((row.get::<_, Option<i64>>(0)?, nonnegative_u64(row.get(1)?)))
        })
        .map_err(|error| RepositoryError::new(operation, error))?;
    for row in rows {
        let (quadrant, seconds) = row.map_err(|error| RepositoryError::new(operation, error))?;
        values.entry(quadrant).or_default().1 = seconds;
    }

    [Some(1), Some(2), Some(3), Some(4), None]
        .into_iter()
        .map(|value| {
            let (completed, focus_seconds) = values.get(&value).copied().unwrap_or_default();
            Ok(ReviewQuadrantValue {
                quadrant: value.map(quadrant_from_db).transpose()?,
                completed,
                focus_seconds,
            })
        })
        .collect::<Result<Vec<_>, MappingError>>()
        .map_err(|error| RepositoryError::new(operation, error))
}

fn read_focus_highlights(
    connection: &Connection,
    range: ReviewDateRange,
    operation: RepositoryOperation,
) -> Result<ReviewFocusHighlights, RepositoryError> {
    let (lower, upper) = range_params(range);
    let longest = connection
        .query_row(
            &format!(
                "SELECT COALESCE(MAX(duration_seconds), 0) FROM focus_sessions
                 WHERE {PRODUCTIVE_FOCUS}
                   AND (?1 IS NULL OR created_local_date >= ?1)
                   AND created_local_date < ?2"
            ),
            params![lower, upper],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|error| RepositoryError::new(operation, error))?;

    let (lower, upper) = range_params(range);
    let task = connection
        .query_row(
            &format!(
                "SELECT task_title_snapshot, SUM(duration_seconds), COUNT(*)
                 FROM focus_sessions WHERE {PRODUCTIVE_FOCUS}
                   AND task_title_snapshot IS NOT NULL AND trim(task_title_snapshot) <> ''
                   AND (?1 IS NULL OR created_local_date >= ?1)
                   AND created_local_date < ?2
                 GROUP BY COALESCE(task_id, 'title:' || task_title_snapshot), task_title_snapshot
                 ORDER BY SUM(duration_seconds) DESC, COUNT(*) DESC, task_title_snapshot LIMIT 1"
            ),
            params![lower, upper],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    nonnegative_u64(row.get(1)?),
                    nonnegative_u64(row.get(2)?),
                ))
            },
        )
        .optional()
        .map_err(|error| RepositoryError::new(operation, error))?;

    let (lower, upper) = range_params(range);
    let quadrant = connection
        .query_row(
            &format!(
                "SELECT quadrant_snapshot, SUM(duration_seconds)
                 FROM focus_sessions WHERE {PRODUCTIVE_FOCUS}
                   AND quadrant_snapshot IS NOT NULL
                   AND (?1 IS NULL OR created_local_date >= ?1)
                   AND created_local_date < ?2
                 GROUP BY quadrant_snapshot
                 ORDER BY SUM(duration_seconds) DESC, quadrant_snapshot LIMIT 1"
            ),
            params![lower, upper],
            |row| Ok((row.get::<_, i64>(0)?, nonnegative_u64(row.get(1)?))),
        )
        .optional()
        .map_err(|error| RepositoryError::new(operation, error))?;

    let (task_title, task_seconds, task_sessions) = task
        .map_or((None, 0, 0), |(title, seconds, sessions)| {
            (Some(title), seconds, sessions)
        });
    let (most_focused_quadrant, quadrant_seconds) = quadrant
        .map_or(Ok((None, 0)), |(value, seconds)| {
            quadrant_from_db(value).map(|quadrant| (Some(quadrant), seconds))
        })
        .map_err(|error| RepositoryError::new(operation, error))?;
    Ok(ReviewFocusHighlights {
        longest_session_seconds: nonnegative_u64(longest),
        most_focused_task_title: task_title,
        most_focused_task_seconds: task_seconds,
        most_focused_task_sessions: task_sessions,
        most_focused_quadrant,
        most_focused_quadrant_seconds: quadrant_seconds,
    })
}

fn read_recent(
    connection: &Connection,
    limit: u32,
    operation: RepositoryOperation,
) -> Result<Vec<ReviewRecentCompletion>, RepositoryError> {
    let mut statement = connection
        .prepare(
            "SELECT task_title_snapshot, completed_at_utc, completed_local_date,
                    quadrant_snapshot, was_overdue
             FROM task_completion_events WHERE reverted_at_utc IS NULL
             ORDER BY completed_at_utc DESC, id DESC LIMIT ?1",
        )
        .map_err(|error| RepositoryError::new(operation, error))?;
    let rows = statement
        .query_map([i64::from(limit)], |row| {
            let quadrant = row
                .get::<_, Option<i64>>(3)?
                .map(quadrant_from_db)
                .transpose()
                .map_err(|error| conversion_error(3, Type::Integer, error))?;
            Ok(ReviewRecentCompletion {
                title: row.get(0)?,
                completed_at: UtcTimestamp::from_unix_seconds(row.get(1)?),
                completed_local_date: parse_date(row, 2)?,
                quadrant,
                was_overdue: row.get(4)?,
            })
        })
        .map_err(|error| RepositoryError::new(operation, error))?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| RepositoryError::new(operation, error))
}

fn read_count<P: rusqlite::Params>(
    connection: &Connection,
    sql: &str,
    params: P,
    operation: RepositoryOperation,
) -> Result<u64, RepositoryError> {
    connection
        .query_row(sql, params, |row| row.get::<_, i64>(0))
        .map(nonnegative_u64)
        .map_err(|error| RepositoryError::new(operation, error))
}

fn range_params(range: ReviewDateRange) -> (Option<String>, String) {
    (
        range.lower_inclusive.map(|date| date.to_string()),
        range.upper_exclusive.to_string(),
    )
}

fn parse_date(row: &Row<'_>, column: usize) -> rusqlite::Result<LocalDate> {
    LocalDate::parse_iso(&row.get::<_, String>(column)?)
        .map_err(|error| conversion_error(column, Type::Text, error))
}

fn nonnegative_u64(value: i64) -> u64 {
    u64::try_from(value).unwrap_or(0)
}

fn quadrant_from_db(value: i64) -> Result<Quadrant, MappingError> {
    match value {
        1 => Ok(Quadrant::Q1),
        2 => Ok(Quadrant::Q2),
        3 => Ok(Quadrant::Q3),
        4 => Ok(Quadrant::Q4),
        _ => Err(MappingError("invalid quadrant value")),
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

#[cfg(test)]
mod tests {
    use quadrant_application::{
        CompletedRepository, ReviewDateRange, ReviewQuery, ReviewRepository, TaskRepository,
    };
    use quadrant_domain::{LocalDate, NewTask, Quadrant, TaskId, TaskPlacement, UtcTimestamp};
    use rusqlite::params;
    use uuid::Uuid;

    use crate::SqliteStore;

    fn task_id(value: u128) -> TaskId {
        TaskId::from_uuid(Uuid::from_u128(value))
    }

    #[test]
    fn review_uses_active_snapshots_and_productive_focus_only() {
        let store = SqliteStore::open_in_memory().expect("storage opens");
        let kept = task_id(501);
        let reverted = task_id(502);
        for (id, title) in [(kept, "Kept snapshot"), (reverted, "Reverted snapshot")] {
            store
                .create_task(
                    id,
                    NewTask::quick_capture(title, TaskPlacement::Quadrant(Quadrant::Q1))
                        .expect("valid task"),
                    UtcTimestamp::from_unix_seconds(10),
                )
                .expect("task created");
            store
                .complete_task(
                    id,
                    task_id(id.as_uuid().as_u128() + 1_000),
                    UtcTimestamp::from_unix_seconds(20),
                    LocalDate::parse_iso("2026-09-01").expect("valid date"),
                )
                .expect("task completed");
        }
        store
            .reopen_task(reverted, UtcTimestamp::from_unix_seconds(30))
            .expect("task reopened");

        let connection = store
            .lock(quadrant_application::RepositoryOperation::ReadHistory)
            .expect("connection lock");
        let rows = [
            ("productive-stopwatch", 0, None, None, 600, 2),
            ("productive-pomodoro", 1, Some(0), Some(1_500), 900, 2),
            ("short-break", 1, Some(1), Some(300), 300, 2),
            ("cancelled", 0, None, None, 120, 3),
        ];
        for (id, mode, kind, target, duration, status) in rows {
            let linked_task = if matches!(kind, Some(1 | 2)) {
                None
            } else {
                Some(kept.to_string())
            };
            connection
                .execute(
                    "INSERT INTO focus_sessions(
                         id, task_id, task_title_snapshot, quadrant_snapshot, mode, pomodoro_kind,
                         started_at_utc, active_segment_started_at_utc, ended_at_utc,
                         target_duration_seconds, duration_seconds, status, created_local_date
                     ) VALUES (?1, ?2, 'Kept snapshot', 1, ?3, ?4, 100, NULL, 200,
                               ?5, ?6, ?7, '2026-09-01')",
                    params![id, linked_task, mode, kind, target, duration, status],
                )
                .expect("focus row inserted");
        }
        drop(connection);

        let query = ReviewQuery {
            current: ReviewDateRange {
                lower_inclusive: Some(
                    LocalDate::parse_iso("2026-09-01").expect("valid lower date"),
                ),
                upper_exclusive: LocalDate::parse_iso("2026-09-02").expect("valid upper date"),
            },
            previous: None,
            now: UtcTimestamp::from_unix_seconds(40),
            recent_limit: 12,
        };
        let review = store.load_review(query).expect("review loads");
        assert_eq!(review.current.completed_tasks, 1);
        assert_eq!(review.current.focus_sessions, 2);
        assert_eq!(review.current.focus_seconds, 1_500);
        assert_eq!(review.focus.longest_session_seconds, 900);
        assert_eq!(
            review.focus.most_focused_task_title.as_deref(),
            Some("Kept snapshot")
        );
        assert_eq!(review.recent_completed.len(), 1);
        assert_eq!(review.recent_completed[0].title, "Kept snapshot");
        assert_eq!(
            store
                .list_completed_tasks(50)
                .expect("completed tasks load")
                .len(),
            1
        );

        store.delete_task(kept).expect("completed task deleted");
        let after_delete = store.load_review(query).expect("review reloads");
        assert_eq!(after_delete.current.completed_tasks, 1);
        assert_eq!(after_delete.recent_completed[0].title, "Kept snapshot");
        assert!(
            store
                .list_completed_tasks(50)
                .expect("completed tasks reload")
                .is_empty()
        );
    }
}
