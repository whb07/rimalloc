#!/usr/bin/env bash

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd_repo_root

run scripts/check-public-abi.py
run cargo test --test release_fast_path_audit --quiet --features "std unstable-raw-internals"
