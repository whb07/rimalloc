#!/usr/bin/env bash

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd_repo_root

run cargo clippy --all-targets -- -D warnings
run cargo clippy --benches --features bench -- -D warnings
run cargo clippy --all-targets --features "guarded secure std bench compare track-valgrind" -- -D warnings
run cargo clippy --all-targets --features "guarded secure std bench compare track-asan" -- -D warnings
