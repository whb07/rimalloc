#!/usr/bin/env bash

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd_repo_root

if ! cargo miri --version >/dev/null 2>&1; then
  echo "cargo-miri is not installed for this toolchain" >&2
  echo "install it with: rustup component add miri" >&2
  exit 2
fi

run cargo miri test --features std --lib
