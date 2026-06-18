#!/usr/bin/env bash

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd_repo_root

toolchain="${SANITIZER_TOOLCHAIN:-nightly}"
rustflags="${RUSTFLAGS:-} -Zsanitizer=address"

run env RUSTFLAGS="${rustflags# }" cargo +"${toolchain}" test \
  -Zbuild-std \
  -Zbuild-std-features=panic_immediate_abort \
  --target x86_64-unknown-linux-gnu \
  --features std \
  --tests
