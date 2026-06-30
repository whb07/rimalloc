#!/usr/bin/env bash

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd_repo_root

run scripts/check-public-symbols.py
run scripts/check-public-symbols.py --dynamic-mi
run scripts/check-public-symbols.py --dynamic-groups
run scripts/check-public-symbols.py --dynamic-manifests
run scripts/check-public-symbols.py --dynamic-manifest-self-test
