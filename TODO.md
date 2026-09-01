# TODO

## Next

- [ ] Tag `v0.1.0` and confirm the release workflow on the three untested targets: musl, Windows
      (NASM), and the cross-built Intel mac. All three are `aws-lc-rs` C builds.
- [ ] `bakasync status --json` — last run, next run, lessons synced, last error. One line of state,
      readable by a status bar or `watch`. Needs somewhere to keep it: `$XDG_STATE_HOME/bakasync`
      for a user install, `StateDirectory` for the systemd unit.

## Maybe

- [ ] Running in the background off NixOS: a launchd plist for macOS, a scheduled task for Windows.
      Only worth writing once someone actually runs it there.
- [ ] Retry/backoff tuning — the Google client retries 4 times; nothing has failed yet to justify
      more.
- [ ] `doctor` proves the calendar is readable, not writable — a read-only share still passes. Only
      an insert would prove it, and that leaves an event behind, so `--plan` covers the gap instead.

## Open questions

### A TUI — recommendation: no

The output surface of this tool is Google Calendar. The process is a headless daemon that should be
invisible when it works, and its interesting moments (a class id going stale, a 403 from the
calendar) are already one `journalctl` away. A TUI only exists while you sit in front of it, which is
the one situation where logs are already good enough.

It also costs more than it looks: `ratatui` + `crossterm`, an event loop, and a second rendering path
that has to keep up with every change to the sync logic — for a screen that gets looked at roughly
never.

The two things a UI would actually be for are better off without one:

1. **See the diff before it lands** → `--plan`, which is now in.
2. **Know it is alive** → `status --json`, which anything can consume.

Revisit only if a real habit of watching it live shows up.

### Status bar

Open: what "bar icons" should mean. A waybar module fed by `status --json` is roughly a JSON print
plus a config block, and would cover *next lesson* / *last sync ok* on the bar. Needs a decision on
what should actually be displayed before it is worth writing.
