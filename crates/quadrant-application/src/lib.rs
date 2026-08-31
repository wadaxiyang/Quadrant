//! Application use cases, ports, and typed events.

#![forbid(unsafe_code)]

pub use quadrant_domain::{Quadrant, TaskPlacement};

/// A typed intent emitted by the presentation layer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UiIntent {
    /// Navigate to a top-level application route.
    Navigate(NavigationRoute),
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
