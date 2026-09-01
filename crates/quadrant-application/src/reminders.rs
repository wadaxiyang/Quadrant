//! Event-driven nearest-deadline reminder scheduling.

use std::{fmt, sync::Arc, time::Duration};

use tokio::sync::mpsc;

use crate::{Clock, ReminderRepository, RepositoryError, Task, TaskId, UtcTimestamp};

/// UI/platform-neutral reminder content decided by the application layer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReminderAlert {
    /// Task that owns the reminder.
    pub task_id: TaskId,
    /// Snapshot title shown by the delivery adapter.
    pub title: String,
    /// Deadline that triggered this delivery.
    pub scheduled_for: UtcTimestamp,
}

impl ReminderAlert {
    fn from_task(task: &Task) -> Option<Self> {
        let record = task.record();
        record.reminder.as_ref().map(|reminder| Self {
            task_id: record.id,
            title: record.title.as_str().to_owned(),
            scheduled_for: reminder.at_utc,
        })
    }
}

/// One recomputation result: due alerts plus the next future deadline.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ReminderPlan {
    /// Reminders due at or before `now`, sorted by deadline and task ID.
    pub due: Vec<ReminderAlert>,
    /// Earliest future deadline, if any.
    pub next_deadline: Option<UtcTimestamp>,
}

impl ReminderPlan {
    /// Computes a deterministic schedule from repository-ordered active tasks.
    #[must_use]
    pub fn from_tasks(tasks: &[Task], now: UtcTimestamp) -> Self {
        let mut alerts = tasks
            .iter()
            .filter_map(ReminderAlert::from_task)
            .collect::<Vec<_>>();
        alerts.sort_by_key(|alert| (alert.scheduled_for, alert.task_id));
        let split = alerts.partition_point(|alert| alert.scheduled_for <= now);
        let future = alerts.split_off(split);
        Self {
            due: alerts,
            next_deadline: future.first().map(|alert| alert.scheduled_for),
        }
    }
}

/// Delivery adapter failure kept free of OS-specific error types.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReminderDeliveryError {
    detail: String,
}

impl ReminderDeliveryError {
    /// Wraps delivery diagnostic context.
    #[must_use]
    pub fn new(detail: impl fmt::Display) -> Self {
        Self {
            detail: detail.to_string(),
        }
    }
}

impl fmt::Display for ReminderDeliveryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.detail)
    }
}

impl std::error::Error for ReminderDeliveryError {}

/// Application port for in-app or native reminder presentation.
pub trait ReminderDelivery: Send + Sync {
    /// Delivers one application-decided reminder.
    ///
    /// # Errors
    ///
    /// Returns a normalized delivery error. Failed deliveries are not consumed.
    fn deliver(&self, alert: ReminderAlert) -> Result<(), ReminderDeliveryError>;
}

impl<F> ReminderDelivery for F
where
    F: Fn(ReminderAlert) -> Result<(), ReminderDeliveryError> + Send + Sync,
{
    fn deliver(&self, alert: ReminderAlert) -> Result<(), ReminderDeliveryError> {
        self(alert)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReminderSignal {
    Changed,
    Shutdown,
}

/// Cloneable signal handle used by task mutations and shutdown coordination.
#[derive(Clone, Debug)]
pub struct ReminderSchedulerHandle {
    sender: mpsc::UnboundedSender<ReminderSignal>,
}

impl ReminderSchedulerHandle {
    /// Wakes the scheduler to recompute after a reminder-affecting mutation.
    pub fn schedule_changed(&self) {
        let _ = self.sender.send(ReminderSignal::Changed);
    }

    /// Requests orderly scheduler shutdown.
    pub fn shutdown(&self) {
        let _ = self.sender.send(ReminderSignal::Shutdown);
    }
}

/// Long-lived application service that waits for one nearest deadline or a mutation signal.
pub struct ReminderScheduler {
    reminders: Arc<dyn ReminderRepository>,
    clock: Arc<dyn Clock>,
    delivery: Arc<dyn ReminderDelivery>,
    signals: mpsc::UnboundedReceiver<ReminderSignal>,
}

impl ReminderScheduler {
    /// Creates the scheduler and its non-blocking mutation signal handle.
    #[must_use]
    pub fn new(
        reminders: Arc<dyn ReminderRepository>,
        clock: Arc<dyn Clock>,
        delivery: Arc<dyn ReminderDelivery>,
    ) -> (Self, ReminderSchedulerHandle) {
        let (sender, signals) = mpsc::unbounded_channel();
        (
            Self {
                reminders,
                clock,
                delivery,
                signals,
            },
            ReminderSchedulerHandle { sender },
        )
    }

    /// Runs until shutdown or all signal senders are dropped.
    pub async fn run(mut self) {
        loop {
            let Ok(tasks) = self.load_pending().await else {
                if !self.wait_for_change().await {
                    break;
                }
                continue;
            };
            let now = self.clock.now();
            let plan = ReminderPlan::from_tasks(&tasks, now);
            let mut delivery_failed = false;
            for alert in plan.due {
                if self.delivery.deliver(alert.clone()).is_err() {
                    delivery_failed = true;
                    continue;
                }
                if self.clear_delivered(alert).await.is_err() {
                    delivery_failed = true;
                }
            }
            if delivery_failed {
                if !self.wait_for_change().await {
                    break;
                }
                continue;
            }
            if let Some(deadline) = plan.next_deadline {
                let seconds = deadline
                    .unix_seconds()
                    .saturating_sub(self.clock.now().unix_seconds());
                let wait = Duration::from_secs(u64::try_from(seconds.max(0)).unwrap_or(0));
                tokio::select! {
                    () = tokio::time::sleep(wait) => {}
                    signal = self.signals.recv() => {
                        if !matches!(signal, Some(ReminderSignal::Changed)) {
                            break;
                        }
                    }
                }
            } else if !self.wait_for_change().await {
                break;
            }
        }
    }

    async fn load_pending(&self) -> Result<Vec<Task>, RepositoryError> {
        let reminders = Arc::clone(&self.reminders);
        tokio::task::spawn_blocking(move || reminders.list_pending_reminders())
            .await
            .map_err(|error| {
                RepositoryError::new(crate::RepositoryOperation::ReadReminders, error)
            })?
    }

    async fn clear_delivered(&self, alert: ReminderAlert) -> Result<(), RepositoryError> {
        let reminders = Arc::clone(&self.reminders);
        let now = self.clock.now();
        tokio::task::spawn_blocking(move || {
            reminders
                .clear_reminder_if_matches(alert.task_id, alert.scheduled_for, now)
                .map(|_| ())
        })
        .await
        .map_err(|error| RepositoryError::new(crate::RepositoryOperation::UpdateReminder, error))?
    }

    async fn wait_for_change(&mut self) -> bool {
        matches!(self.signals.recv().await, Some(ReminderSignal::Changed))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use tokio::sync::mpsc;

    use super::{ReminderPlan, ReminderScheduler};
    use crate::{
        Clock, NewTask, ReminderRepository, RepositoryError, ScheduledInstant, SortKey, Task,
        TaskId, TaskPlacement, TimeZoneId, UtcTimestamp,
    };

    #[derive(Debug)]
    struct FixedClock(UtcTimestamp);

    impl Clock for FixedClock {
        fn now(&self) -> UtcTimestamp {
            self.0
        }
    }

    #[derive(Debug, Default)]
    struct FakeReminderRepository {
        tasks: Mutex<Vec<Task>>,
    }

    impl ReminderRepository for FakeReminderRepository {
        fn list_pending_reminders(&self) -> Result<Vec<Task>, RepositoryError> {
            Ok(self.tasks.lock().expect("task lock").clone())
        }

        fn clear_reminder_if_matches(
            &self,
            id: TaskId,
            scheduled_for: UtcTimestamp,
            now: UtcTimestamp,
        ) -> Result<bool, RepositoryError> {
            let mut tasks = self.tasks.lock().expect("task lock");
            let Some(task) = tasks.iter_mut().find(|task| task.record().id == id) else {
                return Ok(false);
            };
            let matches = task
                .record()
                .reminder
                .as_ref()
                .is_some_and(|reminder| reminder.at_utc == scheduled_for);
            if matches {
                task.clear_reminder(now);
            }
            Ok(matches)
        }
    }

    fn reminder_task(title: &str, reminder_at: i64) -> Task {
        let mut draft = NewTask::quick_capture(title, TaskPlacement::Inbox).expect("valid draft");
        draft.reminder = Some(ScheduledInstant {
            at_utc: UtcTimestamp::from_unix_seconds(reminder_at),
            time_zone: TimeZoneId::new("Asia/Shanghai").expect("valid timezone"),
        });
        Task::create(
            TaskId::generate(),
            draft,
            SortKey::INITIAL,
            UtcTimestamp::from_unix_seconds(1),
        )
        .expect("valid task")
    }

    #[test]
    fn recomputation_returns_all_due_and_one_nearest_future_deadline() {
        let tasks = vec![
            reminder_task("later", 300),
            reminder_task("due", 100),
            reminder_task("nearest", 220),
            reminder_task("due now", 200),
        ];
        let plan = ReminderPlan::from_tasks(&tasks, UtcTimestamp::from_unix_seconds(200));
        assert_eq!(
            plan.due
                .iter()
                .map(|alert| alert.title.as_str())
                .collect::<Vec<_>>(),
            vec!["due", "due now"]
        );
        assert_eq!(
            plan.next_deadline,
            Some(UtcTimestamp::from_unix_seconds(220))
        );
    }

    #[test]
    fn no_reminders_produces_an_idle_schedule() {
        assert_eq!(
            ReminderPlan::from_tasks(&[], UtcTimestamp::from_unix_seconds(200)),
            ReminderPlan::default()
        );
    }

    #[tokio::test]
    async fn mutation_signal_replaces_a_later_wait_and_delivers_the_new_due_reminder() {
        let repository = Arc::new(FakeReminderRepository {
            tasks: Mutex::new(vec![reminder_task("later", 300)]),
        });
        let clock: Arc<dyn Clock> = Arc::new(FixedClock(UtcTimestamp::from_unix_seconds(200)));
        let (delivered_sender, mut delivered_receiver) = mpsc::unbounded_channel();
        let delivery = Arc::new(move |alert| {
            delivered_sender.send(alert).map_err(|error| {
                super::ReminderDeliveryError::new(format!("delivery channel closed: {error}"))
            })
        });
        let (scheduler, handle) = ReminderScheduler::new(
            Arc::clone(&repository) as Arc<dyn ReminderRepository>,
            clock,
            delivery,
        );
        let worker = tokio::spawn(scheduler.run());

        *repository.tasks.lock().expect("task lock") = vec![reminder_task("new due", 200)];
        handle.schedule_changed();

        let delivered =
            tokio::time::timeout(std::time::Duration::from_secs(1), delivered_receiver.recv())
                .await
                .expect("scheduler woke after mutation")
                .expect("delivery channel remains open");
        assert_eq!(delivered.title, "new due");
        assert!(
            repository.tasks.lock().expect("task lock")[0]
                .record()
                .reminder
                .is_none()
        );

        handle.shutdown();
        worker.await.expect("scheduler exits cleanly");
    }
}
