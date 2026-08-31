# TODO

## Next

- [ ] `--plan` and `--once` flags. `bakasync` ignores argv today, so running it by hand in the repo
      picks up `.env` and starts syncing for real on the interval. There is no way to say *show me
      the diff and stop*, which is exactly what you want while testing locally.
- [ ] `bakasync status --json` — last run, next run, lessons synced, last error. One line of state,
      readable by a status bar or `watch`.
- [ ] Tag `v0.1.0` and confirm the release workflow on the three untested targets: musl, Windows
      (NASM), and the cross-built Intel mac. All three are `aws-lc-rs` C builds.
- [ ] Create `kucendro/homebrew-tap`, set the `HOMEBREW_TAP` variable and `HOMEBREW_TAP_TOKEN`
      secret, so `brew install kucendro/tap/bakasync` resolves.
- [ ] Decide whether `bakasync-spec.md` belongs in a public repo. It is a design doc, now
      school-free, but the README already covers everything a user needs.

## Think about

### Where does a packaged install read its config from?

`brew install bakasync` / `scoop install bakasync` puts the binary on `PATH`, but `dotenvy` only
looks in the *current* directory. Someone running `bakasync` from their home directory has no
config file — their only options today are exporting the variables by hand or setting
`BAKASYNC_ENV_FILE`. Neither is discoverable from a package.

Likely answer: fall back to a per-user config after the working directory, so the chain becomes
`BAKASYNC_ENV_FILE` → `./.env.local` → `./.env` → `$XDG_CONFIG_HOME/bakasync/config.env` (
`~/.config/bakasync/config.env`, `%APPDATA%\bakasync\config.env` on Windows). Then the Homebrew
formula can print a caveat with that path and `.env.example` has somewhere to be copied to.

Open: whether the per-user file should merge with a working-directory `.env` or be ignored when one
is present. Merging is the usual dotenv behaviour but means a stray `.env` in some unrelated
directory silently contributes variables.

## Maybe

- [ ] Running in the background off NixOS: a launchd plist for macOS, a scheduled task for Windows.
      Only worth writing once someone actually runs it there.
- [ ] Retry/backoff tuning — the Google client retries 4 times; nothing has failed yet to justify
      more.

## Open questions

### A TUI — recommendation: no

The output surface of this tool is Google Calendar. The process is a headless daemon that should be
invisible when it works, and its interesting moments (a class id going stale, a 403 from the
calendar) are already one `journalctl` away. A TUI only exists while you sit in front of it, which is
the one situation where logs are already good enough.

It also costs more than it looks: `ratatui` + `crossterm`, an event loop, and a second rendering path
that has to keep up with every change to the sync logic — for a screen that gets looked at roughly
never.

The two things a UI would actually be for are worth building *without* a TUI:

1. **See the diff before it lands** → `--plan`, which prints the insert/patch/delete list and exits.
2. **Know it is alive** → `status --json`, which anything can consume.

Revisit only if a real habit of watching it live shows up.

### Status bar

Open: what "bar icons" should mean. A waybar module fed by `status --json` is roughly a JSON print
plus a config block, and would cover *next lesson* / *last sync ok* on the bar. Needs a decision on
what should actually be displayed before it is worth writing.
