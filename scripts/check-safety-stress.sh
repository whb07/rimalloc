#!/usr/bin/env bash

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd_repo_root

run cargo test --quiet --features std --test allocation_boundary_cases
run cargo test --quiet --features std --test aligned_exports
run cargo test --quiet --features std --test oversize_allocations
run cargo test --quiet --features std --test property_alloc_sequences
run cargo test --quiet --features std --test fuzz_alloc_sequences
run cargo test --quiet --features std --test original_test_stress
run cargo test --quiet --features std --test thread_stats_regression
run scripts/check-concurrency-models.sh
