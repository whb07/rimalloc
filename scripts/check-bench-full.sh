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

cat >"$default_target_dir/commands.txt" <<EOF
CARGO_TARGET_DIR=$default_target_dir cargo check --benches --features bench
CARGO_TARGET_DIR=$default_target_dir cargo bench --features bench --bench hotpath
CARGO_TARGET_DIR=$default_target_dir cargo bench --features bench --bench workloads
CARGO_TARGET_DIR=$default_target_dir cargo bench --features bench --bench evidence
CARGO_TARGET_DIR=$default_target_dir cargo bench --features bench --bench mimalloc_bench -- mimalloc-bench-native
scripts/run-bench-resources.py --artifact-dir $default_target_dir --artifact-mode default-rust --feature bench
EOF
cat >"$interpose_target_dir/commands.txt" <<EOF
CARGO_TARGET_DIR=$interpose_target_dir cargo check --benches --features "bench interpose"
CARGO_TARGET_DIR=$interpose_target_dir cargo bench --features "bench interpose" --bench hotpath
scripts/run-bench-resources.py --artifact-dir $interpose_target_dir --artifact-mode interpose --feature bench --feature interpose
EOF

run scripts/write-benchmark-manifest.py \
  --artifact-dir "$default_target_dir" \
  --artifact-mode default-rust \
  --feature bench \
  --commands "$default_target_dir/commands.txt"
run scripts/write-benchmark-manifest.py \
  --artifact-dir "$interpose_target_dir" \
  --artifact-mode interpose \
  --feature bench \
  --feature interpose \
  --commands "$interpose_target_dir/commands.txt"

run env CARGO_TARGET_DIR="$default_target_dir" cargo check --benches --features bench
run env CARGO_TARGET_DIR="$interpose_target_dir" cargo check --benches --features "bench interpose"
run scripts/check-bench-thresholds.py --self-test
run env CARGO_TARGET_DIR="$default_target_dir" cargo bench --features bench --bench hotpath
run env CARGO_TARGET_DIR="$default_target_dir" cargo bench --features bench --bench workloads
run env CARGO_TARGET_DIR="$default_target_dir" cargo bench --features bench --bench evidence
run env CARGO_TARGET_DIR="$default_target_dir" cargo bench --features bench --bench mimalloc_bench -- mimalloc-bench-native
run scripts/check-bench-thresholds.py \
  --thresholds benchmarks/thresholds.json \
  --criterion-dir "$default_target_dir/criterion" \
  --csv-out "$default_target_dir/threshold-results.csv" \
  --artifact-mode default-rust \
  --feature bench
run scripts/run-bench-resources.py \
  --artifact-dir "$default_target_dir" \
  --artifact-mode default-rust \
  --feature bench \
  --operations "${MIMALLOC_BENCH_RESOURCE_OPERATIONS:-100000}" \
  --repeats "${MIMALLOC_BENCH_RESOURCE_REPEATS:-3}"
run scripts/check-bench-resources.py \
  --results "$default_target_dir/resource-results.jsonl" \
  --artifact-mode default-rust \
  --feature bench
run env CARGO_TARGET_DIR="$interpose_target_dir" cargo bench --features "bench interpose" --bench hotpath
run scripts/check-bench-thresholds.py \
  --thresholds benchmarks/thresholds.json \
  --criterion-dir "$interpose_target_dir/criterion" \
  --csv-out "$interpose_target_dir/threshold-results.csv" \
  --artifact-mode interpose \
  --feature bench \
  --feature interpose \
  --allow-missing
run scripts/run-bench-resources.py \
  --artifact-dir "$interpose_target_dir" \
  --artifact-mode interpose \
  --feature bench \
  --feature interpose \
  --operations "${MIMALLOC_BENCH_RESOURCE_OPERATIONS:-100000}" \
  --repeats "${MIMALLOC_BENCH_RESOURCE_REPEATS:-3}"
run scripts/check-bench-resources.py \
  --results "$interpose_target_dir/resource-results.jsonl" \
  --artifact-mode interpose \
  --feature bench \
  --feature interpose
run scripts/check-benchmark-manifest.py \
  --artifact-dir "$default_target_dir" \
  --artifact-mode default-rust \
  --feature bench
run scripts/check-benchmark-manifest.py \
  --artifact-dir "$interpose_target_dir" \
  --artifact-mode interpose \
  --feature bench \
  --feature interpose
