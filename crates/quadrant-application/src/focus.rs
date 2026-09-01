//! Focus use cases and event-driven Pomodoro deadline scheduling.

use std::{sync::Arc, time::Duration};

use tokio::sync::mpsc;

use crate::{
    ApplicationEvent, CalendarError, Clock, FocusMode, FocusRepository, FocusSession,
    FocusSessionIdGenerator, FocusStartRequest, FocusTaskSnapshot, FocusTaskSummary,
    FocusViewState, PomodoroKind, PomodoroSettings, RepositoryError, RepositoryOperation,
    SettingsRepository, TaskRepository, TaskStatus, TodayContextSource, UiIntent, UserFacingError,
};

/// Synchronous Focus use cases intended for the application runtime's blocking pool.
#[derive(Clone)]
pub struct FocusApplication {
    focus: Arc<dyn FocusRepository>,
    tasks: Arc<dyn TaskRepository>,
    settings: Arc<dyn SettingsRepository>,
    clock: Arc<dyn Clock>,
    ids: Arc<dyn FocusSessionIdGenerator>,
    today_context: Arc<dyn TodayContextSource>,
}

impl FocusApplication {
    /// Assembles Focus use cases from application-owned ports.
    #[must_use]
    pub fn new(
        focus: Arc<dyn FocusRepository>,
        tasks: Arc<dyn TaskRepository>,
        settings: Arc<dyn SettingsRepository>,
        clock: Arc<dyn Clock>,
        ids: Arc<dyn FocusSessionIdGenerator>,
        today_context: Arc<dyn TodayContextSource>,
    ) -> Self {
        Self {
            focus,
            tasks,
            settings,
            clock,
            ids,
            today_context,
        }
    }

    /// Loads the complete Focus projection.
    ///
    /// # Errors
    ///
    /// Returns storage or local-calendar failures.
    pub fn load_state(&self) -> Result<FocusViewState, FocusLoadError> {
        let now = self.clock.now();
        let local_date = self.today_context.today_context(now)?.local_date;
        let tasks = self
            .tasks
            .list_active_tasks()?
            .iter()
            .map(|task| FocusTaskSummary {
                id: task.record().id,
                title: task.record().title.as_str().to_owned(),
                placement: task.record().placement,
            })
            .collect();
        Ok(FocusViewState {
            tasks,
            session: self.focus.get_current_focus_session()?,
            settings: self.settings.load_pomodoro_settings()?,
            today: self.focus.productive_focus_summary(local_date)?,
        })
    }

    /// Handles an intent owned by Focus and returns UI-safe events.
    #[must_use]
    pub fn handle(&self, intent: &UiIntent) -> Vec<ApplicationEvent> {
        let result = match intent {
            UiIntent::Navigate(crate::NavigationRoute::Focus) => {
                return self.refresh_events(None);
            }
            UiIntent::StartFocus(request) => self.start(*request),
            UiIntent::PauseFocus => self.transition(FocusTransition::Pause),
            UiIntent::ResumeFocus => self.transition(FocusTransition::Resume),
            UiIntent::FinishFocus => self.transition(FocusTransition::Finish),
            UiIntent::CancelFocus => self.transition(FocusTransition::Cancel),
            UiIntent::SetPomodoroSettings(settings) => self.save_settings(*settings),
            _ => return Vec::new(),
        };
        match result {
            Ok(message) => self.refresh_events(Some(message)),
            Err(error) => vec![focus_failure_event(&error)],
        }
    }

    /// Completes a due Pomodoro and performs configured automatic continuation.
    ///
    /// This is called only by [`FocusScheduler`].
    #[must_use]
    pub fn complete_due(&self) -> Vec<ApplicationEvent> {
        match self.complete_due_inner() {
            Ok(false) => Vec::new(),
            Ok(true) => self.refresh_events(None),
            Err(error) => vec![focus_failure_event(&error)],
        }
    }

    /// Returns the current running Pomodoro deadline.
    ///
    /// # Errors
    ///
    /// Returns a repository failure when current state cannot be read.
    pub fn current_deadline(&self) -> Result<Option<crate::UtcTimestamp>, RepositoryError> {
        self.focus
            .get_current_focus_session()
            .map(|session| session.and_then(|session| session.deadline()))
    }

    fn start(&self, request: FocusStartRequest) -> Result<&'static str, FocusUseCaseError> {
        if self.focus.get_current_focus_session()?.is_some() {
            return Err(FocusUseCaseError::CurrentSessionExists);
        }
        let task = match request.task_id {
            Some(task_id) => {
                let task = self
                    .tasks
                    .get_task(task_id)?
                    .ok_or(FocusUseCaseError::TaskUnavailable)?;
                if task.record().status != TaskStatus::Active {
                    return Err(FocusUseCaseError::TaskUnavailable);
                }
                Some(FocusTaskSnapshot {
                    id: Some(task_id),
                    title: task.record().title.as_str().to_owned(),
                    quadrant: placement_quadrant(task.record().placement),
                })
            }
            None => None,
        };
        let settings = self.settings.load_pomodoro_settings()?;
        let now = self.clock.now();
        let local_date = self.today_context.today_context(now)?.local_date;
        let session = FocusSession::start(
            self.ids.generate(),
            task,
            request.mode,
            request.pomodoro_kind,
            settings,
            now,
            local_date,
        )?;
        self.focus.create_focus_session(session)?;
        Ok("Focus started.")
    }

    fn transition(&self, transition: FocusTransition) -> Result<&'static str, FocusUseCaseError> {
        let mut session = self
            .focus
            .get_current_focus_session()?
            .ok_or(FocusUseCaseError::NoCurrentSession)?;
        let expected = session.record().status;
        let now = self.clock.now();
        let message = match transition {
            FocusTransition::Pause => {
                session.pause(now)?;
                "Focus paused."
            }
            FocusTransition::Resume => {
                session.resume(now)?;
                "Focus resumed."
            }
            FocusTransition::Finish => {
                session.complete(now)?;
                "Focus completed."
            }
            FocusTransition::Cancel => {
                session.cancel(now)?;
                "Focus cancelled."
            }
        };
        self.focus.transition_focus_session(session, expected)?;
        Ok(message)
    }

    fn save_settings(&self, settings: PomodoroSettings) -> Result<&'static str, FocusUseCaseError> {
        let settings = settings.validate()?;
        self.settings
            .save_pomodoro_settings(settings, self.clock.now())?;
        Ok("Pomodoro settings saved.")
    }

    fn complete_due_inner(&self) -> Result<bool, FocusUseCaseError> {
        let Some(mut session) = self.focus.get_current_focus_session()? else {
            return Ok(false);
        };
        let previous = session.record().clone();
        if !session.complete_if_due(self.clock.now())? {
            return Ok(false);
        }
        self.focus
            .transition_focus_session(session, previous.status)?;
        let settings = self.settings.load_pomodoro_settings()?;
        let next_kind = match previous.pomodoro_kind {
            Some(PomodoroKind::Focus) if settings.auto_start_break => {
                let completed = self.focus.completed_pomodoro_focus_count()?;
                if completed % u64::from(settings.long_break_interval) == 0 {
                    Some(PomodoroKind::LongBreak)
                } else {
                    Some(PomodoroKind::ShortBreak)
                }
            }
            Some(PomodoroKind::ShortBreak | PomodoroKind::LongBreak)
                if settings.auto_start_focus =>
            {
                Some(PomodoroKind::Focus)
            }
            _ => None,
        };
        if let Some(kind) = next_kind {
            self.start_automatic(kind, settings)?;
        }
        Ok(true)
    }

    fn start_automatic(
        &self,
        kind: PomodoroKind,
        settings: PomodoroSettings,
    ) -> Result<(), FocusUseCaseError> {
        let task = if kind == PomodoroKind::Focus {
            let task = match self.focus.latest_pomodoro_focus_task_id()? {
                Some(id) => self.tasks.get_task(id)?,
                None => None,
            };
            task.filter(|task| task.record().status == TaskStatus::Active)
                .map(|task| FocusTaskSnapshot {
                    id: Some(task.record().id),
                    title: task.record().title.as_str().to_owned(),
                    quadrant: placement_quadrant(task.record().placement),
                })
        } else {
            None
        };
        let now = self.clock.now();
        let local_date = self.today_context.today_context(now)?.local_date;
        let next = FocusSession::start(
            self.ids.generate(),
            task,
            FocusMode::Pomodoro,
            Some(kind),
            settings,
            now,
            local_date,
        )?;
        self.focus.create_focus_session(next)?;
        Ok(())
    }

    fn refresh_events(&self, success: Option<&str>) -> Vec<ApplicationEvent> {
        match self.load_state() {
            Ok(state) => {
                let mut events = vec![ApplicationEvent::FocusChanged(state)];
                if let Some(message) = success {
                    events.push(ApplicationEvent::OperationSucceeded(message.to_owned()));
                }
                events
            }
            Err(error) => vec![focus_failure_event(&error.into())],
        }
    }
}

fn placement_quadrant(placement: crate::TaskPlacement) -> Option<crate::Quadrant> {
    match placement {
        crate::TaskPlacement::Inbox => None,
        crate::TaskPlacement::Quadrant(quadrant) => Some(quadrant),
    }
}

#[derive(Clone, Copy)]
enum FocusTransition {
    Pause,
    Resume,
    Finish,
    Cancel,
}

/// Focus initial/refresh projection load failure.
#[derive(Debug, thiserror::Error)]
pub enum FocusLoadError {
    /// Repository query failed.
    #[error(transparent)]
    Repository(#[from] RepositoryError),
    /// Platform local-calendar derivation failed.
    #[error(transparent)]
    Calendar(#[from] CalendarError),
}

#[derive(Debug, thiserror::Error)]
enum FocusUseCaseError {
    #[error(transparent)]
    Repository(#[from] RepositoryError),
    #[error(transparent)]
    Calendar(#[from] CalendarError),
    #[error(transparent)]
    Domain(#[from] crate::FocusDomainError),
    #[error("a Focus session is already active")]
    CurrentSessionExists,
    #[error("there is no active Focus session")]
    NoCurrentSession,
    #[error("the selected task is no longer active")]
    TaskUnavailable,
}

impl From<FocusLoadError> for FocusUseCaseError {
    fn from(error: FocusLoadError) -> Self {
        match error {
            FocusLoadError::Repository(error) => Self::Repository(error),
            FocusLoadError::Calendar(error) => Self::Calendar(error),
        }
    }
}

fn focus_failure_event(error: &FocusUseCaseError) -> ApplicationEvent {
    let message = match error {
        FocusUseCaseError::Repository(error) => match error.operation() {
            RepositoryOperation::ReadFocus => "Focus state could not be loaded.",
            RepositoryOperation::WriteFocus => "The Focus session could not be changed.",
            RepositoryOperation::ReadSettings => "Pomodoro settings could not be loaded.",
            RepositoryOperation::WriteSettings => "Pomodoro settings could not be saved.",
            _ => "Focus could not be updated.",
        },
        FocusUseCaseError::Calendar(_) => "The local Focus date could not be determined.",
        FocusUseCaseError::Domain(error) => match error {
            crate::FocusDomainError::InvalidPomodoroSettings => {
                "Pomodoro durations or long-break interval are outside the allowed range."
            }
            crate::FocusDomainError::BreakCannotLinkTask => {
                "Break sessions cannot be associated with a task."
            }
            _ => "That Focus action is not valid right now.",
        },
        FocusUseCaseError::CurrentSessionExists => "Finish or cancel the current session first.",
        FocusUseCaseError::NoCurrentSession => "There is no active Focus session.",
        FocusUseCaseError::TaskUnavailable => "The selected task is no longer active.",
    };
    ApplicationEvent::OperationFailed(UserFacingError {
        message: message.to_owned(),
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FocusSignal {
    Changed,
    Shutdown,
}

/// Cloneable signal handle for Focus mutations and orderly shutdown.
#[derive(Clone, Debug)]
pub struct FocusSchedulerHandle {
    sender: mpsc::UnboundedSender<FocusSignal>,
}

impl FocusSchedulerHandle {
    /// Wakes the scheduler to recompute the current deadline.
    pub fn schedule_changed(&self) {
        let _ = self.sender.send(FocusSignal::Changed);
    }

    /// Requests orderly scheduler shutdown.
    pub fn shutdown(&self) {
        let _ = self.sender.send(FocusSignal::Shutdown);
    }
}

/// Long-lived service waiting on one Pomodoro deadline or an explicit mutation signal.
pub struct FocusScheduler {
    application: FocusApplication,
    events: Arc<dyn Fn(ApplicationEvent) + Send + Sync>,
    signals: mpsc::UnboundedReceiver<FocusSignal>,
}

impl FocusScheduler {
    /// Creates the scheduler and its non-blocking signal handle.
    #[must_use]
    pub fn new(
        application: FocusApplication,
        events: Arc<dyn Fn(ApplicationEvent) + Send + Sync>,
    ) -> (Self, FocusSchedulerHandle) {
        let (sender, signals) = mpsc::unbounded_channel();
        (
            Self {
                application,
                events,
                signals,
            },
            FocusSchedulerHandle { sender },
        )
    }

    /// Runs until shutdown or all signal senders are dropped.
    pub async fn run(mut self) {
        loop {
            let application = self.application.clone();
            let deadline =
                tokio::task::spawn_blocking(move || application.current_deadline()).await;
            let Ok(Ok(deadline)) = deadline else {
                if !self.wait_for_change().await {
                    break;
                }
                continue;
            };
            if let Some(deadline) = deadline {
                let seconds = deadline
                    .unix_seconds()
                    .saturating_sub(self.application.clock.now().unix_seconds());
                let wait = Duration::from_secs(u64::try_from(seconds.max(0)).unwrap_or(0));
                tokio::select! {
                    () = tokio::time::sleep(wait) => {
                        let application = self.application.clone();
                        if let Ok(events) = tokio::task::spawn_blocking(move || application.complete_due()).await {
                            for event in events {
                                (self.events)(event);
                            }
                        }
                    }
                    signal = self.signals.recv() => {
                        if !matches!(signal, Some(FocusSignal::Changed)) {
                            break;
                        }
                    }
                }
            } else if !self.wait_for_change().await {
                break;
            }
        }
    }

    async fn wait_for_change(&mut self) -> bool {
        matches!(self.signals.recv().await, Some(FocusSignal::Changed))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicI64, AtomicU64, Ordering},
    };

    use uuid::Uuid;

    use super::FocusApplication;
    use crate::{
        Clock, DesktopSettings, FocusDaySummary, FocusMode, FocusRepository, FocusSession,
        FocusSessionId, FocusSessionIdGenerator, FocusStartRequest, FocusStatus, LocalDate,
        NewTask, PomodoroKind, PomodoroSettings, RepositoryError, RepositoryOperation,
        SettingsRepository, Task, TaskDetailsUpdate, TaskId, TaskPlacement, TaskRepository,
        ThemeMode, TodayContext, TodayContextSource, UiIntent, UtcTimestamp,
    };

    #[derive(Default)]
    struct MemoryFocusRepository {
        sessions: Mutex<Vec<FocusSession>>,
    }

    impl FocusRepository for MemoryFocusRepository {
        fn get_current_focus_session(&self) -> Result<Option<FocusSession>, RepositoryError> {
            Ok(self
                .sessions
                .lock()
                .expect("focus lock")
                .iter()
                .find(|session| session.record().status.is_current())
                .cloned())
        }

        fn create_focus_session(
            &self,
            session: FocusSession,
        ) -> Result<FocusSession, RepositoryError> {
            let mut sessions = self.sessions.lock().expect("focus lock");
            if sessions
                .iter()
                .any(|stored| stored.record().status.is_current())
            {
                return Err(test_error(RepositoryOperation::WriteFocus));
            }
            sessions.push(session.clone());
            Ok(session)
        }

        fn transition_focus_session(
            &self,
            session: FocusSession,
            expected: FocusStatus,
        ) -> Result<FocusSession, RepositoryError> {
            let mut sessions = self.sessions.lock().expect("focus lock");
            let stored = sessions
                .iter_mut()
                .find(|stored| stored.record().id == session.record().id)
                .ok_or_else(|| test_error(RepositoryOperation::WriteFocus))?;
            if stored.record().status != expected {
                return Err(test_error(RepositoryOperation::WriteFocus));
            }
            stored.clone_from(&session);
            Ok(session)
        }

        fn productive_focus_summary(
            &self,
            local_date: LocalDate,
        ) -> Result<FocusDaySummary, RepositoryError> {
            let sessions = self.sessions.lock().expect("focus lock");
            let productive = sessions
                .iter()
                .filter(|session| {
                    session.record().created_local_date == local_date && session.is_productive()
                })
                .collect::<Vec<_>>();
            Ok(FocusDaySummary {
                total_seconds: productive
                    .iter()
                    .map(|session| u64::from(session.record().duration_seconds))
                    .sum(),
                session_count: u32::try_from(productive.len()).unwrap_or(u32::MAX),
            })
        }

        fn completed_pomodoro_focus_count(&self) -> Result<u64, RepositoryError> {
            let count = self
                .sessions
                .lock()
                .expect("focus lock")
                .iter()
                .filter(|session| {
                    session.record().status == FocusStatus::Completed
                        && session.record().mode == FocusMode::Pomodoro
                        && session.record().pomodoro_kind == Some(PomodoroKind::Focus)
                })
                .count();
            Ok(u64::try_from(count).unwrap_or(u64::MAX))
        }

        fn latest_pomodoro_focus_task_id(&self) -> Result<Option<TaskId>, RepositoryError> {
            Ok(None)
        }
    }

    #[derive(Default)]
    struct EmptyTaskRepository;

    impl TaskRepository for EmptyTaskRepository {
        fn create_task(
            &self,
            _id: TaskId,
            _draft: NewTask,
            _now: UtcTimestamp,
        ) -> Result<Task, RepositoryError> {
            Err(test_error(RepositoryOperation::CreateTask))
        }

        fn list_active_tasks(&self) -> Result<Vec<Task>, RepositoryError> {
            Ok(Vec::new())
        }

        fn get_task(&self, _id: TaskId) -> Result<Option<Task>, RepositoryError> {
            Ok(None)
        }

        fn move_task(
            &self,
            _id: TaskId,
            _placement: TaskPlacement,
            _now: UtcTimestamp,
        ) -> Result<Task, RepositoryError> {
            Err(test_error(RepositoryOperation::UpdateTask))
        }

        fn reorder_task(
            &self,
            _id: TaskId,
            _direction: crate::ReorderDirection,
            _now: UtcTimestamp,
        ) -> Result<Task, RepositoryError> {
            Err(test_error(RepositoryOperation::UpdateTask))
        }

        fn update_task(
            &self,
            _id: TaskId,
            _update: TaskDetailsUpdate,
            _now: UtcTimestamp,
        ) -> Result<Task, RepositoryError> {
            Err(test_error(RepositoryOperation::UpdateTask))
        }

        fn complete_task(
            &self,
            _id: TaskId,
            _next_occurrence_id: TaskId,
            _now: UtcTimestamp,
        ) -> Result<Task, RepositoryError> {
            Err(test_error(RepositoryOperation::TransitionTask))
        }

        fn reopen_task(&self, _id: TaskId, _now: UtcTimestamp) -> Result<Task, RepositoryError> {
            Err(test_error(RepositoryOperation::TransitionTask))
        }

        fn delete_task(&self, _id: TaskId) -> Result<(), RepositoryError> {
            Err(test_error(RepositoryOperation::DeleteTask))
        }
    }

    struct MemorySettings(Mutex<PomodoroSettings>);

    impl SettingsRepository for MemorySettings {
        fn load_theme_mode(&self) -> Result<Option<ThemeMode>, RepositoryError> {
            Ok(None)
        }

        fn save_theme_mode(
            &self,
            _theme_mode: ThemeMode,
            _now: UtcTimestamp,
        ) -> Result<(), RepositoryError> {
            Ok(())
        }

        fn load_desktop_settings(&self) -> Result<DesktopSettings, RepositoryError> {
            Ok(DesktopSettings::default())
        }

        fn save_desktop_settings(
            &self,
            _settings: DesktopSettings,
            _now: UtcTimestamp,
        ) -> Result<(), RepositoryError> {
            Ok(())
        }

        fn load_pomodoro_settings(&self) -> Result<PomodoroSettings, RepositoryError> {
            Ok(*self.0.lock().expect("settings lock"))
        }

        fn save_pomodoro_settings(
            &self,
            settings: PomodoroSettings,
            _now: UtcTimestamp,
        ) -> Result<(), RepositoryError> {
            *self.0.lock().expect("settings lock") = settings;
            Ok(())
        }
    }

    struct MutableClock(AtomicI64);

    impl Clock for MutableClock {
        fn now(&self) -> UtcTimestamp {
            UtcTimestamp::from_unix_seconds(self.0.load(Ordering::SeqCst))
        }
    }

    #[derive(Default)]
    struct IncrementingIds(AtomicU64);

    impl FocusSessionIdGenerator for IncrementingIds {
        fn generate(&self) -> FocusSessionId {
            FocusSessionId::from_uuid(Uuid::from_u128(u128::from(
                self.0.fetch_add(1, Ordering::SeqCst) + 1,
            )))
        }
    }

    struct FixedToday(LocalDate);

    impl TodayContextSource for FixedToday {
        fn today_context(&self, _now: UtcTimestamp) -> Result<TodayContext, crate::CalendarError> {
            Ok(TodayContext {
                local_date: self.0,
                day_start_utc: UtcTimestamp::from_unix_seconds(0),
                next_day_start_utc: UtcTimestamp::from_unix_seconds(86_400),
            })
        }
    }

    fn test_application(
        settings: PomodoroSettings,
    ) -> (
        FocusApplication,
        Arc<MemoryFocusRepository>,
        Arc<MutableClock>,
    ) {
        let repository = Arc::new(MemoryFocusRepository::default());
        let clock = Arc::new(MutableClock(AtomicI64::new(100)));
        let application = FocusApplication::new(
            Arc::clone(&repository) as Arc<dyn FocusRepository>,
            Arc::new(EmptyTaskRepository),
            Arc::new(MemorySettings(Mutex::new(settings))),
            Arc::clone(&clock) as Arc<dyn Clock>,
            Arc::new(IncrementingIds::default()),
            Arc::new(FixedToday(
                LocalDate::parse_iso("2026-09-01").expect("valid date"),
            )),
        );
        (application, repository, clock)
    }

    fn test_error(operation: RepositoryOperation) -> RepositoryError {
        RepositoryError::new(operation, "not used by this test")
    }

    #[test]
    fn use_case_persists_stopwatch_pause_resume_and_completion() {
        let (application, repository, clock) = test_application(PomodoroSettings::default());
        let _ = application.handle(&UiIntent::StartFocus(FocusStartRequest {
            mode: FocusMode::Stopwatch,
            pomodoro_kind: None,
            task_id: None,
        }));
        clock.0.store(110, Ordering::SeqCst);
        let _ = application.handle(&UiIntent::PauseFocus);
        clock.0.store(200, Ordering::SeqCst);
        let _ = application.handle(&UiIntent::ResumeFocus);
        clock.0.store(207, Ordering::SeqCst);
        let _ = application.handle(&UiIntent::FinishFocus);

        let sessions = repository.sessions.lock().expect("focus lock");
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].record().status, FocusStatus::Completed);
        assert_eq!(sessions[0].record().duration_seconds, 17);
    }

    #[test]
    fn due_focus_auto_starts_the_break_selected_by_cadence() {
        let settings = PomodoroSettings {
            focus_minutes: 1,
            auto_start_break: true,
            ..PomodoroSettings::default()
        };
        let (application, repository, clock) = test_application(settings);
        let _ = application.handle(&UiIntent::StartFocus(FocusStartRequest {
            mode: FocusMode::Pomodoro,
            pomodoro_kind: Some(PomodoroKind::Focus),
            task_id: None,
        }));
        clock.0.store(160, Ordering::SeqCst);
        assert!(!application.complete_due().is_empty());

        let current = repository
            .get_current_focus_session()
            .expect("focus query")
            .expect("automatic break");
        assert_eq!(
            current.record().pomodoro_kind,
            Some(PomodoroKind::ShortBreak)
        );
        assert_eq!(current.record().status, FocusStatus::Running);
    }
}
