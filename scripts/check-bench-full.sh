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

run cargo check --benches --features "bench compare"
run scripts/check-bench-thresholds.py --self-test
run cargo bench --features bench --bench hotpath
run cargo bench --features "bench compare" --bench hotpath -- hotpath/paired_alloc_free
run cargo bench --features bench --bench workloads
run cargo bench --features bench --bench mimalloc_bench -- mimalloc-bench-native
run scripts/check-bench-thresholds.py \
  --thresholds benchmarks/thresholds.json \
  --criterion-dir target/criterion \
  --csv-out target/criterion/benchmark-thresholds-full.csv
