# Behaviour

What a sync does to the calendar:

- Only touches events it created. They carry a private `bakasync` property, so an exam you added by
  hand survives.
- Back-to-back lessons of one subject become one event, if the break fits
  `BAKASYNC_MERGE_GAP_MINUTES`.
- Substitutions and cancellations get ⚠ in the title.
- Title is the subject, location is the room, description is teacher, group, change, theme.
- Each event keys off a hash of start time, subject and group. Nothing is stored locally, so there is
  no state to lose.
- A moved lesson patches in place. A cancelled one is deleted.

Two lessons in the timetable:

| Period | Time | Subject | Room | Teacher | Group |
| --- | --- | --- | --- | --- | --- |
| 3 | 09:55 | M | B302 | Nová | S2 |
| 4 | 10:50 | M | B302 | Nová | S2 |

become one event: **Matematika**, 09:55 to 11:35, location `B302`, description `Nová | S2`.

## When it goes wrong

`doctor` proves the calendar is readable, not writable. A read-only share still passes it. Only an
insert would prove the rest, and that leaves an event behind, so `--plan` covers the gap.

A class id that no longer exists shows up as a `timetable FAILED` line, not as an empty sync. An empty
timetable is never treated as "delete everything".
