//! Versioned recurrence value objects.

use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;

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

    #[test]
    fn custom_intervals_are_bounded() {
        assert!(RecurrenceRule::new(RecurrencePattern::CustomDays { interval_days: 1 }).is_ok());
        assert!(RecurrenceRule::new(RecurrencePattern::CustomDays { interval_days: 365 }).is_ok());
        assert_eq!(
            RecurrenceRule::new(RecurrencePattern::CustomDays { interval_days: 0 }),
            Err(RecurrenceRuleError::InvalidCustomInterval)
        );
    }
}
