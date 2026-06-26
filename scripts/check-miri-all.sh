#!/usr/bin/env bash

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd_repo_root

run scripts/check-miri.sh "$@"
run scripts/check-miri-tree-borrows.sh "$@"
