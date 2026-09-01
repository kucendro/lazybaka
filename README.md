<img src="assets/lazybaka.png" align="left" width="260">

<samp>One-way sync from a public Bakaláři timetable into a dedicated Google
Calendar.</samp>

[![ci](https://github.com/kucendro/lazybaka/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/kucendro/lazybaka/actions/workflows/ci.yml)
[![release](https://github.com/kucendro/lazybaka/actions/workflows/release.yml/badge.svg)](https://github.com/kucendro/lazybaka/actions/workflows/release.yml)

<br clear="left">

---

Documentation: **<https://kucendro.github.io/lazybaka/>**

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
On NixOS use the [module](https://kucendro.github.io/lazybaka/nixos.html).

## Setup

In Google:

1. Enable the Calendar API in a GCP project.
2. Create a service account. Download its JSON key.
3. Create a calendar for bakasync alone.
4. Share it with the service account e-mail. Permission: **Make changes to events**.

Then:

```console
$ bakasync init
config  ~/.config/bakasync/config.env  (created)
key     ~/.config/bakasync/service-account.json  (missing)

$ mv ~/Downloads/project-a1b2c3.json ~/.config/bakasync/service-account.json
$ chmod 600 ~/.config/bakasync/service-account.json
```

Fill in three values:

```sh
BAKASYNC_BASE_URL=https://bakalari.example.cz/bakaweb
BAKASYNC_CLASS_ID=1A
BAKASYNC_CALENDAR_ID=abc123@group.calendar.google.com
```

`BAKASYNC_CLASS_ID` is the id in the timetable URL, not the class name. Every other variable is in
`.env.example` and in the [docs](https://kucendro.github.io/lazybaka/variables.html).

## Run

```console
$ bakasync doctor    # check config, key, timetable, calendar
$ bakasync --plan    # print what the next sync would change, write nothing
$ bakasync --once    # sync once, ignore the interval
$ bakasync           # sync on BAKASYNC_INTERVAL, or once if unset
```

`doctor` and `--plan` never write. Set `BAKASYNC_INTERVAL=10min` to leave it running.

It only ever touches events it created, so a calendar of its own stays a hard requirement and an exam
you added by hand survives.

## Development

```console
$ nix develop
$ cargo test
$ nix build .#bakasync
$ mdbook serve docs --open
```

`lefthook` runs fmt, clippy, the test suite and the host guard on `pre-push`, and CI runs the same
four. Push a `vX.Y.Z` tag matching `Cargo.toml` to build the five release targets and update the
Homebrew formula and Scoop manifest on `main`.

More in the [docs](https://kucendro.github.io/lazybaka/development.html).
