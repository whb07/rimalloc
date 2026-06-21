#!/usr/bin/env bash

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd_repo_root

lint_flags=(-D warnings -A dead-code)

run cargo clippy --all-targets --features unstable-raw-internals -- "${lint_flags[@]}"
run cargo clippy --benches --features "bench unstable-raw-internals" -- "${lint_flags[@]}"
run cargo clippy --all-targets --features "guarded secure std bench compare track-valgrind unstable-raw-internals" -- "${lint_flags[@]}"
run cargo clippy --all-targets --features "guarded secure std bench compare track-asan unstable-raw-internals" -- "${lint_flags[@]}"
