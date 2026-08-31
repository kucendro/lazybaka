mod config;
mod gcal;
mod sync;
mod timetable;

use anyhow::{Context, Result};
use chrono::{Duration as Delta, NaiveDate, Utc};
use config::Config;
use std::path::Path;
use std::process::ExitCode;
use std::time::{Duration, Instant};
use timetable::{Lesson, TZ};
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

const AGENT: &str = concat!("bakasync/", env!("CARGO_PKG_VERSION"));

#[tokio::main]
async fn main() -> ExitCode {
    if let Err(error) = load_env() {
        eprintln!("bakasync: {error:#}");
        return ExitCode::FAILURE;
    }
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_target(false)
        .init();
    let config = match Config::from_env() {
        Ok(config) => config,
        Err(error) => {
            error!("configuration: {error:#}");
            return ExitCode::FAILURE;
        }
    };
    loop {
        let outcome = run(&config).await;
        if let Err(error) = &outcome {
            error!("run failed: {error:#}");
        }
        match config.interval {
            Some(interval) => {
                info!("next run in {}", humantime::format_duration(interval));
                tokio::time::sleep(interval).await;
            }
            None if outcome.is_ok() => return ExitCode::SUCCESS,
            None => return ExitCode::FAILURE,
        }
    }
}

async fn run(config: &Config) -> Result<()> {
    let http = reqwest::Client::builder()
        .user_agent(AGENT)
        .timeout(Duration::from_secs(30))
        .build()?;
    let today = Utc::now().with_timezone(&TZ).date_naive();
    let mut lessons: Vec<Lesson> = Vec::new();
    let mut days: Vec<NaiveDate> = Vec::new();
    let mut stats = timetable::Stats::default();
    let urls = config.urls();
    for url in &urls {
        let html = fetch(&http, url).await?;
        if let Some(directory) = &config.dump_html_dir {
            dump(directory, url, &html)?;
        }
        let page = timetable::parse_page(&html, today).with_context(|| {
            format!("parsing {url} (set BAKASYNC_DUMP_HTML_DIR to keep the page)")
        })?;
        check_class_name(config, page.class_name.as_deref(), url);
        stats.atoms += page.stats.atoms;
        stats.removed += page.stats.removed;
        stats.absent += page.stats.absent;
        stats.unparsed += page.stats.unparsed;
        if page.known_days.is_empty() {
            warn!("{url}: no published day in this week, it is left untouched");
            continue;
        }
        let mut dropped = 0usize;
        for lesson in page.lessons {
            if page.known_days.contains(&lesson.start.date()) {
                lessons.push(lesson);
            } else {
                dropped += 1;
            }
        }
        if dropped > 0 {
            warn!("{url}: {dropped} lessons fall outside the published days and are ignored");
        }
        days.extend(page.known_days);
    }
    info!(
        "scrape: {} urls fetched, {} days published",
        urls.len(),
        days.len()
    );
    if stats.unparsed > 0 {
        warn!("{} lessons could not be read from the page", stats.unparsed);
    }
    let scraped = lessons.len();
    lessons.retain(|lesson| timetable::keep_group(lesson.group.as_deref(), &config.groups));
    let kept = lessons.len();
    let desired = timetable::merge(lessons, config.merge_gap_minutes);
    info!(
        "parse: {} atoms, {} removed, {} absent(skipped); {} of {} for groups {:?}; merged -> {} lessons",
        stats.atoms,
        stats.removed,
        stats.absent,
        kept,
        scraped,
        config.groups,
        desired.len()
    );
    let (from, to) = match window(&days) {
        Some(window) => window,
        None => {
            warn!("no published week, nothing to sync");
            return Ok(());
        }
    };
    let mut calendar = gcal::Calendar::new(
        http,
        config.calendar_id.clone(),
        &config.service_account_key,
    )?;
    let existing = calendar.list(&from, &to).await?;
    let plan = sync::plan(&desired, &existing);
    info!(
        "diff: window={from} .. {to} existing={} insert={} patch={} delete={}",
        existing.len(),
        plan.inserts.len(),
        plan.patches.len(),
        plan.deletes.len()
    );
    if plan.is_empty() {
        info!("gcal: already up to date");
        return Ok(());
    }
    let started = Instant::now();
    sync::apply(&mut calendar, &plan, config.dry_run).await?;
    info!("gcal: applied in {:.1?}", started.elapsed());
    Ok(())
}

async fn fetch(http: &reqwest::Client, url: &str) -> Result<String> {
    let response = http
        .get(url)
        .send()
        .await
        .with_context(|| format!("fetching {url}"))?
        .error_for_status()
        .with_context(|| format!("fetching {url}"))?;
    response
        .text()
        .await
        .with_context(|| format!("reading {url}"))
}

fn dump(directory: &Path, url: &str, html: &str) -> Result<()> {
    std::fs::create_dir_all(directory)
        .with_context(|| format!("creating {}", directory.display()))?;
    let name: String = url
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '_'
            }
        })
        .collect();
    let path = directory.join(format!("{name}.html"));
    std::fs::write(&path, html).with_context(|| format!("writing {}", path.display()))?;
    info!("dumped {url} to {}", path.display());
    Ok(())
}

fn check_class_name(config: &Config, actual: Option<&str>, url: &str) {
    match (config.expected_class_name.as_deref(), actual) {
        (_, None) => warn!("{url}: class name not found on the page"),
        (Some(expected), Some(actual)) if expected != actual => warn!(
            "{url}: page shows class {actual:?} but BAKASYNC_EXPECTED_CLASS_NAME is {expected:?}, the class id is probably outdated"
        ),
        (Some(_), Some(actual)) => info!("{url}: class {actual:?} ok"),
        (None, Some(actual)) => info!("{url}: class {actual:?}"),
    }
}

fn window(days: &[NaiveDate]) -> Option<(String, String)> {
    let first = days.iter().min()?;
    let last = days.iter().max()?;
    let from = sync::rfc3339(first.and_hms_opt(0, 0, 0)?);
    let to = sync::rfc3339((*last + Delta::days(1)).and_hms_opt(0, 0, 0)?);
    Some((from, to))
}

fn load_env() -> Result<()> {
    match std::env::var("BAKASYNC_ENV_FILE") {
        Ok(path) if !path.trim().is_empty() => {
            dotenvy::from_path(path.trim()).with_context(|| format!("loading env file {path}"))?;
        }
        _ => {
            let _ = dotenvy::from_filename(".env.local");
            let _ = dotenvy::dotenv();
        }
    }
    Ok(())
}
