#!/usr/bin/env bash

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd_repo_root

run scripts/check-fmt.sh
run cargo test --quiet
run cargo check --examples --features std
run scripts/check-api-inventory.py
run scripts/check-ffi-boundary.py
run scripts/check-features.sh
run scripts/check-lints.sh
run scripts/check-layout.sh
run scripts/check-docs.sh
run scripts/check-package.sh
run scripts/check-bench-smoke.sh
