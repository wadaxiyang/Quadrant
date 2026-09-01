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

    let current = connection
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|error| RepositoryError::new(RepositoryOperation::Migrate, error))?;

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
