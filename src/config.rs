use anyhow::{bail, Context, Result};
use std::path::PathBuf;
use std::time::Duration;

pub struct Config {
    pub base_url: String,
    pub class_id: String,
    pub weeks: Vec<String>,
    pub expected_class_name: Option<String>,
    pub groups: Vec<String>,
    pub calendar_id: String,
    pub service_account_key: PathBuf,
    pub merge_gap_minutes: i64,
    pub interval: Option<Duration>,
    pub dry_run: bool,
    pub dump_html_dir: Option<PathBuf>,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let weeks = list("BAKASYNC_WEEKS");
        let weeks = if weeks.is_empty() {
            vec!["Actual".to_string(), "Next".to_string()]
        } else {
            weeks
        };
        let interval = match optional("BAKASYNC_INTERVAL") {
            None => None,
            Some(raw) => {
                let parsed = humantime::parse_duration(&raw)
                    .with_context(|| format!("BAKASYNC_INTERVAL is not a duration: {raw}"))?;
                if parsed.is_zero() {
                    None
                } else {
                    Some(parsed)
                }
            }
        };
        let cfg = Config {
            base_url: optional("BAKASYNC_BASE_URL")
                .unwrap_or_else(|| "https://bakalari.spse.cz/bakaweb".to_string())
                .trim_end_matches('/')
                .to_string(),
            class_id: required("BAKASYNC_CLASS_ID")?,
            weeks,
            expected_class_name: optional("BAKASYNC_EXPECTED_CLASS_NAME"),
            groups: list("BAKASYNC_GROUPS"),
            calendar_id: required("BAKASYNC_CALENDAR_ID")?,
            service_account_key: PathBuf::from(required("BAKASYNC_SERVICE_ACCOUNT_KEY")?),
            merge_gap_minutes: number("BAKASYNC_MERGE_GAP_MINUTES", 25)?,
            interval,
            dry_run: flag("BAKASYNC_DRY_RUN"),
            dump_html_dir: optional("BAKASYNC_DUMP_HTML_DIR").map(PathBuf::from),
        };
        if cfg.merge_gap_minutes < 0 {
            bail!("BAKASYNC_MERGE_GAP_MINUTES must not be negative");
        }
        Ok(cfg)
    }

    pub fn urls(&self) -> Vec<String> {
        self.weeks
            .iter()
            .map(|week| {
                format!(
                    "{}/Timetable/Public/{}/Class/{}",
                    self.base_url, week, self.class_id
                )
            })
            .collect()
    }
}

fn optional(key: &str) -> Option<String> {
    match std::env::var(key) {
        Ok(value) if !value.trim().is_empty() => Some(value.trim().to_string()),
        _ => None,
    }
}

fn required(key: &str) -> Result<String> {
    optional(key).with_context(|| format!("{key} is not set"))
}

fn list(key: &str) -> Vec<String> {
    optional(key)
        .map(|value| {
            value
                .split(',')
                .map(|part| part.trim().to_string())
                .filter(|part| !part.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn number(key: &str, default: i64) -> Result<i64> {
    match optional(key) {
        None => Ok(default),
        Some(value) => value
            .parse()
            .with_context(|| format!("{key} is not a number: {value}")),
    }
}

fn flag(key: &str) -> bool {
    matches!(
        optional(key).map(|v| v.to_ascii_lowercase()).as_deref(),
        Some("1" | "true" | "yes" | "on")
    )
}
