#!/usr/bin/env bash

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd_repo_root

run cargo check --manifest-path fuzz/Cargo.toml
run cargo run --quiet --manifest-path fuzz/Cargo.toml --bin alloc_sequences -- \
  --runs 64 --max-len 256 fuzz/corpus/alloc_sequences
