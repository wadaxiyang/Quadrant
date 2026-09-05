//! Data-maintenance and distribution-safe external action orchestration.

use std::{path::PathBuf, sync::Arc};

use crate::{
    ApplicationEvent, Clock, ExternalOpener, MaintenanceRepository, UiIntent, UserFacingError,
};

/// One validated backup shown in Settings.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BackupInfo {
    /// Absolute backup file path.
    pub path: PathBuf,
    /// Backup size after durable creation.
    pub size_bytes: u64,
}

/// Repository-backed data-maintenance projection.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MaintenanceState {
    /// Absolute application-private backup directory.
    pub backup_directory: PathBuf,
    /// Newest backup by stable timestamped filename.
    pub latest_backup: Option<BackupInfo>,
    /// Whether a validated restore is waiting for process restart.
    pub restore_pending: bool,
}

/// Data-maintenance use case over storage and platform ports.
#[derive(Clone)]
pub struct MaintenanceApplication {
    repository: Arc<dyn MaintenanceRepository>,
    opener: Arc<dyn ExternalOpener>,
    clock: Arc<dyn Clock>,
}

impl MaintenanceApplication {
    /// Creates a maintenance service without exposing concrete filesystem/OS adapters.
    #[must_use]
    pub fn new(
        repository: Arc<dyn MaintenanceRepository>,
        opener: Arc<dyn ExternalOpener>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            repository,
            opener,
            clock,
        }
    }

    /// Loads the initial Settings projection.
    ///
    /// # Errors
    ///
    /// Returns a repository failure when the backup directory cannot be inspected.
    pub fn load_state(&self) -> Result<MaintenanceState, crate::RepositoryError> {
        self.repository.maintenance_state()
    }

    /// Handles maintenance and release-link intents.
    #[must_use]
    pub fn handle(&self, intent: &UiIntent) -> Vec<ApplicationEvent> {
        match intent {
            UiIntent::CreateBackup => match self.repository.create_backup(self.clock.now()) {
                Ok(backup) => {
                    self.success_with_refresh(format!("Backup created: {}", backup.path.display()))
                }
                Err(_) => vec![maintenance_failure("The backup could not be created.")],
            },
            UiIntent::StageLatestRestore => match self.repository.stage_latest_restore() {
                Ok(_) => self.success_with_refresh(
                    "Restore staged. Exit and reopen Quadrant to apply it.".to_owned(),
                ),
                Err(_) => vec![maintenance_failure(
                    "The latest backup could not be validated or staged for restore.",
                )],
            },
            UiIntent::OpenBackupDirectory => match self.repository.maintenance_state() {
                Ok(state) => match self.opener.open_path(&state.backup_directory) {
                    Ok(()) => Vec::new(),
                    Err(_) => vec![maintenance_failure(
                        "The backup folder could not be opened.",
                    )],
                },
                Err(_) => vec![maintenance_failure("The backup folder is unavailable.")],
            },
            UiIntent::OpenReleasePage => {
                match self
                    .opener
                    .open_url("https://github.com/wadaxiyang/Quadrant/releases")
                {
                    Ok(()) => Vec::new(),
                    Err(_) => vec![maintenance_failure(
                        "The releases page could not be opened.",
                    )],
                }
            }
            _ => Vec::new(),
        }
    }

    fn success_with_refresh(&self, message: String) -> Vec<ApplicationEvent> {
        let mut events = vec![ApplicationEvent::OperationSucceeded(message)];
        match self.repository.maintenance_state() {
            Ok(state) => events.push(ApplicationEvent::MaintenanceChanged(state)),
            Err(_) => events.push(maintenance_failure("Backup status could not be refreshed.")),
        }
        events
    }
}

fn maintenance_failure(message: &str) -> ApplicationEvent {
    ApplicationEvent::OperationFailed(UserFacingError {
        message: message.to_owned(),
    })
}
