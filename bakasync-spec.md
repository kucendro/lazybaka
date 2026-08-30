# bakasync — Bakaláři public timetable → Google Calendar sync

Design & handoff spec. Target: implement with Claude Code. **Rust service, packaged as a Nix flake with a NixOS module (systemd service + timer).** No login to Bakaláři — the school only exposes the public timetable.

---

## 1. Context

- School: SPŠE Pardubice, Bakaláři web at `https://bakalari.spse.cz/bakaweb/`
- Public timetable (no auth): `https://bakalari.spse.cz/bakaweb/Timetable/Public/Actual/Class/<ID>`
- **Class ID caveat:** the ID in the URL is internal, not the class name. `/Class/2D` renders **4.D**; class **2.D** is `/Class/39` (as of Aug 2026). IDs get reshuffled at school-year rollover (~Sept 1) — the config must hold the ID, and the service should log the class name it actually sees on the page (available in page header) and warn if it doesn't match an expected `class_name` config value.
- `/Actual/` = current week including suplování and cancellations. Optionally also scrape `/Next/` (same markup, next week). Recommended: sync both.
- Sync direction: one-way, scraper → a **dedicated** Google Calendar. Never touch events the service didn't create.

## 2. Data source: page structure

Server-rendered HTML (Bakaláři "next" webapp). No JS execution needed. Each lesson cell:

```html
<div class="day-item-hover" data-detail='{...json...}'>
```

`data-detail` JSON, observed keys (all optional — parse defensively, `Option<String>` everywhere):

```json
{
  "type": "atom",
  "subjecttext": "Matematika | po 1.9. | 2 (8:05 - 8:50)",
  "teacher": "Mgr. René Dus",
  "room": "B203",
  "group": "celá",
  "theme": "Derivace",
  "notice": "",
  "changeinfo": "Suplování: Ing. Petr Fišar",
  "homeworks": null,
  "absencetext": null
}
```

`type` values:
- `"atom"` — a real lesson → sync it
- `"removed"` — cancelled lesson (`removedinfo` field) → ensure **no** event exists at that slot
- `"absent"` — whole-day/class absence (school event) → optionally create an all-day or block event from `absentinfo`/`InfoAbsentName`; v1 may skip but must not crash
- hour-header cells also carry `data-detail` with `type:"hour"` → ignore

**Everything needed is inside `subjecttext`** — subject name, weekday+date, lesson number, start/end time. Parse it; do not rely on grid position. Regexes:

- date: `(\d{1,2})\.(\d{1,2})\.` (no year — see §3)
- time: `\((\d{1,2}):(\d{2})\s*-\s*(\d{1,2}):(\d{2})\)`
- subject: text before the first `|`

**Unverified-markup warning:** this structure comes from community projects (matous-volf/rozvrh, bakalari-api docs), not from a raw dump of this school's page (fetcher stripped attributes). First implementation step: `curl` the real URL, save as test fixture, adjust selectors if needed. Build in a `--dump-html <path>` flag and: if zero atoms parse, dump HTML + exit non-zero. A Python prototype with the same parsing rules already exists (`bakalari_ics.py`) — reuse its logic 1:1.

## 3. Parsing rules

- **Year inference:** dates lack a year. Pick the year (of {today−1, today, today+1}) minimizing `|date − today|`. Timezone: `Europe/Prague` for everything.
- **Merging:** consecutive lessons with identical (subject, teacher, room, group, changeinfo) and gap ≤ 25 min merge into one event (covers double lessons across breaks).
- **Stable lesson key** (used as sync identity): `sha1(start_datetime_iso | subject | group)`.
- Event mapping:
  - `summary` = subject, append ` ⚠` when `changeinfo` non-empty
  - `location` = room
  - `description` = `teacher | group | changeinfo | theme` (non-empty parts joined)

## 4. Google Calendar sync

**Auth: service account** (headless, no refresh-token churn):
1. GCP project → enable Calendar API → create service account → JSON key.
2. Create dedicated calendar "Rozvrh" in the personal account; share with the SA email, permission "Make changes to events". Config holds the calendar ID.

**Sync algorithm (idempotent diff, per run):**
1. Scrape `/Actual/` (+ `/Next/`) → desired lesson set with stable keys.
2. `events.list` on the calendar, `timeMin`/`timeMax` = scraped window, with `privateExtendedProperty=bakasync=true` filter.
3. Match existing events to desired via extended private property `bakasync_key=<sha1>`.
4. Insert missing, `patch` changed (compare summary/times/location/description), delete events whose key vanished (covers cancellations and reschedules).
5. Never modify events lacking `bakasync=true`.

Every created event carries:

```json
"extendedProperties": { "private": { "bakasync": "true", "bakasync_key": "<sha1>" } }
```

Example insert body:

```json
{
  "summary": "Matematika",
  "location": "B203",
  "description": "Mgr. René Dus | celá",
  "start": { "dateTime": "2026-09-01T08:05:00", "timeZone": "Europe/Prague" },
  "end":   { "dateTime": "2026-09-01T08:50:00", "timeZone": "Europe/Prague" },
  "extendedProperties": { "private": { "bakasync": "true", "bakasync_key": "ab12…" } }
}
```

Rate limits are a non-issue at this volume; still batch politely and back off on 403/429.

## 5. Rust implementation notes

Crates — decision points, pick during implementation:
- HTTP: `reqwest` (rustls)
- HTML: `scraper` (CSS selectors over html5ever)
- JSON: `serde` / `serde_json`
- Time: `jiff` (tz-aware, modern) or `chrono` + `chrono-tz` — either fine
- Google auth: `yup-oauth2` `ServiceAccountAuthenticator` + hand-rolled REST calls via reqwest. Alternative: `google-calendar3` (google-apis-rs) — saves writing request types but is generated, heavy, awkward. Recommendation: **yup-oauth2 + raw REST**, the API surface used is 3 endpoints (list/insert/patch/delete).
- Errors: `anyhow` (bin) — no need for `thiserror` in a leaf binary
- Logging: `tracing` + `tracing-subscriber` (systemd journal picks up stderr)

Config: single TOML file, path via `--config` / `BAKASYNC_CONFIG`:

```toml
timetable_urls = [
  "https://bakalari.spse.cz/bakaweb/Timetable/Public/Actual/Class/39",
  "https://bakalari.spse.cz/bakaweb/Timetable/Public/Next/Class/39",
]
expected_class_name = "2.D"      # warn if page header differs
calendar_id = "…@group.calendar.google.com"
service_account_key = "/run/secrets/bakasync-sa.json"
merge_gap_minutes = 25
```

Binary is one-shot (scrape → diff → apply → exit). Scheduling belongs to systemd, not the process.

Testing: commit a real fixture HTML of the school page; unit-test parser + merge + year inference against it; golden-test the diff (desired vs mocked existing events → expected insert/patch/delete sets). No network in tests.

## 6. Nix flake

- `flake.nix` outputs: `packages.<system>.bakasync` (`rustPlatform.buildRustPackage`, `cargoLock.lockFile`), `devShells.default` (rustc, cargo, clippy, rust-analyzer), `nixosModules.bakasync`.
- NixOS module sketch:

```nix
services.bakasync = {
  enable = true;
  interval = "10min";                 # systemd OnUnitActiveSec
  configFile = config.age.secrets.bakasync-config.path;  # or split: settings + keyFile
};
```

Module generates:

```ini
# bakasync.service
[Service]
Type=oneshot
ExecStart=${pkgs.bakasync}/bin/bakasync --config ${cfg.configFile}
DynamicUser=true
LoadCredential=sa.json:${cfg.serviceAccountKeyFile}
# hardening: ProtectSystem=strict, PrivateTmp=true, NoNewPrivileges=true,
# RestrictAddressFamilies=AF_INET AF_INET6, CapabilityBoundingSet=

# bakasync.timer
[Timer]
OnBootSec=2min
OnUnitActiveSec=10min
RandomizedDelaySec=1min
Persistent=true
```

Secrets: SA key via agenix/sops-nix, exposed through `LoadCredential` (path `$CREDENTIALS_DIRECTORY/sa.json`), not world-readable in the store.

## 7. Example expected behavior

```
$ bakasync --config /etc/bakasync.toml
INFO scrape: 2 urls fetched, class="2.D" ok
INFO parse: 34 atoms, 2 removed, 1 absent(skipped); merged → 27 lessons
INFO diff: existing=25 insert=3 patch=1 delete=1
INFO gcal: applied in 2.1s
```

Cancelled lesson mid-week → next run's `delete=1` removes it from the calendar within `interval`.

## 8. Open decisions (resolve at implementation time)

1. Handle `type:"absent"` (school events) as calendar events, or skip in v1?
2. Sync window: Actual only, or Actual+Next (recommended)?
3. `jiff` vs `chrono` (taste).
4. Optional `.ics` export subcommand as a fallback output? (Python prototype already does this; probably drop.)

## 9. Known risks

- Markup assumptions unverified against the live page (§2) — first task is fixture capture.
- Class ID rollover at new school year — mitigated by `expected_class_name` check + loud warning.
- Before ~Sept 1 the Actual page may be empty/stale; empty parse on a valid page must be distinguishable from parser breakage (heuristic: page header parsed OK + zero cells with `data-detail` → likely holidays; log, exit 0).
