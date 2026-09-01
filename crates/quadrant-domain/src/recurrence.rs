//! Versioned recurrence value objects.

use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;

use crate::{LocalDate, ScheduledInstant, TimeZoneId, UtcTimestamp};

/// The supported M2 recurrence cadences.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "frequency", rename_all = "snake_case")]
pub enum RecurrencePattern {
    /// Repeats every day.
    Daily,
    /// Repeats every week.
    Weekly,
    /// Repeats every month using local calendar semantics.
    Monthly,
    /// Repeats after a validated number of days.
    CustomDays {
        /// Inclusive interval from 1 through 365 days.
        interval_days: u16,
    },
}

/// A versioned, validated recurrence rule suitable for serialized persistence.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct RecurrenceRule {
    version: u8,
    pattern: RecurrencePattern,
}

impl<'de> Deserialize<'de> for RecurrenceRule {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct SerializedRule {
            version: u8,
            pattern: RecurrencePattern,
        }

        let value = SerializedRule::deserialize(deserializer)?;
        Self::restore(value.version, value.pattern).map_err(serde::de::Error::custom)
    }
}

impl RecurrenceRule {
    /// Current serialized recurrence representation version.
    pub const CURRENT_VERSION: u8 = 1;

    /// Creates a rule using the current representation version.
    ///
    /// # Errors
    ///
    /// Returns [`RecurrenceRuleError`] when the pattern is outside product limits.
    pub fn new(pattern: RecurrencePattern) -> Result<Self, RecurrenceRuleError> {
        Self::restore(Self::CURRENT_VERSION, pattern)
    }

    /// Restores and validates a serialized rule.
    ///
    /// # Errors
    ///
    /// Returns [`RecurrenceRuleError`] for unsupported versions or invalid intervals.
    pub fn restore(version: u8, pattern: RecurrencePattern) -> Result<Self, RecurrenceRuleError> {
        if version != Self::CURRENT_VERSION {
            return Err(RecurrenceRuleError::UnsupportedVersion(version));
        }
        if let RecurrencePattern::CustomDays { interval_days } = pattern
            && !(1..=365).contains(&interval_days)
        {
            return Err(RecurrenceRuleError::InvalidCustomInterval);
        }
        Ok(Self { version, pattern })
    }

    /// Returns the serialized rule version.
    #[must_use]
    pub const fn version(self) -> u8 {
        self.version
    }

    /// Returns the recurrence cadence.
    #[must_use]
    pub const fn pattern(self) -> RecurrencePattern {
        self.pattern
    }

    /// Advances a date by one occurrence using calendar semantics.
    ///
    /// Monthly recurrence clamps to the last valid day of the target month.
    ///
    /// # Errors
    ///
    /// Returns [`RecurrenceAdvanceError`] when the result is outside the supported range.
    pub fn advance_date(self, date: LocalDate) -> Result<LocalDate, RecurrenceAdvanceError> {
        let date = date
            .to_string()
            .parse::<jiff::civil::Date>()
            .map_err(|_| RecurrenceAdvanceError::OutOfRange)?;
        let advanced = date
            .checked_add(self.span())
            .map_err(|_| RecurrenceAdvanceError::OutOfRange)?;
        LocalDate::parse_iso(&advanced.to_string()).map_err(|_| RecurrenceAdvanceError::OutOfRange)
    }

    /// Advances a scheduled instant while preserving its timezone-local wall time.
    ///
    /// Jiff's compatible DST disambiguation moves skipped wall times forward and
    /// chooses the earlier occurrence for repeated wall times.
    ///
    /// # Errors
    ///
    /// Returns [`RecurrenceAdvanceError`] for an unavailable timezone or out-of-range result.
    pub fn advance_instant(
        self,
        value: &ScheduledInstant,
    ) -> Result<ScheduledInstant, RecurrenceAdvanceError> {
        let time_zone = jiff::tz::TimeZone::get(value.time_zone.as_str())
            .map_err(|_| RecurrenceAdvanceError::InvalidTimeZone)?;
        let timestamp = jiff::Timestamp::from_second(value.at_utc.unix_seconds())
            .map_err(|_| RecurrenceAdvanceError::OutOfRange)?;
        let advanced = timestamp
            .to_zoned(time_zone)
            .checked_add(self.span())
            .map_err(|_| RecurrenceAdvanceError::OutOfRange)?;
        Ok(ScheduledInstant {
            at_utc: UtcTimestamp::from_unix_seconds(advanced.timestamp().as_second()),
            time_zone: TimeZoneId::new(value.time_zone.as_str())
                .map_err(|_| RecurrenceAdvanceError::InvalidTimeZone)?,
        })
    }

    fn span(self) -> jiff::Span {
        match self.pattern {
            RecurrencePattern::Daily => jiff::Span::new().days(1),
            RecurrencePattern::Weekly => jiff::Span::new().weeks(1),
            RecurrencePattern::Monthly => jiff::Span::new().months(1),
            RecurrencePattern::CustomDays { interval_days } => {
                jiff::Span::new().days(i64::from(interval_days))
            }
        }
    }
}

/// Failures while deriving a recurrence occurrence.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum RecurrenceAdvanceError {
    /// A scheduled instant referenced a timezone unavailable on this host.
    #[error("recurrence timezone is unavailable")]
    InvalidTimeZone,
    /// Calendar arithmetic exceeded the supported date/timestamp range.
    #[error("next recurrence is outside the supported calendar range")]
    OutOfRange,
}

/// Recurrence validation errors.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum RecurrenceRuleError {
    /// Serialized rules with unknown versions are not silently interpreted.
    #[error("unsupported recurrence version {0}")]
    UnsupportedVersion(u8),
    /// Custom day intervals must be between 1 and 365.
    #[error("custom recurrence interval must be between 1 and 365 days")]
    InvalidCustomInterval,
}

#[cfg(test)]
mod tests {
    use super::{RecurrencePattern, RecurrenceRule, RecurrenceRuleError};
    use crate::{LocalDate, ScheduledInstant, TimeZoneId, UtcTimestamp};

    #[test]
    fn custom_intervals_are_bounded() {
        assert!(RecurrenceRule::new(RecurrencePattern::CustomDays { interval_days: 1 }).is_ok());
        assert!(RecurrenceRule::new(RecurrencePattern::CustomDays { interval_days: 365 }).is_ok());
        assert_eq!(
            RecurrenceRule::new(RecurrencePattern::CustomDays { interval_days: 0 }),
            Err(RecurrenceRuleError::InvalidCustomInterval)
        );
    }

    #[test]
    fn calendar_recurrence_crosses_boundaries_and_clamps_months() {
        let daily = RecurrenceRule::new(RecurrencePattern::Daily).expect("daily rule");
        let weekly = RecurrenceRule::new(RecurrencePattern::Weekly).expect("weekly rule");
        let monthly = RecurrenceRule::new(RecurrencePattern::Monthly).expect("monthly rule");

        assert_eq!(
            daily
                .advance_date(LocalDate::parse_iso("2026-12-31").expect("date"))
                .expect("next date")
                .to_string(),
            "2027-01-01"
        );
        assert_eq!(
            weekly
                .advance_date(LocalDate::parse_iso("2026-12-28").expect("date"))
                .expect("next date")
                .to_string(),
            "2027-01-04"
        );
        assert_eq!(
            monthly
                .advance_date(LocalDate::parse_iso("2026-01-31").expect("date"))
                .expect("next date")
                .to_string(),
            "2026-02-28"
        );
    }

    #[test]
    fn scheduled_recurrence_preserves_local_time_across_dst() {
        let rule = RecurrenceRule::new(RecurrencePattern::Weekly).expect("weekly rule");
        let source = "2026-03-07T09:00:00-05:00[America/New_York]"
            .parse::<jiff::Zoned>()
            .expect("zoned source");
        let next = rule
            .advance_instant(&ScheduledInstant {
                at_utc: UtcTimestamp::from_unix_seconds(source.timestamp().as_second()),
                time_zone: TimeZoneId::new("America/New_York").expect("timezone"),
            })
            .expect("next instant");
        let next_zoned = jiff::Timestamp::from_second(next.at_utc.unix_seconds())
            .expect("timestamp")
            .to_zoned(jiff::tz::TimeZone::get("America/New_York").expect("timezone"));

        assert_eq!(
            next_zoned.to_string(),
            "2026-03-14T09:00:00-04:00[America/New_York]"
        );
    }
}
