use std::str::FromStr;

use chrono::{DateTime, Utc};
use cron::Schedule;

/// The `cron` crate requires a 7-field expression (seconds, minutes, hours,
/// day-of-month, month, day-of-week, year) — seconds and year are not
/// optional. Users naturally type standard 5-field unix cron
/// ("min hour dom month dow") or 6-field quartz-style (with seconds but no
/// year), so this pads either of those up to 7 fields rather than forcing
/// everyone to type a trailing `*` for seconds/year every time.
fn normalize(expression: &str) -> String {
    let fields: Vec<&str> = expression.split_whitespace().collect();
    match fields.len() {
        5 => format!("0 {} *", fields.join(" ")),
        6 => format!("{} *", fields.join(" ")),
        _ => expression.trim().to_string(),
    }
}

/// Parses a cron expression, accepting 5-, 6-, or 7-field forms (see
/// [`normalize`]).
pub fn parse_schedule(expression: &str) -> Result<Schedule, String> {
    Schedule::from_str(&normalize(expression)).map_err(|e| e.to_string())
}

/// Computes the next run time strictly after `now`.
pub fn next_run_after(expression: &str, now: DateTime<Utc>) -> Result<DateTime<Utc>, String> {
    let schedule = parse_schedule(expression)?;
    schedule
        .after(&now)
        .next()
        .ok_or_else(|| "schedule has no upcoming run times".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn accepts_standard_five_field_unix_cron() {
        // Every day at 08:00.
        assert!(parse_schedule("0 8 * * *").is_ok());
    }

    #[test]
    fn accepts_six_and_seven_field_forms() {
        assert!(parse_schedule("0 0 8 * * *").is_ok());
        assert!(parse_schedule("0 0 8 * * * *").is_ok());
    }

    #[test]
    fn rejects_garbage_expressions() {
        assert!(parse_schedule("not a cron expression").is_err());
    }

    #[test]
    fn next_run_after_advances_to_the_following_occurrence() {
        let now = Utc.with_ymd_and_hms(2026, 8, 9, 8, 0, 0).unwrap();
        let next = next_run_after("0 8 * * *", now).unwrap();
        assert_eq!(next, Utc.with_ymd_and_hms(2026, 8, 10, 8, 0, 0).unwrap());
    }
}
