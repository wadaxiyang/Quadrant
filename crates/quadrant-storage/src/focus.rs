//! `SQLite` Focus-session repository implementation.

use std::{error::Error, str::FromStr};

use quadrant_application::{
    FocusDaySummary, FocusRepository, RepositoryError, RepositoryOperation,
};
use quadrant_domain::{
    FocusMode, FocusSession, FocusSessionId, FocusSessionRecord, FocusStatus, FocusTaskSnapshot,
    LocalDate, PomodoroKind, Quadrant, TaskId, UtcTimestamp,
};
use rusqlite::{OptionalExtension, Row, types::Type};

use crate::SqliteStore;

const FOCUS_COLUMNS: &str = "id, task_id, task_title_snapshot, quadrant_snapshot, mode,
    pomodoro_kind, started_at_utc, active_segment_started_at_utc, ended_at_utc,
    target_duration_seconds, duration_seconds, status, created_local_date";

impl FocusRepository for SqliteStore {
    fn get_current_focus_session(&self) -> Result<Option<FocusSession>, RepositoryError> {
        let operation = RepositoryOperation::ReadFocus;
        let connection = self.lock(operation)?;
        let sql = format!(
            "SELECT {FOCUS_COLUMNS} FROM focus_sessions
             WHERE status IN (0, 1) ORDER BY started_at_utc DESC LIMIT 1"
        );
        connection
            .query_row(&sql, [], focus_from_row)
            .optional()
            .map_err(|error| RepositoryError::new(operation, error))
    }

    fn create_focus_session(&self, session: FocusSession) -> Result<FocusSession, RepositoryError> {
        let operation = RepositoryOperation::WriteFocus;
        let connection = self.lock(operation)?;
        insert_focus(&connection, &session)
            .map_err(|error| RepositoryError::new(operation, error))?;
        Ok(session)
    }

    fn transition_focus_session(
        &self,
        session: FocusSession,
        expected: FocusStatus,
    ) -> Result<FocusSession, RepositoryError> {
        let operation = RepositoryOperation::WriteFocus;
        let connection = self.lock(operation)?;
        let record = session.record();
        let changed = connection
            .execute(
                "UPDATE focus_sessions SET
                     task_id = ?2, task_title_snapshot = ?3, quadrant_snapshot = ?4,
                     mode = ?5, pomodoro_kind = ?6, started_at_utc = ?7,
                     active_segment_started_at_utc = ?8, ended_at_utc = ?9,
                     target_duration_seconds = ?10, duration_seconds = ?11,
                     status = ?12, created_local_date = ?13
                 WHERE id = ?1 AND status = ?14",
                rusqlite::params_from_iter(focus_params(record, Some(expected))),
            )
            .map_err(|error| RepositoryError::new(operation, error))?;
        if changed != 1 {
            return Err(RepositoryError::new(
                operation,
                "focus session changed concurrently or no longer exists",
            ));
        }
        Ok(session)
    }

    fn productive_focus_summary(
        &self,
        local_date: LocalDate,
    ) -> Result<FocusDaySummary, RepositoryError> {
        let operation = RepositoryOperation::ReadFocus;
        self.lock(operation)?
            .query_row(
                "SELECT COALESCE(SUM(duration_seconds), 0), COUNT(*)
                 FROM focus_sessions
                 WHERE created_local_date = ?1 AND status = 2
                   AND (mode = 0 OR (mode = 1 AND pomodoro_kind = 0))",
                [local_date.to_string()],
                |row| {
                    let seconds = row.get::<_, i64>(0)?;
                    let count = row.get::<_, i64>(1)?;
                    Ok(FocusDaySummary {
                        total_seconds: u64::try_from(seconds).unwrap_or(u64::MAX),
                        session_count: u32::try_from(count).unwrap_or(u32::MAX),
                    })
                },
            )
            .map_err(|error| RepositoryError::new(operation, error))
    }

    fn completed_pomodoro_focus_count(&self) -> Result<u64, RepositoryError> {
        let operation = RepositoryOperation::ReadFocus;
        self.lock(operation)?
            .query_row(
                "SELECT COUNT(*) FROM focus_sessions
                 WHERE status = 2 AND mode = 1 AND pomodoro_kind = 0",
                [],
                |row| {
                    let value = row.get::<_, i64>(0)?;
                    Ok(u64::try_from(value).unwrap_or(u64::MAX))
                },
            )
            .map_err(|error| RepositoryError::new(operation, error))
    }

    fn latest_pomodoro_focus_task_id(&self) -> Result<Option<TaskId>, RepositoryError> {
        let operation = RepositoryOperation::ReadFocus;
        let value = self
            .lock(operation)?
            .query_row(
                "SELECT task_id FROM focus_sessions
                 WHERE status = 2 AND mode = 1 AND pomodoro_kind = 0 AND task_id IS NOT NULL
                 ORDER BY ended_at_utc DESC, id DESC LIMIT 1",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| RepositoryError::new(operation, error))?;
        value
            .map(|value| {
                TaskId::from_str(&value).map_err(|error| RepositoryError::new(operation, error))
            })
            .transpose()
    }
}

fn insert_focus(connection: &rusqlite::Connection, session: &FocusSession) -> rusqlite::Result<()> {
    let record = session.record();
    connection.execute(
        "INSERT INTO focus_sessions (
             id, task_id, task_title_snapshot, quadrant_snapshot, mode, pomodoro_kind,
             started_at_utc, active_segment_started_at_utc, ended_at_utc,
             target_duration_seconds, duration_seconds, status, created_local_date
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        rusqlite::params_from_iter(focus_params(record, None)),
    )?;
    Ok(())
}

fn focus_params(
    record: &FocusSessionRecord,
    expected: Option<FocusStatus>,
) -> Vec<rusqlite::types::Value> {
    let task_id = record
        .task
        .as_ref()
        .and_then(|task| task.id)
        .map(|id| id.to_string());
    let title = record.task.as_ref().map(|task| task.title.clone());
    let quadrant = record
        .task
        .as_ref()
        .and_then(|task| task.quadrant)
        .map(quadrant_to_db);
    let mut values = vec![
        record.id.to_string().into(),
        task_id.into(),
        title.into(),
        quadrant.into(),
        focus_mode_to_db(record.mode).into(),
        record.pomodoro_kind.map(pomodoro_kind_to_db).into(),
        record.started_at.unix_seconds().into(),
        record
            .active_segment_started_at
            .map(UtcTimestamp::unix_seconds)
            .into(),
        record.ended_at.map(UtcTimestamp::unix_seconds).into(),
        record.target_duration_seconds.map(i64::from).into(),
        i64::from(record.duration_seconds).into(),
        focus_status_to_db(record.status).into(),
        record.created_local_date.to_string().into(),
    ];
    if let Some(expected) = expected {
        values.push(focus_status_to_db(expected).into());
    }
    values
}

fn focus_from_row(row: &Row<'_>) -> rusqlite::Result<FocusSession> {
    let id = FocusSessionId::from_str(row.get_ref(0)?.as_str()?)
        .map_err(|error| conversion_error(0, Type::Text, error))?;
    let task_id = row
        .get::<_, Option<String>>(1)?
        .map(|value| TaskId::from_str(&value))
        .transpose()
        .map_err(|error| conversion_error(1, Type::Text, error))?;
    let title = row.get::<_, Option<String>>(2)?;
    let quadrant = row
        .get::<_, Option<i64>>(3)?
        .map(quadrant_from_db)
        .transpose()
        .map_err(|error| conversion_error(3, Type::Integer, error))?;
    let task = match (task_id, title) {
        (None, None) => None,
        (id, Some(title)) => Some(FocusTaskSnapshot {
            id,
            title,
            quadrant,
        }),
        (Some(_), None) => {
            return Err(conversion_error(
                2,
                Type::Text,
                MappingError("focus task snapshot title is missing"),
            ));
        }
    };
    let mode = focus_mode_from_db(row.get(4)?)
        .map_err(|error| conversion_error(4, Type::Integer, error))?;
    let pomodoro_kind = row
        .get::<_, Option<i64>>(5)?
        .map(pomodoro_kind_from_db)
        .transpose()
        .map_err(|error| conversion_error(5, Type::Integer, error))?;
    let status = focus_status_from_db(row.get(11)?)
        .map_err(|error| conversion_error(11, Type::Integer, error))?;
    let created_local_date = LocalDate::parse_iso(&row.get::<_, String>(12)?)
        .map_err(|error| conversion_error(12, Type::Text, error))?;
    FocusSession::restore(FocusSessionRecord {
        id,
        task,
        mode,
        pomodoro_kind,
        started_at: UtcTimestamp::from_unix_seconds(row.get(6)?),
        active_segment_started_at: row
            .get::<_, Option<i64>>(7)?
            .map(UtcTimestamp::from_unix_seconds),
        ended_at: row
            .get::<_, Option<i64>>(8)?
            .map(UtcTimestamp::from_unix_seconds),
        target_duration_seconds: row
            .get::<_, Option<i64>>(9)?
            .map(u32::try_from)
            .transpose()
            .map_err(|error| conversion_error(9, Type::Integer, error))?,
        duration_seconds: u32::try_from(row.get::<_, i64>(10)?)
            .map_err(|error| conversion_error(10, Type::Integer, error))?,
        status,
        created_local_date,
    })
    .map_err(|error| conversion_error(11, Type::Integer, error))
}

const fn focus_mode_to_db(mode: FocusMode) -> i64 {
    match mode {
        FocusMode::Stopwatch => 0,
        FocusMode::Pomodoro => 1,
    }
}

fn focus_mode_from_db(value: i64) -> Result<FocusMode, MappingError> {
    match value {
        0 => Ok(FocusMode::Stopwatch),
        1 => Ok(FocusMode::Pomodoro),
        _ => Err(MappingError("invalid focus mode")),
    }
}

const fn pomodoro_kind_to_db(kind: PomodoroKind) -> i64 {
    match kind {
        PomodoroKind::Focus => 0,
        PomodoroKind::ShortBreak => 1,
        PomodoroKind::LongBreak => 2,
    }
}

fn pomodoro_kind_from_db(value: i64) -> Result<PomodoroKind, MappingError> {
    match value {
        0 => Ok(PomodoroKind::Focus),
        1 => Ok(PomodoroKind::ShortBreak),
        2 => Ok(PomodoroKind::LongBreak),
        _ => Err(MappingError("invalid Pomodoro kind")),
    }
}

const fn focus_status_to_db(status: FocusStatus) -> i64 {
    match status {
        FocusStatus::Running => 0,
        FocusStatus::Paused => 1,
        FocusStatus::Completed => 2,
        FocusStatus::Cancelled => 3,
    }
}

fn focus_status_from_db(value: i64) -> Result<FocusStatus, MappingError> {
    match value {
        0 => Ok(FocusStatus::Running),
        1 => Ok(FocusStatus::Paused),
        2 => Ok(FocusStatus::Completed),
        3 => Ok(FocusStatus::Cancelled),
        _ => Err(MappingError("invalid focus status")),
    }
}

const fn quadrant_to_db(quadrant: Quadrant) -> i64 {
    match quadrant {
        Quadrant::Q1 => 1,
        Quadrant::Q2 => 2,
        Quadrant::Q3 => 3,
        Quadrant::Q4 => 4,
    }
}

fn quadrant_from_db(value: i64) -> Result<Quadrant, MappingError> {
    match value {
        1 => Ok(Quadrant::Q1),
        2 => Ok(Quadrant::Q2),
        3 => Ok(Quadrant::Q3),
        4 => Ok(Quadrant::Q4),
        _ => Err(MappingError("invalid quadrant snapshot")),
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
    use quadrant_application::{FocusDaySummary, FocusRepository, TaskRepository};
    use quadrant_domain::{
        FocusMode, FocusSession, FocusSessionId, FocusStatus, FocusTaskSnapshot, LocalDate,
        NewTask, PomodoroKind, PomodoroSettings, Quadrant, TaskId, TaskPlacement, UtcTimestamp,
    };
    use uuid::Uuid;

    use crate::SqliteStore;

    fn session_id(value: u128) -> FocusSessionId {
        FocusSessionId::from_uuid(Uuid::from_u128(value))
    }

    fn task_id(value: u128) -> TaskId {
        TaskId::from_uuid(Uuid::from_u128(value))
    }

    fn day() -> LocalDate {
        LocalDate::parse_iso("2026-09-01").expect("valid date")
    }

    #[test]
    fn running_and_paused_sessions_restore_without_counting_paused_time() {
        let store = SqliteStore::open_in_memory().expect("storage opens");
        let mut session = FocusSession::start(
            session_id(1),
            None,
            FocusMode::Stopwatch,
            None,
            PomodoroSettings::default(),
            UtcTimestamp::from_unix_seconds(100),
            day(),
        )
        .expect("session starts");
        store
            .create_focus_session(session.clone())
            .expect("session persists");
        assert_eq!(
            store
                .get_current_focus_session()
                .expect("session loads")
                .expect("current session")
                .elapsed_seconds_at(UtcTimestamp::from_unix_seconds(110)),
            10
        );
        session
            .pause(UtcTimestamp::from_unix_seconds(112))
            .expect("session pauses");
        store
            .transition_focus_session(session, FocusStatus::Running)
            .expect("pause persists");
        assert_eq!(
            store
                .get_current_focus_session()
                .expect("session loads")
                .expect("paused session")
                .elapsed_seconds_at(UtcTimestamp::from_unix_seconds(999)),
            12
        );
    }

    #[test]
    fn one_current_session_is_enforced_by_storage() {
        let store = SqliteStore::open_in_memory().expect("storage opens");
        for value in 1..=2 {
            let session = FocusSession::start(
                session_id(value),
                None,
                FocusMode::Stopwatch,
                None,
                PomodoroSettings::default(),
                UtcTimestamp::from_unix_seconds(100),
                day(),
            )
            .expect("session starts");
            if value == 1 {
                store
                    .create_focus_session(session)
                    .expect("first session persists");
            } else {
                assert!(store.create_focus_session(session).is_err());
            }
        }
    }

    #[test]
    fn task_deletion_detaches_identity_but_keeps_focus_snapshot() {
        let store = SqliteStore::open_in_memory().expect("storage opens");
        let task_id = task_id(9);
        store
            .create_task(
                task_id,
                NewTask::quick_capture("Snapshot title", TaskPlacement::Quadrant(Quadrant::Q2))
                    .expect("valid task"),
                UtcTimestamp::from_unix_seconds(1),
            )
            .expect("task persists");
        let mut session = FocusSession::start(
            session_id(9),
            Some(FocusTaskSnapshot {
                id: Some(task_id),
                title: "Snapshot title".to_owned(),
                quadrant: Some(Quadrant::Q2),
            }),
            FocusMode::Pomodoro,
            Some(PomodoroKind::Focus),
            PomodoroSettings {
                focus_minutes: 1,
                ..PomodoroSettings::default()
            },
            UtcTimestamp::from_unix_seconds(10),
            day(),
        )
        .expect("session starts");
        store
            .create_focus_session(session.clone())
            .expect("session persists");
        store.delete_task(task_id).expect("task deleted");
        session = store
            .get_current_focus_session()
            .expect("session loads")
            .expect("session remains");
        let snapshot = session.record().task.as_ref().expect("snapshot remains");
        assert_eq!(snapshot.id, None);
        assert_eq!(snapshot.title, "Snapshot title");
    }

    #[test]
    fn productive_summary_excludes_breaks_and_cancelled_sessions() {
        let store = SqliteStore::open_in_memory().expect("storage opens");
        let settings = PomodoroSettings {
            focus_minutes: 1,
            short_break_minutes: 1,
            ..PomodoroSettings::default()
        };
        let mut focus = FocusSession::start(
            session_id(20),
            None,
            FocusMode::Pomodoro,
            Some(PomodoroKind::Focus),
            settings,
            UtcTimestamp::from_unix_seconds(0),
            day(),
        )
        .expect("focus starts");
        store
            .create_focus_session(focus.clone())
            .expect("focus persists");
        focus
            .complete(UtcTimestamp::from_unix_seconds(60))
            .expect("focus completes");
        store
            .transition_focus_session(focus, FocusStatus::Running)
            .expect("completion persists");

        let mut break_session = FocusSession::start(
            session_id(21),
            None,
            FocusMode::Pomodoro,
            Some(PomodoroKind::ShortBreak),
            settings,
            UtcTimestamp::from_unix_seconds(100),
            day(),
        )
        .expect("break starts");
        store
            .create_focus_session(break_session.clone())
            .expect("break persists");
        break_session
            .complete(UtcTimestamp::from_unix_seconds(160))
            .expect("break completes");
        store
            .transition_focus_session(break_session, FocusStatus::Running)
            .expect("break completion persists");

        assert_eq!(
            store.productive_focus_summary(day()).expect("summary"),
            FocusDaySummary {
                total_seconds: 60,
                session_count: 1,
            }
        );
    }
}
