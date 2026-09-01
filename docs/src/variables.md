# Variables

Plain `KEY=value` file. The first location that exists wins:

1. `--config <path>`, or `BAKASYNC_ENV_FILE`
2. `.env.local` and `.env` in the working directory
3. `~/.config/bakasync/config.env`, or `%APPDATA%\bakasync\config.env`

Quote a value with a space: `BAKASYNC_GROUPS="S1, PX2"`. A leading `~` expands.

| Variable | Required | Default | Meaning |
| --- | --- | --- | --- |
| `BAKASYNC_BASE_URL` | yes | | Root of the Bakaláři web, e.g. `https://bakalari.example.cz/bakaweb` |
| `BAKASYNC_CLASS_ID` | yes | | Id from the timetable URL |
| `BAKASYNC_CALENDAR_ID` | yes | | Target calendar, `…@group.calendar.google.com` |
| `BAKASYNC_SERVICE_ACCOUNT_KEY` | no | beside the config | Path to the JSON key |
| `BAKASYNC_WEEKS` | no | `Actual,Next` | Weeks to scrape |
| `BAKASYNC_EXPECTED_CLASS_NAME` | no | | Warn if the page shows another class |
| `BAKASYNC_GROUPS` | no | | Your split groups, comma separated |
| `BAKASYNC_MERGE_GAP_MINUTES` | no | `25` | Break that still merges two lessons |
| `BAKASYNC_INTERVAL` | no | | `10min`, `1h`, `600s` |
| `BAKASYNC_DRY_RUN` | no | `false` | Log the plan, write nothing |
| `BAKASYNC_DUMP_HTML_DIR` | no | | Save every fetched page here |
| `RUST_LOG` | no | `info` | `tracing` filter |

## Groups

`BAKASYNC_GROUPS=S1,PX2` keeps those groups. Whole-class lessons are always kept. Matching ignores
case, diacritics and the long form, so `S1` matches `S1 - Skupina 1`.

## Key path

An unset key path means `service-account.json` next to the config file. A relative path resolves from
there too. With no config file at all, under systemd or in a container, every variable comes from the
environment and the key path is required.

The key is a credential. Keep it `chmod 600` and out of any repository. `.env` and `.env.local` are
gitignored, and a pre-push hook refuses a real school host.
