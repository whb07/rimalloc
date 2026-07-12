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

default_target_dir="${MIMALLOC_BENCH_DEFAULT_TARGET_DIR:-target/bench-full/default-rust}"
interpose_target_dir="${MIMALLOC_BENCH_INTERPOSE_TARGET_DIR:-target/bench-full/interpose}"

run scripts/run-bench-clean.sh "$default_target_dir" "$interpose_target_dir"

run env CARGO_TARGET_DIR="$default_target_dir" cargo check --benches --features "bench compare"
run env CARGO_TARGET_DIR="$interpose_target_dir" cargo check --benches --features "bench interpose"
run scripts/check-bench-thresholds.py --self-test
run env CARGO_TARGET_DIR="$default_target_dir" cargo bench --features bench --bench hotpath
run env CARGO_TARGET_DIR="$default_target_dir" cargo bench --features "bench compare" --bench hotpath -- hotpath/paired_alloc_free
run env CARGO_TARGET_DIR="$default_target_dir" cargo bench --features bench --bench workloads
run env CARGO_TARGET_DIR="$default_target_dir" cargo bench --features bench --bench mimalloc_bench -- mimalloc-bench-native
run scripts/check-bench-thresholds.py \
  --thresholds benchmarks/thresholds.json \
  --criterion-dir "$default_target_dir/criterion" \
  --csv-out "$default_target_dir/criterion/benchmark-thresholds-full.csv"
run env CARGO_TARGET_DIR="$interpose_target_dir" cargo bench --features "bench interpose" --bench hotpath
run scripts/check-bench-thresholds.py \
  --thresholds benchmarks/thresholds.json \
  --criterion-dir "$interpose_target_dir/criterion" \
  --csv-out "$interpose_target_dir/criterion/benchmark-thresholds-full.csv" \
  --allow-missing
