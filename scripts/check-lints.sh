#!/usr/bin/env bash

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd_repo_root

lint_flags=(-D warnings)

run cargo test --quiet --lib --features std
run cargo clippy --all-targets --features "std unstable-raw-internals" -- "${lint_flags[@]}"
run cargo clippy --benches --features "std bench unstable-raw-internals" -- "${lint_flags[@]}"
run cargo clippy --all-targets --features "guarded secure std bench compare track-valgrind unstable-raw-internals" -- "${lint_flags[@]}"
run cargo clippy --all-targets --features "guarded secure std bench compare track-asan unstable-raw-internals" -- "${lint_flags[@]}"
