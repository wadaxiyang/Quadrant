// SPDX-License-Identifier: GPL-3.0-only
//! Concrete service assembly and serialized use-case execution.

use crate::{AgentError, HostServices};
use quadrant_application::{
    ApplicationEvent, Clock, ExecutionGate, FocusApplication, HistoryApplication,
    MaintenanceApplication, NavigationRoute, SettingsRepository, SystemThemeSource,
    TaskApplication, UiIntent, UpdateViewState, UserFacingError, UuidFocusSessionIdGenerator,
    UuidTaskIdGenerator,
};
use quadrant_protocol::{AppSnapshot, PlatformCapabilities, ServerEvent};
use quadrant_storage::SqliteStore;
use std::{path::Path, sync::Arc};

#[derive(Clone)]
pub(crate) struct Services {
    pub store: Arc<SqliteStore>,
    tasks: TaskApplication,
    pub focus: FocusApplication,
    history: HistoryApplication,
    maintenance: MaintenanceApplication,
    pub clock: Arc<dyn Clock>,
    pub gate: Arc<ExecutionGate>,
    // Per-service instrumentation shared across broker clones, absent in production.
    #[cfg(test)]
    pub snapshot_calls: Arc<std::sync::atomic::AtomicUsize>,
}

impl Services {
    pub fn open(path: &Path, host: &HostServices) -> Result<Self, AgentError> {
        let store = Arc::new(SqliteStore::open(path)?);
        let calendar = Arc::new(quadrant_platform::PlatformTodayContextSource);
        let tasks = TaskApplication::new(
            store.clone(),
            store.clone(),
            store.clone(),
            host.autostart.clone(),
            host.clock.clone(),
            Arc::new(UuidTaskIdGenerator),
            calendar.clone(),
        );
        let focus = FocusApplication::new(
            store.clone(),
            store.clone(),
            store.clone(),
            host.clock.clone(),
            Arc::new(UuidFocusSessionIdGenerator),
            calendar.clone(),
        );
        let history =
            HistoryApplication::new(store.clone(), store.clone(), host.clock.clone(), calendar);
        let maintenance =
            MaintenanceApplication::new(store.clone(), host.opener.clone(), host.clock.clone());
        Ok(Self {
            store,
            tasks,
            focus,
            history,
            maintenance,
            clock: host.clock.clone(),
            gate: Arc::default(),
            #[cfg(test)]
            snapshot_calls: Arc::default(),
        })
    }

    pub fn snapshot(&self, capabilities: PlatformCapabilities) -> Result<AppSnapshot, AgentError> {
        #[cfg(test)]
        self.snapshot_calls
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.gate.run(|| {
            Ok(AppSnapshot {
                captured_at: self.clock.now(),
                quadrants: self.tasks.load_quadrants()?,
                today: self.tasks.load_today().map_err(AgentError::projection)?,
                focus: self.focus.load_state().map_err(AgentError::projection)?,
                review: self.history.load_review().map_err(AgentError::projection)?,
                completed: self.history.load_completed()?,
                maintenance: self.maintenance.load_state()?,
                desktop_settings: self.store.load_desktop_settings()?,
                theme_mode: self.store.load_theme_mode()?.unwrap_or_default(),
                system_theme: quadrant_platform::PlatformThemeSource.current_theme(),
                platform_capabilities: capabilities,
                update_state: UpdateViewState::from_build(
                    env!("CARGO_PKG_VERSION"),
                    option_env!("QUADRANT_DISTRIBUTION_CHANNEL"),
                ),
            })
        })?
    }

    pub fn command(&self, intent: &UiIntent) -> Result<Vec<ServerEvent>, AgentError> {
        self.gate
            .run(|| {
                let mut events = if intent.is_maintenance_intent() {
                    self.maintenance.handle(intent)
                } else if intent.is_history_intent() {
                    self.history.handle(intent)
                } else if intent.is_focus_intent() {
                    self.focus.handle(intent)
                } else {
                    self.tasks.handle(intent.clone())
                };
                if intent.affects_focus_projection() {
                    events.push(self.focus_event());
                }
                if intent.affects_history_projection() {
                    events.extend(self.history.refresh_after_mutation());
                }
                if matches!(intent, UiIntent::Navigate(NavigationRoute::Quadrants)) {
                    events.push(match self.tasks.load_quadrants() {
                        Ok(state) => ApplicationEvent::QuadrantsChanged(state),
                        Err(_) => failure("Tasks could not be refreshed."),
                    });
                }
                let succeeded = !events.iter().any(is_failure);
                let mut events: Vec<ServerEvent> =
                    events.into_iter().map(ServerEvent::from).collect();
                if succeeded && let UiIntent::SetTheme(theme_mode) = intent {
                    events.push(ServerEvent::ThemeChanged {
                        theme_mode: *theme_mode,
                        system_theme: quadrant_platform::PlatformThemeSource.current_theme(),
                    });
                }
                events
            })
            .map_err(AgentError::from)
    }

    pub fn refresh_background_focus(&self) -> Result<Vec<ServerEvent>, AgentError> {
        // Scheduler events are invalidations, never delayed authoritative DTOs:
        // reload after acquiring the same boundary as commands and snapshots.
        self.gate
            .run(|| {
                let mut events = vec![self.focus_event()];
                events.extend(self.history.refresh_after_mutation());
                events.into_iter().map(ServerEvent::from).collect()
            })
            .map_err(AgentError::from)
    }

    fn focus_event(&self) -> ApplicationEvent {
        match self.focus.load_state() {
            Ok(state) => ApplicationEvent::FocusChanged(state),
            Err(_) => failure("Focus state could not be refreshed."),
        }
    }
}

pub(crate) fn failure(message: &str) -> ApplicationEvent {
    ApplicationEvent::OperationFailed(UserFacingError {
        message: message.to_owned(),
    })
}

pub(crate) fn is_failure(event: &ApplicationEvent) -> bool {
    matches!(
        event,
        ApplicationEvent::OperationFailed(_) | ApplicationEvent::TaskEditorValidationFailed { .. }
    )
}
