# Configure

## 1. Write the config file

```console
$ bakasync init
config  ~/.config/bakasync/config.env  (created)
key     ~/.config/bakasync/service-account.json  (missing)

Download the service account key from GCP and save it
to the path above, then run: bakasync doctor
```

## 2. Put the key where it says

```console
$ mv ~/Downloads/project-a1b2c3.json ~/.config/bakasync/service-account.json
$ chmod 600 ~/.config/bakasync/service-account.json
```

## 3. Fill in three values

```sh
BAKASYNC_BASE_URL=https://bakalari.example.cz/bakaweb
BAKASYNC_CLASS_ID=1A
BAKASYNC_CALENDAR_ID=abc123@group.calendar.google.com
```

`BAKASYNC_CLASS_ID` is the id in the timetable URL, not the class name. Open the public timetable in a
browser and read it off the address bar.

Add `BAKASYNC_GROUPS` if your class splits, and `BAKASYNC_EXPECTED_CLASS_NAME` to be told when the
page stops being your class. Everything else has a default: see [Variables](variables.md).

## 4. Check

```console
$ bakasync doctor
config    ~/.config/bakasync/config.env
key       ok, sync@project.iam.gserviceaccount.com
timetable ok, class "1.A", 43 lessons
calendar  FAILED: google api returned 404 Not Found: ...
          the calendar id is wrong, or it is not shared with the service account
```

A clean run ends with `all good, run bakasync --plan to see what the next sync would change`.
