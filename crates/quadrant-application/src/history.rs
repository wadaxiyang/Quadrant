//! Review aggregation and bounded Completed-history application projections.

use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU32, Ordering},
    },
};

use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::{
    ApplicationEvent, CalendarError, Clock, CompletedRepository, LocalDate, Quadrant,
    RepositoryError, ReviewRepository, Task, TaskId, TaskPlacement, TodayContextSource, UiIntent,
    UserFacingError, UtcTimestamp,
};

const INITIAL_COMPLETED_LIMIT: u32 = 50;
const COMPLETED_PAGE_SIZE: u32 = 50;
const MAX_COMPLETED_LIMIT: u32 = 500;

/// User-selectable Review period.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ReviewRange {
    /// Today plus the preceding six local dates.
    #[default]
    SevenDays,
    /// Today plus the preceding 29 local dates.
    ThirtyDays,
    /// Today plus the preceding 89 local dates.
    NinetyDays,
    /// Every retained event before tomorrow.
    AllTime,
}

impl ReviewRange {
    /// Converts the stable Slint index into a typed range.
    #[must_use]
    pub const fn from_index(index: i32) -> Option<Self> {
        match index {
            0 => Some(Self::SevenDays),
            1 => Some(Self::ThirtyDays),
            2 => Some(Self::NinetyDays),
            3 => Some(Self::AllTime),
            _ => None,
        }
    }

    /// Returns the stable index consumed by Slint.
    #[must_use]
    pub const fn index(self) -> i32 {
        match self {
            Self::SevenDays => 0,
            Self::ThirtyDays => 1,
            Self::NinetyDays => 2,
            Self::AllTime => 3,
        }
    }

    fn day_count(self) -> Option<i64> {
        match self {
            Self::SevenDays => Some(7),
            Self::ThirtyDays => Some(30),
            Self::NinetyDays => Some(90),
            Self::AllTime => None,
        }
    }
}

/// Half-open host-local date interval used by Review queries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReviewDateRange {
    /// Optional lower bound; absent for all retained history.
    pub lower_inclusive: Option<LocalDate>,
    /// Exclusive upper bound, normally tomorrow.
    pub upper_exclusive: LocalDate,
}

/// Aggregate counts for one Review range.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ReviewTotals {
    /// Active (not reverted) completion events.
    pub completed_tasks: u64,
    /// Completed productive Focus sessions.
    pub focus_sessions: u64,
    /// Sum of productive Focus duration.
    pub focus_seconds: u64,
}

impl ReviewTotals {
    /// Returns integer average productive Focus duration.
    #[must_use]
    pub const fn average_focus_seconds(self) -> u64 {
        if self.focus_sessions == 0 {
            0
        } else {
            self.focus_seconds / self.focus_sessions
        }
    }
}

/// One date's unbucketed completion and Focus values.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ReviewActivityPoint {
    /// Local date represented by the point/bucket start.
    pub date: LocalDate,
    /// Completed task count.
    pub completed: u64,
    /// Productive Focus seconds.
    pub focus_seconds: u64,
}

/// Completion and Focus values for a quadrant or unclassified/unlinked work.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ReviewQuadrantValue {
    /// `None` means Inbox for completions and unlinked for Focus.
    pub quadrant: Option<Quadrant>,
    /// Completion-event count.
    pub completed: u64,
    /// Productive Focus seconds.
    pub focus_seconds: u64,
}

/// Rich Focus highlights for the active Review range.
#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ReviewFocusHighlights {
    /// Longest productive session.
    pub longest_session_seconds: u64,
    /// Task snapshot with the most productive Focus, if any.
    pub most_focused_task_title: Option<String>,
    /// Focus duration for the leading task.
    pub most_focused_task_seconds: u64,
    /// Session count for the leading task.
    pub most_focused_task_sessions: u64,
    /// Classified quadrant with the most Focus, if any.
    pub most_focused_quadrant: Option<Quadrant>,
    /// Focus duration for the leading quadrant.
    pub most_focused_quadrant_seconds: u64,
}

/// One active completion event shown in Review's recent list.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ReviewRecentCompletion {
    /// Immutable snapshot title.
    pub title: String,
    /// UTC completion instant.
    pub completed_at: UtcTimestamp,
    /// Host-local completion date.
    pub completed_local_date: LocalDate,
    /// Immutable placement snapshot.
    pub quadrant: Option<Quadrant>,
    /// Whether its due instant was already before completion.
    pub was_overdue: bool,
}

/// Complete read request handled by the Review query adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReviewQuery {
    /// Active period.
    pub current: ReviewDateRange,
    /// Equal-length preceding period, absent for All Time.
    pub previous: Option<ReviewDateRange>,
    /// Current instant for active overdue count.
    pub now: UtcTimestamp,
    /// Bounded recent completion count.
    pub recent_limit: u32,
}

/// Storage-level Review aggregates with no Slint formatting.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewQueryData {
    /// Current period totals.
    pub current: ReviewTotals,
    /// Previous equal-length totals.
    pub previous: Option<ReviewTotals>,
    /// Sparse per-day values in the current period.
    pub daily_activity: Vec<ReviewActivityPoint>,
    /// Q1–Q4 plus unclassified/unlinked values.
    pub quadrants: Vec<ReviewQuadrantValue>,
    /// Current period Focus highlights.
    pub focus: ReviewFocusHighlights,
    /// Recent active completion events across all retained history.
    pub recent_completed: Vec<ReviewRecentCompletion>,
    /// Current active Inbox size.
    pub current_inbox_count: u64,
    /// Current active overdue size.
    pub current_overdue_count: u64,
}

/// Repository-backed Review dashboard projection.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ReviewViewState {
    /// Selected period.
    pub range: ReviewRange,
    /// Current totals.
    pub current: ReviewTotals,
    /// Previous equal-length totals.
    pub previous: Option<ReviewTotals>,
    /// Presentation-sized daily/weekly/monthly buckets.
    pub activity: Vec<ReviewActivityPoint>,
    /// Largest completion bucket, at least one.
    pub completed_activity_max: u64,
    /// Largest Focus bucket, at least one.
    pub focus_activity_max: u64,
    /// Quadrant breakdown.
    pub quadrants: Vec<ReviewQuadrantValue>,
    /// Focus highlights.
    pub focus: ReviewFocusHighlights,
    /// Recent completion snapshots.
    pub recent_completed: Vec<ReviewRecentCompletion>,
    /// Current active Inbox size.
    pub current_inbox_count: u64,
    /// Current active overdue size.
    pub current_overdue_count: u64,
}

/// One completed task rendered by the bounded history page.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CompletedTaskSummary {
    /// Task identity used by restore/delete commands.
    pub id: TaskId,
    /// Current persisted task title.
    pub title: String,
    /// Placement and completion-time context.
    pub metadata: String,
}

/// Bounded Completed history projection.
#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CompletedViewState {
    /// Newest-first completed tasks currently loaded.
    pub tasks: Vec<CompletedTaskSummary>,
    /// Whether another bounded page can be requested.
    pub has_more: bool,
}

/// Query-only Review and Completed application service.
#[derive(Clone)]
pub struct HistoryApplication {
    review: Arc<dyn ReviewRepository>,
    completed: Arc<dyn CompletedRepository>,
    clock: Arc<dyn Clock>,
    today_context: Arc<dyn TodayContextSource>,
    selected_range: Arc<Mutex<ReviewRange>>,
    completed_limit: Arc<AtomicU32>,
}

impl HistoryApplication {
    /// Assembles history queries from application-owned ports.
    #[must_use]
    pub fn new(
        review: Arc<dyn ReviewRepository>,
        completed: Arc<dyn CompletedRepository>,
        clock: Arc<dyn Clock>,
        today_context: Arc<dyn TodayContextSource>,
    ) -> Self {
        Self {
            review,
            completed,
            clock,
            today_context,
            selected_range: Arc::new(Mutex::new(ReviewRange::default())),
            completed_limit: Arc::new(AtomicU32::new(INITIAL_COMPLETED_LIMIT)),
        }
    }

    /// Loads the selected Review dashboard.
    ///
    /// # Errors
    ///
    /// Returns repository, calendar, or range failures.
    pub fn load_review(&self) -> Result<ReviewViewState, HistoryLoadError> {
        let range = *self
            .selected_range
            .lock()
            .map_err(|_| HistoryLoadError::StateLock)?;
        self.load_review_range(range)
    }

    /// Loads the current bounded Completed history.
    ///
    /// # Errors
    ///
    /// Returns repository failures.
    pub fn load_completed(&self) -> Result<CompletedViewState, RepositoryError> {
        let limit = self.completed_limit.load(Ordering::SeqCst);
        let mut tasks = self
            .completed
            .list_completed_tasks(limit.saturating_add(1))?;
        let has_more = limit < MAX_COMPLETED_LIMIT
            && tasks.len() > usize::try_from(limit).unwrap_or(usize::MAX);
        if has_more {
            tasks.pop();
        }
        Ok(CompletedViewState {
            tasks: tasks.iter().map(completed_summary).collect(),
            has_more,
        })
    }

    /// Handles navigation/range/pagination history intents.
    #[must_use]
    pub fn handle(&self, intent: &UiIntent) -> Vec<ApplicationEvent> {
        match intent {
            UiIntent::Navigate(crate::NavigationRoute::Review) => self.review_events(),
            UiIntent::Navigate(crate::NavigationRoute::Completed) => {
                self.completed_limit
                    .store(INITIAL_COMPLETED_LIMIT, Ordering::SeqCst);
                self.completed_events()
            }
            UiIntent::SetReviewRange(range) => {
                let Ok(mut selected) = self.selected_range.lock() else {
                    return vec![history_failure("Review state could not be changed.")];
                };
                *selected = *range;
                drop(selected);
                self.review_events()
            }
            UiIntent::LoadMoreCompleted => {
                let _ = self.completed_limit.fetch_update(
                    Ordering::SeqCst,
                    Ordering::SeqCst,
                    |value| {
                        Some(
                            value
                                .saturating_add(COMPLETED_PAGE_SIZE)
                                .min(MAX_COMPLETED_LIMIT),
                        )
                    },
                );
                self.completed_events()
            }
            _ => Vec::new(),
        }
    }

    /// Refreshes both projections after a task/focus history mutation.
    #[must_use]
    pub fn refresh_after_mutation(&self) -> Vec<ApplicationEvent> {
        let mut events = self.review_events();
        events.extend(self.completed_events());
        events
    }

    fn load_review_range(&self, range: ReviewRange) -> Result<ReviewViewState, HistoryLoadError> {
        let now = self.clock.now();
        let today = self.today_context.today_context(now)?.local_date;
        let upper = today
            .checked_add_days(1)
            .ok_or(HistoryLoadError::DateRange)?;
        let (current, previous) = date_ranges(range, upper)?;
        let data = self.review.load_review(ReviewQuery {
            current,
            previous,
            now,
            recent_limit: 12,
        })?;
        let activity = bucket_activity(range, current, &data.daily_activity)?;
        let completed_activity_max = activity
            .iter()
            .map(|point| point.completed)
            .max()
            .unwrap_or(0)
            .max(1);
        let focus_activity_max = activity
            .iter()
            .map(|point| point.focus_seconds)
            .max()
            .unwrap_or(0)
            .max(1);
        Ok(ReviewViewState {
            range,
            current: data.current,
            previous: data.previous,
            activity,
            completed_activity_max,
            focus_activity_max,
            quadrants: data.quadrants,
            focus: data.focus,
            recent_completed: data.recent_completed,
            current_inbox_count: data.current_inbox_count,
            current_overdue_count: data.current_overdue_count,
        })
    }

    fn review_events(&self) -> Vec<ApplicationEvent> {
        match self.load_review() {
            Ok(state) => vec![ApplicationEvent::ReviewChanged(state)],
            Err(_) => vec![history_failure("Review history could not be loaded.")],
        }
    }

    fn completed_events(&self) -> Vec<ApplicationEvent> {
        match self.load_completed() {
            Ok(state) => vec![ApplicationEvent::CompletedChanged(state)],
            Err(_) => vec![history_failure("Completed tasks could not be loaded.")],
        }
    }
}

/// Review range/projection load failure.
#[derive(Debug, thiserror::Error)]
pub enum HistoryLoadError {
    /// Repository query failed.
    #[error(transparent)]
    Repository(#[from] RepositoryError),
    /// Platform local calendar failed.
    #[error(transparent)]
    Calendar(#[from] CalendarError),
    /// Date arithmetic exceeded supported calendar bounds.
    #[error("Review date range is outside supported bounds")]
    DateRange,
    /// Internal selected-range state was poisoned.
    #[error("Review selection state is unavailable")]
    StateLock,
}

fn date_ranges(
    range: ReviewRange,
    upper: LocalDate,
) -> Result<(ReviewDateRange, Option<ReviewDateRange>), HistoryLoadError> {
    let Some(days) = range.day_count() else {
        return Ok((
            ReviewDateRange {
                lower_inclusive: None,
                upper_exclusive: upper,
            },
            None,
        ));
    };
    let lower = upper
        .checked_add_days(-days)
        .ok_or(HistoryLoadError::DateRange)?;
    let previous_lower = lower
        .checked_add_days(-days)
        .ok_or(HistoryLoadError::DateRange)?;
    Ok((
        ReviewDateRange {
            lower_inclusive: Some(lower),
            upper_exclusive: upper,
        },
        Some(ReviewDateRange {
            lower_inclusive: Some(previous_lower),
            upper_exclusive: lower,
        }),
    ))
}

fn bucket_activity(
    range: ReviewRange,
    dates: ReviewDateRange,
    sparse: &[ReviewActivityPoint],
) -> Result<Vec<ReviewActivityPoint>, HistoryLoadError> {
    let values = sparse
        .iter()
        .map(|point| (point.date, (point.completed, point.focus_seconds)))
        .collect::<BTreeMap<_, _>>();
    if range == ReviewRange::AllTime {
        let mut months = BTreeMap::<(i32, u8), ReviewActivityPoint>::new();
        for point in sparse {
            let key = (point.date.year(), point.date.month());
            let entry = months.entry(key).or_insert(ReviewActivityPoint {
                date: LocalDate::from_calendar_date(key.0, key.1, 1)
                    .map_err(|_| HistoryLoadError::DateRange)?,
                completed: 0,
                focus_seconds: 0,
            });
            entry.completed = entry.completed.saturating_add(point.completed);
            entry.focus_seconds = entry.focus_seconds.saturating_add(point.focus_seconds);
        }
        return Ok(months.into_values().collect());
    }
    let Some(mut date) = dates.lower_inclusive else {
        return Err(HistoryLoadError::DateRange);
    };
    let mut daily = Vec::new();
    while date < dates.upper_exclusive {
        let (completed, focus_seconds) = values.get(&date).copied().unwrap_or_default();
        daily.push(ReviewActivityPoint {
            date,
            completed,
            focus_seconds,
        });
        date = date
            .checked_add_days(1)
            .ok_or(HistoryLoadError::DateRange)?;
    }
    if range != ReviewRange::NinetyDays {
        return Ok(daily);
    }
    Ok(daily
        .chunks(7)
        .map(|chunk| ReviewActivityPoint {
            date: chunk[0].date,
            completed: chunk.iter().map(|point| point.completed).sum(),
            focus_seconds: chunk.iter().map(|point| point.focus_seconds).sum(),
        })
        .collect())
}

fn completed_summary(task: &Task) -> CompletedTaskSummary {
    let record = task.record();
    let placement = placement_label(record.placement);
    let completed = record
        .completed_at
        .and_then(|value| OffsetDateTime::from_unix_timestamp(value.unix_seconds()).ok())
        .and_then(|value| value.format(&Rfc3339).ok())
        .unwrap_or_else(|| "Unknown completion time".to_owned());
    CompletedTaskSummary {
        id: record.id,
        title: record.title.as_str().to_owned(),
        metadata: format!("{placement} · Completed {completed}"),
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

fn history_failure(message: &str) -> ApplicationEvent {
    ApplicationEvent::OperationFailed(UserFacingError {
        message: message.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::{ReviewActivityPoint, ReviewDateRange, ReviewRange, bucket_activity, date_ranges};
    use crate::LocalDate;

    #[test]
    fn finite_ranges_include_today_and_have_equal_previous_periods() {
        let upper = LocalDate::parse_iso("2026-09-02").expect("valid date");
        let (current, previous) =
            date_ranges(ReviewRange::SevenDays, upper).expect("range computes");
        assert_eq!(
            current.lower_inclusive.expect("lower").to_string(),
            "2026-08-26"
        );
        let previous = previous.expect("previous range");
        assert_eq!(
            previous.lower_inclusive.expect("lower").to_string(),
            "2026-08-19"
        );
        assert_eq!(previous.upper_exclusive.to_string(), "2026-08-26");
    }

    #[test]
    fn activity_fills_zero_days_and_buckets_ninety_days_weekly() {
        let dates = ReviewDateRange {
            lower_inclusive: Some(LocalDate::parse_iso("2026-01-01").expect("valid date")),
            upper_exclusive: LocalDate::parse_iso("2026-04-01").expect("valid date"),
        };
        let values = [ReviewActivityPoint {
            date: LocalDate::parse_iso("2026-01-02").expect("valid date"),
            completed: 2,
            focus_seconds: 60,
        }];
        let buckets =
            bucket_activity(ReviewRange::NinetyDays, dates, &values).expect("activity buckets");
        assert_eq!(buckets.len(), 13);
        assert_eq!(buckets[0].completed, 2);
        assert_eq!(
            buckets.iter().map(|point| point.focus_seconds).sum::<u64>(),
            60
        );
    }
}
