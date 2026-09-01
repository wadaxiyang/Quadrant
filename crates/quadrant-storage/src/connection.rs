//! `SQLite` connection creation and invariant PRAGMA configuration.

use std::{path::Path, time::Duration};

use quadrant_application::{RepositoryError, RepositoryOperation};
use rusqlite::Connection;

use crate::migrations;

pub(crate) fn open(path: &Path) -> Result<Connection, RepositoryError> {
    let mut connection = Connection::open(path)
        .map_err(|error| RepositoryError::new(RepositoryOperation::Open, error))?;
    configure(&connection)?;
    migrations::apply(&mut connection)?;
    Ok(connection)
}

pub(crate) fn open_in_memory() -> Result<Connection, RepositoryError> {
    let mut connection = Connection::open_in_memory()
        .map_err(|error| RepositoryError::new(RepositoryOperation::Open, error))?;
    configure(&connection)?;
    migrations::apply(&mut connection)?;
    Ok(connection)
}

fn configure(connection: &Connection) -> Result<(), RepositoryError> {
    connection
        .busy_timeout(Duration::from_secs(5))
        .map_err(|error| RepositoryError::new(RepositoryOperation::Open, error))?;
    connection
        .execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;",
        )
        .map_err(|error| RepositoryError::new(RepositoryOperation::Open, error))?;
    Ok(())
}
