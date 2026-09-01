use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::time::Duration;

const KEY_FILE: &str = "service-account.json";
const USER_FILE: &str = "config.env";

pub struct Located {
    pub files: Vec<PathBuf>,
    pub dir: PathBuf,
}

impl Located {
    fn here() -> Self {
        Located {
            files: Vec::new(),
            dir: PathBuf::from("."),
        }
    }

    pub fn resolve(&self, path: &Path) -> PathBuf {
        if let (Ok(rest), Some(home)) = (path.strip_prefix("~"), home()) {
            return home.join(rest);
        }
        if path.is_absolute() || self.dir == Path::new(".") {
            path.to_path_buf()
        } else {
            self.dir.join(path)
        }
    }
}

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
    pub fn from_env(located: &Located) -> Result<Self> {
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
            base_url: required("BAKASYNC_BASE_URL")?
                .trim_end_matches('/')
                .to_string(),
            class_id: required("BAKASYNC_CLASS_ID")?,
            weeks,
            expected_class_name: optional("BAKASYNC_EXPECTED_CLASS_NAME"),
            groups: list("BAKASYNC_GROUPS"),
            calendar_id: required("BAKASYNC_CALENDAR_ID")?,
            service_account_key: key_path(located)?,
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

pub fn home() -> Option<PathBuf> {
    ["HOME", "USERPROFILE"]
        .into_iter()
        .filter_map(std::env::var_os)
        .find(|value| !value.is_empty())
        .map(PathBuf::from)
}

pub fn user_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("APPDATA")
            .filter(|value| !value.is_empty())
            .map(|value| PathBuf::from(value).join("bakasync"))
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("XDG_CONFIG_HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .or_else(|| home().map(|value| value.join(".config")))
            .map(|value| value.join("bakasync"))
    }
}

pub fn user_file() -> Option<PathBuf> {
    user_dir().map(|dir| dir.join(USER_FILE))
}

pub fn locate(explicit: Option<&Path>) -> Result<Located> {
    if let Some(path) = explicit {
        return load(path);
    }
    if let Some(path) = optional("BAKASYNC_ENV_FILE") {
        return load(Path::new(&path));
    }
    let here = [PathBuf::from(".env.local"), PathBuf::from(".env")];
    if here.iter().any(|path| path.is_file()) {
        let mut located = Located::here();
        for path in here {
            if path.is_file() {
                dotenvy::from_path(&path).with_context(|| format!("loading {}", path.display()))?;
                located.files.push(path);
            }
        }
        return Ok(located);
    }
    match user_file() {
        Some(path) if path.is_file() => load(&path),
        _ => Ok(Located::here()),
    }
}

fn load(path: &Path) -> Result<Located> {
    if !path.is_file() {
        bail!("no config file at {}", path.display());
    }
    dotenvy::from_path(path).with_context(|| format!("loading {}", path.display()))?;
    let dir = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
        _ => PathBuf::from("."),
    };
    Ok(Located {
        files: vec![path.to_path_buf()],
        dir,
    })
}

fn key_path(located: &Located) -> Result<PathBuf> {
    match optional("BAKASYNC_SERVICE_ACCOUNT_KEY") {
        Some(value) => Ok(located.resolve(Path::new(&value))),
        None if !located.files.is_empty() => Ok(located.dir.join(KEY_FILE)),
        None => bail!("BAKASYNC_SERVICE_ACCOUNT_KEY is not set"),
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
