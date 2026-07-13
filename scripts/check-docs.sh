#!/usr/bin/env bash

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd_repo_root

run cargo doc --no-deps --no-default-features
run cargo doc --no-deps --features std
run cargo test --doc --features std
run cargo test --doc --features "std c-header-macros"
run scripts/check-safe-docs.py
run scripts/check-benchmark-doc-claims.py
