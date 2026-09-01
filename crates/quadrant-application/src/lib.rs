//! Application use cases, ports, projections, and typed events.

#![forbid(unsafe_code)]

mod ports;
mod tasks;

pub use ports::{
    Clock, RepositoryError, RepositoryOperation, SettingsRepository, SystemClock, TaskIdGenerator,
    TaskRepository, UuidTaskIdGenerator,
};
pub use quadrant_domain::{
    LocalDate, NewTask, Quadrant, RecurrencePattern, RecurrenceRule, ScheduledInstant, SortKey,
    Task, TaskDetailsUpdate, TaskDomainError, TaskId, TaskPlacement, TaskStatus, TaskTitle,
    TimeZoneId, UtcTimestamp,
};
pub use tasks::TaskApplication;

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
    /// Move an active task into Inbox or a quadrant.
    MoveTask {
        /// Task to move.
        task_id: TaskId,
        /// Destination placement.
        placement: TaskPlacement,
    },
    /// Complete an active task.
    CompleteTask(TaskId),
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
    /// Show stable positive feedback.
    OperationSucceeded(String),
    /// Show a stable user-facing failure without exposing raw diagnostics.
    OperationFailed(UserFacingError),
}

/// Stable UI-safe error information.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserFacingError {
    /// Short message safe to render directly.
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::{NavigationRoute, QuadrantsViewState, ThemeMode};

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
}
