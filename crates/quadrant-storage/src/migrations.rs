//! Embedded, ordered `SQLite` migrations.

use quadrant_application::{RepositoryError, RepositoryOperation};
use rusqlite::{Connection, TransactionBehavior, params};

struct Migration {
    version: i64,
    name: &'static str,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "initial",
        sql: include_str!("../../../migrations/0001_initial.sql"),
    },
    Migration {
        version: 2,
        name: "reminder_delivery_state",
        sql: include_str!("../../../migrations/0002_reminder_delivery_state.sql"),
    },
    Migration {
        version: 3,
        name: "focus_state",
        sql: include_str!("../../../migrations/0003_focus_state.sql"),
    },
    Migration {
        version: 4,
        name: "review_history",
        sql: include_str!("../../../migrations/0004_review_history.sql"),
    },
];

pub(crate) fn apply(connection: &mut Connection) -> Result<(), RepositoryError> {
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                 version INTEGER PRIMARY KEY NOT NULL,
                 name TEXT NOT NULL,
                 applied_at_utc INTEGER NOT NULL DEFAULT (unixepoch())
             ) STRICT;",
        )
        .map_err(|error| RepositoryError::new(RepositoryOperation::Migrate, error))?;

    let mut current = connection
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|error| RepositoryError::new(RepositoryOperation::Migrate, error))?;

    if current == 0 && table_exists(connection, "tasks")? {
        current = adopt_untracked_schema(connection)?;
    }

    for migration in MIGRATIONS
        .iter()
        .filter(|migration| migration.version > current)
    {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| RepositoryError::new(RepositoryOperation::Migrate, error))?;
        transaction
            .execute_batch(migration.sql)
            .map_err(|error| RepositoryError::new(RepositoryOperation::Migrate, error))?;
        transaction
            .execute(
                "INSERT INTO schema_migrations(version, name) VALUES (?1, ?2)",
                params![migration.version, migration.name],
            )
            .map_err(|error| RepositoryError::new(RepositoryOperation::Migrate, error))?;
        transaction
            .commit()
            .map_err(|error| RepositoryError::new(RepositoryOperation::Migrate, error))?;
    }
    Ok(())
}

fn adopt_untracked_schema(connection: &mut Connection) -> Result<i64, RepositoryError> {
    let operation = RepositoryOperation::Migrate;
    for table in [
        "tasks",
        "task_completion_events",
        "focus_sessions",
        "settings",
    ] {
        if !table_exists(connection, table)? {
            return Err(RepositoryError::new(
                operation,
                format!("untracked database is missing required table {table}"),
            ));
        }
    }
    let task_columns = table_columns(connection, "tasks")?;
    let focus_columns = table_columns(connection, "focus_sessions")?;
    let completion_columns = table_columns(connection, "task_completion_events")?;
    let base_version = if completion_columns
        .iter()
        .any(|column| column == "reverted_at_utc")
    {
        4
    } else if focus_columns.iter().any(|column| column == "pomodoro_kind")
        && focus_columns.iter().any(|column| column == "status")
    {
        3
    } else if focus_columns.iter().any(|column| column == "outcome") {
        if task_columns
            .iter()
            .any(|column| column == "reminder_delivered_for_utc")
        {
            2
        } else {
            1
        }
    } else {
        return Err(RepositoryError::new(
            operation,
            "untracked focus_sessions schema is not recognized",
        ));
    };
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| RepositoryError::new(operation, error))?;
    for migration in MIGRATIONS
        .iter()
        .filter(|migration| migration.version <= base_version)
    {
        transaction
            .execute(
                "INSERT INTO schema_migrations(version, name) VALUES (?1, ?2)",
                params![migration.version, migration.name],
            )
            .map_err(|error| RepositoryError::new(operation, error))?;
    }
    transaction
        .commit()
        .map_err(|error| RepositoryError::new(operation, error))?;
    Ok(base_version)
}

fn table_exists(connection: &Connection, table: &str) -> Result<bool, RepositoryError> {
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
            [table],
            |row| row.get::<_, bool>(0),
        )
        .map_err(|error| RepositoryError::new(RepositoryOperation::Migrate, error))
}

fn table_columns(connection: &Connection, table: &str) -> Result<Vec<String>, RepositoryError> {
    let operation = RepositoryOperation::Migrate;
    let mut statement = connection
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(|error| RepositoryError::new(operation, error))?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|error| RepositoryError::new(operation, error))?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| RepositoryError::new(operation, error))
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::apply;

    #[test]
    fn untracked_m2_database_is_adopted_and_upgraded_without_recreating_tasks() {
        let mut connection = Connection::open_in_memory().expect("database opens");
        connection
            .execute_batch(include_str!("../../../migrations/0001_initial.sql"))
            .expect("legacy initial schema exists");
        connection
            .execute_batch(include_str!(
                "../../../migrations/0002_reminder_delivery_state.sql"
            ))
            .expect("legacy reminder column exists");

        apply(&mut connection).expect("untracked schema is adopted");

        let version = connection
            .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
                row.get::<_, i64>(0)
            })
            .expect("version query");
        assert_eq!(version, 4);
        let focus_columns =
            super::table_columns(&connection, "focus_sessions").expect("focus columns load");
        assert!(focus_columns.iter().any(|column| column == "pomodoro_kind"));
        assert!(focus_columns.iter().any(|column| column == "status"));
        let completion_columns = super::table_columns(&connection, "task_completion_events")
            .expect("completion columns load");
        assert!(
            completion_columns
                .iter()
                .any(|column| column == "reverted_at_utc")
        );
    }
}
