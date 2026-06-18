#!/usr/bin/env bash

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd_repo_root

: "${MIMALLOC_BENCH_SAMPLE_SIZE:=100}"
: "${MIMALLOC_BENCH_MEASUREMENT_SECS:=10}"
: "${MIMALLOC_BENCH_WARMUP_SECS:=1}"

export MIMALLOC_BENCH_SAMPLE_SIZE
export MIMALLOC_BENCH_MEASUREMENT_SECS
export MIMALLOC_BENCH_WARMUP_SECS
export MIMALLOC_BENCH_ALLOCATORS="${MIMALLOC_BENCH_ALLOCATORS:-rust,c}"

run cargo bench --features bench --bench hotpath
run cargo bench --features bench --bench mimalloc_bench -- mimalloc-bench-native
