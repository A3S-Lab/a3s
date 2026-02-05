//! Natural language parser for cron expressions
//!
//! Converts human-readable schedule descriptions to cron expressions.
//!
//! ## Supported Formats
//!
//! ### English
//! - "every minute" / "every 5 minutes"
//! - "every hour" / "every 2 hours"
//! - "every day at 2am" / "daily at 14:30"
//! - "every monday at 9am" / "every weekday at 8:30"
//! - "every month on the 1st at midnight"
//!
//! ### Chinese (中文)
//! - "每分钟" / "每5分钟"
//! - "每小时" / "每2小时"
//! - "每天凌晨2点" / "每天下午3点30分"
//! - "每周一上午9点" / "工作日早上8点半"
//! - "每月1号零点"

use crate::types::{CronError, Result};

/// Parse natural language to cron expression
///
/// # Examples
///
/// ```
/// use a3s_cron::natural::parse_natural;
///
/// // English
/// assert_eq!(parse_natural("every 5 minutes").unwrap(), "*/5 * * * *");
/// assert_eq!(parse_natural("every day at 2am").unwrap(), "0 2 * * *");
/// assert_eq!(parse_natural("every monday at 9am").unwrap(), "0 9 * * 1");
///
/// // Chinese
/// assert_eq!(parse_natural("每5分钟").unwrap(), "*/5 * * * *");
/// assert_eq!(parse_natural("每天凌晨2点").unwrap(), "0 2 * * *");
/// ```
pub fn parse_natural(input: &str) -> Result<String> {
    let input = input.trim().to_lowercase();

    // If it looks like a cron expression already, validate and return it
    if looks_like_cron(&input) {
        return Ok(input);
    }

    // Try English patterns first
    if let Some(expr) = try_parse_english(&input) {
        return Ok(expr);
    }

    // Try Chinese patterns
    if let Some(expr) = try_parse_chinese(&input) {
        return Ok(expr);
    }

    Err(CronError::InvalidExpression(format!(
        "Could not parse '{}' as a schedule. Try formats like 'every 5 minutes', 'daily at 9am', '每天上午9点'",
        input
    )))
}

/// Check if input looks like a cron expression
fn looks_like_cron(input: &str) -> bool {
    let parts: Vec<&str> = input.split_whitespace().collect();
    if parts.len() != 5 {
        return false;
    }
    parts.iter().all(|p| {
        p.chars()
            .all(|c| c.is_ascii_digit() || c == '*' || c == ',' || c == '-' || c == '/')
    })
}

// ============================================================================
// English Parser
// ============================================================================

fn try_parse_english(input: &str) -> Option<String> {
    // Simple patterns first
    match input {
        "minute" | "every minute" => return Some("* * * * *".to_string()),
        "hour" | "hourly" | "every hour" => return Some("0 * * * *".to_string()),
        "day" | "daily" | "every day" | "midnight" => return Some("0 0 * * *".to_string()),
        "week" | "weekly" | "every week" => return Some("0 0 * * 0".to_string()),
        "month" | "monthly" | "every month" => return Some("0 0 1 * *".to_string()),
        "year" | "yearly" | "annually" | "every year" => return Some("0 0 1 1 *".to_string()),
        _ => {}
    }

    // Every N minutes
    if input.contains("minute") {
        if let Some(n) = extract_number(input) {
            if n > 0 && n <= 59 {
                return Some(format!("*/{} * * * *", n));
            }
        }
    }

    // Every N hours
    if input.contains("hour") {
        if let Some(n) = extract_number(input) {
            if n > 0 && n <= 23 {
                return Some(format!("0 */{} * * *", n));
            }
        }
    }

    // Weekday/workday patterns
    if input.contains("weekday") || input.contains("workday") {
        let (hour, minute) = extract_time_english(input).unwrap_or((9, 0));
        return Some(format!("{} {} * * 1-5", minute, hour));
    }

    // Weekend patterns
    if input.contains("weekend") {
        let (hour, minute) = extract_time_english(input).unwrap_or((10, 0));
        return Some(format!("{} {} * * 0,6", minute, hour));
    }

    // Monthly patterns (must check before weekday patterns because "monthly" contains "mon")
    if input.contains("month") {
        let day = extract_ordinal(input).unwrap_or(1);
        let (hour, minute) = extract_time_english(input).unwrap_or((0, 0));
        if day >= 1 && day <= 31 {
            return Some(format!("{} {} {} * *", minute, hour, day));
        }
    }

    // Specific weekday patterns
    let weekdays = [
        ("sunday", 0),
        ("sun", 0),
        ("monday", 1),
        ("mon", 1),
        ("tuesday", 2),
        ("tue", 2),
        ("wednesday", 3),
        ("wed", 3),
        ("thursday", 4),
        ("thu", 4),
        ("friday", 5),
        ("fri", 5),
        ("saturday", 6),
        ("sat", 6),
    ];

    for (name, day) in weekdays {
        if input.contains(name) {
            let (hour, minute) = extract_time_english(input).unwrap_or((9, 0));
            return Some(format!("{} {} * * {}", minute, hour, day));
        }
    }

    // Daily patterns (must check after weekday patterns)
    if input.contains("day") || input.contains("daily") {
        let (hour, minute) = extract_time_english(input).unwrap_or((0, 0));
        return Some(format!("{} {} * * *", minute, hour));
    }

    None
}

/// Extract a number from the input string
fn extract_number(input: &str) -> Option<u32> {
    let mut num_str = String::new();
    let mut found = false;

    for c in input.chars() {
        if c.is_ascii_digit() {
            num_str.push(c);
            found = true;
        } else if found {
            break;
        }
    }

    if found {
        num_str.parse().ok()
    } else {
        None
    }
}

/// Extract ordinal number (1st, 2nd, 3rd, etc.)
fn extract_ordinal(input: &str) -> Option<u32> {
    // Look for patterns like "1st", "2nd", "15th"
    let mut num_str = String::new();

    for (i, c) in input.char_indices() {
        if c.is_ascii_digit() {
            num_str.push(c);
        } else if !num_str.is_empty() {
            // Check if followed by ordinal suffix
            let rest = &input[i..];
            if rest.starts_with("st")
                || rest.starts_with("nd")
                || rest.starts_with("rd")
                || rest.starts_with("th")
            {
                return num_str.parse().ok();
            }
            num_str.clear();
        }
    }

    None
}

/// Extract time from English input (e.g., "2am", "14:30", "2:30pm")
fn extract_time_english(input: &str) -> Option<(u32, u32)> {
    // Try to find time patterns
    let input_lower = input.to_lowercase();

    // Look for HH:MM pattern
    if let Some(colon_pos) = input_lower.find(':') {
        // Find hour before colon
        let before = &input_lower[..colon_pos];
        let hour_start = before
            .rfind(|c: char| !c.is_ascii_digit())
            .map(|i| i + 1)
            .unwrap_or(0);
        let hour_str = &before[hour_start..];

        // Find minute after colon
        let after = &input_lower[colon_pos + 1..];
        let minute_end = after
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(after.len());
        let minute_str = &after[..minute_end];

        if let (Ok(mut hour), Ok(minute)) = (hour_str.parse::<u32>(), minute_str.parse::<u32>()) {
            // Check for am/pm after the time
            let rest = &after[minute_end..];
            if rest.contains("pm") && hour < 12 {
                hour += 12;
            } else if rest.contains("am") && hour == 12 {
                hour = 0;
            }
            if hour <= 23 && minute <= 59 {
                return Some((hour, minute));
            }
        }
    }

    // Look for Xam/Xpm pattern
    for suffix in ["am", "pm"] {
        if let Some(pos) = input_lower.find(suffix) {
            // Find number before am/pm
            let before = &input_lower[..pos];
            let num_start = before
                .rfind(|c: char| !c.is_ascii_digit())
                .map(|i| i + 1)
                .unwrap_or(0);
            let num_str = &before[num_start..];

            if let Ok(mut hour) = num_str.parse::<u32>() {
                if suffix == "pm" && hour < 12 {
                    hour += 12;
                } else if suffix == "am" && hour == 12 {
                    hour = 0;
                }
                if hour <= 23 {
                    return Some((hour, 0));
                }
            }
        }
    }

    None
}

// ============================================================================
// Chinese Parser
// ============================================================================

fn try_parse_chinese(input: &str) -> Option<String> {
    // Simple patterns
    if input == "每分钟" {
        return Some("* * * * *".to_string());
    }
    if input == "每小时" {
        return Some("0 * * * *".to_string());
    }

    // Every N minutes: 每5分钟, 每10分钟
    if input.contains("分钟")
        || (input.contains("分") && !input.contains("下午") && !input.contains("上午"))
    {
        if let Some(n) = extract_chinese_number(input) {
            if n > 0 && n <= 59 {
                return Some(format!("*/{} * * * *", n));
            }
        }
    }

    // Every N hours: 每2小时
    if input.contains("小时")
        && input.starts_with("每")
        && !input.contains("天")
        && !input.contains("周")
    {
        if let Some(n) = extract_chinese_number(input) {
            if n > 0 && n <= 23 {
                return Some(format!("0 */{} * * *", n));
            }
        }
    }

    // Workday: 工作日
    if input.contains("工作日") {
        let (hour, minute) = extract_chinese_time(input).unwrap_or((9, 0));
        return Some(format!("{} {} * * 1-5", minute, hour));
    }

    // Weekend: 周末
    if input.contains("周末") {
        let (hour, minute) = extract_chinese_time(input).unwrap_or((10, 0));
        return Some(format!("{} {} * * 0,6", minute, hour));
    }

    // Weekly patterns: 每周一, 每星期三
    let weekday_patterns = [
        ("周日", 0),
        ("星期日", 0),
        ("星期天", 0),
        ("周天", 0),
        ("周一", 1),
        ("星期一", 1),
        ("周二", 2),
        ("星期二", 2),
        ("周三", 3),
        ("星期三", 3),
        ("周四", 4),
        ("星期四", 4),
        ("周五", 5),
        ("星期五", 5),
        ("周六", 6),
        ("星期六", 6),
    ];

    for (pattern, day) in weekday_patterns {
        if input.contains(pattern) {
            let (hour, minute) = extract_chinese_time(input).unwrap_or((9, 0));
            return Some(format!("{} {} * * {}", minute, hour, day));
        }
    }

    // Monthly: 每月1号, 每月15日
    if input.contains("月") && (input.contains("号") || input.contains("日")) {
        let day = extract_chinese_day(input).unwrap_or(1);
        let (hour, minute) = extract_chinese_time(input).unwrap_or((0, 0));
        if day >= 1 && day <= 31 {
            return Some(format!("{} {} {} * *", minute, hour, day));
        }
    }

    // Daily: 每天
    if input.contains("每天") || input.contains("天天") {
        let (hour, minute) = extract_chinese_time(input).unwrap_or((0, 0));
        return Some(format!("{} {} * * *", minute, hour));
    }

    None
}

/// Extract number from Chinese input (supports both Arabic and Chinese numerals)
fn extract_chinese_number(input: &str) -> Option<u32> {
    // Try Arabic numerals first
    if let Some(n) = extract_number(input) {
        return Some(n);
    }

    // Chinese numerals
    let mut result = 0u32;
    let mut current = 0u32;
    let mut has_number = false;

    for c in input.chars() {
        match c {
            '零' | '〇' => {
                current = 0;
                has_number = true;
            }
            '一' | '壹' => {
                current = 1;
                has_number = true;
            }
            '二' | '贰' | '两' => {
                current = 2;
                has_number = true;
            }
            '三' | '叁' => {
                current = 3;
                has_number = true;
            }
            '四' | '肆' => {
                current = 4;
                has_number = true;
            }
            '五' | '伍' => {
                current = 5;
                has_number = true;
            }
            '六' | '陆' => {
                current = 6;
                has_number = true;
            }
            '七' | '柒' => {
                current = 7;
                has_number = true;
            }
            '八' | '捌' => {
                current = 8;
                has_number = true;
            }
            '九' | '玖' => {
                current = 9;
                has_number = true;
            }
            '十' | '拾' => {
                if current == 0 && !has_number {
                    current = 1;
                }
                result += current * 10;
                current = 0;
                has_number = true;
            }
            _ => {}
        }
    }
    result += current;

    if has_number && result > 0 {
        Some(result)
    } else {
        None
    }
}

/// Extract day number from Chinese input (e.g., "1号", "15日")
fn extract_chinese_day(input: &str) -> Option<u32> {
    // Find number before 号 or 日
    for suffix in ["号", "日"] {
        if let Some(pos) = input.find(suffix) {
            let before = &input[..pos];
            // Try Arabic numeral first
            if let Some(n) = extract_number(before) {
                return Some(n);
            }
            // Try Chinese numeral
            if let Some(n) = extract_chinese_number(before) {
                return Some(n);
            }
        }
    }
    None
}

/// Extract time from Chinese input
fn extract_chinese_time(input: &str) -> Option<(u32, u32)> {
    let mut hour: Option<u32> = None;
    let mut minute: u32 = 0;

    // Time period modifiers
    let is_pm = input.contains("下午") || input.contains("晚上") || input.contains("傍晚");
    let is_early_morning = input.contains("凌晨");
    let is_morning = input.contains("早上") || input.contains("上午");
    let is_noon = input.contains("中午");

    // Extract hour: N点 or N时
    if let Some(pos) = input.find('点').or_else(|| input.find('时')) {
        let before = &input[..pos];
        // Get the last number before 点/时
        if let Some(h) = extract_last_number(before) {
            hour = Some(h);
        }
    }

    // Extract minute: N分 or 半
    if input.contains("半") {
        minute = 30;
    } else if let Some(pos) = input.find('分') {
        let before = &input[..pos];
        // Find number right before 分
        if let Some(m) = extract_last_number(before) {
            minute = m.min(59);
        }
    }

    // Adjust hour based on time period
    if let Some(mut h) = hour {
        if is_pm && h < 12 {
            h += 12;
        } else if is_early_morning && h > 6 {
            // 凌晨 typically means 0-6
        } else if is_noon && h < 12 {
            h = 12;
        }
        hour = Some(h.min(23));
    } else {
        // Default hours based on time period
        if is_early_morning {
            hour = Some(2);
        } else if is_morning {
            hour = Some(9);
        } else if is_noon {
            hour = Some(12);
        } else if is_pm {
            hour = Some(14);
        }
    }

    hour.map(|h| (h, minute))
}

/// Extract the last number from a string
fn extract_last_number(input: &str) -> Option<u32> {
    // Try Arabic numeral first (find last sequence of digits)
    let mut last_num: Option<u32> = None;
    let mut current_num = String::new();

    for c in input.chars() {
        if c.is_ascii_digit() {
            current_num.push(c);
        } else if !current_num.is_empty() {
            if let Ok(n) = current_num.parse() {
                last_num = Some(n);
            }
            current_num.clear();
        }
    }
    if !current_num.is_empty() {
        if let Ok(n) = current_num.parse() {
            last_num = Some(n);
        }
    }

    if last_num.is_some() {
        return last_num;
    }

    // Try Chinese numeral
    extract_chinese_number(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // English Tests
    // ========================================================================

    #[test]
    fn test_every_minute() {
        assert_eq!(parse_natural("every minute").unwrap(), "* * * * *");
        assert_eq!(parse_natural("minute").unwrap(), "* * * * *");
    }

    #[test]
    fn test_every_n_minutes() {
        assert_eq!(parse_natural("every 5 minutes").unwrap(), "*/5 * * * *");
        assert_eq!(parse_natural("every 15 minutes").unwrap(), "*/15 * * * *");
        assert_eq!(parse_natural("every 30 minutes").unwrap(), "*/30 * * * *");
    }

    #[test]
    fn test_every_hour() {
        assert_eq!(parse_natural("every hour").unwrap(), "0 * * * *");
        assert_eq!(parse_natural("hourly").unwrap(), "0 * * * *");
    }

    #[test]
    fn test_every_n_hours() {
        assert_eq!(parse_natural("every 2 hours").unwrap(), "0 */2 * * *");
        assert_eq!(parse_natural("every 6 hours").unwrap(), "0 */6 * * *");
    }

    #[test]
    fn test_daily() {
        assert_eq!(parse_natural("daily").unwrap(), "0 0 * * *");
        assert_eq!(parse_natural("every day").unwrap(), "0 0 * * *");
        assert_eq!(parse_natural("midnight").unwrap(), "0 0 * * *");
    }

    #[test]
    fn test_daily_at_time() {
        assert_eq!(parse_natural("daily at 2am").unwrap(), "0 2 * * *");
        assert_eq!(parse_natural("every day at 14:30").unwrap(), "30 14 * * *");
        assert_eq!(parse_natural("daily at 9pm").unwrap(), "0 21 * * *");
    }

    #[test]
    fn test_weekly() {
        assert_eq!(parse_natural("weekly").unwrap(), "0 0 * * 0");
        assert_eq!(parse_natural("every week").unwrap(), "0 0 * * 0");
    }

    #[test]
    fn test_weekday_patterns() {
        assert_eq!(parse_natural("every monday at 9am").unwrap(), "0 9 * * 1");
        assert_eq!(parse_natural("every friday at 5pm").unwrap(), "0 17 * * 5");
        assert_eq!(parse_natural("every sunday").unwrap(), "0 9 * * 0");
    }

    #[test]
    fn test_weekday_workday() {
        assert_eq!(
            parse_natural("every weekday at 8am").unwrap(),
            "0 8 * * 1-5"
        );
        assert_eq!(parse_natural("workday at 9:30").unwrap(), "30 9 * * 1-5");
    }

    #[test]
    fn test_weekend() {
        assert_eq!(
            parse_natural("every weekend at 10am").unwrap(),
            "0 10 * * 0,6"
        );
    }

    #[test]
    fn test_monthly() {
        assert_eq!(parse_natural("monthly").unwrap(), "0 0 1 * *");
        assert_eq!(parse_natural("every month").unwrap(), "0 0 1 * *");
        assert_eq!(parse_natural("monthly on the 15th").unwrap(), "0 0 15 * *");
        assert_eq!(
            parse_natural("every month on the 1st at 2am").unwrap(),
            "0 2 1 * *"
        );
    }

    #[test]
    fn test_yearly() {
        assert_eq!(parse_natural("yearly").unwrap(), "0 0 1 1 *");
        assert_eq!(parse_natural("annually").unwrap(), "0 0 1 1 *");
    }

    // ========================================================================
    // Chinese Tests
    // ========================================================================

    #[test]
    fn test_chinese_every_minute() {
        assert_eq!(parse_natural("每分钟").unwrap(), "* * * * *");
    }

    #[test]
    fn test_chinese_every_n_minutes() {
        assert_eq!(parse_natural("每5分钟").unwrap(), "*/5 * * * *");
        assert_eq!(parse_natural("每10分钟").unwrap(), "*/10 * * * *");
        assert_eq!(parse_natural("每30分钟").unwrap(), "*/30 * * * *");
    }

    #[test]
    fn test_chinese_every_hour() {
        assert_eq!(parse_natural("每小时").unwrap(), "0 * * * *");
    }

    #[test]
    fn test_chinese_every_n_hours() {
        assert_eq!(parse_natural("每2小时").unwrap(), "0 */2 * * *");
        assert_eq!(parse_natural("每6小时").unwrap(), "0 */6 * * *");
    }

    #[test]
    fn test_chinese_daily() {
        assert_eq!(parse_natural("每天凌晨2点").unwrap(), "0 2 * * *");
        assert_eq!(parse_natural("每天上午9点").unwrap(), "0 9 * * *");
        assert_eq!(parse_natural("每天下午3点").unwrap(), "0 15 * * *");
        assert_eq!(parse_natural("每天晚上8点").unwrap(), "0 20 * * *");
    }

    #[test]
    fn test_chinese_daily_with_minutes() {
        assert_eq!(parse_natural("每天上午9点30分").unwrap(), "30 9 * * *");
        assert_eq!(parse_natural("每天下午2点半").unwrap(), "30 14 * * *");
    }

    #[test]
    fn test_chinese_weekly() {
        assert_eq!(parse_natural("每周一上午9点").unwrap(), "0 9 * * 1");
        assert_eq!(parse_natural("每周五下午5点").unwrap(), "0 17 * * 5");
        assert_eq!(parse_natural("每周日").unwrap(), "0 9 * * 0");
        assert_eq!(parse_natural("每星期三下午3点").unwrap(), "0 15 * * 3");
    }

    #[test]
    fn test_chinese_workday() {
        assert_eq!(parse_natural("工作日上午9点").unwrap(), "0 9 * * 1-5");
    }

    #[test]
    fn test_chinese_weekend() {
        assert_eq!(parse_natural("周末上午10点").unwrap(), "0 10 * * 0,6");
    }

    #[test]
    fn test_chinese_monthly() {
        assert_eq!(parse_natural("每月1号").unwrap(), "0 0 1 * *");
        assert_eq!(parse_natural("每月15日凌晨2点").unwrap(), "0 2 15 * *");
    }

    // ========================================================================
    // Edge Cases
    // ========================================================================

    #[test]
    fn test_cron_passthrough() {
        assert_eq!(parse_natural("*/5 * * * *").unwrap(), "*/5 * * * *");
        assert_eq!(parse_natural("0 2 * * *").unwrap(), "0 2 * * *");
    }

    #[test]
    fn test_invalid_input() {
        assert!(parse_natural("invalid gibberish").is_err());
        assert!(parse_natural("").is_err());
    }

    #[test]
    fn test_case_insensitive() {
        assert_eq!(parse_natural("EVERY 5 MINUTES").unwrap(), "*/5 * * * *");
        assert_eq!(parse_natural("Daily At 2AM").unwrap(), "0 2 * * *");
    }

    // ========================================================================
    // Helper Function Tests
    // ========================================================================

    #[test]
    fn test_extract_chinese_number() {
        assert_eq!(extract_chinese_number("5分钟"), Some(5));
        assert_eq!(extract_chinese_number("十分钟"), Some(10));
        assert_eq!(extract_chinese_number("十五分钟"), Some(15));
        assert_eq!(extract_chinese_number("二十分钟"), Some(20));
    }

    #[test]
    fn test_extract_time_english() {
        assert_eq!(extract_time_english("at 14:30"), Some((14, 30)));
        assert_eq!(extract_time_english("at 2am"), Some((2, 0)));
        assert_eq!(extract_time_english("at 2pm"), Some((14, 0)));
        assert_eq!(extract_time_english("at 12am"), Some((0, 0)));
        assert_eq!(extract_time_english("at 12pm"), Some((12, 0)));
    }

    #[test]
    fn test_looks_like_cron() {
        assert!(looks_like_cron("* * * * *"));
        assert!(looks_like_cron("*/5 * * * *"));
        assert!(looks_like_cron("0 2 * * 1-5"));
        assert!(!looks_like_cron("every minute"));
        assert!(!looks_like_cron("* * *"));
    }
}
