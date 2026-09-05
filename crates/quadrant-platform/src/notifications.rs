//! Native desktop notification delivery.

use quadrant_application::{ReminderAlert, ReminderDelivery, ReminderDeliveryError, UtcTimestamp};

/// Platform implementation of the application reminder-delivery port.
#[derive(Clone, Copy, Debug, Default)]
pub struct PlatformNotificationDelivery;

impl PlatformNotificationDelivery {
    /// Delivers a generic Focus deadline notification without requiring a task or GUI.
    ///
    /// # Errors
    /// Returns unavailable/native delivery failures at the platform boundary.
    pub fn focus_completed() -> Result<(), crate::PlatformIntegrationError> {
        #[cfg(target_os = "windows")]
        {
            notify_rust::Notification::new()
                .appname("Quadrant")
                .summary("Focus interval completed")
                .body("Your Quadrant focus interval has finished.")
                .show()
                .map(|_| ())
                .map_err(crate::PlatformIntegrationError::new)
        }
        #[cfg(not(target_os = "windows"))]
        {
            Err(crate::PlatformIntegrationError::new(
                "native Focus notifications are unavailable",
            ))
        }
    }
}

impl ReminderDelivery for PlatformNotificationDelivery {
    fn deliver(&self, alert: ReminderAlert) -> Result<(), ReminderDeliveryError> {
        deliver_notification(&alert)
    }
}

#[cfg(target_os = "windows")]
fn deliver_notification(alert: &ReminderAlert) -> Result<(), ReminderDeliveryError> {
    let content = NotificationContent::from_alert(alert);
    notify_rust::Notification::new()
        .appname("Quadrant")
        .summary(&content.title)
        .body(&content.body)
        .timeout(notify_rust::Timeout::Milliseconds(10_000))
        .show()
        .map(|_| ())
        .map_err(ReminderDeliveryError::new)
}

#[cfg(not(target_os = "windows"))]
fn deliver_notification(_alert: &ReminderAlert) -> Result<(), ReminderDeliveryError> {
    Err(ReminderDeliveryError::new(
        "native notifications are not implemented on this platform",
    ))
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct NotificationContent {
    title: String,
    body: String,
}

impl NotificationContent {
    fn from_alert(alert: &ReminderAlert) -> Self {
        Self {
            title: alert.title.clone(),
            body: format!(
                "Quadrant reminder · {}",
                format_utc_timestamp(alert.scheduled_for)
            ),
        }
    }
}

fn format_utc_timestamp(timestamp: UtcTimestamp) -> String {
    jiff::Timestamp::from_second(timestamp.unix_seconds())
        .map_or_else(|_| "scheduled now".to_owned(), |value| value.to_string())
}

#[cfg(test)]
mod tests {
    use quadrant_application::{ReminderAlert, ReminderDelivery, TaskId, UtcTimestamp};

    use super::{NotificationContent, PlatformNotificationDelivery};

    #[test]
    fn reminder_notification_content_is_stable_and_task_focused() {
        let content = NotificationContent::from_alert(&ReminderAlert {
            task_id: TaskId::generate(),
            title: "Send the report".to_owned(),
            scheduled_for: UtcTimestamp::from_unix_seconds(1_788_192_000),
        });

        assert_eq!(content.title, "Send the report");
        assert!(content.body.starts_with("Quadrant reminder · 2026-"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    #[ignore = "shows a real Windows notification"]
    fn native_windows_notification_smoke_test() {
        PlatformNotificationDelivery
            .deliver(ReminderAlert {
                task_id: TaskId::generate(),
                title: "Quadrant native notification test".to_owned(),
                scheduled_for: UtcTimestamp::from_unix_seconds(1_788_192_000),
            })
            .expect("Windows notification delivery");
    }
}
