#!/usr/bin/env bash

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd_repo_root

run cargo check --benches --features bench

run env \
  MIMALLOC_HOTPATH_SAMPLE_SIZE="${MIMALLOC_HOTPATH_SAMPLE_SIZE:-10}" \
  MIMALLOC_HOTPATH_MEASUREMENT_SECS="${MIMALLOC_HOTPATH_MEASUREMENT_SECS:-1}" \
  MIMALLOC_HOTPATH_WARMUP_SECS="${MIMALLOC_HOTPATH_WARMUP_SECS:-1}" \
  MIMALLOC_HOTPATH_NRESAMPLES="${MIMALLOC_HOTPATH_NRESAMPLES:-1000}" \
  cargo bench --features bench --bench hotpath -- hotpath/paired_alloc_free/mechanical_mimalloc/16

run env \
  MIMALLOC_BENCH_SCENARIOS="${MIMALLOC_BENCH_SCENARIOS:-glibc-thread,malloc-large,cache-scratchN,xmalloc-testN}" \
  MIMALLOC_BENCH_ALLOCATORS="${MIMALLOC_BENCH_ALLOCATORS:-rust,c}" \
  MIMALLOC_BENCH_SAMPLE_SIZE="${MIMALLOC_BENCH_SAMPLE_SIZE:-10}" \
  MIMALLOC_BENCH_MEASUREMENT_SECS="${MIMALLOC_BENCH_MEASUREMENT_SECS:-1}" \
  MIMALLOC_BENCH_WARMUP_SECS="${MIMALLOC_BENCH_WARMUP_SECS:-1}" \
  cargo bench --features bench --bench mimalloc_bench -- mimalloc-bench-native
