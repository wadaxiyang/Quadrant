//! Deterministic Today selection and presentation projection.

use std::{cmp::Ordering, collections::HashSet};

use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::{
    LocalDate, Quadrant, RecurrencePattern, RecurrenceRule, Task, TaskId, TaskPlacement,
    UtcTimestamp,
};

/// Local calendar date plus the precise UTC interval that represents it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TodayContext {
    /// User-visible local date.
    pub local_date: LocalDate,
    /// First representable instant of that local date.
    pub day_start_utc: UtcTimestamp,
    /// First representable instant of the following local date.
    pub next_day_start_utc: UtcTimestamp,
}

/// One task row rendered by the Today page.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TodayTaskSummary {
    /// Stable task identity.
    pub id: TaskId,
    /// User-visible title.
    pub title: String,
    /// Compact placement/schedule/recurrence context.
    pub metadata: String,
}

impl From<&Task> for TodayTaskSummary {
    fn from(task: &Task) -> Self {
        let record = task.record();
        let mut metadata = vec![placement_label(record.placement).to_owned()];
        if let Some(due) = record.due.as_ref()
            && let Ok(instant) = OffsetDateTime::from_unix_timestamp(due.at_utc.unix_seconds())
            && let Ok(formatted) = instant.format(&Rfc3339)
        {
            metadata.push(format!("Due {formatted}"));
        }
        if let Some(rule) = record.recurrence {
            metadata.push(recurrence_label(rule).to_owned());
        }
        Self {
            id: record.id,
            title: record.title.as_str().to_owned(),
            metadata: metadata.join(" · "),
        }
    }
}

/// Precedence-grouped, duplicate-free Today projection.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TodayViewState {
    /// Due before the current instant.
    pub overdue: Vec<TodayTaskSummary>,
    /// Explicitly planned for the local date.
    pub planned_today: Vec<TodayTaskSummary>,
    /// Due during the local date but not already assigned above.
    pub due_today: Vec<TodayTaskSummary>,
    /// Planned before the local date but not already assigned above.
    pub needs_reschedule: Vec<TodayTaskSummary>,
    /// Unique tasks across all sections.
    pub unique_task_count: usize,
}

impl TodayViewState {
    /// Applies centralized Today precedence and stable section ordering.
    #[must_use]
    pub fn from_tasks(tasks: &[Task], now: UtcTimestamp, context: TodayContext) -> Self {
        let mut assigned = HashSet::new();
        let overdue = assign(
            tasks,
            |task| {
                task.record()
                    .due
                    .as_ref()
                    .is_some_and(|due| due.at_utc < now)
            },
            &mut assigned,
        );
        let planned_today = assign(
            tasks,
            |task| task.record().planned_on == Some(context.local_date),
            &mut assigned,
        );
        let due_today = assign(
            tasks,
            |task| {
                task.record().due.as_ref().is_some_and(|due| {
                    due.at_utc >= context.day_start_utc && due.at_utc < context.next_day_start_utc
                })
            },
            &mut assigned,
        );
        let needs_reschedule = assign(
            tasks,
            |task| {
                task.record()
                    .planned_on
                    .is_some_and(|date| date < context.local_date)
            },
            &mut assigned,
        );
        let unique_task_count = assigned.len();
        Self {
            overdue,
            planned_today,
            due_today,
            needs_reschedule,
            unique_task_count,
        }
    }
}

fn assign(
    tasks: &[Task],
    predicate: impl Fn(&Task) -> bool,
    assigned: &mut HashSet<TaskId>,
) -> Vec<TodayTaskSummary> {
    let mut selected = tasks
        .iter()
        .filter(|task| predicate(task) && assigned.insert(task.record().id))
        .collect::<Vec<_>>();
    selected.sort_by(|left, right| compare_tasks(left, right));
    selected.into_iter().map(TodayTaskSummary::from).collect()
}

fn compare_tasks(left: &Task, right: &Task) -> Ordering {
    let left = left.record();
    let right = right.record();
    let left_due = left.due.as_ref().map(|due| due.at_utc);
    let right_due = right.due.as_ref().map(|due| due.at_utc);
    (
        left_due.is_none(),
        left_due,
        placement_rank(left.placement),
        left.created_at,
        left.id,
    )
        .cmp(&(
            right_due.is_none(),
            right_due,
            placement_rank(right.placement),
            right.created_at,
            right.id,
        ))
}

const fn placement_rank(placement: TaskPlacement) -> u8 {
    match placement {
        TaskPlacement::Quadrant(Quadrant::Q1) => 1,
        TaskPlacement::Quadrant(Quadrant::Q2) => 2,
        TaskPlacement::Quadrant(Quadrant::Q3) => 3,
        TaskPlacement::Quadrant(Quadrant::Q4) => 4,
        TaskPlacement::Inbox => 5,
    }
}

const fn placement_label(placement: TaskPlacement) -> &'static str {
    match placement {
        TaskPlacement::Inbox => "Inbox",
        TaskPlacement::Quadrant(Quadrant::Q1) => "Q1",
        TaskPlacement::Quadrant(Quadrant::Q2) => "Q2",
        TaskPlacement::Quadrant(Quadrant::Q3) => "Q3",
        TaskPlacement::Quadrant(Quadrant::Q4) => "Q4",
    }
}

const fn recurrence_label(rule: RecurrenceRule) -> &'static str {
    match rule.pattern() {
        RecurrencePattern::Daily => "Daily",
        RecurrencePattern::Weekly => "Weekly",
        RecurrencePattern::Monthly => "Monthly",
        RecurrencePattern::CustomDays { .. } => "Custom recurrence",
    }
}

#[cfg(test)]
mod tests {
    use super::{TodayContext, TodayViewState};
    use crate::{
        LocalDate, NewTask, Quadrant, ScheduledInstant, SortKey, Task, TaskId, TaskPlacement,
        TaskTitle, TimeZoneId, UtcTimestamp,
    };

    fn task(
        title: &str,
        placement: TaskPlacement,
        planned_on: Option<&str>,
        due_at: Option<i64>,
        created_at: i64,
    ) -> Task {
        Task::create(
            TaskId::generate(),
            NewTask {
                title: TaskTitle::new(title).expect("valid title"),
                notes: String::new(),
                placement,
                planned_on: planned_on.map(|date| LocalDate::parse_iso(date).expect("valid date")),
                due: due_at.map(|seconds| ScheduledInstant {
                    at_utc: UtcTimestamp::from_unix_seconds(seconds),
                    time_zone: TimeZoneId::new("Asia/Shanghai").expect("valid timezone"),
                }),
                reminder: None,
                recurrence: None,
            },
            SortKey::INITIAL,
            UtcTimestamp::from_unix_seconds(created_at),
        )
        .expect("valid task")
    }

    #[test]
    fn today_precedence_assigns_each_task_once() {
        let context = TodayContext {
            local_date: LocalDate::parse_iso("2026-09-02").expect("valid date"),
            day_start_utc: UtcTimestamp::from_unix_seconds(100),
            next_day_start_utc: UtcTimestamp::from_unix_seconds(200),
        };
        let tasks = vec![
            task(
                "overdue and planned",
                TaskPlacement::Inbox,
                Some("2026-09-02"),
                Some(140),
                1,
            ),
            task(
                "planned",
                TaskPlacement::Quadrant(Quadrant::Q2),
                Some("2026-09-02"),
                None,
                2,
            ),
            task(
                "due",
                TaskPlacement::Quadrant(Quadrant::Q1),
                None,
                Some(180),
                3,
            ),
            task(
                "reschedule",
                TaskPlacement::Inbox,
                Some("2026-09-01"),
                None,
                4,
            ),
            task(
                "future",
                TaskPlacement::Inbox,
                Some("2026-09-03"),
                Some(220),
                5,
            ),
        ];

        let state =
            TodayViewState::from_tasks(&tasks, UtcTimestamp::from_unix_seconds(150), context);
        assert_eq!(state.overdue[0].title, "overdue and planned");
        assert_eq!(state.planned_today[0].title, "planned");
        assert_eq!(state.due_today[0].title, "due");
        assert_eq!(state.needs_reschedule[0].title, "reschedule");
        assert_eq!(state.unique_task_count, 4);
    }

    #[test]
    fn due_equal_to_now_is_due_today_not_overdue() {
        let context = TodayContext {
            local_date: LocalDate::parse_iso("2026-09-02").expect("valid date"),
            day_start_utc: UtcTimestamp::from_unix_seconds(100),
            next_day_start_utc: UtcTimestamp::from_unix_seconds(200),
        };
        let tasks = vec![task(
            "due now",
            TaskPlacement::Quadrant(Quadrant::Q1),
            None,
            Some(150),
            1,
        )];
        let state =
            TodayViewState::from_tasks(&tasks, UtcTimestamp::from_unix_seconds(150), context);
        assert!(state.overdue.is_empty());
        assert_eq!(state.due_today[0].title, "due now");
    }
}
