use crate::config::{self, Config, Located};
use crate::gcal::{self, ApiError, Calendar, Event};
use crate::sync::{self, Plan};
use crate::timetable::{self, Lesson, TZ};
use anyhow::{Context, Result};
use chrono::{Duration as Delta, Utc};
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Parser)]
#[command(name = "bakasync", version, about)]
pub struct Cli {
    #[arg(
        long,
        value_name = "PATH",
        global = true,
        help = "Config file to read instead of the usual places"
    )]
    pub config: Option<PathBuf>,

    #[arg(
        long,
        conflicts_with = "plan",
        help = "Sync once and exit, ignoring BAKASYNC_INTERVAL"
    )]
    pub once: bool,

    #[arg(long, help = "Print what the next sync would change, write nothing")]
    pub plan: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    #[command(about = "Create the config file and say where the key belongs")]
    Init,
    #[command(about = "Check the config, the key, the timetable and the calendar")]
    Doctor,
}

pub fn init(explicit: Option<&Path>) -> Result<()> {
    let path = match explicit {
        Some(path) => path.to_path_buf(),
        None => config::user_file().context("no home directory to put the config in")?,
    };
    let dir = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
        _ => PathBuf::from("."),
    };
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    let state = if path.exists() {
        "already there, left alone"
    } else {
        std::fs::write(&path, template()).with_context(|| format!("writing {}", path.display()))?;
        "created"
    };
    let key = dir.join("service-account.json");
    println!("config  {}  ({state})", pretty(&path));
    println!(
        "key     {}  ({})",
        pretty(&key),
        if key.is_file() { "ok" } else { "missing" }
    );
    println!();
    if !key.is_file() {
        println!("Download the service account key from GCP and save it");
        println!("to the path above, then run: bakasync doctor");
    } else {
        println!("Fill in the config, then run: bakasync doctor");
    }
    Ok(())
}

pub async fn doctor(located: &Located) -> Result<bool> {
    let mut ok = true;
    if located.files.is_empty() {
        let expected = config::user_file()
            .map(|path| format!(", expected {} (run: bakasync init)", pretty(&path)))
            .unwrap_or_default();
        line("config", &format!("none, using the environment{expected}"));
    } else {
        let files: Vec<String> = located.files.iter().map(|path| pretty(path)).collect();
        line("config", &files.join(", "));
    }
    let config = match Config::from_env(located) {
        Ok(config) => config,
        Err(error) => {
            line("settings", &format!("FAILED: {error:#}"));
            return Ok(false);
        }
    };
    match gcal::key_owner(&config.service_account_key) {
        Ok(email) => {
            let note = if exposed(&config.service_account_key) {
                " (readable by others, chmod 600)"
            } else {
                ""
            };
            line("key", &format!("ok, {email}{note}"));
        }
        Err(error) => {
            ok = false;
            line("key", &format!("FAILED: {error:#}"));
        }
    }
    let http = reqwest::Client::builder()
        .user_agent(crate::AGENT)
        .timeout(Duration::from_secs(30))
        .build()?;
    match timetable(&http, &config).await {
        Ok(message) => line("timetable", &message),
        Err(error) => {
            ok = false;
            line("timetable", &format!("FAILED: {error:#}"));
        }
    }
    match calendar(&http, &config).await {
        Ok(count) => line(
            "calendar",
            &format!("ok, {count} managed events in the next week"),
        ),
        Err(error) => {
            ok = false;
            line("calendar", &format!("FAILED: {error:#}"));
            if let Some(hint) = hint(&error) {
                line("", hint);
            }
        }
    }
    println!();
    if ok {
        println!("all good, run `bakasync --plan` to see what the next sync would change");
    }
    Ok(ok)
}

pub fn print_plan(plan: &Plan, existing: &[Event]) {
    if plan.is_empty() {
        println!("nothing to do, the calendar is up to date");
        return;
    }
    for lesson in &plan.inserts {
        println!("insert  {}", describe(lesson));
    }
    for (_, lesson) in &plan.patches {
        println!("patch   {}", describe(lesson));
    }
    for id in &plan.deletes {
        match existing.iter().find(|event| &event.id == id) {
            Some(event) => println!("delete  {}", describe_event(event)),
            None => println!("delete  {id}"),
        }
    }
    println!();
    println!(
        "{} insert, {} patch, {} delete; nothing was written",
        plan.inserts.len(),
        plan.patches.len(),
        plan.deletes.len()
    );
}

async fn timetable(http: &reqwest::Client, config: &Config) -> Result<String> {
    let url = config
        .urls()
        .first()
        .cloned()
        .context("no week to fetch, BAKASYNC_WEEKS is empty")?;
    let html = crate::fetch(http, &url).await?;
    let today = Utc::now().with_timezone(&TZ).date_naive();
    let page = timetable::parse_page(&html, today).with_context(|| format!("parsing {url}"))?;
    let name = match &page.class_name {
        None => anyhow::bail!("{url} shows no class, the class id is probably wrong"),
        Some(name) => name,
    };
    if let Some(expected) = &config.expected_class_name {
        if expected != name {
            anyhow::bail!(
                "page shows class {name:?} but BAKASYNC_EXPECTED_CLASS_NAME is {expected:?}, the class id is probably outdated"
            );
        }
    }
    let unparsed = match page.stats.unparsed {
        0 => String::new(),
        count => format!(", {count} could not be read"),
    };
    Ok(format!(
        "ok, class {name:?}, {} lessons{unparsed}",
        page.lessons.len()
    ))
}

async fn calendar(http: &reqwest::Client, config: &Config) -> Result<usize> {
    let mut calendar = Calendar::new(
        http.clone(),
        config.calendar_id.clone(),
        &config.service_account_key,
    )?;
    let today = Utc::now().with_timezone(&TZ).date_naive();
    let midnight = |day: chrono::NaiveDate| -> Result<String> {
        Ok(sync::rfc3339(day.and_hms_opt(0, 0, 0).context("bad day")?))
    };
    let events = calendar
        .list(&midnight(today)?, &midnight(today + Delta::days(7))?)
        .await?;
    Ok(events.len())
}

fn hint(error: &anyhow::Error) -> Option<&'static str> {
    let api = error.downcast_ref::<ApiError>()?;
    match api.status.as_u16() {
        404 => Some("the calendar id is wrong, or it is not shared with the service account"),
        403 => Some("share the calendar with the service account as \"Make changes to events\", and enable the Calendar API"),
        _ => None,
    }
}

fn template() -> String {
    include_str!("../.env.example")
        .lines()
        .filter(|line| !line.starts_with("BAKASYNC_SERVICE_ACCOUNT_KEY="))
        .map(|line| format!("{line}\n"))
        .collect()
}

fn line(label: &str, text: &str) {
    println!("{label:<10}{text}");
}

fn pretty(path: &Path) -> String {
    let text = path.display().to_string();
    match config::home() {
        Some(home) => match text.strip_prefix(&home.display().to_string()) {
            Some(rest) => format!("~{rest}"),
            None => text,
        },
        None => text,
    }
}

fn exposed(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path)
            .map(|data| data.permissions().mode() & 0o077 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        false
    }
}

fn describe(lesson: &Lesson) -> String {
    format!(
        "{}-{}  {:<26}  {:<6}  {}",
        lesson.start.format("%a %d.%m %H:%M"),
        lesson.end.format("%H:%M"),
        lesson.summary(),
        lesson.room.clone().unwrap_or_default(),
        lesson.group.clone().unwrap_or_default()
    )
    .trim_end()
    .to_string()
}

fn describe_event(event: &Event) -> String {
    let when = match gcal::parse_instant(&event.start) {
        Some(start) => start
            .with_timezone(&TZ)
            .format("%a %d.%m %H:%M")
            .to_string(),
        None => "?".to_string(),
    };
    format!(
        "{when}        {}",
        event.summary.clone().unwrap_or_default()
    )
    .trim_end()
    .to_string()
}
