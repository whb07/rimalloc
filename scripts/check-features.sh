#!/usr/bin/env bash

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd_repo_root

run cargo check
run cargo check --features std
run cargo check --features secure
run cargo check --features guarded
run cargo check --features compare
run cargo check --features unstable-raw-internals
run cargo check --features track-valgrind
run cargo check --features track-asan
run cargo check --features "secure guarded std"
run cargo check --benches --features bench
run cargo check --benches --features "bench compare"
run cargo test --quiet --features "std unstable-raw-internals"
run cargo check --features "track-valgrind track-asan"

if ! command -v cargo-hack >/dev/null 2>&1; then
  echo "cargo-hack is required for feature powerset validation" >&2
  echo "install it with: cargo install cargo-hack --locked" >&2
  exit 1
fi

run cargo hack check --feature-powerset --exclude-features bench,compare,default --no-dev-deps
