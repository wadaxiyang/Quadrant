//! Application use cases, ports, and typed events.

#![forbid(unsafe_code)]

pub use quadrant_domain::{Quadrant, TaskPlacement};

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

#[cfg(test)]
mod tests {
    use super::{NavigationRoute, ThemeMode};

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
}
