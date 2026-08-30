# bakasync

One-way sync from the **public** SPŠE Pardubice Bakaláři timetable into a dedicated Google
Calendar. No Bakaláři login, no manual copying: scrape → diff → apply, on an interval.

The service only ever touches events it created itself (they carry a `bakasync=true` private
extended property). Everything else in the calendar is left alone.

## How the data is read

The Bakaláři web application (2.2.826) renders the timetable **client side** with Knockout, so the
grid cells in the HTML are empty templates — the `data-detail` attributes only exist in the browser.
The lesson data sits in the page as a single JSON object:

```js
const timetableData = {"Days":[{"Date":"24.8.","DayOff":false,"Hours":[ ... ]}], ...};
```

bakasync extracts that object, which is richer and far more stable than the rendered markup: each
atom already carries the full date (`24.8.2026 (pondělí)`), the exact `Begin`/`End` times,
`SubjectText`, `TeacherFullname`, `Room`, `GroupsNames` and `ChangeInfo`. The class name is read
from the `<select id="selectedClass">` header and compared with `BAKASYNC_EXPECTED_CLASS_NAME`, so
a class-id reshuffle at the school-year rollover shows up as a warning instead of silent nonsense.

Days where `Hours` is `null` and `DayOff` is `false` are days the school has not published yet.
They are excluded from the sync window, so an unpublished week never deletes existing events. The
same holds if the page structure ever changes: parsing fails loudly and nothing is written.

## Configuration

All configuration is environment variables, loaded from a `.env` file in the working directory
(or from `BAKASYNC_ENV_FILE`). Copy `.env.example` and fill it in.

| Variable | Required | Default | Meaning |
| --- | --- | --- | --- |
| `BAKASYNC_CLASS_ID` | yes | — | Id in the timetable URL, **not** the class name (`2T` renders as 3.D) |
| `BAKASYNC_CALENDAR_ID` | yes | — | Target calendar, e.g. `…@group.calendar.google.com` |
| `BAKASYNC_SERVICE_ACCOUNT_KEY` | yes | — | Path to the Google service account JSON key |
| `BAKASYNC_BASE_URL` | no | `https://bakalari.spse.cz/bakaweb` | Root of the Bakaláři web |
| `BAKASYNC_WEEKS` | no | `Actual,Next` | Which weeks to scrape |
| `BAKASYNC_EXPECTED_CLASS_NAME` | no | — | Warn if the page shows a different class |
| `BAKASYNC_GROUPS` | no | — | Your split groups, comma separated |
| `BAKASYNC_MERGE_GAP_MINUTES` | no | `25` | Break length that still merges two lessons |
| `BAKASYNC_INTERVAL` | no | — | `10min`, `1h`, `600s`; unset runs once and exits |
| `BAKASYNC_DRY_RUN` | no | `false` | Log the plan, write nothing |
| `BAKASYNC_DUMP_HTML_DIR` | no | — | Save every fetched page here, for debugging |
| `BAKASYNC_ENV_FILE` | no | `.env` | Alternative env file path |
| `RUST_LOG` | no | `info` | `tracing` filter |

### Groups

Classes are split, and the timetable marks each lesson with the group it belongs to (`S1`, `S2`,
`PX1`…). List the groups **you** are in:

```
BAKASYNC_GROUPS=S1,PX2
```

Lessons for the whole class (empty group, or `celá`) are always kept. Matching ignores case and
Czech diacritics and accepts both the short and the long form (`S1` matches `S1 - Skupina 1`).
With `BAKASYNC_GROUPS` unset, every lesson of the class is synced.

## Google Calendar setup

1. Create a GCP project and enable the **Google Calendar API**.
2. Create a service account and download its JSON key.
3. Create a **dedicated** calendar (e.g. "Rozvrh") in your Google account.
4. Share that calendar with the service account e-mail, permission **Make changes to events**.
5. Put the calendar id in `BAKASYNC_CALENDAR_ID` and the key path in
   `BAKASYNC_SERVICE_ACCOUNT_KEY`.

Authentication is a service-account JWT exchanged for an access token, scope
`https://www.googleapis.com/auth/calendar.events`.

## Running

```console
$ cargo run
INFO scrape: 2 urls fetched, 10 days published
INFO parse: 68 atoms, 2 removed, 0 absent(skipped); 41 of 68 for groups ["S1", "PX2"]; merged -> 26 lessons
INFO diff: window=2026-09-01T00:00:00+02:00 .. 2026-09-12T00:00:00+02:00 existing=25 insert=2 patch=1 delete=1
INFO gcal: applied in 1.4s
INFO next run in 10m
```

With `BAKASYNC_INTERVAL` set the process stays alive and repeats; without it, it runs once and
exits (non-zero on failure), which suits a systemd timer or a cron entry.

Lessons are merged into one event when they are consecutive, identical (subject, teacher, room,
group, change) and separated by at most `BAKASYNC_MERGE_GAP_MINUTES`, so a double lesson across a
break is one calendar entry. Each event is identified by `sha1(start | subject | group)`, kept in
its `bakasync_key` extended property; a lesson that disappears from the page is deleted on the next
run.

## NixOS

```nix
{
  inputs.bakasync.url = "github:kucendro/lazybaka";

  outputs = { nixpkgs, bakasync, ... }: {
    nixosConfigurations.myhost = nixpkgs.lib.nixosSystem {
      modules = [
        bakasync.nixosModules.bakasync
        ({ config, ... }: {
          services.bakasync = {
            enable = true;
            classId = "2T";
            expectedClassName = "3.D";
            groups = [ "S1" "PX2" ];
            calendarId = "abc123@group.calendar.google.com";
            serviceAccountKeyFile = config.age.secrets.bakasync-sa.path;
            interval = "10min";
          };
        })
      ];
    };
  };
}
```

The module runs a hardened `DynamicUser` service and passes the key through `LoadCredential`, so
the secret never lands in the Nix store. Use `environmentFile` for anything else that should stay
out of the store, and `extraSettings` for the remaining variables from the table above.

`nix build .#bakasync` builds the binary, `nix flake check` builds it and runs the tests, and
`nix develop` gives a shell with cargo, clippy and rust-analyzer.

## Tests

```console
$ cargo test
```

The fixtures in `tests/fixtures/` are real pages of class 3.D: a published week (55 lessons, S1/S2
and PX1–PX3 splits) and a holiday week. The suite covers parsing, group filtering, lesson merging,
year inference, key stability and the insert/patch/delete plan. No test touches the network.
