use anyhow::{anyhow, bail, Context, Result};
use chrono::{Datelike, Duration as Delta, NaiveDate, NaiveDateTime, NaiveTime};
use chrono_tz::Tz;
use scraper::{Html, Selector};
use serde::Deserialize;
use sha1::{Digest, Sha1};

pub const TZ: Tz = chrono_tz::Europe::Prague;

const MODEL_MARKER: &str = "const timetableData = ";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lesson {
    pub key: String,
    pub subject: String,
    pub teacher: Option<String>,
    pub room: Option<String>,
    pub group: Option<String>,
    pub change: Option<String>,
    pub theme: Option<String>,
    pub start: NaiveDateTime,
    pub end: NaiveDateTime,
}

impl Lesson {
    pub fn summary(&self) -> String {
        match &self.change {
            Some(_) => format!("{} \u{26a0}", self.subject),
            None => self.subject.clone(),
        }
    }

    pub fn location(&self) -> String {
        self.room.clone().unwrap_or_default()
    }

    pub fn description(&self) -> String {
        [&self.teacher, &self.group, &self.change, &self.theme]
            .into_iter()
            .flatten()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(" | ")
    }

    fn identity(&self) -> (&str, &str, &str, &str, &str) {
        (
            &self.subject,
            text(&self.teacher),
            text(&self.room),
            text(&self.group),
            text(&self.change),
        )
    }

    fn assign_key(&mut self) {
        let material = format!(
            "{}|{}|{}",
            self.start.format("%Y-%m-%dT%H:%M:%S"),
            self.subject,
            text(&self.group)
        );
        let mut hasher = Sha1::new();
        hasher.update(material.as_bytes());
        self.key = hex::encode(hasher.finalize());
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub atoms: usize,
    pub removed: usize,
    pub absent: usize,
    pub unparsed: usize,
    pub unknown_days: usize,
}

#[derive(Debug)]
pub struct Page {
    pub class_name: Option<String>,
    pub known_days: Vec<NaiveDate>,
    pub lessons: Vec<Lesson>,
    pub stats: Stats,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Model {
    #[serde(default)]
    days: Vec<RawDay>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawDay {
    #[serde(default)]
    date: Option<String>,
    #[serde(default)]
    day_off: bool,
    #[serde(default)]
    hours: Option<Vec<RawCell>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawCell {
    #[serde(default)]
    r#type: Option<String>,
    #[serde(default)]
    atoms: Option<Vec<RawAtom>>,
    #[serde(default)]
    begin: Option<String>,
    #[serde(default)]
    end: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawAtom {
    #[serde(default)]
    r#type: Option<String>,
    #[serde(default)]
    subject_text: Option<String>,
    #[serde(default)]
    subject_abbrev: Option<String>,
    #[serde(default)]
    teacher_fullname: Option<String>,
    #[serde(default)]
    teacher: Option<String>,
    #[serde(default)]
    room: Option<String>,
    #[serde(default)]
    room_full_name: Option<String>,
    #[serde(default)]
    groups_names: Option<String>,
    #[serde(default)]
    groups_full_names: Option<String>,
    #[serde(default)]
    theme: Option<String>,
    #[serde(default)]
    change_info: Option<String>,
    #[serde(default)]
    day: Option<String>,
    #[serde(default)]
    begin: Option<String>,
    #[serde(default)]
    end: Option<String>,
}

pub fn parse_page(html: &str, today: NaiveDate) -> Result<Page> {
    let model: Model =
        serde_json::from_str(extract_model(html)?).context("timetableData is not valid JSON")?;
    let mut page = Page {
        class_name: parse_class_name(html),
        known_days: Vec::new(),
        lessons: Vec::new(),
        stats: Stats::default(),
    };
    for day in &model.days {
        let date = day
            .date
            .as_deref()
            .and_then(parse_day_month)
            .and_then(|(d, m)| infer_year(d, m, today));
        match (day.day_off || day.hours.is_some(), date) {
            (true, Some(date)) => page.known_days.push(date),
            _ => page.stats.unknown_days += 1,
        }
        for cell in day.hours.iter().flatten() {
            read_cell(cell, date, &mut page);
        }
    }
    Ok(page)
}

fn read_cell(cell: &RawCell, day_date: Option<NaiveDate>, page: &mut Page) {
    let atoms = cell.atoms.as_deref().unwrap_or_default();
    if atoms.is_empty() {
        match cell.r#type.as_deref() {
            Some("removed") => page.stats.removed += 1,
            Some("absent") => page.stats.absent += 1,
            _ => {}
        }
        return;
    }
    for atom in atoms {
        match atom.r#type.as_deref() {
            Some("removed") => page.stats.removed += 1,
            Some("absent") => page.stats.absent += 1,
            _ => {
                page.stats.atoms += 1;
                match build_lesson(atom, cell, day_date) {
                    Some(lesson) => page.lessons.push(lesson),
                    None => page.stats.unparsed += 1,
                }
            }
        }
    }
}

fn build_lesson(atom: &RawAtom, cell: &RawCell, day_date: Option<NaiveDate>) -> Option<Lesson> {
    let subject = first_text([&atom.subject_text, &atom.subject_abbrev])?;
    let date = atom.day.as_deref().and_then(parse_full_date).or(day_date)?;
    let start = parse_time(first_text([&atom.begin, &cell.begin])?.as_str())?;
    let end = parse_time(first_text([&atom.end, &cell.end])?.as_str())?;
    if end <= start {
        return None;
    }
    Some(Lesson {
        key: String::new(),
        subject,
        teacher: first_text([&atom.teacher_fullname, &atom.teacher]),
        room: first_text([&atom.room, &atom.room_full_name]),
        group: first_text([&atom.groups_names, &atom.groups_full_names]),
        change: first_text([&atom.change_info]),
        theme: first_text([&atom.theme]),
        start: date.and_time(start),
        end: date.and_time(end),
    })
}

pub fn keep_group(group: Option<&str>, wanted: &[String]) -> bool {
    let group = fold(group.unwrap_or_default());
    if group.is_empty() || group.starts_with("cel") || wanted.is_empty() {
        return true;
    }
    wanted
        .iter()
        .map(|w| fold(w))
        .any(|w| !w.is_empty() && group.contains(&w))
}

pub fn merge(mut lessons: Vec<Lesson>, gap_minutes: i64) -> Vec<Lesson> {
    lessons.sort_by(|a, b| {
        a.identity()
            .cmp(&b.identity())
            .then(a.start.cmp(&b.start))
            .then(a.end.cmp(&b.end))
    });
    let gap = Delta::minutes(gap_minutes);
    let mut merged: Vec<Lesson> = Vec::new();
    for lesson in lessons {
        match merged.last_mut() {
            Some(previous)
                if previous.identity() == lesson.identity()
                    && previous.end.date() == lesson.start.date()
                    && lesson.start >= previous.end
                    && lesson.start - previous.end <= gap =>
            {
                previous.end = previous.end.max(lesson.end);
            }
            _ => merged.push(lesson),
        }
    }
    merged.sort_by(|a, b| a.start.cmp(&b.start).then(a.subject.cmp(&b.subject)));
    for lesson in &mut merged {
        lesson.assign_key();
    }
    merged
}

fn extract_model(html: &str) -> Result<&str> {
    let marker = html
        .find(MODEL_MARKER)
        .ok_or_else(|| anyhow!("timetableData not found in page"))?;
    let rest = &html[marker + MODEL_MARKER.len()..];
    let open = rest
        .find('{')
        .ok_or_else(|| anyhow!("timetableData is not an object"))?;
    let bytes = rest.as_bytes();
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for index in open..bytes.len() {
        let byte = bytes[index];
        if in_string {
            match byte {
                _ if escaped => escaped = false,
                b'\\' => escaped = true,
                b'"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Ok(&rest[open..=index]);
                }
            }
            _ => {}
        }
    }
    bail!("timetableData object is not terminated")
}

fn parse_class_name(html: &str) -> Option<String> {
    let selector = Selector::parse("select#selectedClass option[selected]").ok()?;
    let text = Html::parse_document(html)
        .select(&selector)
        .next()?
        .text()
        .collect::<String>()
        .trim()
        .to_string();
    Some(text).filter(|value| !value.is_empty())
}

fn parse_day_month(value: &str) -> Option<(u32, u32)> {
    let mut parts = value.trim().split('.');
    let day = parts.next()?.trim().parse().ok()?;
    let month = parts.next()?.trim().parse().ok()?;
    Some((day, month))
}

fn parse_full_date(value: &str) -> Option<NaiveDate> {
    let mut parts = value.trim().split('.');
    let day = parts.next()?.trim().parse().ok()?;
    let month = parts.next()?.trim().parse().ok()?;
    let year = parts.next()?.split_whitespace().next()?.parse().ok()?;
    NaiveDate::from_ymd_opt(year, month, day)
}

fn infer_year(day: u32, month: u32, today: NaiveDate) -> Option<NaiveDate> {
    (today.year() - 1..=today.year() + 1)
        .filter_map(|year| NaiveDate::from_ymd_opt(year, month, day))
        .min_by_key(|date| (*date - today).num_days().abs())
}

fn parse_time(value: &str) -> Option<NaiveTime> {
    NaiveTime::parse_from_str(value, "%H:%M:%S")
        .or_else(|_| NaiveTime::parse_from_str(value, "%H:%M"))
        .ok()
}

fn first_text<'a, I>(candidates: I) -> Option<String>
where
    I: IntoIterator<Item = &'a Option<String>>,
{
    candidates
        .into_iter()
        .flatten()
        .map(|value| value.trim())
        .find(|value| !value.is_empty())
        .map(str::to_string)
}

fn text(value: &Option<String>) -> &str {
    value.as_deref().unwrap_or_default()
}

fn fold(value: &str) -> String {
    value
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| match c {
            '\u{e1}' | '\u{e0}' | '\u{e2}' => 'a',
            '\u{10d}' => 'c',
            '\u{10f}' => 'd',
            '\u{e9}' | '\u{11b}' => 'e',
            '\u{ed}' => 'i',
            '\u{148}' => 'n',
            '\u{f3}' | '\u{f4}' => 'o',
            '\u{159}' => 'r',
            '\u{161}' => 's',
            '\u{165}' => 't',
            '\u{fa}' | '\u{16f}' => 'u',
            '\u{fd}' => 'y',
            '\u{17e}' => 'z',
            other => other,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const PERMANENT: &str = include_str!("../tests/fixtures/permanent-3d.html");
    const HOLIDAYS: &str = include_str!("../tests/fixtures/actual-holidays-3d.html");

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 8, 28).unwrap()
    }

    fn at(day: u32, hour: u32, minute: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 8, day)
            .unwrap()
            .and_hms_opt(hour, minute, 0)
            .unwrap()
    }

    #[test]
    fn parses_every_atom_of_the_real_page() {
        let page = parse_page(PERMANENT, today()).unwrap();
        assert_eq!(page.class_name.as_deref(), Some("3.D"));
        assert_eq!(page.stats.atoms, 55);
        assert_eq!(page.stats.unparsed, 0);
        assert_eq!(page.stats.removed, 0);
        assert_eq!(page.lessons.len(), 55);
        let first = page
            .lessons
            .iter()
            .find(|lesson| lesson.group.as_deref() == Some("S1"))
            .unwrap();
        assert_eq!(first.subject, "Opera\u{10d}n\u{ed} syst\u{e9}my");
        assert_eq!(first.room.as_deref(), Some("A005"));
        assert_eq!(
            first.teacher.as_deref(),
            Some("Mgr. Michal Va\u{10d}k\u{e1}\u{159}")
        );
        assert_eq!(first.start, at(24, 8, 0));
        assert_eq!(first.end, at(24, 8, 45));
    }

    #[test]
    fn holiday_week_has_known_days_and_no_lessons() {
        let page = parse_page(HOLIDAYS, today()).unwrap();
        assert_eq!(page.class_name.as_deref(), Some("3.D"));
        assert!(page.lessons.is_empty());
        assert_eq!(page.stats.unknown_days, 0);
        assert_eq!(
            page.known_days,
            vec![
                NaiveDate::from_ymd_opt(2026, 8, 24).unwrap(),
                NaiveDate::from_ymd_opt(2026, 8, 25).unwrap(),
                NaiveDate::from_ymd_opt(2026, 8, 26).unwrap(),
                NaiveDate::from_ymd_opt(2026, 8, 27).unwrap(),
                NaiveDate::from_ymd_opt(2026, 8, 28).unwrap(),
            ]
        );
    }

    #[test]
    fn unpublished_day_is_not_a_known_day() {
        let html = r#"<script>const timetableData = {"Days":[{"Date":"1.9.","DayOff":false,"Hours":null}]};</script>"#;
        let page = parse_page(html, today()).unwrap();
        assert!(page.known_days.is_empty());
        assert_eq!(page.stats.unknown_days, 1);
    }

    #[test]
    fn missing_model_is_an_error() {
        assert!(parse_page("<html><body>nothing here</body></html>", today()).is_err());
    }

    #[test]
    fn group_filter_keeps_whole_class_and_selected_groups() {
        let wanted = vec!["s1".to_string(), "PX2".to_string()];
        assert!(keep_group(None, &wanted));
        assert!(keep_group(Some(""), &wanted));
        assert!(keep_group(Some("cel\u{e1}"), &wanted));
        assert!(keep_group(Some("S1"), &wanted));
        assert!(keep_group(Some("PX2 - Praxe 2"), &wanted));
        assert!(!keep_group(Some("S2"), &wanted));
        assert!(!keep_group(Some("PX1"), &wanted));
        assert!(keep_group(Some("S2"), &[]));
    }

    #[test]
    fn merges_consecutive_lessons_of_the_same_group() {
        let page = parse_page(PERMANENT, today()).unwrap();
        let mine: Vec<Lesson> = page
            .lessons
            .into_iter()
            .filter(|lesson| keep_group(lesson.group.as_deref(), &["S1".to_string()]))
            .collect();
        assert_eq!(mine.len(), 29);
        let merged = merge(mine, 25);
        let monday_start = merged
            .iter()
            .find(|lesson| lesson.start == at(24, 8, 0))
            .unwrap();
        assert_eq!(monday_start.subject, "Opera\u{10d}n\u{ed} syst\u{e9}my");
        assert_eq!(monday_start.end, at(24, 10, 30));
        assert!(!merged.iter().any(|lesson| lesson.start == at(24, 8, 50)));
        assert_eq!(merged.len(), 21);
    }

    #[test]
    fn merge_keeps_lessons_apart_when_the_gap_is_too_long() {
        let page = parse_page(PERMANENT, today()).unwrap();
        let merged = merge(page.lessons, 0);
        assert_eq!(merged.len(), 55);
    }

    #[test]
    fn key_is_stable_and_group_specific() {
        let page = parse_page(PERMANENT, today()).unwrap();
        let merged = merge(page.lessons, 25);
        let again = merge(parse_page(PERMANENT, today()).unwrap().lessons, 25);
        let keys: Vec<&str> = merged.iter().map(|lesson| lesson.key.as_str()).collect();
        let keys_again: Vec<&str> = again.iter().map(|lesson| lesson.key.as_str()).collect();
        assert_eq!(keys, keys_again);
        let monday: Vec<&Lesson> = merged
            .iter()
            .filter(|lesson| lesson.start == at(24, 8, 0))
            .collect();
        assert_eq!(monday.len(), 2);
        assert_ne!(monday[0].key, monday[1].key);
    }

    #[test]
    fn year_is_inferred_from_the_closest_date() {
        let today = NaiveDate::from_ymd_opt(2026, 12, 30).unwrap();
        assert_eq!(infer_year(2, 1, today), NaiveDate::from_ymd_opt(2027, 1, 2));
        assert_eq!(
            infer_year(29, 12, today),
            NaiveDate::from_ymd_opt(2026, 12, 29)
        );
    }

    #[test]
    fn summary_and_description_follow_the_change_info() {
        let page = parse_page(PERMANENT, today()).unwrap();
        let lesson = &merge(page.lessons, 25)[0];
        assert!(!lesson.summary().contains('\u{26a0}'));
        assert!(lesson.description().contains(" | "));
    }
}
