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

## Configuration

Environment variables, loaded from `.env.local` then `.env` in the working directory, or from
`BAKASYNC_ENV_FILE` if it is set. Copy `.env.example` and fill it in. Both `.env.local` and `.env`
are gitignored, so that is where your real school host belongs — a pre-push hook refuses a commit
that carries one.

| Variable | Required | Default | Meaning |
| --- | --- | --- | --- |
| `BAKASYNC_BASE_URL` | yes | — | Root of the Bakaláři web, e.g. `https://bakalari.example.cz/bakaweb` |
| `BAKASYNC_CLASS_ID` | yes | — | Id from the timetable URL, not the class name |
| `BAKASYNC_CALENDAR_ID` | yes | — | Target calendar, `…@group.calendar.google.com` |
| `BAKASYNC_SERVICE_ACCOUNT_KEY` | yes | — | Path to the Google service account JSON key |
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
stick. Use `cargo run` while iterating — the shell's copy only rebuilds when you re-enter it. Note
that running it in this directory picks up `.env` and syncs for real, so set `BAKASYNC_DRY_RUN=true`
first.

Pushing a `vX.Y.Z` tag that matches `Cargo.toml` builds all five targets, publishes the release and
rewrites `Formula/bakasync.rb` and `bucket/bakasync.json`. `brew install kucendro/tap/bakasync` needs
a `kucendro/homebrew-tap` repository; point the `HOMEBREW_TAP` variable and `HOMEBREW_TAP_TOKEN`
secret at it and the workflow keeps it in step.
