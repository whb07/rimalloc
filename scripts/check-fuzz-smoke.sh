#!/usr/bin/env bash

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd_repo_root

run cargo check --manifest-path fuzz/Cargo.toml
run cargo run --quiet --manifest-path fuzz/Cargo.toml --bin alloc_sequences -- \
  --runs 64 --max-len 256 fuzz/corpus/alloc_sequences

if command -v cargo-fuzz >/dev/null 2>&1; then
  run cargo fuzz run allocator_transitions -- -runs=256
elif [[ "${REQUIRE_REAL_FUZZ:-0}" == "1" ]]; then
  echo "error: cargo-fuzz is required for this fuzz job" >&2
  exit 1
fi
