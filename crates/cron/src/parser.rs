//! Cron expression parser
//!
//! Supports standard 5-field cron syntax:
//! ```text
//! ┌───────────── minute (0-59)
//! │ ┌───────────── hour (0-23)
//! │ │ ┌───────────── day of month (1-31)
//! │ │ │ ┌───────────── month (1-12)
//! │ │ │ │ ┌───────────── day of week (0-6, 0=Sunday)
//! │ │ │ │ │
//! * * * * *
//! ```
//!
//! Special characters:
//! - `*` - any value
//! - `,` - value list separator (e.g., `1,3,5`)
//! - `-` - range (e.g., `1-5`)
//! - `/` - step (e.g., `*/5` or `0-30/5`)

use crate::types::{CronError, Result};
use chrono::{DateTime, Datelike, Duration, TimeZone, Timelike, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// A parsed cron expression
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronExpression {
    /// Original expression string
    pub expression: String,
    /// Allowed minutes (0-59)
    minutes: BTreeSet<u32>,
    /// Allowed hours (0-23)
    hours: BTreeSet<u32>,
    /// Allowed days of month (1-31)
    days: BTreeSet<u32>,
    /// Allowed months (1-12)
    months: BTreeSet<u32>,
    /// Allowed days of week (0-6, 0=Sunday)
    weekdays: BTreeSet<u32>,
}

impl CronExpression {
    /// Parse a cron expression string
    ///
    /// # Examples
    ///
    /// ```
    /// use a3s_cron::CronExpression;
    ///
    /// // Every 5 minutes
    /// let expr = CronExpression::parse("*/5 * * * *").unwrap();
    ///
    /// // Every day at 2:30 AM
    /// let expr = CronExpression::parse("30 2 * * *").unwrap();
    ///
    /// // Every Monday at 9 AM
    /// let expr = CronExpression::parse("0 9 * * 1").unwrap();
    /// ```
    pub fn parse(expression: &str) -> Result<Self> {
        let parts: Vec<&str> = expression.split_whitespace().collect();

        if parts.len() != 5 {
            return Err(CronError::InvalidExpression(format!(
                "Expected 5 fields, got {}",
                parts.len()
            )));
        }

        let minutes = parse_field(parts[0], 0, 59, "minute")?;
        let hours = parse_field(parts[1], 0, 23, "hour")?;
        let days = parse_field(parts[2], 1, 31, "day")?;
        let months = parse_field(parts[3], 1, 12, "month")?;
        let weekdays = parse_field(parts[4], 0, 6, "weekday")?;

        Ok(Self {
            expression: expression.to_string(),
            minutes,
            hours,
            days,
            months,
            weekdays,
        })
    }

    /// Calculate the next run time after the given datetime
    pub fn next_after(&self, after: DateTime<Utc>) -> Option<DateTime<Utc>> {
        // Start from the next minute
        let mut current = after + Duration::minutes(1);
        current = Utc
            .with_ymd_and_hms(
                current.year(),
                current.month(),
                current.day(),
                current.hour(),
                current.minute(),
                0,
            )
            .single()?;

        // Search for up to 4 years (to handle leap years and edge cases)
        let max_iterations = 4 * 366 * 24 * 60;

        for _ in 0..max_iterations {
            if self.matches(&current) {
                return Some(current);
            }
            current = current + Duration::minutes(1);
        }

        None
    }

    /// Check if a datetime matches this cron expression
    pub fn matches(&self, dt: &DateTime<Utc>) -> bool {
        let minute = dt.minute();
        let hour = dt.hour();
        let day = dt.day();
        let month = dt.month();
        let weekday = dt.weekday().num_days_from_sunday();

        self.minutes.contains(&minute)
            && self.hours.contains(&hour)
            && self.days.contains(&day)
            && self.months.contains(&month)
            && self.weekdays.contains(&weekday)
    }

    /// Get a human-readable description of the schedule
    pub fn describe(&self) -> String {
        let mut parts = Vec::new();

        // Minutes
        if self.minutes.len() == 60 {
            parts.push("every minute".to_string());
        } else if self.minutes.len() == 1 {
            let min = *self.minutes.iter().next().unwrap();
            if min == 0 {
                parts.push("at the start of the hour".to_string());
            } else {
                parts.push(format!("at minute {}", min));
            }
        } else {
            parts.push(format!("at minutes {:?}", self.minutes));
        }

        // Hours
        if self.hours.len() < 24 {
            if self.hours.len() == 1 {
                let hour = *self.hours.iter().next().unwrap();
                parts.push(format!("at {}:00", hour));
            } else {
                parts.push(format!("during hours {:?}", self.hours));
            }
        }

        // Days
        if self.days.len() < 31 {
            parts.push(format!("on days {:?}", self.days));
        }

        // Months
        if self.months.len() < 12 {
            parts.push(format!("in months {:?}", self.months));
        }

        // Weekdays
        if self.weekdays.len() < 7 {
            let weekday_names: Vec<&str> = self
                .weekdays
                .iter()
                .map(|&d| match d {
                    0 => "Sun",
                    1 => "Mon",
                    2 => "Tue",
                    3 => "Wed",
                    4 => "Thu",
                    5 => "Fri",
                    6 => "Sat",
                    _ => "?",
                })
                .collect();
            parts.push(format!("on {}", weekday_names.join(", ")));
        }

        parts.join(", ")
    }
}

/// Parse a single cron field
fn parse_field(field: &str, min: u32, max: u32, name: &str) -> Result<BTreeSet<u32>> {
    let mut values = BTreeSet::new();

    for part in field.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        // Handle step values (e.g., */5 or 0-30/5)
        let (range_part, step) = if let Some(idx) = part.find('/') {
            let step_str = &part[idx + 1..];
            let step: u32 = step_str.parse().map_err(|_| {
                CronError::InvalidExpression(format!(
                    "Invalid step value '{}' in {}",
                    step_str, name
                ))
            })?;
            if step == 0 {
                return Err(CronError::InvalidExpression(format!(
                    "Step value cannot be 0 in {}",
                    name
                )));
            }
            (&part[..idx], Some(step))
        } else {
            (part, None)
        };

        // Parse the range part
        let (start, end) = if range_part == "*" {
            (min, max)
        } else if let Some(idx) = range_part.find('-') {
            let start: u32 = range_part[..idx].parse().map_err(|_| {
                CronError::InvalidExpression(format!(
                    "Invalid range start '{}' in {}",
                    &range_part[..idx],
                    name
                ))
            })?;
            let end: u32 = range_part[idx + 1..].parse().map_err(|_| {
                CronError::InvalidExpression(format!(
                    "Invalid range end '{}' in {}",
                    &range_part[idx + 1..],
                    name
                ))
            })?;
            (start, end)
        } else {
            let value: u32 = range_part.parse().map_err(|_| {
                CronError::InvalidExpression(format!("Invalid value '{}' in {}", range_part, name))
            })?;
            (value, value)
        };

        // Validate range
        if start < min || start > max {
            return Err(CronError::InvalidExpression(format!(
                "Value {} out of range ({}-{}) in {}",
                start, min, max, name
            )));
        }
        if end < min || end > max {
            return Err(CronError::InvalidExpression(format!(
                "Value {} out of range ({}-{}) in {}",
                end, min, max, name
            )));
        }
        if start > end {
            return Err(CronError::InvalidExpression(format!(
                "Invalid range {}-{} in {}",
                start, end, name
            )));
        }

        // Add values with step
        let step = step.unwrap_or(1);
        let mut current = start;
        while current <= end {
            values.insert(current);
            current += step;
        }
    }

    if values.is_empty() {
        return Err(CronError::InvalidExpression(format!(
            "No valid values in {}",
            name
        )));
    }

    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_every_minute() {
        let expr = CronExpression::parse("* * * * *").unwrap();
        assert_eq!(expr.minutes.len(), 60);
        assert_eq!(expr.hours.len(), 24);
        assert_eq!(expr.days.len(), 31);
        assert_eq!(expr.months.len(), 12);
        assert_eq!(expr.weekdays.len(), 7);
    }

    #[test]
    fn test_parse_specific_time() {
        let expr = CronExpression::parse("30 2 * * *").unwrap();
        assert_eq!(expr.minutes, BTreeSet::from([30]));
        assert_eq!(expr.hours, BTreeSet::from([2]));
    }

    #[test]
    fn test_parse_step() {
        let expr = CronExpression::parse("*/5 * * * *").unwrap();
        assert_eq!(
            expr.minutes,
            BTreeSet::from([0, 5, 10, 15, 20, 25, 30, 35, 40, 45, 50, 55])
        );
    }

    #[test]
    fn test_parse_range() {
        let expr = CronExpression::parse("0 9-17 * * *").unwrap();
        assert_eq!(
            expr.hours,
            BTreeSet::from([9, 10, 11, 12, 13, 14, 15, 16, 17])
        );
    }

    #[test]
    fn test_parse_list() {
        let expr = CronExpression::parse("0 0 * * 1,3,5").unwrap();
        assert_eq!(expr.weekdays, BTreeSet::from([1, 3, 5]));
    }

    #[test]
    fn test_parse_range_with_step() {
        let expr = CronExpression::parse("0-30/10 * * * *").unwrap();
        assert_eq!(expr.minutes, BTreeSet::from([0, 10, 20, 30]));
    }

    #[test]
    fn test_parse_invalid_field_count() {
        let result = CronExpression::parse("* * *");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_invalid_value() {
        let result = CronExpression::parse("60 * * * *");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_invalid_range() {
        let result = CronExpression::parse("30-10 * * * *");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_zero_step() {
        let result = CronExpression::parse("*/0 * * * *");
        assert!(result.is_err());
    }

    #[test]
    fn test_next_after() {
        let expr = CronExpression::parse("0 * * * *").unwrap();
        let now = Utc.with_ymd_and_hms(2026, 2, 5, 10, 30, 0).unwrap();
        let next = expr.next_after(now).unwrap();
        assert_eq!(next.hour(), 11);
        assert_eq!(next.minute(), 0);
    }

    #[test]
    fn test_next_after_specific_time() {
        let expr = CronExpression::parse("30 14 * * *").unwrap();
        let now = Utc.with_ymd_and_hms(2026, 2, 5, 10, 0, 0).unwrap();
        let next = expr.next_after(now).unwrap();
        assert_eq!(next.day(), 5);
        assert_eq!(next.hour(), 14);
        assert_eq!(next.minute(), 30);
    }

    #[test]
    fn test_next_after_next_day() {
        let expr = CronExpression::parse("0 2 * * *").unwrap();
        let now = Utc.with_ymd_and_hms(2026, 2, 5, 10, 0, 0).unwrap();
        let next = expr.next_after(now).unwrap();
        assert_eq!(next.day(), 6);
        assert_eq!(next.hour(), 2);
        assert_eq!(next.minute(), 0);
    }

    #[test]
    fn test_matches() {
        let expr = CronExpression::parse("30 14 * * 1").unwrap();
        // Monday, Feb 3, 2026 at 14:30
        let dt = Utc.with_ymd_and_hms(2026, 2, 2, 14, 30, 0).unwrap();
        assert!(expr.matches(&dt));

        // Same time but Tuesday
        let dt = Utc.with_ymd_and_hms(2026, 2, 3, 14, 30, 0).unwrap();
        assert!(!expr.matches(&dt));
    }

    #[test]
    fn test_describe() {
        let expr = CronExpression::parse("0 9 * * 1-5").unwrap();
        let desc = expr.describe();
        assert!(desc.contains("Mon"));
        assert!(desc.contains("Fri"));
    }
}
