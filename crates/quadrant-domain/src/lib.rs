//! Pure domain model and rules for Quadrant.

#![forbid(unsafe_code)]

/// Identifies the four architectural quadrants without UI or storage coupling.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Quadrant {
    /// Important and urgent.
    Q1,
    /// Important and not urgent.
    Q2,
    /// Not important and urgent.
    Q3,
    /// Not important and not urgent.
    Q4,
}

/// A task's placement in the capture and classification workflow.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TaskPlacement {
    /// The task is captured but not classified.
    #[default]
    Inbox,
    /// The task has been classified into a quadrant.
    Quadrant(Quadrant),
}

#[cfg(test)]
mod tests {
    use super::{Quadrant, TaskPlacement};

    #[test]
    fn tasks_start_in_inbox_by_default() {
        assert_eq!(TaskPlacement::default(), TaskPlacement::Inbox);
    }

    #[test]
    fn every_quadrant_is_representable() {
        let placements =
            [Quadrant::Q1, Quadrant::Q2, Quadrant::Q3, Quadrant::Q4].map(TaskPlacement::Quadrant);

        assert_eq!(placements.len(), 4);
    }
}
