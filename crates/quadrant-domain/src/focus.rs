//! Focus timer state and Pomodoro value objects.

use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::{LocalDate, Quadrant, TaskId, UtcTimestamp};

/// Opaque identity for one persisted focus session.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct FocusSessionId(Uuid);

impl FocusSessionId {
    /// Wraps a validated UUID.
    #[must_use]
    pub const fn from_uuid(value: Uuid) -> Self {
        Self(value)
    }

    /// Returns the UUID value.
    #[must_use]
    pub const fn as_uuid(self) -> Uuid {
        self.0
    }
}

impl fmt::Display for FocusSessionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for FocusSessionId {
    type Err = uuid::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(value).map(Self)
    }
}

/// Timer style chosen for a session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FocusMode {
    /// Count upward until the user finishes.
    Stopwatch,
    /// Count down from a validated Pomodoro duration.
    Pomodoro,
}

/// Pomodoro phase represented by a session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PomodoroKind {
    /// Productive focus interval.
    Focus,
    /// Short restorative break.
    ShortBreak,
    /// Long restorative break.
    LongBreak,
}

/// Persisted session lifecycle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FocusStatus {
    /// A time-anchored segment is accumulating.
    Running,
    /// Accumulation is stopped until resume.
    Paused,
    /// The user or Pomodoro deadline finished the session.
    Completed,
    /// The user discarded the active session.
    Cancelled,
}

impl FocusStatus {
    /// Returns whether the session may still transition.
    #[must_use]
    pub const fn is_current(self) -> bool {
        matches!(self, Self::Running | Self::Paused)
    }
}

/// Task details copied into focus history so later task edits/deletion do not rewrite Review.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FocusTaskSnapshot {
    /// Associated task identity while the source task still exists.
    pub id: Option<TaskId>,
    /// Title at focus start.
    pub title: String,
    /// Quadrant at focus start; `None` means Inbox.
    pub quadrant: Option<Quadrant>,
}

/// Validated Pomodoro defaults and automatic continuation behavior.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PomodoroSettings {
    /// Productive interval length.
    pub focus_minutes: u16,
    /// Short break length.
    pub short_break_minutes: u16,
    /// Long break length.
    pub long_break_minutes: u16,
    /// Productive intervals between long breaks.
    pub long_break_interval: u8,
    /// Automatically start the suggested break after a focus deadline.
    pub auto_start_break: bool,
    /// Automatically start a focus interval after a break deadline.
    pub auto_start_focus: bool,
}

impl Default for PomodoroSettings {
    fn default() -> Self {
        Self {
            focus_minutes: 25,
            short_break_minutes: 5,
            long_break_minutes: 15,
            long_break_interval: 4,
            auto_start_break: false,
            auto_start_focus: false,
        }
    }
}

impl PomodoroSettings {
    /// Validates user-configurable bounds.
    ///
    /// # Errors
    ///
    /// Returns [`FocusDomainError::InvalidPomodoroSettings`] outside supported limits.
    pub const fn validate(self) -> Result<Self, FocusDomainError> {
        if self.focus_minutes < 1
            || self.focus_minutes > 240
            || self.short_break_minutes < 1
            || self.short_break_minutes > 120
            || self.long_break_minutes < 1
            || self.long_break_minutes > 120
            || self.long_break_interval < 2
            || self.long_break_interval > 12
        {
            return Err(FocusDomainError::InvalidPomodoroSettings);
        }
        Ok(self)
    }

    /// Returns one phase duration in seconds.
    #[must_use]
    pub const fn duration_seconds(self, kind: PomodoroKind) -> u32 {
        let minutes = match kind {
            PomodoroKind::Focus => self.focus_minutes,
            PomodoroKind::ShortBreak => self.short_break_minutes,
            PomodoroKind::LongBreak => self.long_break_minutes,
        };
        minutes as u32 * 60
    }
}

/// Persistence-shaped validated focus state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FocusSessionRecord {
    /// Session identity.
    pub id: FocusSessionId,
    /// Optional immutable task snapshot.
    pub task: Option<FocusTaskSnapshot>,
    /// Stopwatch or Pomodoro.
    pub mode: FocusMode,
    /// Pomodoro phase; absent for stopwatch.
    pub pomodoro_kind: Option<PomodoroKind>,
    /// First start instant.
    pub started_at: UtcTimestamp,
    /// Current running-segment anchor.
    pub active_segment_started_at: Option<UtcTimestamp>,
    /// Terminal instant.
    pub ended_at: Option<UtcTimestamp>,
    /// Fixed Pomodoro duration; absent for stopwatch.
    pub target_duration_seconds: Option<u32>,
    /// Whole accumulated running seconds before the current segment.
    pub duration_seconds: u32,
    /// Lifecycle state.
    pub status: FocusStatus,
    /// Host-local date at initial start, used by Review aggregation.
    pub created_local_date: LocalDate,
}

/// Validated focus aggregate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FocusSession(FocusSessionRecord);

impl FocusSession {
    /// Starts a session from an explicit clock value.
    ///
    /// # Errors
    ///
    /// Returns a configuration error for invalid mode/kind/task combinations.
    pub fn start(
        id: FocusSessionId,
        task: Option<FocusTaskSnapshot>,
        mode: FocusMode,
        pomodoro_kind: Option<PomodoroKind>,
        pomodoro_settings: PomodoroSettings,
        now: UtcTimestamp,
        created_local_date: LocalDate,
    ) -> Result<Self, FocusDomainError> {
        let target_duration_seconds = match (mode, pomodoro_kind) {
            (FocusMode::Stopwatch, None) => None,
            (FocusMode::Pomodoro, Some(kind)) => {
                if matches!(kind, PomodoroKind::ShortBreak | PomodoroKind::LongBreak)
                    && task.is_some()
                {
                    return Err(FocusDomainError::BreakCannotLinkTask);
                }
                Some(pomodoro_settings.validate()?.duration_seconds(kind))
            }
            _ => return Err(FocusDomainError::InvalidSessionConfiguration),
        };
        Self::restore(FocusSessionRecord {
            id,
            task,
            mode,
            pomodoro_kind,
            started_at: now,
            active_segment_started_at: Some(now),
            ended_at: None,
            target_duration_seconds,
            duration_seconds: 0,
            status: FocusStatus::Running,
            created_local_date,
        })
    }

    /// Restores a persistence record after validating every invariant.
    ///
    /// # Errors
    ///
    /// Returns [`FocusDomainError::InvalidStoredSession`] for incoherent state.
    pub fn restore(record: FocusSessionRecord) -> Result<Self, FocusDomainError> {
        let mode_valid = matches!(
            (
                record.mode,
                record.pomodoro_kind,
                record.target_duration_seconds
            ),
            (FocusMode::Stopwatch, None, None) | (FocusMode::Pomodoro, Some(_), Some(1..=u32::MAX))
        );
        let lifecycle_valid = match record.status {
            FocusStatus::Running => {
                record.active_segment_started_at.is_some() && record.ended_at.is_none()
            }
            FocusStatus::Paused => {
                record.active_segment_started_at.is_none() && record.ended_at.is_none()
            }
            FocusStatus::Completed | FocusStatus::Cancelled => {
                record.active_segment_started_at.is_none() && record.ended_at.is_some()
            }
        };
        let times_valid = record
            .active_segment_started_at
            .is_none_or(|value| value >= record.started_at)
            && record
                .ended_at
                .is_none_or(|value| value >= record.started_at);
        let duration_valid = record
            .target_duration_seconds
            .is_none_or(|target| record.duration_seconds <= target);
        let break_valid = !matches!(
            record.pomodoro_kind,
            Some(PomodoroKind::ShortBreak | PomodoroKind::LongBreak)
        ) || record.task.is_none();
        if !mode_valid || !lifecycle_valid || !times_valid || !duration_valid || !break_valid {
            return Err(FocusDomainError::InvalidStoredSession);
        }
        Ok(Self(record))
    }

    /// Returns the immutable persistence record.
    #[must_use]
    pub const fn record(&self) -> &FocusSessionRecord {
        &self.0
    }

    /// Returns elapsed whole running seconds at `now`, independent of UI tick cadence.
    #[must_use]
    pub fn elapsed_seconds_at(&self, now: UtcTimestamp) -> u32 {
        let segment = if self.0.status == FocusStatus::Running {
            self.0.active_segment_started_at.map_or(0, |started| {
                nonnegative_seconds(now.unix_seconds().saturating_sub(started.unix_seconds()))
            })
        } else {
            0
        };
        let elapsed = self.0.duration_seconds.saturating_add(segment);
        self.0
            .target_duration_seconds
            .map_or(elapsed, |target| elapsed.min(target))
    }

    /// Returns remaining Pomodoro seconds, or `None` for stopwatch.
    #[must_use]
    pub fn remaining_seconds_at(&self, now: UtcTimestamp) -> Option<u32> {
        self.0
            .target_duration_seconds
            .map(|target| target.saturating_sub(self.elapsed_seconds_at(now)))
    }

    /// Returns the current Pomodoro deadline while running.
    #[must_use]
    pub fn deadline(&self) -> Option<UtcTimestamp> {
        if self.0.status != FocusStatus::Running {
            return None;
        }
        let remaining = self
            .0
            .target_duration_seconds?
            .saturating_sub(self.0.duration_seconds);
        checked_add_seconds(self.0.active_segment_started_at?, remaining)
    }

    /// Pauses a running session and freezes accumulated time.
    ///
    /// # Errors
    ///
    /// Returns an invalid-transition error unless currently running.
    pub fn pause(&mut self, now: UtcTimestamp) -> Result<(), FocusDomainError> {
        self.require_status(FocusStatus::Running)?;
        self.0.duration_seconds = self.elapsed_seconds_at(now);
        self.0.active_segment_started_at = None;
        self.0.status = FocusStatus::Paused;
        Ok(())
    }

    /// Resumes a paused session from a fresh clock anchor.
    ///
    /// # Errors
    ///
    /// Returns an invalid-transition error unless currently paused.
    pub fn resume(&mut self, now: UtcTimestamp) -> Result<(), FocusDomainError> {
        self.require_status(FocusStatus::Paused)?;
        self.0.active_segment_started_at = Some(now);
        self.0.status = FocusStatus::Running;
        Ok(())
    }

    /// Finishes a running or paused session at the requested instant.
    ///
    /// # Errors
    ///
    /// Returns an invalid-transition error from terminal state.
    pub fn complete(&mut self, now: UtcTimestamp) -> Result<(), FocusDomainError> {
        self.require_current()?;
        self.0.duration_seconds = self.elapsed_seconds_at(now);
        self.0.active_segment_started_at = None;
        self.0.ended_at = Some(now.max(self.0.started_at));
        self.0.status = FocusStatus::Completed;
        Ok(())
    }

    /// Completes a due running Pomodoro exactly at its deadline.
    ///
    /// # Errors
    ///
    /// Returns an error when the session is not a due running Pomodoro.
    pub fn complete_if_due(&mut self, now: UtcTimestamp) -> Result<bool, FocusDomainError> {
        let Some(deadline) = self.deadline() else {
            return Ok(false);
        };
        if now < deadline {
            return Ok(false);
        }
        self.0.duration_seconds = self
            .0
            .target_duration_seconds
            .ok_or(FocusDomainError::InvalidStoredSession)?;
        self.0.active_segment_started_at = None;
        self.0.ended_at = Some(deadline);
        self.0.status = FocusStatus::Completed;
        Ok(true)
    }

    /// Cancels a running or paused session while retaining an audit row.
    ///
    /// # Errors
    ///
    /// Returns an invalid-transition error from terminal state.
    pub fn cancel(&mut self, now: UtcTimestamp) -> Result<(), FocusDomainError> {
        self.require_current()?;
        self.0.duration_seconds = self.elapsed_seconds_at(now);
        self.0.active_segment_started_at = None;
        self.0.ended_at = Some(now.max(self.0.started_at));
        self.0.status = FocusStatus::Cancelled;
        Ok(())
    }

    /// Returns whether Review should count this completed session as productive focus.
    #[must_use]
    pub fn is_productive(&self) -> bool {
        self.0.status == FocusStatus::Completed
            && (self.0.mode == FocusMode::Stopwatch
                || matches!(self.0.pomodoro_kind, Some(PomodoroKind::Focus)))
    }

    fn require_status(&self, expected: FocusStatus) -> Result<(), FocusDomainError> {
        if self.0.status == expected {
            Ok(())
        } else {
            Err(FocusDomainError::InvalidTransition {
                from: self.0.status,
            })
        }
    }

    fn require_current(&self) -> Result<(), FocusDomainError> {
        if self.0.status.is_current() {
            Ok(())
        } else {
            Err(FocusDomainError::InvalidTransition {
                from: self.0.status,
            })
        }
    }
}

fn nonnegative_seconds(value: i64) -> u32 {
    u32::try_from(value.max(0)).unwrap_or(u32::MAX)
}

fn checked_add_seconds(value: UtcTimestamp, seconds: u32) -> Option<UtcTimestamp> {
    value
        .unix_seconds()
        .checked_add(i64::from(seconds))
        .map(UtcTimestamp::from_unix_seconds)
}

/// Focus state/configuration validation errors.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum FocusDomainError {
    /// Mode, Pomodoro kind, and target duration were inconsistent.
    #[error("focus session configuration is invalid")]
    InvalidSessionConfiguration,
    /// Break sessions cannot be associated with a task.
    #[error("Pomodoro breaks cannot be linked to a task")]
    BreakCannotLinkTask,
    /// User settings were outside supported bounds.
    #[error("Pomodoro settings are invalid")]
    InvalidPomodoroSettings,
    /// A persisted session violated domain invariants.
    #[error("stored focus session is invalid")]
    InvalidStoredSession,
    /// A command was not valid from the current state.
    #[error("focus session cannot transition from {from:?}")]
    InvalidTransition {
        /// State observed by the rejected transition.
        from: FocusStatus,
    },
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::{
        FocusDomainError, FocusMode, FocusSession, FocusSessionId, FocusStatus, FocusTaskSnapshot,
        PomodoroKind, PomodoroSettings,
    };
    use crate::{LocalDate, Quadrant, TaskId, UtcTimestamp};

    fn id(value: u128) -> FocusSessionId {
        FocusSessionId::from_uuid(Uuid::from_u128(value))
    }

    fn day() -> LocalDate {
        LocalDate::parse_iso("2026-09-01").expect("valid date")
    }

    #[test]
    fn stopwatch_counts_only_running_segments() {
        let mut session = FocusSession::start(
            id(1),
            None,
            FocusMode::Stopwatch,
            None,
            PomodoroSettings::default(),
            UtcTimestamp::from_unix_seconds(100),
            day(),
        )
        .expect("session starts");
        assert_eq!(
            session.elapsed_seconds_at(UtcTimestamp::from_unix_seconds(110)),
            10
        );
        session
            .pause(UtcTimestamp::from_unix_seconds(111))
            .expect("session pauses");
        assert_eq!(
            session.elapsed_seconds_at(UtcTimestamp::from_unix_seconds(999)),
            11
        );
        session
            .resume(UtcTimestamp::from_unix_seconds(200))
            .expect("session resumes");
        session
            .complete(UtcTimestamp::from_unix_seconds(207))
            .expect("session completes");
        assert_eq!(session.record().duration_seconds, 18);
        assert!(session.is_productive());
    }

    #[test]
    fn pomodoro_uses_deadline_and_clamps_late_wakeup() {
        let settings = PomodoroSettings {
            focus_minutes: 1,
            ..PomodoroSettings::default()
        };
        let mut session = FocusSession::start(
            id(2),
            None,
            FocusMode::Pomodoro,
            Some(PomodoroKind::Focus),
            settings,
            UtcTimestamp::from_unix_seconds(1_000),
            day(),
        )
        .expect("session starts");
        assert_eq!(
            session.deadline(),
            Some(UtcTimestamp::from_unix_seconds(1_060))
        );
        assert_eq!(
            session.remaining_seconds_at(UtcTimestamp::from_unix_seconds(1_015)),
            Some(45)
        );
        assert!(
            session
                .complete_if_due(UtcTimestamp::from_unix_seconds(2_000))
                .expect("deadline completes")
        );
        assert_eq!(session.record().duration_seconds, 60);
        assert_eq!(
            session.record().ended_at,
            Some(UtcTimestamp::from_unix_seconds(1_060))
        );
    }

    #[test]
    fn breaks_reject_task_association_and_are_not_productive() {
        let task = FocusTaskSnapshot {
            id: Some(TaskId::from_uuid(Uuid::from_u128(3))),
            title: "Task".to_owned(),
            quadrant: Some(Quadrant::Q2),
        };
        assert_eq!(
            FocusSession::start(
                id(3),
                Some(task),
                FocusMode::Pomodoro,
                Some(PomodoroKind::ShortBreak),
                PomodoroSettings::default(),
                UtcTimestamp::from_unix_seconds(0),
                day(),
            ),
            Err(FocusDomainError::BreakCannotLinkTask)
        );
        let mut break_session = FocusSession::start(
            id(4),
            None,
            FocusMode::Pomodoro,
            Some(PomodoroKind::LongBreak),
            PomodoroSettings::default(),
            UtcTimestamp::from_unix_seconds(0),
            day(),
        )
        .expect("break starts");
        break_session
            .complete(UtcTimestamp::from_unix_seconds(30))
            .expect("break completes");
        assert!(!break_session.is_productive());
        assert_eq!(break_session.record().status, FocusStatus::Completed);
    }

    #[test]
    fn pomodoro_settings_enforce_product_bounds() {
        assert!(PomodoroSettings::default().validate().is_ok());
        assert_eq!(
            PomodoroSettings {
                focus_minutes: 0,
                ..PomodoroSettings::default()
            }
            .validate(),
            Err(FocusDomainError::InvalidPomodoroSettings)
        );
        assert_eq!(
            PomodoroSettings {
                long_break_interval: 13,
                ..PomodoroSettings::default()
            }
            .validate(),
            Err(FocusDomainError::InvalidPomodoroSettings)
        );
    }
}
