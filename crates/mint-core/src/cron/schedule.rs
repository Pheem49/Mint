use std::str::FromStr;

use chrono::{DateTime, Datelike, NaiveDate, TimeZone, Timelike, Utc, Weekday};
use chrono_tz::Tz;
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

/// Cron's day-of-week field is 0=Sun..6=Sat.
fn cron_dow_to_weekday(cron_dow: i64) -> Result<Weekday, String> {
    Ok(match cron_dow {
        0 => Weekday::Sun,
        1 => Weekday::Mon,
        2 => Weekday::Tue,
        3 => Weekday::Wed,
        4 => Weekday::Thu,
        5 => Weekday::Fri,
        6 => Weekday::Sat,
        _ => return Err(format!("weekday {cron_dow} is out of range 0-6 (Sun-Sat)")),
    })
}

fn weekday_to_cron_dow(weekday: Weekday) -> i64 {
    // `num_days_from_sunday` is exactly cron's own 0=Sun..6=Sat numbering.
    weekday.num_days_from_sunday() as i64
}

fn parse_literal(field: &str) -> Option<i64> {
    field.parse::<i64>().ok()
}

/// `None` for `"*"`, `Some(values)` for a comma-separated list of literal
/// integers, `Some(Err)`-shaped by returning `None` for anything else
/// (ranges, steps, named values) — those aren't safe to shift by a UTC
/// offset field-by-field, so callers reject them instead of guessing.
fn parse_literal_list(field: &str) -> Option<Vec<i64>> {
    if field == "*" {
        return None;
    }
    field.split(',').map(parse_literal).collect()
}

/// Converts an hour:minute (and, depending on shape, weekday/day-of-month/
/// month/year) written in `tz`'s local wall-clock time into the equivalent
/// UTC cron expression the scheduler actually evaluates against (see
/// `next_run_after`/`crate::cron::scheduler::tick`, which always compares
/// against `Utc::now()`).
///
/// Only handles the same four shapes the web UI's schedule picker can
/// produce — daily (`M H * * *`), weekly (`M H * * <dow-list>`), monthly
/// (`M H <dom> * *`), and a fully-pinned one-time date — because a general
/// UTC-offset shift of `minute`/`hour`/`dom`/`dow` ranges or step values
/// (`*/15`, `1-5`, …) isn't a well-defined operation (a shifted range can
/// wrap or split unpredictably). Anything else is rejected with a message
/// telling the caller to write the schedule in UTC directly instead of
/// silently producing a schedule that doesn't mean what was typed.
pub fn localize_schedule(expression: &str, tz_name: &str, now: DateTime<Utc>) -> Result<String, String> {
    let tz: Tz = tz_name
        .parse()
        .map_err(|_| format!("unknown timezone {tz_name:?} (expected an IANA name like \"Asia/Bangkok\")"))?;

    let normalized = normalize(expression);
    let fields: Vec<&str> = normalized.split_whitespace().collect();
    let [sec, min, hour, dom, month, dow, year]: [&str; 7] = fields
        .try_into()
        .map_err(|_| format!("invalid cron expression {expression:?}"))?;

    let minute = parse_literal(min)
        .filter(|m| (0..60).contains(m))
        .ok_or_else(|| "timezone conversion needs a specific minute (no *, ranges, or steps)".to_string())?;
    let hour_val = parse_literal(hour)
        .filter(|h| (0..24).contains(h))
        .ok_or_else(|| "timezone conversion needs a specific hour (no *, ranges, or steps)".to_string())?;

    if dow != "*" && dom != "*" {
        return Err(
            "timezone conversion doesn't support a schedule with both a day-of-week and a day-of-month set"
                .to_string(),
        );
    }

    let localize = |naive_date: NaiveDate| -> Result<DateTime<Utc>, String> {
        let naive_dt = naive_date
            .and_hms_opt(hour_val as u32, minute as u32, 0)
            .ok_or_else(|| "invalid time of day".to_string())?;
        tz.from_local_datetime(&naive_dt)
            .single()
            .map(|dt| dt.with_timezone(&Utc))
            .ok_or_else(|| {
                "that local date/time doesn't exist or is ambiguous in this timezone (a DST transition) — pick a different time"
                    .to_string()
            })
    };

    // One-time: month and/or year pinned to a literal (dow/dom already
    // confirmed mutually exclusive above, so `dom` here is either the
    // pinned day or "*" for "same day every month", which "once" doesn't use).
    if month != "*" || year != "*" {
        let d = parse_literal(dom)
            .ok_or_else(|| "a one-time schedule needs a specific day-of-month".to_string())?;
        let mo = parse_literal(month)
            .ok_or_else(|| "a one-time schedule needs a specific month".to_string())?;
        let y = parse_literal(year)
            .ok_or_else(|| "a one-time schedule needs a specific year".to_string())?;
        let naive_date = NaiveDate::from_ymd_opt(y as i32, mo as u32, d as u32)
            .ok_or_else(|| format!("{y:04}-{mo:02}-{d:02} is not a valid date"))?;
        let utc = localize(naive_date)?;
        return Ok(format!(
            "{sec} {} {} {} {} * {}",
            utc.minute(),
            utc.hour(),
            utc.day(),
            utc.month(),
            utc.year()
        ));
    }

    let today = now.with_timezone(&tz).date_naive();

    if let Some(cron_dows) = parse_literal_list(dow) {
        let mut utc_hour = None;
        let mut utc_minute = None;
        let mut utc_dows = std::collections::BTreeSet::new();
        for cron_dow in cron_dows {
            let target_weekday = cron_dow_to_weekday(cron_dow)?;
            let days_ahead =
                (weekday_to_cron_dow(target_weekday) - weekday_to_cron_dow(today.weekday()) + 7) % 7;
            let ref_date = today + chrono::Duration::days(days_ahead);
            let utc = localize(ref_date)?;
            utc_hour = Some(utc.hour());
            utc_minute = Some(utc.minute());
            utc_dows.insert(weekday_to_cron_dow(utc.weekday()));
        }
        let dows_str = utc_dows
            .into_iter()
            .map(|d| d.to_string())
            .collect::<Vec<_>>()
            .join(",");
        return Ok(format!(
            "{sec} {} {} * * {dows_str} *",
            utc_minute.unwrap(),
            utc_hour.unwrap()
        ));
    }

    if dom != "*" {
        let d = parse_literal(dom)
            .filter(|d| (1..=31).contains(d))
            .ok_or_else(|| "timezone conversion needs a specific day-of-month (no *, ranges, or steps)".to_string())?;
        let naive_date = NaiveDate::from_ymd_opt(today.year(), today.month(), d as u32)
            .ok_or_else(|| format!("day {d} doesn't exist in the current month — try again next month, or write the schedule in UTC directly"))?;
        let utc = localize(naive_date)?;
        return Ok(format!(
            "{sec} {} {} {} * * *",
            utc.minute(),
            utc.hour(),
            utc.day()
        ));
    }

    // Daily: dow == "*" and dom == "*".
    let utc = localize(today)?;
    Ok(format!("{sec} {} {} * * * *", utc.minute(), utc.hour()))
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

    // 2026-08-16 12:00 UTC = a Sunday, used as `now` for the localize tests
    // below (any time of day works — only the calendar date matters).
    fn sunday_noon_utc() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 8, 16, 12, 0, 0).unwrap()
    }

    #[test]
    fn localize_daily_converts_bangkok_wall_clock_to_utc() {
        // 08:00 in Bangkok (UTC+7, no DST) is 01:00 UTC.
        let localized = localize_schedule("0 8 * * *", "Asia/Bangkok", sunday_noon_utc()).unwrap();
        assert_eq!(localized, "0 0 1 * * * *");
    }

    #[test]
    fn localize_weekly_rolls_the_weekday_back_across_midnight() {
        // Monday 00:30 in Bangkok (UTC+7) is Sunday 17:30 UTC — a different
        // calendar day *and* a different day-of-week from what was typed.
        let localized = localize_schedule("30 0 * * 1", "Asia/Bangkok", sunday_noon_utc()).unwrap();
        assert_eq!(localized, "0 30 17 * * 0 *");
    }

    #[test]
    fn localize_monthly_rolls_the_day_of_month_back_a_month_when_it_crosses_midnight() {
        // 00:30 on the 1st in Bangkok (UTC+7) is 17:30 UTC on the last day
        // of the *previous* month — `now` is fixed to August, so this
        // should land on July 31st.
        let localized = localize_schedule("30 0 1 * *", "Asia/Bangkok", sunday_noon_utc()).unwrap();
        assert_eq!(localized, "0 30 17 31 * * *");
    }

    #[test]
    fn localize_once_converts_a_specific_local_date_and_time_to_utc() {
        // 7-field order is sec min hour dom month dow year, so 22:20 is
        // `min=20 hour=22`, not `min=22 hour=20`.
        let localized =
            localize_schedule("0 20 22 16 8 * 2026", "Asia/Bangkok", sunday_noon_utc()).unwrap();
        // 2026-08-16 22:20 Bangkok -> 2026-08-16 15:20 UTC.
        assert_eq!(localized, "0 20 15 16 8 * 2026");
    }

    #[test]
    fn localize_accounts_for_dst_across_the_new_york_transition() {
        // 09:00 New York time in January is EST (UTC-5); in July it's EDT
        // (UTC-4) — the same wall-clock time yields a different UTC hour
        // depending on the calendar date, which is exactly what a naive
        // fixed-offset shift (rather than a real IANA timezone) would get
        // wrong.
        let january = Utc.with_ymd_and_hms(2026, 1, 15, 12, 0, 0).unwrap();
        let july = Utc.with_ymd_and_hms(2026, 7, 15, 12, 0, 0).unwrap();
        let jan_localized = localize_schedule("0 9 * * *", "America/New_York", january).unwrap();
        let jul_localized = localize_schedule("0 9 * * *", "America/New_York", july).unwrap();
        assert_eq!(jan_localized, "0 0 14 * * * *");
        assert_eq!(jul_localized, "0 0 13 * * * *");
    }

    #[test]
    fn localize_rejects_a_minute_range_or_step() {
        assert!(localize_schedule("*/15 8 * * *", "Asia/Bangkok", sunday_noon_utc()).is_err());
    }

    #[test]
    fn localize_rejects_combining_day_of_week_and_day_of_month() {
        assert!(localize_schedule("0 8 15 * 1", "Asia/Bangkok", sunday_noon_utc()).is_err());
    }

    #[test]
    fn localize_rejects_an_unknown_timezone() {
        assert!(localize_schedule("0 8 * * *", "Not/AZone", sunday_noon_utc()).is_err());
    }
}
