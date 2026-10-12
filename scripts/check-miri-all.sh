#!/usr/bin/env bash

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd_repo_root

run bash scripts/check-miri.sh "$@"
run bash scripts/check-miri-tree-borrows.sh "$@"
run bash scripts/check-miri-strict-provenance.sh "$@"
run bash scripts/check-miri-strict-tree-borrows.sh "$@"
