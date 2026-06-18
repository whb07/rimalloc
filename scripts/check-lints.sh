#!/usr/bin/env bash

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd_repo_root

run cargo clippy --all-targets --features unstable-raw-internals -- -D warnings
run cargo clippy --benches --features "bench unstable-raw-internals" -- -D warnings
run cargo clippy --all-targets --features "guarded secure std bench compare track-valgrind unstable-raw-internals" -- -D warnings
run cargo clippy --all-targets --features "guarded secure std bench compare track-asan unstable-raw-internals" -- -D warnings
