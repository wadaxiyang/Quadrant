//! Explicit date, timestamp, and timezone values.

use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use time::{Date, Month};

/// An absolute Unix timestamp in whole UTC seconds.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct UtcTimestamp(i64);

impl UtcTimestamp {
    /// Creates a timestamp from Unix UTC seconds.
    #[must_use]
    pub const fn from_unix_seconds(seconds: i64) -> Self {
        Self(seconds)
    }

    /// Returns the represented Unix UTC seconds.
    #[must_use]
    pub const fn unix_seconds(self) -> i64 {
        self.0
    }
}

/// A validated IANA-style timezone identifier retained with a scheduled instant.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct TimeZoneId(String);

impl TimeZoneId {
    /// Validates and owns a timezone identifier.
    ///
    /// # Errors
    ///
    /// Returns [`TimeValueError::InvalidTimeZone`] for blank, oversized, or
    /// whitespace-containing identifiers.
    pub fn new(value: impl Into<String>) -> Result<Self, TimeValueError> {
        let value = value.into();
        let trimmed = value.trim();
        if trimmed.is_empty() || trimmed.len() > 255 || trimmed.chars().any(char::is_whitespace) {
            return Err(TimeValueError::InvalidTimeZone);
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// Returns the normalized identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for TimeZoneId {
    type Error = TimeValueError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<TimeZoneId> for String {
    fn from(value: TimeZoneId) -> Self {
        value.0
    }
}

/// A calendar date without an implicit timezone.
#[derive(
    Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize,
)]
#[serde(try_from = "String", into = "String")]
pub struct LocalDate(Date);

impl TryFrom<String> for LocalDate {
    type Error = TimeValueError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse_iso(&value)
    }
}

impl From<LocalDate> for String {
    fn from(value: LocalDate) -> Self {
        value.to_string()
    }
}

impl LocalDate {
    /// Builds a validated calendar date.
    ///
    /// # Errors
    ///
    /// Returns [`TimeValueError::InvalidDate`] when the components do not form
    /// a real calendar date.
    pub fn from_calendar_date(year: i32, month: u8, day: u8) -> Result<Self, TimeValueError> {
        let month = Month::try_from(month).map_err(|_| TimeValueError::InvalidDate)?;
        Date::from_calendar_date(year, month, day)
            .map(Self)
            .map_err(|_| TimeValueError::InvalidDate)
    }

    /// Parses the stable `YYYY-MM-DD` persistence representation.
    ///
    /// # Errors
    ///
    /// Returns [`TimeValueError::InvalidDate`] for malformed or impossible dates.
    pub fn parse_iso(value: &str) -> Result<Self, TimeValueError> {
        let mut parts = value.split('-');
        let year = parts
            .next()
            .and_then(|part| part.parse::<i32>().ok())
            .ok_or(TimeValueError::InvalidDate)?;
        let month = parts
            .next()
            .and_then(|part| part.parse::<u8>().ok())
            .ok_or(TimeValueError::InvalidDate)?;
        let day = parts
            .next()
            .and_then(|part| part.parse::<u8>().ok())
            .ok_or(TimeValueError::InvalidDate)?;
        if parts.next().is_some() || value.len() != 10 {
            return Err(TimeValueError::InvalidDate);
        }
        Self::from_calendar_date(year, month, day)
    }

    /// Adds a signed number of calendar days without introducing timezone semantics.
    #[must_use]
    pub fn checked_add_days(self, days: i64) -> Option<Self> {
        self.0.checked_add(time::Duration::days(days)).map(Self)
    }

    /// Returns the calendar year.
    #[must_use]
    pub const fn year(self) -> i32 {
        self.0.year()
    }

    /// Returns the one-based calendar month.
    #[must_use]
    pub const fn month(self) -> u8 {
        self.0.month() as u8
    }

    /// Returns the one-based day of month.
    #[must_use]
    pub const fn day(self) -> u8 {
        self.0.day()
    }
}

impl fmt::Display for LocalDate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{:04}-{:02}-{:02}",
            self.0.year(),
            u8::from(self.0.month()),
            self.0.day()
        )
    }
}

/// An absolute instant paired with the timezone semantics used for editing/recurrence.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ScheduledInstant {
    /// Absolute due/reminder time.
    pub at_utc: UtcTimestamp,
    /// Timezone in which the user expressed the local schedule.
    pub time_zone: TimeZoneId,
}

/// Validation failures for explicit time values.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum TimeValueError {
    /// A timezone identifier was blank or structurally invalid.
    #[error("timezone identifier is invalid")]
    InvalidTimeZone,
    /// A date was malformed or not a real calendar date.
    #[error("local date is invalid")]
    InvalidDate,
}

#[cfg(test)]
mod tests {
    use super::{LocalDate, TimeZoneId};

    #[test]
    fn local_dates_use_stable_iso_text() {
        let date = LocalDate::parse_iso("2026-09-01").expect("valid date");
        assert_eq!(date.to_string(), "2026-09-01");
        assert!(LocalDate::parse_iso("2026-02-30").is_err());
    }

    #[test]
    fn timezone_ids_reject_whitespace() {
        assert!(TimeZoneId::new("Asia/Shanghai").is_ok());
        assert!(TimeZoneId::new("Asia / Shanghai").is_err());
    }
}
