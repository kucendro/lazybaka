<img src="assets/lazybaka.png" align="left" width="260">

<samp>One-way sync from a public Bakaláři timetable into a dedicated Google
Calendar.</samp>

[![ci](https://github.com/kucendro/lazybaka/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/kucendro/lazybaka/actions/workflows/ci.yml)
[![release](https://github.com/kucendro/lazybaka/actions/workflows/release.yml/badge.svg)](https://github.com/kucendro/lazybaka/actions/workflows/release.yml)

<br clear="left">

---

## Install

macOS and Linux:

```console
$ brew tap kucendro/lazybaka https://github.com/kucendro/lazybaka
$ brew install bakasync
```

Windows:

```console
$ scoop bucket add lazybaka https://github.com/kucendro/lazybaka
$ scoop install bakasync
```

Anything else: [Releases](https://github.com/kucendro/lazybaka/releases). Static binary,
`.env.example`, this readme, `SHA256SUMS`. Linux and macOS on Intel and ARM, Windows on x86-64.

## Google

1. Enable the Calendar API in a GCP project.
2. Create a service account. Download its JSON key.
3. Create a calendar for bakasync alone.
4. Share it with the service account e-mail. Permission: **Make changes to events**.

## Configure

Write the config file:

```console
$ bakasync init
config  ~/.config/bakasync/config.env  (created)
key     ~/.config/bakasync/service-account.json  (missing)

Download the service account key from GCP and save it
to the path above, then run: bakasync doctor
```

Put the key where it says:

```console
$ mv ~/Downloads/project-a1b2c3.json ~/.config/bakasync/service-account.json
$ chmod 600 ~/.config/bakasync/service-account.json
```

Fill in three values:

```sh
BAKASYNC_BASE_URL=https://bakalari.example.cz/bakaweb
BAKASYNC_CLASS_ID=1A
BAKASYNC_CALENDAR_ID=abc123@group.calendar.google.com
```

`BAKASYNC_CLASS_ID` is the id in the timetable URL, not the class name.

Check:

```console
$ bakasync doctor
config    ~/.config/bakasync/config.env
key       ok, sync@project.iam.gserviceaccount.com
timetable ok, class "1.A", 43 lessons
calendar  FAILED: google api returned 404 Not Found: ...
          the calendar id is wrong, or it is not shared with the service account
```

A clean run ends with `all good, run bakasync --plan to see what the next sync would change`.

## Run

```console
$ bakasync init      # write the config file, say where the key goes
$ bakasync doctor    # check config, key, timetable, calendar
$ bakasync --plan    # print what the next sync would change, write nothing
$ bakasync --once    # sync once, ignore the interval
$ bakasync           # sync on BAKASYNC_INTERVAL, or once if unset
```

Set `BAKASYNC_INTERVAL=10min` to leave it running. Otherwise it syncs once and exits, which is what
you want under a timer. `doctor` and `--plan` never write.

## Variables

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

`BAKASYNC_GROUPS=S1,PX2` keeps those groups. Whole-class lessons are always kept. Matching ignores
case, diacritics and the long form, so `S1` matches `S1 - Skupina 1`.

An unset key path means `service-account.json` next to the config file. A relative path resolves from
there too. With no config file at all, under systemd or in a container, every variable comes from the
environment and the key path is required.

The key is a credential. Keep it `chmod 600` and out of any repository. `.env` and `.env.local` are
gitignored, and a pre-push hook refuses a real school host.

## Behaviour

- Only touches events it created. They carry a private `bakasync` property, so an exam you added by
  hand survives.
- Back-to-back lessons of one subject become one event, if the break fits
  `BAKASYNC_MERGE_GAP_MINUTES`.
- Substitutions and cancellations get ⚠ in the title.
- Title is the subject, location is the room, description is teacher, group, change, theme.
- Each event keys off a hash of start time, subject and group. Nothing is stored locally, so there is
  no state to lose.
- A moved lesson patches in place. A cancelled one is deleted.

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
            baseUrl = "https://bakalari.example.cz/bakaweb";
            classId = "1A";
            expectedClassName = "1.A";
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

Hardened `DynamicUser` service. The key goes in through `LoadCredential`, so it never reaches the Nix
store. Use `environmentFile` for other secrets and `extraSettings` for the rest of the table.

## Development

```console
$ nix develop
$ cargo test
$ nix build .#bakasync
```

The parser is tested twice. Against checked-in pages, which is all CI has, and against the live
timetable named by `.env.local`. The live check asserts the class name is still there, matches
`BAKASYNC_EXPECTED_CLASS_NAME`, and that every atom parsed. Those are the two ways this breaks in
practice. It skips itself with no env file, and `lefthook` runs the suite on `pre-push`.

`nix develop` puts the flake-built binary on `PATH` next to the Rust toolchain. Running it in the
source directory picks up `.env`, so use `--plan`, not a bare `bakasync`.

### Release

Push a `vX.Y.Z` tag matching `Cargo.toml`. That builds five targets, publishes the release, and
commits `Formula/bakasync.rb` and `bucket/bakasync.json` to `main`. Those two files make the
repository a Homebrew tap and a Scoop bucket. Homebrew reads the formula from the default branch, so
an install only resolves after a tag has run.
