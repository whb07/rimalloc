#!/usr/bin/env bash

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd_repo_root

run cargo package --list --allow-dirty
run cargo package --allow-dirty --no-verify
