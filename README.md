<img src="assets/lazybaka.png" align="left" width="260">

<samp>One-way sync from a public Bakaláři timetable into a dedicated Google
Calendar.</samp>

<br clear="left">

---

## Configuration

Environment variables, loaded from `.env` in the working directory (or `BAKASYNC_ENV_FILE`).
Copy `.env.example` and fill it in.

| Variable | Required | Default | Meaning |
| --- | --- | --- | --- |
| `BAKASYNC_CLASS_ID` | yes | — | Id from the timetable URL, not the class name |
| `BAKASYNC_CALENDAR_ID` | yes | — | Target calendar, `…@group.calendar.google.com` |
| `BAKASYNC_SERVICE_ACCOUNT_KEY` | yes | — | Path to the Google service account JSON key |
| `BAKASYNC_BASE_URL` | no | see `.env.example` | Root of the Bakaláři web |
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

Hardened `DynamicUser` service; the key goes through `LoadCredential`, never into the store. Use
`environmentFile` for other secrets and `extraSettings` for the rest of the table above.

## Development

```console
$ nix develop
$ cargo test
$ nix build .#bakasync
```
