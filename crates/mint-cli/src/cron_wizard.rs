//! Interactive `/cron add` / `mint cron add` wizard — walks through the same
//! four schedule shapes the web UI's picker form offers (daily/weekly/
//! monthly/one-time, plus a raw-cron escape hatch) instead of requiring
//! everyone to hand-write a pipe-delimited or `--schedule` cron expression.
//! Reuses the onboarding flow's own prompt helpers (`prompt_choice`,
//! `prompt_input`) rather than inventing a second set.

use anyhow::Result;
use mint_core::CronJob;
use std::path::{Path, PathBuf};

use crate::onboard::{prompt_choice, prompt_input};

const WEEKDAY_NAMES: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

fn parse_time(input: &str) -> Result<(u32, u32), String> {
    let (h, m) = input
        .split_once(':')
        .ok_or_else(|| "expected HH:MM, e.g. 08:00".to_string())?;
    let hour: u32 = h
        .trim()
        .parse()
        .map_err(|_| "hour must be a number".to_string())?;
    let minute: u32 = m
        .trim()
        .parse()
        .map_err(|_| "minute must be a number".to_string())?;
    if hour > 23 || minute > 59 {
        return Err("hour must be 0-23 and minute 0-59".to_string());
    }
    Ok((hour, minute))
}

/// Accepts weekday names (`Mon`, `Tuesday`, case-insensitive, any
/// unambiguous prefix of 3+ letters) or literal cron digits (0=Sun..6=Sat),
/// comma-separated.
fn parse_weekdays(input: &str) -> Result<Vec<i64>, String> {
    input
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| {
            if let Ok(n) = part.parse::<i64>() {
                if (0..=6).contains(&n) {
                    return Ok(n);
                }
                return Err(format!("{part:?} is out of range 0-6 (Sun-Sat)"));
            }
            let lower = part.to_ascii_lowercase();
            WEEKDAY_NAMES
                .iter()
                .position(|name| {
                    name.to_ascii_lowercase().starts_with(&lower)
                        || lower.starts_with(&name.to_ascii_lowercase())
                })
                .map(|idx| idx as i64)
                .ok_or_else(|| format!("{part:?} isn't a day of the week"))
        })
        .collect()
}

fn detect_default_timezone() -> String {
    iana_time_zone::get_timezone().unwrap_or_else(|_| "UTC".to_string())
}

/// Prompts for a value, calling `parse` on each answer and re-prompting
/// (with `parse`'s error message shown) until it succeeds.
fn prompt_until_valid<T>(
    label: &str,
    default: Option<&str>,
    parse: impl Fn(&str) -> Result<T, String>,
) -> Result<T> {
    loop {
        let answer = prompt_input(label, default)?;
        match parse(&answer) {
            Ok(value) => return Ok(value),
            Err(message) => println!("  \x1b[31m{message}\x1b[0m"),
        }
    }
}

fn prompt_required(label: &str) -> Result<String> {
    loop {
        let answer = prompt_input(label, None)?;
        if !answer.trim().is_empty() {
            return Ok(answer.trim().to_string());
        }
        println!("  \x1b[31mrequired\x1b[0m");
    }
}

/// Runs the interactive wizard and creates the job. `default_workspace` is
/// offered as the workspace path's default (Enter to accept). Returns the
/// created job, or an error if the user cancelled (Ctrl+C, surfaced via the
/// shared `prompt_choice`/`prompt_input` helpers) or the store write failed.
pub fn run_add_wizard(
    cron_jobs: &mint_core::CronStore,
    default_workspace: &Path,
) -> Result<CronJob> {
    println!("\x1b[1mNew scheduled task\x1b[0m");
    let name = prompt_required("Name")?;

    let repeat_options = [
        "Daily".to_string(),
        "Weekly".to_string(),
        "Monthly".to_string(),
        "One-time".to_string(),
        "Custom (type a cron expression directly, in UTC)".to_string(),
    ];
    let repeat_mode = prompt_choice("How often should this run?", &repeat_options, 0)?;

    let schedule = if repeat_mode.starts_with("Custom") {
        prompt_required("Cron expression (UTC — 5, 6, or 7 fields, e.g. \"0 8 * * *\")")?
    } else {
        let (hour, minute) = prompt_until_valid("Time (HH:MM, local)", Some("08:00"), parse_time)?;

        let raw = if repeat_mode == "Daily" {
            format!("{minute} {hour} * * *")
        } else if repeat_mode == "Weekly" {
            let dows = prompt_until_valid(
                "Days (e.g. Mon,Wed,Fri — or 0-6 with Sun=0)",
                Some("Mon"),
                parse_weekdays,
            )?;
            let dows_str = dows
                .iter()
                .map(|d| d.to_string())
                .collect::<Vec<_>>()
                .join(",");
            format!("{minute} {hour} * * {dows_str}")
        } else if repeat_mode == "Monthly" {
            let day = prompt_until_valid("Day of month (1-31)", Some("1"), |s| {
                s.trim()
                    .parse::<i64>()
                    .ok()
                    .filter(|d| (1..=31).contains(d))
                    .ok_or_else(|| "must be a number 1-31".to_string())
            })?;
            format!("{minute} {hour} {day} * *")
        } else {
            // One-time.
            let (year, month, day) = prompt_until_valid("Date (YYYY-MM-DD)", None, |s| {
                let parts: Vec<&str> = s.trim().split('-').collect();
                let [y, mo, d] = parts.as_slice() else {
                    return Err("expected YYYY-MM-DD".to_string());
                };
                let y: i64 = y.parse().map_err(|_| "invalid year".to_string())?;
                let mo: i64 = mo.parse().map_err(|_| "invalid month".to_string())?;
                let d: i64 = d.parse().map_err(|_| "invalid day".to_string())?;
                Ok((y, mo, d))
            })?;
            format!("0 {minute} {hour} {day} {month} * {year}")
        };

        let default_tz = detect_default_timezone();
        let tz = prompt_until_valid("Timezone (IANA name)", Some(&default_tz), |s| {
            s.parse::<chrono_tz::Tz>()
                .map(|_| s.trim().to_string())
                .map_err(|_| {
                    "not a recognized IANA timezone (e.g. \"Asia/Bangkok\", \"America/New_York\")"
                        .to_string()
                })
        })?;

        mint_core::localize_schedule(&raw, &tz, chrono::Utc::now())
            .map_err(|e| anyhow::anyhow!("couldn't convert to UTC: {e}"))?
    };

    let task = prompt_required("Task / prompt for the agent")?;
    let workspace_str = prompt_input("Workspace path", Some(&default_workspace.to_string_lossy()))?;
    let workspace = PathBuf::from(workspace_str);

    let job = cron_jobs
        .add(name, schedule, task, workspace)
        .map_err(|e| anyhow::anyhow!(e))?;
    Ok(job)
}
