//! Transactional `rusqlite` task and settings repository implementation.

use std::{
    path::Path,
    sync::{Mutex, MutexGuard},
};

use quadrant_application::{
    RepositoryError, RepositoryOperation, SettingsRepository, TaskRepository, ThemeMode,
};
use quadrant_domain::{
    NewTask, SortKey, Task, TaskDetailsUpdate, TaskId, TaskPlacement, TaskStatus, UtcTimestamp,
};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use uuid::Uuid;

use crate::{connection, mapping};

const TASK_COLUMNS: &str = "id, title, notes, quadrant, status, planned_on,
    due_at_utc, due_tz, reminder_at_utc, reminder_tz, recurrence_json,
    sort_key, created_at_utc, updated_at_utc, completed_at_utc";

/// Thread-safe `SQLite` adapter. The connection is used only by application-runtime
/// blocking jobs; the mutex preserves one coherent transaction owner.
#[derive(Debug)]
pub struct SqliteStore {
    connection: Mutex<Connection>,
}

impl SqliteStore {
    /// Opens/configures a database and applies all embedded migrations.
    ///
    /// # Errors
    ///
    /// Returns an operation-classified repository error on open/configuration/migration failure.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, RepositoryError> {
        connection::open(path.as_ref()).map(|connection| Self {
            connection: Mutex::new(connection),
        })
    }

    /// Creates a fully configured in-memory database, primarily for deterministic tests.
    ///
    /// # Errors
    ///
    /// Returns an operation-classified repository error on setup/migration failure.
    pub fn open_in_memory() -> Result<Self, RepositoryError> {
        connection::open_in_memory().map(|connection| Self {
            connection: Mutex::new(connection),
        })
    }

    /// Returns the latest applied migration version.
    ///
    /// # Errors
    ///
    /// Returns a read error if the migration table cannot be queried.
    pub fn schema_version(&self) -> Result<i64, RepositoryError> {
        self.lock(RepositoryOperation::ReadTasks)?
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
                [],
                |row| row.get(0),
            )
            .map_err(|error| RepositoryError::new(RepositoryOperation::ReadTasks, error))
    }

    fn lock(
        &self,
        operation: RepositoryOperation,
    ) -> Result<MutexGuard<'_, Connection>, RepositoryError> {
        self.connection
            .lock()
            .map_err(|_| RepositoryError::new(operation, "SQLite connection lock was poisoned"))
    }
}

impl TaskRepository for SqliteStore {
    fn create_task(
        &self,
        id: TaskId,
        draft: NewTask,
        now: UtcTimestamp,
    ) -> Result<Task, RepositoryError> {
        let operation = RepositoryOperation::CreateTask;
        let mut connection = self.lock(operation)?;
        let transaction = immediate_transaction(&mut connection, operation)?;
        let sort_key = next_sort_key(&transaction, draft.placement)
            .map_err(|error| RepositoryError::new(operation, error))?;
        let task = Task::create(id, draft, sort_key, now)
            .map_err(|error| RepositoryError::new(operation, error))?;
        insert_task(&transaction, &task).map_err(|error| RepositoryError::new(operation, error))?;
        transaction
            .commit()
            .map_err(|error| RepositoryError::new(operation, error))?;
        Ok(task)
    }

    fn list_active_tasks(&self) -> Result<Vec<Task>, RepositoryError> {
        let operation = RepositoryOperation::ReadTasks;
        let connection = self.lock(operation)?;
        let sql = format!(
            "SELECT {TASK_COLUMNS} FROM tasks
             WHERE status = 0
             ORDER BY quadrant IS NOT NULL, quadrant, sort_key, created_at_utc, id"
        );
        let mut statement = connection
            .prepare(&sql)
            .map_err(|error| RepositoryError::new(operation, error))?;
        let rows = statement
            .query_map([], mapping::task_from_row)
            .map_err(|error| RepositoryError::new(operation, error))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|error| RepositoryError::new(operation, error))
    }

    fn get_task(&self, id: TaskId) -> Result<Option<Task>, RepositoryError> {
        let operation = RepositoryOperation::ReadTasks;
        let connection = self.lock(operation)?;
        get_task(&connection, id).map_err(|error| RepositoryError::new(operation, error))
    }

    fn move_task(
        &self,
        id: TaskId,
        placement: TaskPlacement,
        now: UtcTimestamp,
    ) -> Result<Task, RepositoryError> {
        let operation = RepositoryOperation::UpdateTask;
        let mut connection = self.lock(operation)?;
        let transaction = immediate_transaction(&mut connection, operation)?;
        let mut task = require_task(&transaction, id, operation)?;
        if task.record().status != TaskStatus::Active {
            return Err(RepositoryError::new(
                operation,
                "completed task cannot be moved",
            ));
        }
        if task.record().placement != placement {
            let sort_key = next_sort_key(&transaction, placement)
                .map_err(|error| RepositoryError::new(operation, error))?;
            task.move_to(placement, sort_key, now);
            update_task_row(&transaction, &task)
                .map_err(|error| RepositoryError::new(operation, error))?;
        }
        transaction
            .commit()
            .map_err(|error| RepositoryError::new(operation, error))?;
        Ok(task)
    }

    fn update_task(
        &self,
        id: TaskId,
        mut update: TaskDetailsUpdate,
        now: UtcTimestamp,
    ) -> Result<Task, RepositoryError> {
        let operation = RepositoryOperation::UpdateTask;
        let mut connection = self.lock(operation)?;
        let transaction = immediate_transaction(&mut connection, operation)?;
        let mut task = require_task(&transaction, id, operation)?;
        if task.record().placement != update.placement {
            let sort_key = next_sort_key(&transaction, update.placement)
                .map_err(|error| RepositoryError::new(operation, error))?;
            task.move_to(update.placement, sort_key, now);
            update.placement = task.record().placement;
        }
        task.update_details(update, now)
            .map_err(|error| RepositoryError::new(operation, error))?;
        update_task_row(&transaction, &task)
            .map_err(|error| RepositoryError::new(operation, error))?;
        transaction
            .commit()
            .map_err(|error| RepositoryError::new(operation, error))?;
        Ok(task)
    }

    fn complete_task(&self, id: TaskId, now: UtcTimestamp) -> Result<Task, RepositoryError> {
        let operation = RepositoryOperation::TransitionTask;
        let mut connection = self.lock(operation)?;
        let transaction = immediate_transaction(&mut connection, operation)?;
        let mut task = require_task(&transaction, id, operation)?;
        let snapshot = task
            .complete(now)
            .map_err(|error| RepositoryError::new(operation, error))?;
        update_task_row(&transaction, &task)
            .map_err(|error| RepositoryError::new(operation, error))?;
        transaction
            .execute(
                "INSERT INTO task_completion_events(
                     id, task_id, task_title_snapshot, quadrant_snapshot, completed_at_utc,
                     recurrence_occurrence_key
                 ) VALUES (?1, ?2, ?3, ?4, ?5, NULL)",
                params![
                    Uuid::now_v7().to_string(),
                    snapshot.task_id.to_string(),
                    snapshot.title.as_str(),
                    mapping::placement_to_db(snapshot.placement),
                    snapshot.completed_at.unix_seconds(),
                ],
            )
            .map_err(|error| RepositoryError::new(operation, error))?;
        transaction
            .commit()
            .map_err(|error| RepositoryError::new(operation, error))?;
        Ok(task)
    }

    fn reopen_task(&self, id: TaskId, now: UtcTimestamp) -> Result<Task, RepositoryError> {
        let operation = RepositoryOperation::TransitionTask;
        let mut connection = self.lock(operation)?;
        let transaction = immediate_transaction(&mut connection, operation)?;
        let mut task = require_task(&transaction, id, operation)?;
        let completed_at = task
            .record()
            .completed_at
            .ok_or_else(|| RepositoryError::new(operation, "active task cannot be reopened"))?;
        task.reopen(now)
            .map_err(|error| RepositoryError::new(operation, error))?;
        update_task_row(&transaction, &task)
            .map_err(|error| RepositoryError::new(operation, error))?;
        transaction
            .execute(
                "DELETE FROM task_completion_events
                 WHERE id = (
                     SELECT id FROM task_completion_events
                     WHERE task_id = ?1 AND completed_at_utc = ?2
                     ORDER BY id DESC LIMIT 1
                 )",
                params![id.to_string(), completed_at.unix_seconds()],
            )
            .map_err(|error| RepositoryError::new(operation, error))?;
        transaction
            .commit()
            .map_err(|error| RepositoryError::new(operation, error))?;
        Ok(task)
    }

    fn delete_task(&self, id: TaskId) -> Result<(), RepositoryError> {
        let operation = RepositoryOperation::DeleteTask;
        let connection = self.lock(operation)?;
        let affected = connection
            .execute("DELETE FROM tasks WHERE id = ?1", [id.to_string()])
            .map_err(|error| RepositoryError::new(operation, error))?;
        if affected == 0 {
            return Err(RepositoryError::new(operation, "task was not found"));
        }
        Ok(())
    }
}

impl SettingsRepository for SqliteStore {
    fn load_theme_mode(&self) -> Result<Option<ThemeMode>, RepositoryError> {
        let operation = RepositoryOperation::ReadSettings;
        let connection = self.lock(operation)?;
        let value = connection
            .query_row(
                "SELECT value_json FROM settings WHERE key = 'appearance.theme'",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| RepositoryError::new(operation, error))?;
        value
            .map(|json| {
                serde_json::from_str::<String>(&json)
                    .map_err(|error| RepositoryError::new(operation, error))
                    .and_then(|mode| match mode.as_str() {
                        "system" => Ok(ThemeMode::System),
                        "light" => Ok(ThemeMode::Light),
                        "dark" => Ok(ThemeMode::Dark),
                        _ => Err(RepositoryError::new(operation, "unknown stored theme mode")),
                    })
            })
            .transpose()
    }

    fn save_theme_mode(
        &self,
        theme_mode: ThemeMode,
        now: UtcTimestamp,
    ) -> Result<(), RepositoryError> {
        let operation = RepositoryOperation::WriteSettings;
        let value = match theme_mode {
            ThemeMode::System => "system",
            ThemeMode::Light => "light",
            ThemeMode::Dark => "dark",
        };
        let json =
            serde_json::to_string(value).map_err(|error| RepositoryError::new(operation, error))?;
        self.lock(operation)?
            .execute(
                "INSERT INTO settings(key, value_json, updated_at_utc)
                 VALUES ('appearance.theme', ?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET
                     value_json = excluded.value_json,
                     updated_at_utc = excluded.updated_at_utc",
                params![json, now.unix_seconds()],
            )
            .map_err(|error| RepositoryError::new(operation, error))?;
        Ok(())
    }
}

fn immediate_transaction(
    connection: &mut Connection,
    operation: RepositoryOperation,
) -> Result<Transaction<'_>, RepositoryError> {
    connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| RepositoryError::new(operation, error))
}

fn next_sort_key(connection: &Connection, placement: TaskPlacement) -> rusqlite::Result<SortKey> {
    let maximum = connection.query_row(
        "SELECT MAX(sort_key) FROM tasks WHERE status = 0 AND quadrant IS ?1",
        [mapping::placement_to_db(placement)],
        |row| row.get::<_, Option<i64>>(0),
    )?;
    maximum.map_or(Ok(SortKey::INITIAL), |value| {
        SortKey::from_i64(value).checked_next().ok_or_else(|| {
            rusqlite::Error::InvalidParameterName("task sort key overflow".to_owned())
        })
    })
}

fn get_task(connection: &Connection, id: TaskId) -> rusqlite::Result<Option<Task>> {
    let sql = format!("SELECT {TASK_COLUMNS} FROM tasks WHERE id = ?1");
    connection
        .query_row(&sql, [id.to_string()], mapping::task_from_row)
        .optional()
}

fn require_task(
    connection: &Connection,
    id: TaskId,
    operation: RepositoryOperation,
) -> Result<Task, RepositoryError> {
    get_task(connection, id)
        .map_err(|error| RepositoryError::new(operation, error))?
        .ok_or_else(|| RepositoryError::new(operation, "task was not found"))
}

fn insert_task(connection: &Connection, task: &Task) -> rusqlite::Result<()> {
    let record = task.record();
    let recurrence = record
        .recurrence
        .map(|rule| serde_json::to_string(&rule))
        .transpose()
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
    let due_at = record.due.as_ref().map(|value| value.at_utc.unix_seconds());
    let due_tz = record.due.as_ref().map(|value| value.time_zone.as_str());
    let reminder_at = record
        .reminder
        .as_ref()
        .map(|value| value.at_utc.unix_seconds());
    let reminder_tz = record
        .reminder
        .as_ref()
        .map(|value| value.time_zone.as_str());
    connection.execute(
        "INSERT INTO tasks(
             id, title, notes, quadrant, status, planned_on, due_at_utc, due_tz,
             reminder_at_utc, reminder_tz, recurrence_json, sort_key,
             created_at_utc, updated_at_utc, completed_at_utc
         ) VALUES (
             ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15
         )",
        params![
            record.id.to_string(),
            record.title.as_str(),
            record.notes,
            mapping::placement_to_db(record.placement),
            mapping::status_to_db(record.status),
            record.planned_on.map(|date| date.to_string()),
            due_at,
            due_tz,
            reminder_at,
            reminder_tz,
            recurrence,
            record.sort_key.value(),
            record.created_at.unix_seconds(),
            record.updated_at.unix_seconds(),
            record.completed_at.map(UtcTimestamp::unix_seconds),
        ],
    )?;
    Ok(())
}

fn update_task_row(connection: &Connection, task: &Task) -> rusqlite::Result<()> {
    let record = task.record();
    let recurrence = record
        .recurrence
        .map(|rule| serde_json::to_string(&rule))
        .transpose()
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
    let due_at = record.due.as_ref().map(|value| value.at_utc.unix_seconds());
    let due_tz = record.due.as_ref().map(|value| value.time_zone.as_str());
    let reminder_at = record
        .reminder
        .as_ref()
        .map(|value| value.at_utc.unix_seconds());
    let reminder_tz = record
        .reminder
        .as_ref()
        .map(|value| value.time_zone.as_str());
    connection.execute(
        "UPDATE tasks SET
             title = ?2, notes = ?3, quadrant = ?4, status = ?5, planned_on = ?6,
             due_at_utc = ?7, due_tz = ?8, reminder_at_utc = ?9, reminder_tz = ?10,
             recurrence_json = ?11, sort_key = ?12, updated_at_utc = ?13,
             completed_at_utc = ?14
         WHERE id = ?1",
        params![
            record.id.to_string(),
            record.title.as_str(),
            record.notes,
            mapping::placement_to_db(record.placement),
            mapping::status_to_db(record.status),
            record.planned_on.map(|date| date.to_string()),
            due_at,
            due_tz,
            reminder_at,
            reminder_tz,
            recurrence,
            record.sort_key.value(),
            record.updated_at.unix_seconds(),
            record.completed_at.map(UtcTimestamp::unix_seconds),
        ],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use quadrant_application::{SettingsRepository, TaskRepository, ThemeMode};
    use quadrant_domain::{NewTask, Quadrant, TaskId, TaskPlacement, TaskStatus, UtcTimestamp};
    use uuid::Uuid;

    use super::SqliteStore;

    fn task_id(value: u128) -> TaskId {
        TaskId::from_uuid(Uuid::from_u128(value))
    }

    #[test]
    fn empty_database_migrates_and_enables_foreign_keys() {
        let store = SqliteStore::open_in_memory().expect("storage opens");
        assert_eq!(store.schema_version().expect("schema version"), 1);
        let enabled = store
            .connection
            .lock()
            .expect("connection lock")
            .query_row("PRAGMA foreign_keys", [], |row| row.get::<_, i64>(0))
            .expect("foreign key setting");
        assert_eq!(enabled, 1);
    }

    #[test]
    fn create_move_complete_reopen_and_delete_are_transactional() {
        let store = SqliteStore::open_in_memory().expect("storage opens");
        let id = task_id(1);
        let draft =
            NewTask::quick_capture("Persist me", TaskPlacement::Inbox).expect("valid task draft");
        store
            .create_task(id, draft, UtcTimestamp::from_unix_seconds(10))
            .expect("task created");
        let moved = store
            .move_task(
                id,
                TaskPlacement::Quadrant(Quadrant::Q1),
                UtcTimestamp::from_unix_seconds(11),
            )
            .expect("task moved");
        assert_eq!(
            moved.record().placement,
            TaskPlacement::Quadrant(Quadrant::Q1)
        );

        let completed = store
            .complete_task(id, UtcTimestamp::from_unix_seconds(12))
            .expect("task completed");
        assert_eq!(completed.record().status, TaskStatus::Completed);
        let reopened = store
            .reopen_task(id, UtcTimestamp::from_unix_seconds(13))
            .expect("task reopened");
        assert_eq!(reopened.record().status, TaskStatus::Active);

        store.delete_task(id).expect("task deleted");
        assert!(store.get_task(id).expect("task query").is_none());
    }

    #[test]
    fn failed_completion_rolls_back_task_and_history() {
        let store = SqliteStore::open_in_memory().expect("storage opens");
        let id = task_id(2);
        store
            .create_task(
                id,
                NewTask::quick_capture("Rollback", TaskPlacement::Inbox).expect("valid draft"),
                UtcTimestamp::from_unix_seconds(20),
            )
            .expect("task created");
        store
            .connection
            .lock()
            .expect("connection lock")
            .execute_batch(
                "CREATE TRIGGER reject_completion
                 BEFORE INSERT ON task_completion_events
                 BEGIN SELECT RAISE(ABORT, 'test rollback'); END;",
            )
            .expect("failure trigger installed");

        assert!(
            store
                .complete_task(id, UtcTimestamp::from_unix_seconds(21))
                .is_err()
        );
        let task = store
            .get_task(id)
            .expect("task query")
            .expect("task exists");
        assert_eq!(task.record().status, TaskStatus::Active);
        assert_eq!(task.record().completed_at, None);
    }

    #[test]
    fn theme_setting_round_trips_as_validated_json() {
        let store = SqliteStore::open_in_memory().expect("storage opens");
        assert_eq!(store.load_theme_mode().expect("theme query"), None);
        store
            .save_theme_mode(ThemeMode::Dark, UtcTimestamp::from_unix_seconds(30))
            .expect("theme saved");
        assert_eq!(
            store.load_theme_mode().expect("theme query"),
            Some(ThemeMode::Dark)
        );
    }
}
