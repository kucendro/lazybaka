#!/usr/bin/env bash
set -euo pipefail

version="${1:?version}"
dist="${2:?dist directory}"
repo="kucendro/lazybaka"
desc="One-way sync from a public Bakalari timetable to a dedicated Google Calendar"
base="https://github.com/$repo/releases/download/v$version"

digest() {
  local file="$dist/bakasync-$version-$1"
  test -f "$file" || { echo "missing $file" >&2; exit 1; }
  sha256sum "$file" | cut -d' ' -f1
}

mac_arm="$(digest aarch64-apple-darwin.tar.gz)"
mac_intel="$(digest x86_64-apple-darwin.tar.gz)"
linux_arm="$(digest aarch64-unknown-linux-musl.tar.gz)"
linux_intel="$(digest x86_64-unknown-linux-musl.tar.gz)"
windows="$(digest x86_64-pc-windows-msvc.zip)"

mkdir -p Formula bucket

cat > Formula/bakasync.rb <<RUBY
class Bakasync < Formula
  desc "$desc"
  homepage "https://github.com/$repo"
  version "$version"
  license "MIT"

  on_macos do
    on_arm do
      url "$base/bakasync-$version-aarch64-apple-darwin.tar.gz"
      sha256 "$mac_arm"
    end
    on_intel do
      url "$base/bakasync-$version-x86_64-apple-darwin.tar.gz"
      sha256 "$mac_intel"
    end
  end

  on_linux do
    on_arm do
      url "$base/bakasync-$version-aarch64-unknown-linux-musl.tar.gz"
      sha256 "$linux_arm"
    end
    on_intel do
      url "$base/bakasync-$version-x86_64-unknown-linux-musl.tar.gz"
      sha256 "$linux_intel"
    end
  end

  def install
    bin.install "bakasync"
    pkgshare.install ".env.example"
  end

  def caveats
    <<~EOS
      Run \`bakasync init\` to write ~/.config/bakasync/config.env, put the
      Google service account key next to it as service-account.json, then
      check the setup with \`bakasync doctor\`.
    EOS
  end

  test do
    assert_match "BAKASYNC_BASE_URL is not set", shell_output("#{bin}/bakasync 2>&1", 1)
  end
end
RUBY

cat > bucket/bakasync.json <<JSON
{
    "version": "$version",
    "description": "$desc",
    "homepage": "https://github.com/$repo",
    "license": "MIT",
    "architecture": {
        "64bit": {
            "url": "$base/bakasync-$version-x86_64-pc-windows-msvc.zip",
            "hash": "$windows",
            "extract_dir": "bakasync-$version-x86_64-pc-windows-msvc"
        }
    },
    "bin": "bakasync.exe",
    "checkver": {
        "github": "https://github.com/$repo"
    },
    "autoupdate": {
        "architecture": {
            "64bit": {
                "url": "https://github.com/$repo/releases/download/v\$version/bakasync-\$version-x86_64-pc-windows-msvc.zip",
                "extract_dir": "bakasync-\$version-x86_64-pc-windows-msvc"
            }
        },
        "hash": {
            "url": "https://github.com/$repo/releases/download/v\$version/SHA256SUMS"
        }
    }
}
JSON
