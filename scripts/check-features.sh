#!/usr/bin/env bash

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd_repo_root

run cargo check
run cargo check --features std
run cargo check --features secure
run cargo check --features guarded
run cargo check --features track-valgrind
run cargo check --features track-asan
run cargo check --features "secure guarded std"
run cargo check --features "bench compare"

if cargo check --features "track-valgrind track-asan" >/tmp/mechanical_mimalloc-feature-conflict.log 2>&1; then
  cat /tmp/mechanical_mimalloc-feature-conflict.log >&2
  echo "expected track-valgrind + track-asan to be rejected" >&2
  exit 1
fi

cat /tmp/mechanical_mimalloc-feature-conflict.log
rm -f /tmp/mechanical_mimalloc-feature-conflict.log
