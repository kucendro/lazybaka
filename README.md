<img src="assets/lazybaka.png" align="left" width="260">

<samp>One-way sync from a public Bakaláři timetable into a dedicated Google
Calendar.</samp>

<br clear="left">

---

## Install

macOS and Linux:

```console
$ brew install kucendro/tap/bakasync
```

Windows:

```console
$ scoop bucket add lazybaka https://github.com/kucendro/lazybaka
$ scoop install bakasync
```

Or take an archive for your platform from [Releases](https://github.com/kucendro/lazybaka/releases) —
static binary, `.env.example` and this README, checksummed in `SHA256SUMS`. Linux and macOS on both
Intel and ARM, Windows on x86-64.

Then run `bakasync init`, which creates the config file and tells you where the service account key
belongs.

## Usage

```console
$ bakasync init      # write the config file, say where the key goes
$ bakasync doctor    # check the config, the key, the timetable and the calendar
$ bakasync --plan    # print what the next sync would change, write nothing
$ bakasync --once    # sync once and exit, ignoring the interval
$ bakasync           # sync on BAKASYNC_INTERVAL, or once if it is unset
```

`doctor` never writes; `--plan` is the last look before anything reaches the calendar.

```console
$ bakasync doctor
config    ~/.config/bakasync/config.env
key       ok, sync@project.iam.gserviceaccount.com
timetable ok, class "1.A", 43 lessons
calendar  FAILED: google api returned 404 Not Found: ...
          the calendar id is wrong, or it is not shared with the service account
```

## Configuration

Environment variables. The first of these that exists wins, and the rest are left alone:

1. `--config <path>`, or `BAKASYNC_ENV_FILE`
2. `.env.local` and `.env` in the working directory
3. `~/.config/bakasync/config.env`, `%APPDATA%\bakasync\config.env` on Windows

A value containing a space needs quotes — `BAKASYNC_GROUPS="S1, PX2"` — and a leading `~` in a path
is expanded. Both `.env.local` and `.env` are gitignored, so that is where your real school host
belongs; a pre-push hook refuses a commit that carries one.

| Variable | Required | Default | Meaning |
| --- | --- | --- | --- |
| `BAKASYNC_BASE_URL` | yes | — | Root of the Bakaláři web, e.g. `https://bakalari.example.cz/bakaweb` |
| `BAKASYNC_CLASS_ID` | yes | — | Id from the timetable URL, not the class name |
| `BAKASYNC_CALENDAR_ID` | yes | — | Target calendar, `…@group.calendar.google.com` |
| `BAKASYNC_SERVICE_ACCOUNT_KEY` | no | beside the config | Path to the Google service account JSON key |
| `BAKASYNC_WEEKS` | no | `Actual,Next` | Which weeks to scrape |
| `BAKASYNC_EXPECTED_CLASS_NAME` | no | — | Warn if the page shows a different class |
| `BAKASYNC_GROUPS` | no | — | Your split groups, comma separated |
| `BAKASYNC_MERGE_GAP_MINUTES` | no | `25` | Break length that still merges two lessons |
| `BAKASYNC_INTERVAL` | no | — | `10min`, `1h`, `600s`; unset runs once and exits |
| `BAKASYNC_DRY_RUN` | no | `false` | Log the plan, write nothing |
| `BAKASYNC_DUMP_HTML_DIR` | no | — | Save every fetched page here |
| `RUST_LOG` | no | `info` | `tracing` filter |

`BAKASYNC_GROUPS=S1,PX2` keeps only those groups; whole-class lessons are always kept. Matching
ignores case, Czech diacritics and the long form (`S1` matches `S1 - Skupina 1`).

With no `BAKASYNC_SERVICE_ACCOUNT_KEY` the key is read from `service-account.json` next to the
config file, which is where `bakasync init` tells you to put it; a relative path is resolved from
there too. With no config file at all — a systemd unit, a container — every variable has to come
from the environment and the key path is required.

## Google

1. Enable the Calendar API in a GCP project, create a service account, download its JSON key.
2. Create a dedicated calendar and share it with the service account e-mail, **Make changes to
   events**.

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

Hardened `DynamicUser` service; the key goes through `LoadCredential`, never into the store. Use
`environmentFile` for other secrets and `extraSettings` for the rest of the table above.

## Development

```console
$ nix develop
$ cargo test
$ nix build .#bakasync
```

`cargo test` covers the parser twice: against checked-in pages, which is all CI has, and against the
live timetable named by `.env.local`. The live check fetches the current week and asserts the class
name is still there, matches `BAKASYNC_EXPECTED_CLASS_NAME`, and that every atom parsed — the two
ways this breaks in practice. It skips itself when no env file is present, and `lefthook` runs the
suite on `pre-push`, so a stale class id is caught before it ships.

`nix develop` puts the flake-built `bakasync` on `PATH` alongside the Rust toolchain, so you can try
the real binary without installing anything; `nix profile install .#bakasync` if you want it to
stick. Use `cargo run` while iterating — the shell's copy only rebuilds when you re-enter it. Running
it in this directory picks up `.env`, so reach for `--plan` rather than a bare `bakasync`.

Pushing a `vX.Y.Z` tag that matches `Cargo.toml` builds all five targets, publishes the release and
rewrites `Formula/bakasync.rb` and `bucket/bakasync.json`. `brew install kucendro/tap/bakasync` needs
a `kucendro/homebrew-tap` repository; point the `HOMEBREW_TAP` variable and `HOMEBREW_TAP_TOKEN`
secret at it and the workflow keeps it in step.
