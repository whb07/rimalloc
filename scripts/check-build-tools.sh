#!/usr/bin/env bash

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd_repo_root

tmpdir="$(mktemp -d)"
trap 'rm -rf "$tmpdir"' EXIT

log="$tmpdir/build-tools.log"
missing_cc="$tmpdir/definitely-missing-cc"

if env CARGO_TARGET_DIR="$tmpdir/target" CC="$missing_cc" cargo check --features track-valgrind >"$log" 2>&1; then
  cat "$log" >&2
  echo "expected bogus CC to make the build script fail" >&2
  exit 1
fi

cat "$log"

if ! grep -q "failed to run .* compiler tool" "$log"; then
  echo "missing actionable compiler failure message" >&2
  exit 1
fi

if ! grep -q "set CC to a working executable" "$log"; then
  echo "missing CC remediation hint" >&2
  exit 1
fi
