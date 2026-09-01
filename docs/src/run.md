# Run

```console
$ bakasync init      # write the config file, say where the key goes
$ bakasync doctor    # check config, key, timetable, calendar
$ bakasync --plan    # print what the next sync would change, write nothing
$ bakasync --once    # sync once, ignore the interval
$ bakasync           # sync on BAKASYNC_INTERVAL, or once if unset
```

`doctor` and `--plan` never write.

Set `BAKASYNC_INTERVAL=10min` to leave it running. Otherwise it syncs once and exits, which is what
you want under a timer or cron.

`--config <path>` works on every command, so one machine can drive two classes:

```console
$ bakasync --config ~/.config/bakasync/sister.env --plan
```

On NixOS the [module](nixos.md) runs it as a systemd service. Elsewhere, a timer or a `--once` cron
line is enough.
