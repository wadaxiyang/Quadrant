//! `SQLite` online backup, validation, and next-start restore staging.

use std::{
    ffi::OsString,
    fs::{self, OpenOptions},
    path::{Path, PathBuf},
};

use quadrant_application::{
    BackupInfo, MaintenanceRepository, MaintenanceState, RepositoryError, RepositoryOperation,
    UtcTimestamp,
};
use rusqlite::{Connection, OpenFlags, params};
use uuid::Uuid;

use crate::{SqliteStore, migrations::CURRENT_SCHEMA_VERSION};

const BACKUP_FORMAT_VERSION: i64 = 1;
const BACKUP_EXTENSION: &str = "quadrant-backup";
const METADATA_TABLE: &str = "quadrant_backup_metadata";

/// Result of applying a previously validated restore before storage opens.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppliedRestore {
    /// Directory containing the previous live database and any sidecars.
    pub recovery_directory: Option<PathBuf>,
}

impl MaintenanceRepository for SqliteStore {
    fn maintenance_state(&self) -> Result<MaintenanceState, RepositoryError> {
        let database = self.require_database_path()?;
        let directory = backup_directory(database)?;
        fs::create_dir_all(&directory).map_err(maintenance_error)?;
        let latest_backup = latest_backup_path(&directory)?
            .map(|path| backup_info(&path))
            .transpose()?;
        Ok(MaintenanceState {
            backup_directory: directory,
            latest_backup,
            restore_pending: pending_restore_path(database).is_file(),
        })
    }

    fn create_backup(&self, now: UtcTimestamp) -> Result<BackupInfo, RepositoryError> {
        let database = self.require_database_path()?;
        let directory = backup_directory(database)?;
        fs::create_dir_all(&directory).map_err(maintenance_error)?;
        let filename = format!(
            "quadrant-{}-{}.{}",
            now.unix_seconds(),
            Uuid::now_v7().simple(),
            BACKUP_EXTENSION
        );
        let destination = directory.join(filename);
        let temporary = temporary_path(&destination);
        let result = (|| {
            let connection = self.lock(RepositoryOperation::MaintainData)?;
            connection
                .backup(rusqlite::MAIN_DB, &temporary, None)
                .map_err(maintenance_error)?;
            let schema_version = connection
                .query_row(
                    "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .map_err(maintenance_error)?;
            drop(connection);

            write_backup_metadata(&temporary, now, schema_version)?;
            OpenOptions::new()
                .write(true)
                .open(&temporary)
                .and_then(|file| file.sync_all())
                .map_err(maintenance_error)?;
            validate_backup(&temporary)?;
            fs::rename(&temporary, &destination).map_err(maintenance_error)?;
            backup_info(&destination)
        })();
        if result.is_err() {
            drop(fs::remove_file(&temporary));
        }
        result
    }

    fn stage_latest_restore(&self) -> Result<BackupInfo, RepositoryError> {
        let database = self.require_database_path()?;
        let directory = backup_directory(database)?;
        let latest = latest_backup_path(&directory)?.ok_or_else(|| {
            RepositoryError::new(
                RepositoryOperation::MaintainData,
                "no Quadrant backup exists",
            )
        })?;
        let info = validate_backup(&latest)?;
        let pending = pending_restore_path(database);
        let temporary = temporary_path(&pending);
        let result = (|| {
            fs::copy(&latest, &temporary).map_err(maintenance_error)?;
            OpenOptions::new()
                .write(true)
                .open(&temporary)
                .and_then(|file| file.sync_all())
                .map_err(maintenance_error)?;
            validate_backup(&temporary)?;
            if pending.exists() {
                fs::remove_file(&pending).map_err(maintenance_error)?;
            }
            fs::rename(&temporary, &pending).map_err(maintenance_error)?;
            Ok(info)
        })();
        if result.is_err() {
            drop(fs::remove_file(&temporary));
        }
        result
    }
}

impl SqliteStore {
    fn require_database_path(&self) -> Result<&Path, RepositoryError> {
        self.database_path.as_deref().ok_or_else(|| {
            RepositoryError::new(
                RepositoryOperation::MaintainData,
                "maintenance is unavailable for an in-memory database",
            )
        })
    }
}

/// Applies a staged restore while no `SQLite` connection is open.
///
/// The former database and WAL/SHM sidecars are moved into a uniquely named
/// recovery directory. They are never deleted by this operation.
///
/// # Errors
///
/// Returns a validation or exact-path filesystem failure. The current database
/// is left in place when validation fails and rolled back when replacement fails.
pub fn apply_pending_restore(database: &Path) -> Result<Option<AppliedRestore>, RepositoryError> {
    let pending = pending_restore_path(database);
    if !pending.is_file() {
        return Ok(None);
    }
    validate_backup(&pending)?;
    let existing = existing_database_files(database);
    let recovery_directory = if existing.is_empty() {
        None
    } else {
        let parent = database.parent().ok_or_else(|| {
            RepositoryError::new(
                RepositoryOperation::MaintainData,
                "database has no parent directory",
            )
        })?;
        let directory = parent
            .join("recovery")
            .join(format!("before-restore-{}", Uuid::now_v7().simple()));
        fs::create_dir_all(&directory).map_err(maintenance_error)?;
        for path in &existing {
            let filename = path.file_name().ok_or_else(|| {
                RepositoryError::new(
                    RepositoryOperation::MaintainData,
                    "database sidecar has no filename",
                )
            })?;
            if let Err(error) = fs::rename(path, directory.join(filename)) {
                rollback_database_files(database, &directory);
                return Err(maintenance_error(error));
            }
        }
        Some(directory)
    };

    if let Err(error) = fs::rename(&pending, database) {
        if let Some(directory) = recovery_directory.as_deref() {
            rollback_database_files(database, directory);
        }
        return Err(maintenance_error(error));
    }
    Ok(Some(AppliedRestore { recovery_directory }))
}

fn write_backup_metadata(
    path: &Path,
    now: UtcTimestamp,
    schema_version: i64,
) -> Result<(), RepositoryError> {
    let connection = Connection::open(path).map_err(maintenance_error)?;
    connection
        .execute_batch(&format!(
            "DROP TABLE IF EXISTS {METADATA_TABLE};
             CREATE TABLE {METADATA_TABLE} (
                 format_version INTEGER NOT NULL,
                 application_version TEXT NOT NULL,
                 schema_version INTEGER NOT NULL,
                 created_at_utc INTEGER NOT NULL
             ) STRICT;"
        ))
        .map_err(maintenance_error)?;
    connection
        .execute(
            &format!(
                "INSERT INTO {METADATA_TABLE}(
                     format_version, application_version, schema_version, created_at_utc
                 ) VALUES (?1, ?2, ?3, ?4)"
            ),
            params![
                BACKUP_FORMAT_VERSION,
                env!("CARGO_PKG_VERSION"),
                schema_version,
                now.unix_seconds()
            ],
        )
        .map_err(maintenance_error)?;
    Ok(())
}

fn validate_backup(path: &Path) -> Result<BackupInfo, RepositoryError> {
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(maintenance_error)?;
    let integrity = connection
        .query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0))
        .map_err(maintenance_error)?;
    if integrity != "ok" {
        return Err(RepositoryError::new(
            RepositoryOperation::MaintainData,
            format!("backup integrity check failed: {integrity}"),
        ));
    }
    let (format_version, application_version, metadata_schema) = connection
        .query_row(
            &format!(
                "SELECT format_version, application_version, schema_version
                 FROM {METADATA_TABLE} LIMIT 1"
            ),
            [],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )
        .map_err(maintenance_error)?;
    let actual_schema = connection
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map_err(maintenance_error)?;
    if format_version != BACKUP_FORMAT_VERSION
        || application_version.trim().is_empty()
        || metadata_schema != actual_schema
        || !(1..=CURRENT_SCHEMA_VERSION).contains(&actual_schema)
    {
        return Err(RepositoryError::new(
            RepositoryOperation::MaintainData,
            format!(
                "unsupported backup metadata: format {format_version}, schema {metadata_schema}/{actual_schema}"
            ),
        ));
    }
    backup_info(path)
}

fn backup_info(path: &Path) -> Result<BackupInfo, RepositoryError> {
    let size_bytes = fs::metadata(path).map_err(maintenance_error)?.len();
    Ok(BackupInfo {
        path: path.to_path_buf(),
        size_bytes,
    })
}

fn backup_directory(database: &Path) -> Result<PathBuf, RepositoryError> {
    database
        .parent()
        .map(|parent| parent.join("backups"))
        .ok_or_else(|| {
            RepositoryError::new(
                RepositoryOperation::MaintainData,
                "database has no parent directory",
            )
        })
}

fn latest_backup_path(directory: &Path) -> Result<Option<PathBuf>, RepositoryError> {
    if !directory.exists() {
        return Ok(None);
    }
    let mut candidates = fs::read_dir(directory)
        .map_err(maintenance_error)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|value| value == BACKUP_EXTENSION)
        })
        .collect::<Vec<_>>();
    candidates.sort();
    Ok(candidates.pop())
}

fn pending_restore_path(database: &Path) -> PathBuf {
    append_to_filename(database, ".restore-pending")
}

fn temporary_path(destination: &Path) -> PathBuf {
    append_to_filename(destination, &format!(".{}.tmp", Uuid::now_v7().simple()))
}

fn sidecar_path(database: &Path, suffix: &str) -> PathBuf {
    append_to_filename(database, suffix)
}

fn append_to_filename(path: &Path, suffix: &str) -> PathBuf {
    let mut value = OsString::from(path.as_os_str());
    value.push(suffix);
    PathBuf::from(value)
}

fn existing_database_files(database: &Path) -> Vec<PathBuf> {
    [
        database.to_path_buf(),
        sidecar_path(database, "-wal"),
        sidecar_path(database, "-shm"),
    ]
    .into_iter()
    .filter(|path| path.exists())
    .collect()
}

fn rollback_database_files(database: &Path, directory: &Path) {
    for target in [
        database.to_path_buf(),
        sidecar_path(database, "-wal"),
        sidecar_path(database, "-shm"),
    ] {
        let Some(filename) = target.file_name() else {
            continue;
        };
        let source = directory.join(filename);
        if source.exists() && !target.exists() {
            drop(fs::rename(source, target));
        }
    }
}

fn maintenance_error(error: impl std::fmt::Display) -> RepositoryError {
    RepositoryError::new(RepositoryOperation::MaintainData, error)
}

#[cfg(test)]
mod tests {
    use quadrant_application::{MaintenanceRepository, TaskRepository};
    use quadrant_domain::{NewTask, TaskId, TaskPlacement, UtcTimestamp};
    use rusqlite::{Connection, OpenFlags};
    use tempfile::TempDir;
    use uuid::Uuid;

    use super::{BACKUP_FORMAT_VERSION, METADATA_TABLE, apply_pending_restore};
    use crate::SqliteStore;

    fn task_id(value: u128) -> TaskId {
        TaskId::from_uuid(Uuid::from_u128(value))
    }

    #[test]
    fn backup_is_validated_and_staged_restore_preserves_the_previous_database() {
        let directory = TempDir::new().expect("temporary directory");
        let database = directory.path().join("quadrant-rust.db");
        let store = SqliteStore::open(&database).expect("storage opens");
        store
            .create_task(
                task_id(701),
                NewTask::quick_capture("Inside backup", TaskPlacement::Inbox).expect("valid task"),
                UtcTimestamp::from_unix_seconds(10),
            )
            .expect("task created");

        let backup = store
            .create_backup(UtcTimestamp::from_unix_seconds(20))
            .expect("backup created");
        assert!(backup.path.is_file());
        assert!(backup.size_bytes > 0);
        let snapshot = Connection::open_with_flags(
            &backup.path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .expect("backup opens");
        let (format_version, schema_version) = snapshot
            .query_row(
                &format!("SELECT format_version, schema_version FROM {METADATA_TABLE} LIMIT 1"),
                [],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
            )
            .expect("metadata reads");
        assert_eq!(format_version, BACKUP_FORMAT_VERSION);
        assert_eq!(schema_version, 4);
        drop(snapshot);

        store
            .create_task(
                task_id(702),
                NewTask::quick_capture("Only in previous live data", TaskPlacement::Inbox)
                    .expect("valid task"),
                UtcTimestamp::from_unix_seconds(30),
            )
            .expect("second task created");
        store.stage_latest_restore().expect("restore staged");
        assert!(
            store
                .maintenance_state()
                .expect("maintenance state")
                .restore_pending
        );
        drop(store);

        let applied = apply_pending_restore(&database)
            .expect("restore applies")
            .expect("restore existed");
        let recovery = applied
            .recovery_directory
            .expect("previous database retained");
        assert!(recovery.join("quadrant-rust.db").is_file());

        let restored = SqliteStore::open(&database).expect("restored storage opens");
        let titles = restored
            .list_active_tasks()
            .expect("restored tasks load")
            .into_iter()
            .map(|task| task.record().title.as_str().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(titles, vec!["Inside backup"]);
    }

    #[test]
    fn invalid_latest_backup_is_not_staged() {
        let directory = TempDir::new().expect("temporary directory");
        let database = directory.path().join("quadrant-rust.db");
        let store = SqliteStore::open(&database).expect("storage opens");
        let state = store.maintenance_state().expect("maintenance state");
        let corrupt = state
            .backup_directory
            .join("quadrant-9999999999-corrupt.quadrant-backup");
        std::fs::write(&corrupt, b"not sqlite").expect("corrupt backup written");

        assert!(store.stage_latest_restore().is_err());
        assert!(
            !store
                .maintenance_state()
                .expect("maintenance state reloads")
                .restore_pending
        );
    }

    #[test]
    fn invalid_pending_restore_leaves_the_live_database_untouched() {
        let directory = TempDir::new().expect("temporary directory");
        let database = directory.path().join("quadrant-rust.db");
        let store = SqliteStore::open(&database).expect("storage opens");
        store
            .create_task(
                task_id(703),
                NewTask::quick_capture("Keep live data", TaskPlacement::Inbox).expect("valid task"),
                UtcTimestamp::from_unix_seconds(10),
            )
            .expect("task created");
        drop(store);
        std::fs::write(
            super::pending_restore_path(&database),
            b"invalid pending restore",
        )
        .expect("pending file written");

        assert!(apply_pending_restore(&database).is_err());
        let reopened = SqliteStore::open(&database).expect("live database still opens");
        let tasks = reopened.list_active_tasks().expect("live tasks load");
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].record().title.as_str(), "Keep live data");
    }
}
