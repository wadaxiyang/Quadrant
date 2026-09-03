//! Application use cases, ports, projections, and typed events.

#![forbid(unsafe_code)]

mod focus;
mod history;
mod maintenance;
mod ports;
mod reminders;
mod tasks;
mod today;
mod updates;

pub use focus::{FocusApplication, FocusScheduler, FocusSchedulerHandle};
pub use history::{
    CompletedTaskSummary, CompletedViewState, HistoryApplication, HistoryLoadError,
    ReviewActivityPoint, ReviewDateRange, ReviewFocusHighlights, ReviewQuadrantValue, ReviewQuery,
    ReviewQueryData, ReviewRange, ReviewRecentCompletion, ReviewTotals, ReviewViewState,
};
pub use maintenance::{BackupInfo, MaintenanceApplication, MaintenanceState};
pub use ports::{
    AutostartError, AutostartService, CalendarError, Clock, CompletedRepository, ExternalOpener,
    FocusRepository, FocusSessionIdGenerator, MaintenanceRepository, PlatformActionError,
    ReminderRepository, RepositoryError, RepositoryOperation, ReviewRepository, SettingsRepository,
    SystemClock, TaskIdGenerator, TaskRepository, TodayContextSource, TodayRepository,
    UuidFocusSessionIdGenerator, UuidTaskIdGenerator,
};
pub use quadrant_domain::{
    FocusDomainError, FocusMode, FocusSession, FocusSessionId, FocusSessionRecord, FocusStatus,
    FocusTaskSnapshot, LocalDate, NewTask, PomodoroKind, PomodoroSettings, Quadrant,
    RecurrencePattern, RecurrenceRule, ScheduledInstant, SortKey, Task, TaskDetailsUpdate,
    TaskDomainError, TaskId, TaskPlacement, TaskStatus, TaskTitle, TimeZoneId, UtcTimestamp,
};
pub use reminders::{
    ReminderAlert, ReminderDelivery, ReminderDeliveryError, ReminderPlan, ReminderScheduler,
    ReminderSchedulerHandle,
};
pub use tasks::{ApplicationLoadError, TaskApplication};
pub use today::{TodayContext, TodayTaskSummary, TodayViewState};
pub use updates::{DistributionChannel, UpdateViewState};

use std::fmt;

use time::{OffsetDateTime, format_description::well_known::Rfc3339};

/// A typed intent emitted by the presentation layer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UiIntent {
    /// Navigate to a top-level application route.
    Navigate(NavigationRoute),
    /// Open the dedicated Quick Add surface.
    OpenQuickAdd,
    /// Submit a captured task from Quick Add.
    SubmitQuickAdd(QuickAddSubmission),
    /// Change the user's preferred color theme.
    SetTheme(ThemeMode),
    /// Persist and apply desktop startup/window behavior.
    SetDesktopSettings(DesktopSettings),
    /// Start a stopwatch or Pomodoro phase, optionally associated with an active task.
    StartFocus(FocusStartRequest),
    /// Freeze the current running focus session.
    PauseFocus,
    /// Resume the current paused focus session.
    ResumeFocus,
    /// Complete the current focus session at its present elapsed duration.
    FinishFocus,
    /// Discard the current session while retaining an audit row.
    CancelFocus,
    /// Persist validated Pomodoro defaults and automatic continuation choices.
    SetPomodoroSettings(PomodoroSettings),
    /// Change the active Review date range.
    SetReviewRange(ReviewRange),
    /// Increase the bounded Completed history page size.
    LoadMoreCompleted,
    /// Create a validated `SQLite` backup in the application backup directory.
    CreateBackup,
    /// Validate and stage the newest backup for the next startup.
    StageLatestRestore,
    /// Open the application-private backup directory.
    OpenBackupDirectory,
    /// Open the distribution-neutral GitHub Releases page.
    OpenReleasePage,
    /// Move an active task into Inbox or a quadrant.
    MoveTask {
        /// Task to move.
        task_id: TaskId,
        /// Destination placement.
        placement: TaskPlacement,
    },
    /// Move an active task one step within its current placement.
    ReorderTask {
        /// Task to reorder.
        task_id: TaskId,
        /// Relative movement requested by the user.
        direction: ReorderDirection,
    },
    /// Load one task into the dedicated editor surface.
    OpenTaskEditor(TaskId),
    /// Validate and persist the task editor fields.
    SubmitTaskEditor(TaskEditorSubmission),
    /// Complete an active task.
    CompleteTask(TaskId),
    /// Restore a completed task and revert its latest active completion event.
    ReopenTask(TaskId),
    /// Permanently delete a task while retaining immutable completion snapshots.
    DeleteTask(TaskId),
    /// Persist edited task details.
    UpdateTask {
        /// Task to update.
        task_id: TaskId,
        /// Validated replacement details.
        update: TaskDetailsUpdate,
    },
}

impl UiIntent {
    /// Returns whether this intent can change the active reminder schedule.
    #[must_use]
    pub const fn affects_reminder_schedule(&self) -> bool {
        matches!(
            self,
            Self::SubmitQuickAdd(_)
                | Self::SubmitTaskEditor(_)
                | Self::CompleteTask(_)
                | Self::ReopenTask(_)
                | Self::DeleteTask(_)
                | Self::UpdateTask { .. }
        )
    }

    /// Returns whether the Focus application service owns this intent.
    #[must_use]
    pub const fn is_focus_intent(&self) -> bool {
        matches!(
            self,
            Self::Navigate(NavigationRoute::Focus)
                | Self::StartFocus(_)
                | Self::PauseFocus
                | Self::ResumeFocus
                | Self::FinishFocus
                | Self::CancelFocus
                | Self::SetPomodoroSettings(_)
        )
    }

    /// Returns whether the Review/Completed query service owns this intent.
    #[must_use]
    pub const fn is_history_intent(&self) -> bool {
        matches!(
            self,
            Self::Navigate(NavigationRoute::Review | NavigationRoute::Completed)
                | Self::SetReviewRange(_)
                | Self::LoadMoreCompleted
        )
    }

    /// Returns whether the maintenance/release service owns this intent.
    #[must_use]
    pub const fn is_maintenance_intent(&self) -> bool {
        matches!(
            self,
            Self::CreateBackup
                | Self::StageLatestRestore
                | Self::OpenBackupDirectory
                | Self::OpenReleasePage
        )
    }

    /// Returns whether the nearest-deadline Focus scheduler must recompute.
    #[must_use]
    pub const fn affects_focus_schedule(&self) -> bool {
        matches!(
            self,
            Self::StartFocus(_)
                | Self::PauseFocus
                | Self::ResumeFocus
                | Self::FinishFocus
                | Self::CancelFocus
                | Self::SetPomodoroSettings(_)
        )
    }

    /// Returns whether active-task choices or a current task association may have changed.
    #[must_use]
    pub const fn affects_focus_projection(&self) -> bool {
        matches!(
            self,
            Self::SubmitQuickAdd(_)
                | Self::SubmitTaskEditor(_)
                | Self::MoveTask { .. }
                | Self::CompleteTask(_)
                | Self::ReopenTask(_)
                | Self::DeleteTask(_)
                | Self::UpdateTask { .. }
        )
    }

    /// Returns whether Review or Completed projections may have changed.
    #[must_use]
    pub const fn affects_history_projection(&self) -> bool {
        matches!(
            self,
            Self::CompleteTask(_) | Self::ReopenTask(_) | Self::DeleteTask(_) | Self::FinishFocus
        )
    }
}

/// User selection for beginning a Focus session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FocusStartRequest {
    /// Count upward or down from a Pomodoro duration.
    pub mode: FocusMode,
    /// Required for Pomodoro and absent for stopwatch.
    pub pomodoro_kind: Option<PomodoroKind>,
    /// Optional task for productive sessions; breaks ignore task selection.
    pub task_id: Option<TaskId>,
}

/// Active task available for Focus association.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FocusTaskSummary {
    /// Stable task identity.
    pub id: TaskId,
    /// Current title.
    pub title: String,
    /// Compact placement label.
    pub placement: TaskPlacement,
}

/// Productive Focus completed on one host-local date.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FocusDaySummary {
    /// Sum of productive completed running time.
    pub total_seconds: u64,
    /// Number of productive completed sessions.
    pub session_count: u32,
}

/// Repository-backed state consumed by the Focus view.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FocusViewState {
    /// Active tasks available to associate with a new productive session.
    pub tasks: Vec<FocusTaskSummary>,
    /// The only running or paused session.
    pub session: Option<FocusSession>,
    /// Validated Pomodoro defaults.
    pub settings: PomodoroSettings,
    /// Productive Focus completed today.
    pub today: FocusDaySummary,
}

/// Relative manual-order operation within one task placement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReorderDirection {
    /// Move one visible position toward the start.
    Up,
    /// Move one visible position toward the end.
    Down,
}

/// Recurrence choice exposed by the task editor without leaking Slint indices.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RecurrenceChoice {
    /// No recurrence.
    #[default]
    None,
    /// Repeat daily.
    Daily,
    /// Repeat weekly.
    Weekly,
    /// Repeat monthly.
    Monthly,
    /// Repeat every validated number of days.
    CustomDays,
}

/// Repository-backed values used to populate the task editor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskEditorState {
    /// Edited task identity.
    pub task_id: TaskId,
    /// Current title.
    pub title: String,
    /// Current notes.
    pub notes: String,
    /// Current placement.
    pub placement: TaskPlacement,
    /// Optional ISO calendar date.
    pub planned_on: String,
    /// Optional RFC 3339 UTC instant.
    pub due_at: String,
    /// Timezone semantics retained for the due instant.
    pub due_time_zone: String,
    /// Optional RFC 3339 UTC instant.
    pub reminder_at: String,
    /// Timezone semantics retained for the reminder instant.
    pub reminder_time_zone: String,
    /// Current recurrence choice.
    pub recurrence: RecurrenceChoice,
    /// Custom-day interval, empty for other recurrence choices.
    pub custom_interval_days: String,
}

impl From<&Task> for TaskEditorState {
    fn from(task: &Task) -> Self {
        let record = task.record();
        let (recurrence, custom_interval_days) =
            match record.recurrence.map(RecurrenceRule::pattern) {
                None => (RecurrenceChoice::None, String::new()),
                Some(RecurrencePattern::Daily) => (RecurrenceChoice::Daily, String::new()),
                Some(RecurrencePattern::Weekly) => (RecurrenceChoice::Weekly, String::new()),
                Some(RecurrencePattern::Monthly) => (RecurrenceChoice::Monthly, String::new()),
                Some(RecurrencePattern::CustomDays { interval_days }) => {
                    (RecurrenceChoice::CustomDays, interval_days.to_string())
                }
            };
        Self {
            task_id: record.id,
            title: record.title.as_str().to_owned(),
            notes: record.notes.clone(),
            placement: record.placement,
            planned_on: record
                .planned_on
                .map(|date| date.to_string())
                .unwrap_or_default(),
            due_at: format_scheduled(record.due.as_ref()),
            due_time_zone: record
                .due
                .as_ref()
                .map_or_else(String::new, |value| value.time_zone.as_str().to_owned()),
            reminder_at: format_scheduled(record.reminder.as_ref()),
            reminder_time_zone: record
                .reminder
                .as_ref()
                .map_or_else(String::new, |value| value.time_zone.as_str().to_owned()),
            recurrence,
            custom_interval_days,
        }
    }
}

fn format_scheduled(value: Option<&ScheduledInstant>) -> String {
    value
        .and_then(|scheduled| {
            OffsetDateTime::from_unix_timestamp(scheduled.at_utc.unix_seconds()).ok()
        })
        .and_then(|instant| instant.format(&Rfc3339).ok())
        .unwrap_or_default()
}

/// Raw editor fields crossing from presentation into application validation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskEditorSubmission {
    /// Edited task identity.
    pub task_id: TaskId,
    /// Replacement title.
    pub title: String,
    /// Replacement notes.
    pub notes: String,
    /// Replacement placement.
    pub placement: TaskPlacement,
    /// Optional `YYYY-MM-DD` planned date.
    pub planned_on: String,
    /// Optional RFC 3339 due instant.
    pub due_at: String,
    /// Required timezone when `due_at` is set.
    pub due_time_zone: String,
    /// Optional RFC 3339 reminder instant.
    pub reminder_at: String,
    /// Required timezone when `reminder_at` is set.
    pub reminder_time_zone: String,
    /// Replacement recurrence choice.
    pub recurrence: RecurrenceChoice,
    /// Custom-day interval when the corresponding recurrence is selected.
    pub custom_interval_days: String,
}

impl TaskEditorSubmission {
    /// Parses presentation strings into validated domain update values.
    ///
    /// # Errors
    ///
    /// Returns a field-specific editor validation failure.
    pub fn into_update(self) -> Result<TaskDetailsUpdate, TaskEditorValidationError> {
        let title = TaskTitle::new(self.title).map_err(TaskEditorValidationError::Domain)?;
        let planned_on = parse_optional_date(&self.planned_on)?;
        let due = parse_scheduled("Due", &self.due_at, &self.due_time_zone)?;
        let reminder = parse_scheduled("Reminder", &self.reminder_at, &self.reminder_time_zone)?;
        let recurrence = match self.recurrence {
            RecurrenceChoice::None => None,
            RecurrenceChoice::Daily => Some(RecurrenceRule::new(RecurrencePattern::Daily)),
            RecurrenceChoice::Weekly => Some(RecurrenceRule::new(RecurrencePattern::Weekly)),
            RecurrenceChoice::Monthly => Some(RecurrenceRule::new(RecurrencePattern::Monthly)),
            RecurrenceChoice::CustomDays => {
                let days = self
                    .custom_interval_days
                    .trim()
                    .parse::<u16>()
                    .map_err(|_| TaskEditorValidationError::InvalidCustomInterval)?;
                Some(RecurrenceRule::new(RecurrencePattern::CustomDays {
                    interval_days: days,
                }))
            }
        }
        .transpose()
        .map_err(|_| TaskEditorValidationError::InvalidCustomInterval)?;
        if let (Some(reminder), Some(due)) = (&reminder, &due)
            && reminder.at_utc > due.at_utc
        {
            return Err(TaskEditorValidationError::ReminderAfterDue);
        }
        Ok(TaskDetailsUpdate {
            title,
            notes: self.notes,
            placement: self.placement,
            planned_on,
            due,
            reminder,
            recurrence,
        })
    }
}

fn parse_optional_date(value: &str) -> Result<Option<LocalDate>, TaskEditorValidationError> {
    let value = value.trim();
    if value.is_empty() {
        Ok(None)
    } else {
        LocalDate::parse_iso(value)
            .map(Some)
            .map_err(|_| TaskEditorValidationError::InvalidPlannedDate)
    }
}

fn parse_scheduled(
    label: &'static str,
    at: &str,
    time_zone: &str,
) -> Result<Option<ScheduledInstant>, TaskEditorValidationError> {
    let at = at.trim();
    let time_zone = time_zone.trim();
    if at.is_empty() && time_zone.is_empty() {
        return Ok(None);
    }
    if at.is_empty() || time_zone.is_empty() {
        return Err(TaskEditorValidationError::IncompleteSchedule(label));
    }
    let instant = OffsetDateTime::parse(at, &Rfc3339)
        .map_err(|_| TaskEditorValidationError::InvalidTimestamp(label))?;
    let time_zone = TimeZoneId::new(time_zone)
        .map_err(|_| TaskEditorValidationError::InvalidTimeZone(label))?;
    Ok(Some(ScheduledInstant {
        at_utc: UtcTimestamp::from_unix_seconds(instant.unix_timestamp()),
        time_zone,
    }))
}

/// Stable field-level task editor validation errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TaskEditorValidationError {
    /// Domain title or schedule invariant failed.
    Domain(TaskDomainError),
    /// Planned date was not a real ISO date.
    InvalidPlannedDate,
    /// One half of a timestamp/timezone pair was missing.
    IncompleteSchedule(&'static str),
    /// Timestamp was not valid RFC 3339.
    InvalidTimestamp(&'static str),
    /// Timezone identifier was structurally invalid.
    InvalidTimeZone(&'static str),
    /// Custom recurrence was not in the supported range.
    InvalidCustomInterval,
    /// Reminder occurred after due time.
    ReminderAfterDue,
}

/// Task editor field that should own a validation message.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskEditorField {
    /// Validation is not attributable to a single editable field.
    General,
    /// Task title.
    Title,
    /// Planned calendar date.
    PlannedDate,
    /// Due calendar date or local time.
    DueDateTime,
    /// Due IANA timezone identifier.
    DueTimeZone,
    /// Reminder calendar date or local time.
    ReminderDateTime,
    /// Reminder IANA timezone identifier.
    ReminderTimeZone,
    /// Recurrence choice or custom interval.
    Recurrence,
}

impl TaskEditorValidationError {
    /// Returns the field that should present this error.
    #[must_use]
    pub fn field(&self) -> TaskEditorField {
        match self {
            Self::Domain(TaskDomainError::EmptyTitle | TaskDomainError::TitleTooLong) => {
                TaskEditorField::Title
            }
            Self::Domain(TaskDomainError::ReminderAfterDue)
            | Self::ReminderAfterDue
            | Self::IncompleteSchedule("Reminder")
            | Self::InvalidTimestamp("Reminder") => TaskEditorField::ReminderDateTime,
            Self::Domain(_) => TaskEditorField::General,
            Self::InvalidPlannedDate => TaskEditorField::PlannedDate,
            Self::IncompleteSchedule("Due") | Self::InvalidTimestamp("Due") => {
                TaskEditorField::DueDateTime
            }
            Self::InvalidTimeZone("Due") => TaskEditorField::DueTimeZone,
            Self::InvalidTimeZone("Reminder") => TaskEditorField::ReminderTimeZone,
            Self::IncompleteSchedule(_) | Self::InvalidTimestamp(_) | Self::InvalidTimeZone(_) => {
                TaskEditorField::General
            }
            Self::InvalidCustomInterval => TaskEditorField::Recurrence,
        }
    }
}

impl fmt::Display for TaskEditorValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Domain(error) => error.fmt(formatter),
            Self::InvalidPlannedDate => formatter.write_str("Planned date must use YYYY-MM-DD."),
            Self::IncompleteSchedule(label) => {
                write!(
                    formatter,
                    "{label} time and timezone must both be filled or both be empty."
                )
            }
            Self::InvalidTimestamp(label) => {
                write!(
                    formatter,
                    "{label} time must be RFC 3339, including its UTC offset."
                )
            }
            Self::InvalidTimeZone(label) => {
                write!(formatter, "{label} timezone is invalid.")
            }
            Self::InvalidCustomInterval => {
                formatter.write_str("Custom recurrence must be between 1 and 365 days.")
            }
            Self::ReminderAfterDue => formatter.write_str("Reminder cannot be after due time."),
        }
    }
}

/// Top-level routes shared by the application and UI adapter.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum NavigationRoute {
    /// Four-quadrant task view.
    #[default]
    Quadrants,
    /// Today's execution view.
    Today,
    /// Focus timer view.
    Focus,
    /// Review and history summary.
    Review,
    /// Completed task history.
    Completed,
    /// Application settings.
    Settings,
    /// Product and license information.
    About,
}

impl NavigationRoute {
    /// Converts the stable Slint route index into the application route.
    #[must_use]
    pub const fn from_index(index: i32) -> Option<Self> {
        match index {
            0 => Some(Self::Quadrants),
            1 => Some(Self::Today),
            2 => Some(Self::Focus),
            3 => Some(Self::Review),
            4 => Some(Self::Completed),
            5 => Some(Self::Settings),
            6 => Some(Self::About),
            _ => None,
        }
    }

    /// Returns the stable index consumed by the Slint shell.
    #[must_use]
    pub const fn index(self) -> i32 {
        match self {
            Self::Quadrants => 0,
            Self::Today => 1,
            Self::Focus => 2,
            Self::Review => 3,
            Self::Completed => 4,
            Self::Settings => 5,
            Self::About => 6,
        }
    }
}

/// User-selected theme behavior.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ThemeMode {
    /// Follow the normalized platform theme source.
    #[default]
    System,
    /// Always render the light palette.
    Light,
    /// Always render the dark palette.
    Dark,
}

/// Validated desktop lifecycle preferences.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DesktopSettings {
    /// Register Quadrant to launch after user login where supported.
    pub launch_at_startup: bool,
    /// Keep the main window hidden when the process starts and tray is available.
    pub start_hidden: bool,
    /// Behavior when the main-window Close action is requested.
    pub close_behavior: WindowCloseBehavior,
    /// Behavior when the main-window Minimize action is requested.
    pub minimize_behavior: WindowMinimizeBehavior,
}

impl Default for DesktopSettings {
    fn default() -> Self {
        Self {
            launch_at_startup: false,
            start_hidden: false,
            close_behavior: WindowCloseBehavior::HideToTray,
            minimize_behavior: WindowMinimizeBehavior::Taskbar,
        }
    }
}

/// Main-window Close behavior.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum WindowCloseBehavior {
    /// End the application through its normal shutdown path.
    Quit,
    /// Hide the window and keep the tray application running.
    #[default]
    HideToTray,
}

/// Main-window Minimize behavior.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum WindowMinimizeBehavior {
    /// Minimize to the normal platform taskbar/dock representation.
    #[default]
    Taskbar,
    /// Hide the window and keep it recoverable from the tray.
    HideToTray,
}

/// Normalized platform theme reported to application/UI code.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SystemTheme {
    /// Light platform appearance.
    #[default]
    Light,
    /// Dark platform appearance.
    Dark,
}

/// Port used by the composition root to obtain the platform appearance.
pub trait SystemThemeSource {
    /// Returns the current normalized platform theme.
    fn current_theme(&self) -> SystemTheme;
}

/// A keyboard-first capture submitted by the M1 Quick Add shell.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuickAddSubmission {
    /// Trimmed task title entered by the user.
    pub title: String,
    /// Inbox or quadrant destination selected during capture.
    pub placement: TaskPlacement,
}

/// Lightweight task projection consumed by the Quadrants UI.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskSummary {
    /// Stable task identity.
    pub id: TaskId,
    /// User-visible title.
    pub title: String,
    /// Inbox/quadrant location.
    pub placement: TaskPlacement,
}

impl From<&Task> for TaskSummary {
    fn from(task: &Task) -> Self {
        let record = task.record();
        Self {
            id: record.id,
            title: record.title.as_str().to_owned(),
            placement: record.placement,
        }
    }
}

/// Active-task projection grouped for the four-quadrant screen.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct QuadrantsViewState {
    /// Unclassified captures.
    pub inbox: Vec<TaskSummary>,
    /// Important and urgent tasks.
    pub q1: Vec<TaskSummary>,
    /// Important and not urgent tasks.
    pub q2: Vec<TaskSummary>,
    /// Not important and urgent tasks.
    pub q3: Vec<TaskSummary>,
    /// Not important and not urgent tasks.
    pub q4: Vec<TaskSummary>,
}

impl QuadrantsViewState {
    /// Groups a repository-ordered active task list by placement.
    #[must_use]
    pub fn from_tasks(tasks: &[Task]) -> Self {
        let mut state = Self::default();
        for task in tasks {
            let summary = TaskSummary::from(task);
            match summary.placement {
                TaskPlacement::Inbox => state.inbox.push(summary),
                TaskPlacement::Quadrant(Quadrant::Q1) => state.q1.push(summary),
                TaskPlacement::Quadrant(Quadrant::Q2) => state.q2.push(summary),
                TaskPlacement::Quadrant(Quadrant::Q3) => state.q3.push(summary),
                TaskPlacement::Quadrant(Quadrant::Q4) => state.q4.push(summary),
            }
        }
        state
    }
}

/// Typed events sent from application work back to the UI adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ApplicationEvent {
    /// Replace the active Quadrants projection.
    QuadrantsChanged(QuadrantsViewState),
    /// Replace the derived Today projection.
    TodayChanged(TodayViewState),
    /// Replace the repository-backed Focus projection.
    FocusChanged(FocusViewState),
    /// Replace the Review dashboard projection.
    ReviewChanged(ReviewViewState),
    /// Replace the bounded Completed history projection.
    CompletedChanged(CompletedViewState),
    /// Replace the Settings data-maintenance projection.
    MaintenanceChanged(MaintenanceState),
    /// Surface an application reminder through the active presentation adapter.
    ReminderDue(ReminderAlert),
    /// Populate and open the dedicated task editor.
    TaskEditorLoaded(TaskEditorState),
    /// Close the task editor after a successful save.
    TaskEditorSaved,
    /// Keep the editor open and show a field-validation message.
    TaskEditorValidationFailed {
        /// Field that should present the message.
        field: TaskEditorField,
        /// Stable, user-facing validation message.
        message: String,
    },
    /// Apply the persisted desktop lifecycle policy to the UI adapter.
    DesktopSettingsChanged(DesktopSettings),
    /// Show stable positive feedback.
    OperationSucceeded(String),
    /// Show a stable user-facing failure without exposing raw diagnostics.
    OperationFailed(UserFacingError),
}

/// Desktop-shell events emitted by platform integrations or redirected launches.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopEvent {
    /// Restore and show the main application window.
    ShowMainWindow,
    /// Open the lightweight Quick Add surface without requiring main-window focus.
    OpenQuickAdd,
    /// End the UI event loop through the normal application shutdown path.
    ExitRequested,
}

/// Stable UI-safe error information.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserFacingError {
    /// Short message safe to render directly.
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::{
        NavigationRoute, QuadrantsViewState, RecurrenceChoice, RecurrencePattern, TaskEditorField,
        TaskEditorSubmission, TaskEditorValidationError, TaskId, TaskPlacement, ThemeMode,
    };
    use quadrant_domain::TaskDomainError;

    #[test]
    fn route_indices_round_trip() {
        for index in 0..=6 {
            let route = NavigationRoute::from_index(index).expect("known route index");
            assert_eq!(route.index(), index);
        }
        assert_eq!(NavigationRoute::from_index(7), None);
    }

    #[test]
    fn system_is_the_default_theme_mode() {
        assert_eq!(ThemeMode::default(), ThemeMode::System);
    }

    #[test]
    fn empty_quadrants_projection_is_well_formed() {
        assert_eq!(QuadrantsViewState::default().inbox.len(), 0);
    }

    #[test]
    fn task_editor_parses_dates_offsets_timezones_and_recurrence() {
        let update = TaskEditorSubmission {
            task_id: TaskId::generate(),
            title: "  Edited task  ".to_owned(),
            notes: "Details".to_owned(),
            placement: TaskPlacement::Inbox,
            planned_on: "2026-09-03".to_owned(),
            due_at: "2026-09-03T09:00:00+08:00".to_owned(),
            due_time_zone: "Asia/Shanghai".to_owned(),
            reminder_at: "2026-09-03T08:30:00+08:00".to_owned(),
            reminder_time_zone: "Asia/Shanghai".to_owned(),
            recurrence: RecurrenceChoice::CustomDays,
            custom_interval_days: "14".to_owned(),
        }
        .into_update()
        .expect("valid editor fields");

        assert_eq!(update.title.as_str(), "Edited task");
        assert_eq!(
            update.planned_on.expect("planned date").to_string(),
            "2026-09-03"
        );
        assert_eq!(
            update.due.expect("due time").time_zone.as_str(),
            "Asia/Shanghai"
        );
        assert_eq!(
            update.recurrence.expect("recurrence").pattern(),
            RecurrencePattern::CustomDays { interval_days: 14 }
        );
    }

    #[test]
    fn task_editor_rejects_incomplete_and_inverted_schedules() {
        let base = TaskEditorSubmission {
            task_id: TaskId::generate(),
            title: "Task".to_owned(),
            notes: String::new(),
            placement: TaskPlacement::Inbox,
            planned_on: String::new(),
            due_at: "2026-09-03T09:00:00+08:00".to_owned(),
            due_time_zone: "Asia/Shanghai".to_owned(),
            reminder_at: "2026-09-03T10:00:00+08:00".to_owned(),
            reminder_time_zone: "Asia/Shanghai".to_owned(),
            recurrence: RecurrenceChoice::None,
            custom_interval_days: String::new(),
        };
        assert!(base.clone().into_update().is_err());

        let mut incomplete = base;
        incomplete.reminder_at.clear();
        assert!(incomplete.into_update().is_err());
    }

    #[test]
    fn task_editor_validation_errors_identify_their_owning_fields() {
        assert_eq!(
            TaskEditorValidationError::Domain(TaskDomainError::EmptyTitle).field(),
            TaskEditorField::Title
        );
        assert_eq!(
            TaskEditorValidationError::InvalidTimeZone("Due").field(),
            TaskEditorField::DueTimeZone
        );
        assert_eq!(
            TaskEditorValidationError::ReminderAfterDue.field(),
            TaskEditorField::ReminderDateTime
        );
        assert_eq!(
            TaskEditorValidationError::InvalidCustomInterval.field(),
            TaskEditorField::Recurrence
        );
    }
}
