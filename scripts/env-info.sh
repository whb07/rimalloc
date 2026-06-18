#!/usr/bin/env bash

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd_repo_root

run git rev-parse --show-toplevel
run git rev-parse HEAD
run rustc --version --verbose
run cargo --version --verbose
run uname -a
if command -v lscpu >/dev/null 2>&1; then
  run lscpu
fi
if command -v cc >/dev/null 2>&1; then
  run cc --version
fi
