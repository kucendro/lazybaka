{ lib, rustPlatform }:

rustPlatform.buildRustPackage {
  pname = "bakasync";
  version = "0.1.0";

  src = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.unions [
      ../.env.example
      ../Cargo.toml
      ../Cargo.lock
      ../src
      ../tests
    ];
  };

  cargoLock.lockFile = ../Cargo.lock;

  meta = {
    description = "One-way sync from a public Bakalari timetable to a dedicated Google Calendar";
    license = lib.licenses.mit;
    mainProgram = "bakasync";
    platforms = lib.platforms.unix;
  };
}
