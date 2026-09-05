//! Transactional `rusqlite` task and settings repository implementation.

use std::{
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard},
};

use quadrant_application::{
    DesktopSettings, ReminderRepository, ReorderDirection, RepositoryError, RepositoryOperation,
    SettingsRepository, TaskRepository, ThemeMode, TodayRepository, WindowCloseBehavior,
    WindowMinimizeBehavior,
};
use quadrant_domain::{
    LocalDate, NewTask, PomodoroSettings, SortKey, Task, TaskDetailsUpdate, TaskId, TaskPlacement,
    TaskStatus, UtcTimestamp,
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
    pub(crate) database_path: Option<PathBuf>,
}

impl SqliteStore {
    /// Opens/configures a database and applies all embedded migrations.
    ///
    /// # Errors
    ///
    /// Returns an operation-classified repository error on open/configuration/migration failure.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, RepositoryError> {
        let path = path.as_ref();
        connection::open(path).map(|connection| Self {
            connection: Mutex::new(connection),
            database_path: Some(path.to_path_buf()),
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
            database_path: None,
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

    pub(crate) fn lock(
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

    fn reorder_task(
        &self,
        id: TaskId,
        direction: ReorderDirection,
        now: UtcTimestamp,
    ) -> Result<Task, RepositoryError> {
        let operation = RepositoryOperation::UpdateTask;
        let mut connection = self.lock(operation)?;
        let transaction = immediate_transaction(&mut connection, operation)?;
        let mut task = require_task(&transaction, id, operation)?;
        if task.record().status != TaskStatus::Active {
            return Err(RepositoryError::new(
                operation,
                "completed task cannot be reordered",
            ));
        }

        let mut ordered = ordered_task_keys(&transaction, task.record().placement)
            .map_err(|error| RepositoryError::new(operation, error))?;
        let task_id = id.to_string();
        let Some(current_index) = ordered.iter().position(|(row_id, _)| row_id == &task_id) else {
            return Err(RepositoryError::new(
                operation,
                "task was not found in its placement",
            ));
        };
        let insertion_index = match direction {
            ReorderDirection::Up if current_index > 0 => current_index - 1,
            ReorderDirection::Down if current_index + 1 < ordered.len() => current_index + 1,
            ReorderDirection::Up | ReorderDirection::Down => {
                transaction
                    .commit()
                    .map_err(|error| RepositoryError::new(operation, error))?;
                return Ok(task);
            }
        };
        ordered.remove(current_index);

        let sort_key = if let Some(key) = insertion_sort_key(&ordered, insertion_index) {
            key
        } else {
            rebalance_task_keys(&transaction, &mut ordered)
                .map_err(|error| RepositoryError::new(operation, error))?;
            insertion_sort_key(&ordered, insertion_index).ok_or_else(|| {
                RepositoryError::new(operation, "task order key could not be allocated")
            })?
        };
        task.move_to(task.record().placement, SortKey::from_i64(sort_key), now);
        update_task_row(&transaction, &task)
            .map_err(|error| RepositoryError::new(operation, error))?;
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
        let previous_reminder = task.record().reminder.clone();
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
        if previous_reminder != task.record().reminder {
            transaction
                .execute(
                    "UPDATE tasks SET reminder_delivered_for_utc = NULL WHERE id = ?1",
                    [id.to_string()],
                )
                .map_err(|error| RepositoryError::new(operation, error))?;
        }
        transaction
            .commit()
            .map_err(|error| RepositoryError::new(operation, error))?;
        Ok(task)
    }

    fn complete_task(
        &self,
        id: TaskId,
        next_occurrence_id: TaskId,
        now: UtcTimestamp,
        completed_local_date: LocalDate,
    ) -> Result<Task, RepositoryError> {
        let operation = RepositoryOperation::TransitionTask;
        let mut connection = self.lock(operation)?;
        let transaction = immediate_transaction(&mut connection, operation)?;
        let mut task = require_task(&transaction, id, operation)?;
        let due_at_snapshot = task.record().due.as_ref().map(|due| due.at_utc);
        let planned_on_snapshot = task.record().planned_on;
        let was_overdue = due_at_snapshot.is_some_and(|due| due < now);
        let next_draft = task
            .next_recurrence_draft()
            .map_err(|error| RepositoryError::new(operation, error))?;
        let snapshot = task
            .complete(now)
            .map_err(|error| RepositoryError::new(operation, error))?;
        update_task_row(&transaction, &task)
            .map_err(|error| RepositoryError::new(operation, error))?;
        transaction
            .execute(
                "INSERT INTO task_completion_events(
                     id, task_id, task_title_snapshot, quadrant_snapshot, completed_at_utc,
                     recurrence_occurrence_key, completed_local_date,
                     due_at_utc_snapshot, planned_on_snapshot, was_overdue
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    Uuid::now_v7().to_string(),
                    snapshot.task_id.to_string(),
                    snapshot.title.as_str(),
                    mapping::placement_to_db(snapshot.placement),
                    snapshot.completed_at.unix_seconds(),
                    task.record()
                        .recurrence
                        .map(|_| snapshot.task_id.to_string()),
                    completed_local_date.to_string(),
                    due_at_snapshot.map(UtcTimestamp::unix_seconds),
                    planned_on_snapshot.map(|date| date.to_string()),
                    was_overdue,
                ],
            )
            .map_err(|error| RepositoryError::new(operation, error))?;
        if let Some(draft) = next_draft {
            let sort_key = next_sort_key(&transaction, draft.placement)
                .map_err(|error| RepositoryError::new(operation, error))?;
            let next_task = Task::create(next_occurrence_id, draft, sort_key, now)
                .map_err(|error| RepositoryError::new(operation, error))?;
            insert_task(&transaction, &next_task)
                .map_err(|error| RepositoryError::new(operation, error))?;
        }
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
                "UPDATE tasks SET reminder_delivered_for_utc = reminder_at_utc WHERE id = ?1",
                [id.to_string()],
            )
            .map_err(|error| RepositoryError::new(operation, error))?;
        transaction
            .execute(
                "UPDATE task_completion_events SET reverted_at_utc = ?3
                 WHERE id = (
                     SELECT id FROM task_completion_events
                     WHERE task_id = ?1 AND completed_at_utc = ?2 AND reverted_at_utc IS NULL
                     ORDER BY id DESC LIMIT 1
                 )",
                params![
                    id.to_string(),
                    completed_at.unix_seconds(),
                    now.unix_seconds()
                ],
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

impl TodayRepository for SqliteStore {
    fn list_today_candidates(&self, local_today: LocalDate) -> Result<Vec<Task>, RepositoryError> {
        let operation = RepositoryOperation::ReadTasks;
        let connection = self.lock(operation)?;
        let sql = format!(
            "SELECT {TASK_COLUMNS} FROM tasks
             WHERE status = 0 AND (due_at_utc IS NOT NULL OR planned_on <= ?1)
             ORDER BY due_at_utc, planned_on, created_at_utc, id"
        );
        let mut statement = connection
            .prepare(&sql)
            .map_err(|error| RepositoryError::new(operation, error))?;
        let rows = statement
            .query_map([local_today.to_string()], mapping::task_from_row)
            .map_err(|error| RepositoryError::new(operation, error))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|error| RepositoryError::new(operation, error))
    }
}

impl ReminderRepository for SqliteStore {
    fn list_pending_reminders(&self) -> Result<Vec<Task>, RepositoryError> {
        let operation = RepositoryOperation::ReadReminders;
        let connection = self.lock(operation)?;
        let sql = format!(
            "SELECT {TASK_COLUMNS} FROM tasks
             WHERE status = 0 AND reminder_at_utc IS NOT NULL
               AND reminder_delivered_for_utc IS NOT reminder_at_utc
             ORDER BY reminder_at_utc, id"
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

    fn clear_reminder_if_matches(
        &self,
        id: TaskId,
        scheduled_for: UtcTimestamp,
        now: UtcTimestamp,
    ) -> Result<bool, RepositoryError> {
        let operation = RepositoryOperation::UpdateReminder;
        let mut connection = self.lock(operation)?;
        let transaction = immediate_transaction(&mut connection, operation)?;
        let Some(mut task) =
            get_task(&transaction, id).map_err(|error| RepositoryError::new(operation, error))?
        else {
            transaction
                .commit()
                .map_err(|error| RepositoryError::new(operation, error))?;
            return Ok(false);
        };
        let matches = task.record().status == TaskStatus::Active
            && task
                .record()
                .reminder
                .as_ref()
                .is_some_and(|reminder| reminder.at_utc == scheduled_for);
        if matches {
            if task.record().recurrence.is_some() {
                transaction
                    .execute(
                        "UPDATE tasks SET reminder_delivered_for_utc = ?2
                         WHERE id = ?1 AND status = 0 AND reminder_at_utc = ?2",
                        params![id.to_string(), scheduled_for.unix_seconds()],
                    )
                    .map_err(|error| RepositoryError::new(operation, error))?;
            } else {
                task.clear_reminder(now);
                update_task_row(&transaction, &task)
                    .map_err(|error| RepositoryError::new(operation, error))?;
            }
        }
        transaction
            .commit()
            .map_err(|error| RepositoryError::new(operation, error))?;
        Ok(matches)
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

    fn load_desktop_settings(&self) -> Result<DesktopSettings, RepositoryError> {
        let operation = RepositoryOperation::ReadSettings;
        let connection = self.lock(operation)?;
        let defaults = DesktopSettings::default();
        Ok(DesktopSettings {
            launch_at_startup: load_bool_setting(
                &connection,
                "desktop.launch_at_startup",
                operation,
            )?
            .unwrap_or(defaults.launch_at_startup),
            start_hidden: load_bool_setting(&connection, "desktop.start_hidden", operation)?
                .unwrap_or(defaults.start_hidden),
            close_behavior: if load_bool_setting(&connection, "desktop.close_to_tray", operation)?
                .unwrap_or(defaults.close_behavior == WindowCloseBehavior::CloseGuiKeepAgent)
            {
                WindowCloseBehavior::CloseGuiKeepAgent
            } else {
                WindowCloseBehavior::Quit
            },
            // Retired preference: read old profiles without restoring hide-on-minimize.
            minimize_behavior: WindowMinimizeBehavior::Taskbar,
        })
    }

    fn save_desktop_settings(
        &self,
        settings: DesktopSettings,
        now: UtcTimestamp,
    ) -> Result<(), RepositoryError> {
        let operation = RepositoryOperation::WriteSettings;
        let mut connection = self.lock(operation)?;
        let transaction = immediate_transaction(&mut connection, operation)?;
        let values = [
            ("desktop.launch_at_startup", settings.launch_at_startup),
            ("desktop.start_hidden", settings.start_hidden),
            (
                "desktop.close_to_tray",
                settings.close_behavior == WindowCloseBehavior::CloseGuiKeepAgent,
            ),
            ("desktop.minimize_to_tray", false),
        ];
        for (key, value) in values {
            let json = serde_json::to_string(&value)
                .map_err(|error| RepositoryError::new(operation, error))?;
            transaction
                .execute(
                    "INSERT INTO settings(key, value_json, updated_at_utc)
                     VALUES (?1, ?2, ?3)
                     ON CONFLICT(key) DO UPDATE SET
                         value_json = excluded.value_json,
                         updated_at_utc = excluded.updated_at_utc",
                    params![key, json, now.unix_seconds()],
                )
                .map_err(|error| RepositoryError::new(operation, error))?;
        }
        transaction
            .commit()
            .map_err(|error| RepositoryError::new(operation, error))
    }

    fn load_pomodoro_settings(&self) -> Result<PomodoroSettings, RepositoryError> {
        let operation = RepositoryOperation::ReadSettings;
        let connection = self.lock(operation)?;
        let defaults = PomodoroSettings::default();
        let settings = PomodoroSettings {
            focus_minutes: load_json_setting(&connection, "focus.focus_minutes", operation)?
                .unwrap_or(defaults.focus_minutes),
            short_break_minutes: load_json_setting(
                &connection,
                "focus.short_break_minutes",
                operation,
            )?
            .unwrap_or(defaults.short_break_minutes),
            long_break_minutes: load_json_setting(
                &connection,
                "focus.long_break_minutes",
                operation,
            )?
            .unwrap_or(defaults.long_break_minutes),
            long_break_interval: load_json_setting(
                &connection,
                "focus.long_break_interval",
                operation,
            )?
            .unwrap_or(defaults.long_break_interval),
            auto_start_break: load_json_setting(&connection, "focus.auto_start_break", operation)?
                .unwrap_or(defaults.auto_start_break),
            auto_start_focus: load_json_setting(&connection, "focus.auto_start_focus", operation)?
                .unwrap_or(defaults.auto_start_focus),
        };
        settings
            .validate()
            .map_err(|error| RepositoryError::new(operation, error))
    }

    fn save_pomodoro_settings(
        &self,
        settings: PomodoroSettings,
        now: UtcTimestamp,
    ) -> Result<(), RepositoryError> {
        let operation = RepositoryOperation::WriteSettings;
        let settings = settings
            .validate()
            .map_err(|error| RepositoryError::new(operation, error))?;
        let mut connection = self.lock(operation)?;
        let transaction = immediate_transaction(&mut connection, operation)?;
        let values = [
            (
                "focus.focus_minutes",
                serde_json::to_string(&settings.focus_minutes),
            ),
            (
                "focus.short_break_minutes",
                serde_json::to_string(&settings.short_break_minutes),
            ),
            (
                "focus.long_break_minutes",
                serde_json::to_string(&settings.long_break_minutes),
            ),
            (
                "focus.long_break_interval",
                serde_json::to_string(&settings.long_break_interval),
            ),
            (
                "focus.auto_start_break",
                serde_json::to_string(&settings.auto_start_break),
            ),
            (
                "focus.auto_start_focus",
                serde_json::to_string(&settings.auto_start_focus),
            ),
        ];
        for (key, json) in values {
            let json = json.map_err(|error| RepositoryError::new(operation, error))?;
            transaction
                .execute(
                    "INSERT INTO settings(key, value_json, updated_at_utc)
                     VALUES (?1, ?2, ?3)
                     ON CONFLICT(key) DO UPDATE SET
                         value_json = excluded.value_json,
                         updated_at_utc = excluded.updated_at_utc",
                    params![key, json, now.unix_seconds()],
                )
                .map_err(|error| RepositoryError::new(operation, error))?;
        }
        transaction
            .commit()
            .map_err(|error| RepositoryError::new(operation, error))
    }
}

fn load_bool_setting(
    connection: &Connection,
    key: &str,
    operation: RepositoryOperation,
) -> Result<Option<bool>, RepositoryError> {
    connection
        .query_row(
            "SELECT value_json FROM settings WHERE key = ?1",
            [key],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| RepositoryError::new(operation, error))?
        .map(|json| {
            serde_json::from_str::<bool>(&json)
                .map_err(|error| RepositoryError::new(operation, error))
        })
        .transpose()
}

fn load_json_setting<T: serde::de::DeserializeOwned>(
    connection: &Connection,
    key: &str,
    operation: RepositoryOperation,
) -> Result<Option<T>, RepositoryError> {
    connection
        .query_row(
            "SELECT value_json FROM settings WHERE key = ?1",
            [key],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| RepositoryError::new(operation, error))?
        .map(|json| {
            serde_json::from_str::<T>(&json).map_err(|error| RepositoryError::new(operation, error))
        })
        .transpose()
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

fn ordered_task_keys(
    connection: &Connection,
    placement: TaskPlacement,
) -> rusqlite::Result<Vec<(String, i64)>> {
    let mut statement = connection.prepare(
        "SELECT id, sort_key FROM tasks
         WHERE status = 0 AND quadrant IS ?1
         ORDER BY sort_key, created_at_utc, id",
    )?;
    statement
        .query_map([mapping::placement_to_db(placement)], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?
        .collect()
}

fn insertion_sort_key(ordered: &[(String, i64)], insertion_index: usize) -> Option<i64> {
    let before = insertion_index
        .checked_sub(1)
        .and_then(|index| ordered.get(index))
        .map(|(_, key)| *key);
    let after = ordered.get(insertion_index).map(|(_, key)| *key);
    match (before, after) {
        (None, None) => Some(SortKey::INITIAL.value()),
        (None, Some(after)) => after.checked_sub(SortKey::STEP),
        (Some(before), None) => before.checked_add(SortKey::STEP),
        (Some(before), Some(after)) => {
            let gap = after.checked_sub(before)?;
            (gap > 1).then(|| before + gap / 2)
        }
    }
}

fn rebalance_task_keys(
    connection: &Connection,
    ordered: &mut [(String, i64)],
) -> rusqlite::Result<()> {
    for (index, (id, key)) in ordered.iter_mut().enumerate() {
        let ordinal = i64::try_from(index + 1)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
        *key = ordinal.checked_mul(SortKey::STEP).ok_or_else(|| {
            rusqlite::Error::InvalidParameterName("task sort key overflow".to_owned())
        })?;
        connection.execute(
            "UPDATE tasks SET sort_key = ?2 WHERE id = ?1",
            params![id.as_str(), *key],
        )?;
    }
    Ok(())
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
    use quadrant_application::{
        DesktopSettings, ReminderRepository, ReorderDirection, SettingsRepository, TaskRepository,
        ThemeMode, TodayRepository, WindowCloseBehavior, WindowMinimizeBehavior,
    };
    use quadrant_domain::{
        LocalDate, NewTask, PomodoroSettings, Quadrant, RecurrencePattern, RecurrenceRule,
        ScheduledInstant, TaskDetailsUpdate, TaskId, TaskPlacement, TaskStatus, TaskTitle,
        TimeZoneId, UtcTimestamp,
    };
    use uuid::Uuid;

    use super::SqliteStore;

    fn task_id(value: u128) -> TaskId {
        TaskId::from_uuid(Uuid::from_u128(value))
    }

    fn rename_without_rescheduling(
        store: &SqliteStore,
        id: TaskId,
        now: UtcTimestamp,
    ) -> Result<(), quadrant_application::RepositoryError> {
        let task = store.get_task(id)?.expect("source exists");
        store.update_task(
            id,
            TaskDetailsUpdate {
                title: TaskTitle::new("Recurring renamed").expect("valid title"),
                notes: "notes changed after delivery".to_owned(),
                placement: task.record().placement,
                planned_on: task.record().planned_on,
                due: task.record().due.clone(),
                reminder: task.record().reminder.clone(),
                recurrence: task.record().recurrence,
            },
            now,
        )?;
        Ok(())
    }

    fn assert_recurring_completion(
        store: &SqliteStore,
        source_id: TaskId,
        next_id: TaskId,
        previous_reminder: UtcTimestamp,
    ) {
        let source = store
            .get_task(source_id)
            .expect("source query")
            .expect("source exists");
        assert_eq!(source.record().status, TaskStatus::Completed);
        let next = store
            .get_task(next_id)
            .expect("next query")
            .expect("next occurrence exists");
        assert_eq!(next.record().status, TaskStatus::Active);
        assert_eq!(
            next.record().planned_on.expect("next plan").to_string(),
            "2026-02-28"
        );
        assert!(
            next.record()
                .reminder
                .as_ref()
                .is_some_and(|reminder| reminder.at_utc > previous_reminder)
        );
        assert_eq!(
            store
                .list_pending_reminders()
                .expect("next reminder pending")
                .len(),
            1
        );
        let recurrence_key = store
            .connection
            .lock()
            .expect("connection lock")
            .query_row(
                "SELECT recurrence_occurrence_key FROM task_completion_events
                 WHERE task_id = ?1",
                [source_id.to_string()],
                |row| row.get::<_, Option<String>>(0),
            )
            .expect("completion event");
        assert_eq!(recurrence_key, Some(source_id.to_string()));
    }

    #[test]
    fn empty_database_migrates_and_enables_foreign_keys() {
        let store = SqliteStore::open_in_memory().expect("storage opens");
        assert_eq!(store.schema_version().expect("schema version"), 4);
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
        let mut draft =
            NewTask::quick_capture("Persist me", TaskPlacement::Inbox).expect("valid task draft");
        draft.planned_on = Some(LocalDate::parse_iso("2026-09-01").expect("valid date"));
        draft.due = Some(ScheduledInstant {
            at_utc: UtcTimestamp::from_unix_seconds(11),
            time_zone: TimeZoneId::new("Asia/Shanghai").expect("valid timezone"),
        });
        draft.reminder = Some(ScheduledInstant {
            at_utc: UtcTimestamp::from_unix_seconds(11),
            time_zone: TimeZoneId::new("Asia/Shanghai").expect("valid timezone"),
        });
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
            .complete_task(
                id,
                task_id(101),
                UtcTimestamp::from_unix_seconds(12),
                LocalDate::parse_iso("2026-09-01").expect("valid date"),
            )
            .expect("task completed");
        assert_eq!(completed.record().status, TaskStatus::Completed);
        let reopened = store
            .reopen_task(id, UtcTimestamp::from_unix_seconds(13))
            .expect("task reopened");
        assert_eq!(reopened.record().status, TaskStatus::Active);
        assert!(
            store
                .list_pending_reminders()
                .expect("restored reminder query")
                .is_empty(),
            "restoring a task must not revive its old reminder"
        );
        let completion_snapshot = store
            .connection
            .lock()
            .expect("connection lock")
            .query_row(
                "SELECT task_title_snapshot, quadrant_snapshot, completed_local_date,
                        due_at_utc_snapshot, planned_on_snapshot, was_overdue, reverted_at_utc
                 FROM task_completion_events WHERE task_id = ?1",
                [id.to_string()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, bool>(5)?,
                        row.get::<_, i64>(6)?,
                    ))
                },
            )
            .expect("completion snapshot");
        assert_eq!(
            completion_snapshot,
            (
                "Persist me".to_owned(),
                1,
                "2026-09-01".to_owned(),
                11,
                "2026-09-01".to_owned(),
                true,
                13,
            )
        );

        store.delete_task(id).expect("task deleted");
        assert!(store.get_task(id).expect("task query").is_none());
        let retained_task_id = store
            .connection
            .lock()
            .expect("connection lock")
            .query_row("SELECT task_id FROM task_completion_events", [], |row| {
                row.get::<_, Option<String>>(0)
            })
            .expect("retained completion snapshot");
        assert_eq!(retained_task_id, None);
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
                .complete_task(
                    id,
                    task_id(102),
                    UtcTimestamp::from_unix_seconds(21),
                    LocalDate::parse_iso("2026-09-01").expect("valid date"),
                )
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
    fn full_task_details_update_round_trips_transactionally() {
        let store = SqliteStore::open_in_memory().expect("storage opens");
        let id = task_id(3);
        store
            .create_task(
                id,
                NewTask::quick_capture("Original", TaskPlacement::Inbox).expect("valid draft"),
                UtcTimestamp::from_unix_seconds(30),
            )
            .expect("task created");
        let updated = store
            .update_task(
                id,
                TaskDetailsUpdate {
                    title: TaskTitle::new("Edited").expect("valid title"),
                    notes: "Stored notes".to_owned(),
                    placement: TaskPlacement::Quadrant(Quadrant::Q2),
                    planned_on: Some(LocalDate::parse_iso("2026-09-04").expect("valid date")),
                    due: Some(ScheduledInstant {
                        at_utc: UtcTimestamp::from_unix_seconds(1_788_451_200),
                        time_zone: TimeZoneId::new("Asia/Shanghai").expect("valid timezone"),
                    }),
                    reminder: Some(ScheduledInstant {
                        at_utc: UtcTimestamp::from_unix_seconds(1_788_447_600),
                        time_zone: TimeZoneId::new("Asia/Shanghai").expect("valid timezone"),
                    }),
                    recurrence: Some(
                        RecurrenceRule::new(RecurrencePattern::Weekly).expect("valid recurrence"),
                    ),
                },
                UtcTimestamp::from_unix_seconds(31),
            )
            .expect("task updated");

        assert_eq!(updated.record().title.as_str(), "Edited");
        assert_eq!(updated.record().notes, "Stored notes");
        assert_eq!(
            updated.record().placement,
            TaskPlacement::Quadrant(Quadrant::Q2)
        );
        let restored = store
            .get_task(id)
            .expect("task query")
            .expect("task exists");
        assert_eq!(restored, updated);
    }

    #[test]
    fn today_candidates_include_active_due_or_current_plan_only() {
        let store = SqliteStore::open_in_memory().expect("storage opens");
        let today = LocalDate::parse_iso("2026-09-02").expect("valid date");
        let cases = [
            (task_id(30), "Due", None, Some(300)),
            (task_id(31), "Old plan", Some("2026-09-01"), None),
            (task_id(32), "Today plan", Some("2026-09-02"), None),
            (task_id(33), "Future plan", Some("2026-09-03"), None),
        ];
        for (index, (id, title, planned_on, due_at)) in cases.into_iter().enumerate() {
            let mut draft =
                NewTask::quick_capture(title, TaskPlacement::Inbox).expect("valid draft");
            draft.planned_on =
                planned_on.map(|date| LocalDate::parse_iso(date).expect("valid planned date"));
            draft.due = due_at.map(|seconds| ScheduledInstant {
                at_utc: UtcTimestamp::from_unix_seconds(seconds),
                time_zone: TimeZoneId::new("Asia/Shanghai").expect("valid timezone"),
            });
            store
                .create_task(
                    id,
                    draft,
                    UtcTimestamp::from_unix_seconds(i64::try_from(index).expect("index")),
                )
                .expect("task created");
        }
        store
            .complete_task(
                task_id(30),
                task_id(103),
                UtcTimestamp::from_unix_seconds(10),
                LocalDate::parse_iso("2026-09-01").expect("valid date"),
            )
            .expect("due task completed");

        let candidates = store
            .list_today_candidates(today)
            .expect("today candidates");
        let titles = candidates
            .iter()
            .map(|task| task.record().title.as_str())
            .collect::<Vec<_>>();
        assert_eq!(titles, vec!["Old plan", "Today plan"]);
    }

    #[test]
    fn reminder_consumption_requires_the_expected_deadline() {
        let store = SqliteStore::open_in_memory().expect("storage opens");
        let id = task_id(34);
        let mut draft =
            NewTask::quick_capture("Remind me", TaskPlacement::Inbox).expect("valid draft");
        draft.reminder = Some(ScheduledInstant {
            at_utc: UtcTimestamp::from_unix_seconds(500),
            time_zone: TimeZoneId::new("Asia/Shanghai").expect("valid timezone"),
        });
        store
            .create_task(id, draft, UtcTimestamp::from_unix_seconds(1))
            .expect("task created");
        assert_eq!(store.list_pending_reminders().expect("reminders").len(), 1);
        assert!(
            !store
                .clear_reminder_if_matches(
                    id,
                    UtcTimestamp::from_unix_seconds(499),
                    UtcTimestamp::from_unix_seconds(2),
                )
                .expect("stale reminder ignored")
        );
        assert!(
            store
                .clear_reminder_if_matches(
                    id,
                    UtcTimestamp::from_unix_seconds(500),
                    UtcTimestamp::from_unix_seconds(3),
                )
                .expect("reminder consumed")
        );
        assert!(
            store
                .list_pending_reminders()
                .expect("reminders")
                .is_empty()
        );
    }

    #[test]
    fn recurring_completion_preserves_history_and_creates_the_next_occurrence_atomically() {
        let store = SqliteStore::open_in_memory().expect("storage opens");
        let source_id = task_id(40);
        let next_id = task_id(41);
        let reminder_at = UtcTimestamp::from_unix_seconds(1_769_856_000);
        let mut draft =
            NewTask::quick_capture("Recurring", TaskPlacement::Inbox).expect("valid draft");
        draft.planned_on = Some(LocalDate::parse_iso("2026-01-31").expect("valid date"));
        draft.reminder = Some(ScheduledInstant {
            at_utc: reminder_at,
            time_zone: TimeZoneId::new("UTC").expect("valid timezone"),
        });
        draft.recurrence =
            Some(RecurrenceRule::new(RecurrencePattern::Monthly).expect("valid recurrence"));
        store
            .create_task(
                source_id,
                draft,
                UtcTimestamp::from_unix_seconds(1_769_000_000),
            )
            .expect("task created");

        assert!(
            store
                .clear_reminder_if_matches(
                    source_id,
                    reminder_at,
                    UtcTimestamp::from_unix_seconds(1_769_856_001),
                )
                .expect("delivery recorded")
        );
        assert!(
            store
                .list_pending_reminders()
                .expect("delivered reminder hidden")
                .is_empty()
        );
        assert!(
            store
                .get_task(source_id)
                .expect("source query")
                .expect("source exists")
                .record()
                .reminder
                .is_some(),
            "the recurrence template survives reminder delivery"
        );

        rename_without_rescheduling(
            &store,
            source_id,
            UtcTimestamp::from_unix_seconds(1_769_856_002),
        )
        .expect("non-schedule edit succeeds");
        assert!(
            store
                .list_pending_reminders()
                .expect("delivered reminder stays hidden after a non-schedule edit")
                .is_empty()
        );

        store
            .complete_task(
                source_id,
                next_id,
                UtcTimestamp::from_unix_seconds(1_770_000_000),
                LocalDate::parse_iso("2026-02-02").expect("valid date"),
            )
            .expect("recurring task completes");
        assert_recurring_completion(&store, source_id, next_id, reminder_at);
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

    #[test]
    fn desktop_settings_default_and_round_trip_as_one_group() {
        let store = SqliteStore::open_in_memory().expect("storage opens");
        assert_eq!(
            store
                .load_desktop_settings()
                .expect("desktop settings query"),
            DesktopSettings::default()
        );
        let settings = DesktopSettings {
            launch_at_startup: true,
            start_hidden: true,
            close_behavior: WindowCloseBehavior::Quit,
            minimize_behavior: WindowMinimizeBehavior::Taskbar,
        };
        store
            .save_desktop_settings(settings, UtcTimestamp::from_unix_seconds(31))
            .expect("desktop settings saved");
        assert_eq!(
            store
                .load_desktop_settings()
                .expect("desktop settings query"),
            settings
        );
    }

    #[test]
    fn close_behavior_keeps_existing_boolean_storage_format() {
        let store = SqliteStore::open_in_memory().unwrap();
        let schema_version = store.schema_version().unwrap();
        for (stored_bool, behavior) in [
            ("true", WindowCloseBehavior::CloseGuiKeepAgent),
            ("false", WindowCloseBehavior::Quit),
        ] {
            // Seed exactly the existing persisted representation, not the Rust enum.
            store
                .connection
                .lock()
                .unwrap()
                .execute(
                    "INSERT OR REPLACE INTO settings(key, value_json, updated_at_utc)
                 VALUES ('desktop.close_to_tray', ?1, 1)",
                    [stored_bool],
                )
                .unwrap();
            let settings = store.load_desktop_settings().unwrap();
            assert_eq!(settings.close_behavior, behavior);
            store
                .save_desktop_settings(settings, UtcTimestamp::from_unix_seconds(2))
                .unwrap();
            let stored: String = store
                .connection
                .lock()
                .unwrap()
                .query_row(
                    "SELECT value_json FROM settings WHERE key = 'desktop.close_to_tray'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(stored, stored_bool);
        }
        assert_eq!(store.schema_version().unwrap(), schema_version);
    }

    #[test]
    fn retired_minimize_preference_is_ignored_and_normalized_on_write() {
        let store = SqliteStore::open_in_memory().unwrap();
        store.connection.lock().unwrap().execute(
            "INSERT INTO settings(key, value_json, updated_at_utc) VALUES ('desktop.minimize_to_tray', 'true', 1)", [],
        ).unwrap();
        assert_eq!(
            store.load_desktop_settings().unwrap().minimize_behavior,
            WindowMinimizeBehavior::Taskbar
        );
        store
            .save_desktop_settings(
                DesktopSettings {
                    minimize_behavior: WindowMinimizeBehavior::HideToTray,
                    ..DesktopSettings::default()
                },
                UtcTimestamp::from_unix_seconds(2),
            )
            .unwrap();
        let stored: String = store
            .connection
            .lock()
            .unwrap()
            .query_row(
                "SELECT value_json FROM settings WHERE key = 'desktop.minimize_to_tray'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(stored, "false");
    }

    #[test]
    fn desktop_settings_group_rolls_back_on_partial_failure() {
        let store = SqliteStore::open_in_memory().expect("storage opens");
        store
            .connection
            .lock()
            .expect("connection lock")
            .execute_batch(
                "CREATE TRIGGER reject_start_hidden
                 BEFORE INSERT ON settings
                 WHEN NEW.key = 'desktop.start_hidden'
                 BEGIN SELECT RAISE(ABORT, 'test rollback'); END;",
            )
            .expect("failure trigger installed");
        assert!(
            store
                .save_desktop_settings(
                    DesktopSettings {
                        launch_at_startup: true,
                        ..DesktopSettings::default()
                    },
                    UtcTimestamp::from_unix_seconds(32),
                )
                .is_err()
        );
        let stored_count = store
            .connection
            .lock()
            .expect("connection lock")
            .query_row(
                "SELECT COUNT(*) FROM settings WHERE key LIKE 'desktop.%'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .expect("settings count");
        assert_eq!(stored_count, 0);
    }

    #[test]
    fn pomodoro_settings_default_validate_and_round_trip_as_one_group() {
        let store = SqliteStore::open_in_memory().expect("storage opens");
        assert_eq!(
            store
                .load_pomodoro_settings()
                .expect("Pomodoro settings query"),
            PomodoroSettings::default()
        );
        let settings = PomodoroSettings {
            focus_minutes: 50,
            short_break_minutes: 10,
            long_break_minutes: 25,
            long_break_interval: 3,
            auto_start_break: true,
            auto_start_focus: true,
        };
        store
            .save_pomodoro_settings(settings, UtcTimestamp::from_unix_seconds(33))
            .expect("Pomodoro settings save");
        assert_eq!(
            store
                .load_pomodoro_settings()
                .expect("Pomodoro settings query"),
            settings
        );
        assert!(
            store
                .save_pomodoro_settings(
                    PomodoroSettings {
                        focus_minutes: 0,
                        ..settings
                    },
                    UtcTimestamp::from_unix_seconds(34),
                )
                .is_err()
        );
    }

    #[test]
    fn reorder_uses_gaps_and_rebalances_when_keys_are_exhausted() {
        let store = SqliteStore::open_in_memory().expect("storage opens");
        let ids = [task_id(10), task_id(11), task_id(12)];
        for (index, id) in ids.into_iter().enumerate() {
            store
                .create_task(
                    id,
                    NewTask::quick_capture(format!("Task {index}"), TaskPlacement::Inbox)
                        .expect("valid draft"),
                    UtcTimestamp::from_unix_seconds(40 + i64::try_from(index).expect("index")),
                )
                .expect("task created");
        }
        {
            let connection = store.connection.lock().expect("connection lock");
            for (index, id) in ids.into_iter().enumerate() {
                connection
                    .execute(
                        "UPDATE tasks SET sort_key = ?2 WHERE id = ?1",
                        rusqlite::params![id.to_string(), i64::try_from(index + 1).expect("index")],
                    )
                    .expect("compressed key");
            }
        }

        store
            .reorder_task(
                ids[2],
                ReorderDirection::Up,
                UtcTimestamp::from_unix_seconds(50),
            )
            .expect("task reordered with rebalance");
        let ordered = store.list_active_tasks().expect("ordered tasks");
        let ordered_ids = ordered
            .iter()
            .map(|task| task.record().id)
            .collect::<Vec<_>>();
        assert_eq!(ordered_ids, vec![ids[0], ids[2], ids[1]]);
        assert!(
            ordered
                .windows(2)
                .all(|pair| pair[0].record().sort_key < pair[1].record().sort_key)
        );

        store
            .reorder_task(
                ids[2],
                ReorderDirection::Down,
                UtcTimestamp::from_unix_seconds(51),
            )
            .expect("task reordered down");
        let ordered = store.list_active_tasks().expect("ordered tasks");
        assert_eq!(
            ordered
                .iter()
                .map(|task| task.record().id)
                .collect::<Vec<_>>(),
            ids
        );
    }
}
