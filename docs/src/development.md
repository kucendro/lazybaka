# Development

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

## These docs

```console
$ mdbook serve docs --open
```

`mdbook` is in the dev shell. A push to `main` that touches `docs/` publishes them.

## Release

Push a `vX.Y.Z` tag matching `Cargo.toml`. That builds five targets, publishes the release, and
commits `Formula/bakasync.rb` and `bucket/bakasync.json` to `main`. Those two files make the
repository a Homebrew tap and a Scoop bucket. Homebrew reads the formula from the default branch, so
an install only resolves after a tag has run.
