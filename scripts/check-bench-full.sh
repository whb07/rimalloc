#!/usr/bin/env bash

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd_repo_root

: "${MIMALLOC_BENCH_SAMPLE_SIZE:=100}"
: "${MIMALLOC_BENCH_MEASUREMENT_SECS:=10}"
: "${MIMALLOC_BENCH_WARMUP_SECS:=1}"
: "${MIMALLOC_HOTPATH_SAMPLE_SIZE:=100}"
: "${MIMALLOC_HOTPATH_MEASUREMENT_SECS:=10}"
: "${MIMALLOC_HOTPATH_WARMUP_SECS:=1}"
: "${MIMALLOC_BENCH_RESOURCE_OPERATIONS:=100000}"
: "${MIMALLOC_BENCH_RESOURCE_REPEATS:=3}"
: "${MIMALLOC_BENCH_SCENARIOS:=glibc-thread,malloc-large,cache-scratch1,xmalloc-testN}"

export MIMALLOC_BENCH_SAMPLE_SIZE
export MIMALLOC_BENCH_MEASUREMENT_SECS
export MIMALLOC_BENCH_WARMUP_SECS
export MIMALLOC_HOTPATH_SAMPLE_SIZE
export MIMALLOC_HOTPATH_MEASUREMENT_SECS
export MIMALLOC_HOTPATH_WARMUP_SECS
export MIMALLOC_BENCH_RESOURCE_OPERATIONS
export MIMALLOC_BENCH_RESOURCE_REPEATS
export MIMALLOC_BENCH_SCENARIOS
export MIMALLOC_BENCH_ALLOCATORS="${MIMALLOC_BENCH_ALLOCATORS:-mechanical_mimalloc_raw,c-mimalloc-3.3.2}"

default_target_dir="${MIMALLOC_BENCH_DEFAULT_TARGET_DIR:-target/bench-full/default-rust}"
interpose_target_dir="${MIMALLOC_BENCH_INTERPOSE_TARGET_DIR:-target/bench-full/interpose-feature-linked}"
preload_target_dir="${MIMALLOC_BENCH_PRELOAD_TARGET_DIR:-target/bench-full/ld-preload-interpose}"

run scripts/run-bench-clean.sh "$default_target_dir" "$interpose_target_dir" "$preload_target_dir"

cat >"$default_target_dir/commands.txt" <<EOF
CARGO_TARGET_DIR=$default_target_dir cargo check --benches --features bench
MIMALLOC_HOTPATH_SAMPLE_SIZE=$MIMALLOC_HOTPATH_SAMPLE_SIZE MIMALLOC_HOTPATH_MEASUREMENT_SECS=$MIMALLOC_HOTPATH_MEASUREMENT_SECS MIMALLOC_HOTPATH_WARMUP_SECS=$MIMALLOC_HOTPATH_WARMUP_SECS CARGO_TARGET_DIR=$default_target_dir cargo bench --features bench --bench hotpath
CARGO_TARGET_DIR=$default_target_dir cargo bench --features bench --bench workloads
CARGO_TARGET_DIR=$default_target_dir cargo bench --features bench --bench evidence
MIMALLOC_BENCH_SCENARIOS=$MIMALLOC_BENCH_SCENARIOS MIMALLOC_BENCH_ALLOCATORS=$MIMALLOC_BENCH_ALLOCATORS MIMALLOC_BENCH_SAMPLE_SIZE=$MIMALLOC_BENCH_SAMPLE_SIZE MIMALLOC_BENCH_MEASUREMENT_SECS=$MIMALLOC_BENCH_MEASUREMENT_SECS MIMALLOC_BENCH_WARMUP_SECS=$MIMALLOC_BENCH_WARMUP_SECS CARGO_TARGET_DIR=$default_target_dir cargo bench --features bench --bench mimalloc_bench -- mimalloc-bench-native
scripts/run-evidence-report.py --artifact-dir $default_target_dir
scripts/check-evidence-report.py --report $default_target_dir/evidence-report.json
scripts/check-bench-thresholds.py --thresholds benchmarks/thresholds.json --criterion-dir $default_target_dir/criterion --csv-out $default_target_dir/threshold-results.csv --review-out $default_target_dir/benchmark-review.json --artifact-mode default-rust --feature bench
scripts/run-bench-resources.py --artifact-dir $default_target_dir --artifact-mode default-rust --feature bench --operations $MIMALLOC_BENCH_RESOURCE_OPERATIONS --repeats $MIMALLOC_BENCH_RESOURCE_REPEATS
EOF
cat >"$interpose_target_dir/commands.txt" <<EOF
CARGO_TARGET_DIR=$interpose_target_dir cargo check --benches --features "bench interpose"
MIMALLOC_HOTPATH_SAMPLE_SIZE=$MIMALLOC_HOTPATH_SAMPLE_SIZE MIMALLOC_HOTPATH_MEASUREMENT_SECS=$MIMALLOC_HOTPATH_MEASUREMENT_SECS MIMALLOC_HOTPATH_WARMUP_SECS=$MIMALLOC_HOTPATH_WARMUP_SECS CARGO_TARGET_DIR=$interpose_target_dir cargo bench --features "bench interpose" --bench hotpath
scripts/check-bench-thresholds.py --thresholds benchmarks/thresholds.json --criterion-dir $interpose_target_dir/criterion --csv-out $interpose_target_dir/threshold-results.csv --review-out $interpose_target_dir/benchmark-review.json --artifact-mode interpose-feature-linked --feature bench --feature interpose --allow-no-applicable
scripts/run-bench-resources.py --artifact-dir $interpose_target_dir --artifact-mode interpose-feature-linked --feature bench --feature interpose --operations $MIMALLOC_BENCH_RESOURCE_OPERATIONS --repeats $MIMALLOC_BENCH_RESOURCE_REPEATS
EOF
cat >"$preload_target_dir/commands.txt" <<EOF
scripts/run-bench-resources.py --artifact-dir $preload_target_dir --artifact-mode ld-preload-interpose --feature interpose --ld-preload --operations $MIMALLOC_BENCH_RESOURCE_OPERATIONS --repeats $MIMALLOC_BENCH_RESOURCE_REPEATS
EOF

run scripts/write-benchmark-manifest.py \
  --artifact-dir "$default_target_dir" \
  --artifact-mode default-rust \
  --feature bench \
  --commands "$default_target_dir/commands.txt"
run scripts/write-benchmark-manifest.py \
  --artifact-dir "$interpose_target_dir" \
  --artifact-mode interpose-feature-linked \
  --feature bench \
  --feature interpose \
  --commands "$interpose_target_dir/commands.txt"
run scripts/write-benchmark-manifest.py \
  --artifact-dir "$preload_target_dir" \
  --artifact-mode ld-preload-interpose \
  --feature interpose \
  --commands "$preload_target_dir/commands.txt"

run env CARGO_TARGET_DIR="$default_target_dir" cargo check --benches --features bench
run env CARGO_TARGET_DIR="$interpose_target_dir" cargo check --benches --features "bench interpose"
run scripts/check-bench-thresholds.py --self-test
run env CARGO_TARGET_DIR="$default_target_dir" cargo bench --features bench --bench hotpath
run env CARGO_TARGET_DIR="$default_target_dir" cargo bench --features bench --bench workloads
run env CARGO_TARGET_DIR="$default_target_dir" cargo bench --features bench --bench evidence
run env CARGO_TARGET_DIR="$default_target_dir" cargo bench --features bench --bench mimalloc_bench -- mimalloc-bench-native
run scripts/run-evidence-report.py --artifact-dir "$default_target_dir"
run scripts/check-evidence-report.py --report "$default_target_dir/evidence-report.json"
run scripts/check-bench-thresholds.py \
  --thresholds benchmarks/thresholds.json \
  --criterion-dir "$default_target_dir/criterion" \
  --csv-out "$default_target_dir/threshold-results.csv" \
  --review-out "$default_target_dir/benchmark-review.json" \
  --artifact-mode default-rust \
  --feature bench
run scripts/run-bench-resources.py \
  --artifact-dir "$default_target_dir" \
  --artifact-mode default-rust \
  --feature bench \
  --operations "$MIMALLOC_BENCH_RESOURCE_OPERATIONS" \
  --repeats "$MIMALLOC_BENCH_RESOURCE_REPEATS"
run scripts/check-bench-resources.py \
  --results "$default_target_dir/resource-results.jsonl" \
  --artifact-mode default-rust \
  --feature bench
run env CARGO_TARGET_DIR="$interpose_target_dir" cargo bench --features "bench interpose" --bench hotpath
run scripts/check-bench-thresholds.py \
  --thresholds benchmarks/thresholds.json \
  --criterion-dir "$interpose_target_dir/criterion" \
  --csv-out "$interpose_target_dir/threshold-results.csv" \
  --review-out "$interpose_target_dir/benchmark-review.json" \
  --artifact-mode interpose-feature-linked \
  --feature bench \
  --feature interpose \
  --allow-no-applicable
run scripts/run-bench-resources.py \
  --artifact-dir "$interpose_target_dir" \
  --artifact-mode interpose-feature-linked \
  --feature bench \
  --feature interpose \
  --operations "$MIMALLOC_BENCH_RESOURCE_OPERATIONS" \
  --repeats "$MIMALLOC_BENCH_RESOURCE_REPEATS"
run scripts/check-bench-resources.py \
  --results "$interpose_target_dir/resource-results.jsonl" \
  --artifact-mode interpose-feature-linked \
  --feature bench \
  --feature interpose
run scripts/run-bench-resources.py \
  --artifact-dir "$preload_target_dir" \
  --artifact-mode ld-preload-interpose \
  --feature interpose \
  --ld-preload \
  --operations "$MIMALLOC_BENCH_RESOURCE_OPERATIONS" \
  --repeats "$MIMALLOC_BENCH_RESOURCE_REPEATS"
run scripts/check-bench-resources.py \
  --results "$preload_target_dir/resource-results.jsonl" \
  --artifact-mode ld-preload-interpose \
  --feature interpose \
  --required-allocator preloaded-libc
run scripts/check-benchmark-manifest.py \
  --artifact-dir "$default_target_dir" \
  --artifact-mode default-rust \
  --feature bench
run scripts/check-benchmark-manifest.py \
  --artifact-dir "$interpose_target_dir" \
  --artifact-mode interpose-feature-linked \
  --feature bench \
  --feature interpose
run scripts/check-benchmark-manifest.py \
  --artifact-dir "$preload_target_dir" \
  --artifact-mode ld-preload-interpose \
  --feature interpose
run scripts/capture-release-assembly.sh
run scripts/archive-benchmark-evidence.py \
  --artifact-dir "$default_target_dir" \
  --artifact-dir "$interpose_target_dir" \
  --artifact-dir "$preload_target_dir" \
  --assembly-dir "${MIMALLOC_ASM_OUT_DIR:-target/strict-type-assembly}"
