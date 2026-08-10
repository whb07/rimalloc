#!/usr/bin/env bash

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd_repo_root

model_filter="types::atomic::concurrency_models"
if [[ "${MIMALLOC_TEST_CONCURRENCY_MODEL_ZERO_MATCH:-0}" == "1" ]]; then
  model_filter="does::not::exist"
fi
printf '+ cargo test --quiet --lib %q -- --list\n' "$model_filter"
model_list="$(cargo test --quiet --lib "$model_filter" -- --list)"
model_count="$(printf '%s\n' "$model_list" | awk '/: test$/ { count++ } END { print count + 0 }')"
if ((model_count == 0)); then
  printf 'error: concurrency model filter %q selected zero tests\n' "$model_filter" >&2
  exit 1
fi

printf 'selected %d concurrency model tests\n' "$model_count"
run env LOOM_MAX_PREEMPTIONS="${LOOM_MAX_PREEMPTIONS:-3}" \
  cargo test --quiet --lib "$model_filter"
