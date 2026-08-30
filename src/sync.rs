use crate::gcal::{parse_instant, Calendar, Event};
use crate::timetable::{Lesson, TZ};
use anyhow::Result;
use chrono::{NaiveDateTime, TimeZone};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use tracing::{debug, info};

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Plan {
    pub inserts: Vec<Lesson>,
    pub patches: Vec<(String, Lesson)>,
    pub deletes: Vec<String>,
}

impl Plan {
    pub fn is_empty(&self) -> bool {
        self.inserts.is_empty() && self.patches.is_empty() && self.deletes.is_empty()
    }
}

pub fn plan(desired: &[Lesson], existing: &[Event]) -> Plan {
    let mut by_key: HashMap<&str, &Event> = HashMap::new();
    let mut plan = Plan::default();
    for event in existing {
        if !event.is_managed() {
            continue;
        }
        match event.lesson_key() {
            Some(key) if !by_key.contains_key(key) => {
                by_key.insert(key, event);
            }
            _ => plan.deletes.push(event.id.clone()),
        }
    }
    let mut matched: HashSet<&str> = HashSet::new();
    for lesson in desired {
        match by_key.get(lesson.key.as_str()) {
            None => plan.inserts.push(lesson.clone()),
            Some(event) => {
                matched.insert(lesson.key.as_str());
                if differs(event, lesson) {
                    plan.patches.push((event.id.clone(), lesson.clone()));
                }
            }
        }
    }
    for (key, event) in by_key {
        if !matched.contains(key) {
            plan.deletes.push(event.id.clone());
        }
    }
    plan.deletes.sort();
    plan
}

pub async fn apply(calendar: &mut Calendar, plan: &Plan, dry_run: bool) -> Result<()> {
    for lesson in &plan.inserts {
        debug!("insert {} {}", lesson.start, lesson.subject);
        if !dry_run {
            calendar.insert(&body(lesson)).await?;
        }
    }
    for (id, lesson) in &plan.patches {
        debug!("patch {id} {} {}", lesson.start, lesson.subject);
        if !dry_run {
            calendar.patch(id, &body(lesson)).await?;
        }
    }
    for id in &plan.deletes {
        debug!("delete {id}");
        if !dry_run {
            calendar.delete(id).await?;
        }
    }
    if dry_run {
        info!("dry run: nothing was written to the calendar");
    }
    Ok(())
}

pub fn body(lesson: &Lesson) -> Value {
    json!({
        "summary": lesson.summary(),
        "location": lesson.location(),
        "description": lesson.description(),
        "start": stamp(lesson.start),
        "end": stamp(lesson.end),
        "extendedProperties": {
            "private": { "bakasync": "true", "bakasync_key": lesson.key }
        }
    })
}

pub fn rfc3339(value: NaiveDateTime) -> String {
    TZ.from_local_datetime(&value)
        .earliest()
        .map(|value| value.to_rfc3339())
        .unwrap_or_else(|| format!("{}Z", value.format("%Y-%m-%dT%H:%M:%S")))
}

fn stamp(value: NaiveDateTime) -> Value {
    json!({
        "dateTime": value.format("%Y-%m-%dT%H:%M:%S").to_string(),
        "timeZone": TZ.name(),
    })
}

fn differs(event: &Event, lesson: &Lesson) -> bool {
    if event.summary.as_deref().unwrap_or_default() != lesson.summary()
        || event.location.as_deref().unwrap_or_default() != lesson.location()
        || event.description.as_deref().unwrap_or_default() != lesson.description()
    {
        return true;
    }
    !same_moment(&event.start, lesson.start) || !same_moment(&event.end, lesson.end)
}

fn same_moment(actual: &Option<crate::gcal::EventTime>, wanted: NaiveDateTime) -> bool {
    let wanted = match TZ.from_local_datetime(&wanted).earliest() {
        Some(value) => value.timestamp(),
        None => return false,
    };
    parse_instant(actual).map(|value| value.timestamp()) == Some(wanted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gcal::{Event, EventTime, ExtendedProperties};
    use chrono::NaiveDate;

    fn lesson(key: &str, subject: &str, hour: u32) -> Lesson {
        Lesson {
            key: key.to_string(),
            subject: subject.to_string(),
            teacher: Some("Mgr. Michal Va\u{10d}k\u{e1}\u{159}".to_string()),
            room: Some("A005".to_string()),
            group: Some("S1".to_string()),
            change: None,
            theme: None,
            start: NaiveDate::from_ymd_opt(2026, 9, 1)
                .unwrap()
                .and_hms_opt(hour, 0, 0)
                .unwrap(),
            end: NaiveDate::from_ymd_opt(2026, 9, 1)
                .unwrap()
                .and_hms_opt(hour, 45, 0)
                .unwrap(),
        }
    }

    fn event(id: &str, key: Option<&str>, lesson: Option<&Lesson>) -> Event {
        let mut private = std::collections::HashMap::new();
        if let Some(key) = key {
            private.insert("bakasync".to_string(), "true".to_string());
            private.insert("bakasync_key".to_string(), key.to_string());
        }
        Event {
            id: id.to_string(),
            summary: lesson.map(|l| l.summary()),
            location: lesson.map(|l| l.location()),
            description: lesson.map(|l| l.description()),
            start: lesson.map(|l| EventTime {
                date_time: Some(rfc3339(l.start)),
            }),
            end: lesson.map(|l| EventTime {
                date_time: Some(rfc3339(l.end)),
            }),
            extended_properties: Some(ExtendedProperties {
                private: Some(private),
            }),
        }
    }

    #[test]
    fn plans_insert_patch_and_delete() {
        let unchanged = lesson("aaa", "Matematika", 8);
        let changed = lesson("bbb", "Programov\u{e1}n\u{ed}", 9);
        let fresh = lesson("ccc", "Praxe", 10);
        let stale = lesson("ddd", "Zru\u{161}eno", 11);
        let mut outdated = event("id-bbb", Some("bbb"), Some(&changed));
        outdated.location = Some("A003".to_string());
        let existing = vec![
            event("id-aaa", Some("aaa"), Some(&unchanged)),
            outdated,
            event("id-ddd", Some("ddd"), Some(&stale)),
        ];
        let plan = plan(&[unchanged, changed.clone(), fresh.clone()], &existing);
        assert_eq!(plan.inserts, vec![fresh]);
        assert_eq!(plan.patches, vec![("id-bbb".to_string(), changed)]);
        assert_eq!(plan.deletes, vec!["id-ddd".to_string()]);
    }

    #[test]
    fn foreign_events_are_never_touched() {
        let mine = lesson("aaa", "Matematika", 8);
        let mut foreign = event("id-foreign", None, Some(&mine));
        foreign.extended_properties = None;
        let plan = plan(&[], &[foreign]);
        assert!(plan.is_empty());
    }

    #[test]
    fn managed_events_without_a_key_are_deleted() {
        let mine = lesson("aaa", "Matematika", 8);
        let mut orphan = event("id-orphan", Some("aaa"), Some(&mine));
        orphan
            .extended_properties
            .as_mut()
            .unwrap()
            .private
            .as_mut()
            .unwrap()
            .remove("bakasync_key");
        let plan = plan(&[], &[orphan]);
        assert_eq!(plan.deletes, vec!["id-orphan".to_string()]);
    }

    #[test]
    fn identical_events_produce_no_work() {
        let mine = lesson("aaa", "Matematika", 8);
        let existing = vec![event("id-aaa", Some("aaa"), Some(&mine))];
        assert!(plan(&[mine], &existing).is_empty());
    }

    #[test]
    fn body_carries_the_sync_markers() {
        let mine = lesson("aaa", "Matematika", 8);
        let body = body(&mine);
        assert_eq!(body["extendedProperties"]["private"]["bakasync"], "true");
        assert_eq!(body["extendedProperties"]["private"]["bakasync_key"], "aaa");
        assert_eq!(body["start"]["dateTime"], "2026-09-01T08:00:00");
        assert_eq!(body["start"]["timeZone"], "Europe/Prague");
        assert_eq!(body["location"], "A005");
    }
}
