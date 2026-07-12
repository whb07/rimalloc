#!/usr/bin/env bash

set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd_repo_root

base_dir="${MIMALLOC_BASELINE_DIR:-target/bench-full/baseline}"
sample_size="${MIMALLOC_BASELINE_SAMPLE_SIZE:-20}"
measurement_secs="${MIMALLOC_BASELINE_MEASUREMENT_SECS:-5}"
warmup_secs="${MIMALLOC_BASELINE_WARMUP_SECS:-2}"
resource_operations="${MIMALLOC_BASELINE_RESOURCE_OPERATIONS:-100000}"
resource_repeats="${MIMALLOC_BASELINE_RESOURCE_REPEATS:-3}"
host_profile="${MIMALLOC_BASELINE_HOST_PROFILE:-release-linux-x86_64-primary}"
scenarios="${MIMALLOC_BASELINE_SCENARIOS:-glibc-thread,malloc-large,cache-scratch1,xmalloc-testN}"
thresholds="${MIMALLOC_BASELINE_THRESHOLDS:-benchmarks/thresholds.json}"
runs=()

for run_number in 1 2 3 4 5; do
  artifact_dir="$base_dir/run-$run_number/default-rust"
  runs+=("$artifact_dir")
  run scripts/run-bench-clean.sh "$artifact_dir"
  cat >"$artifact_dir/commands.txt" <<EOF
MIMALLOC_HOTPATH_SAMPLE_SIZE=$sample_size MIMALLOC_HOTPATH_MEASUREMENT_SECS=$measurement_secs MIMALLOC_HOTPATH_WARMUP_SECS=$warmup_secs CARGO_TARGET_DIR=$artifact_dir cargo bench --features bench --bench hotpath -- hotpath/paired_alloc_free/mechanical_mimalloc_public/16
MIMALLOC_BENCH_SCENARIOS=$scenarios MIMALLOC_BENCH_ALLOCATORS=mechanical_mimalloc_raw,c-mimalloc-3.3.2 MIMALLOC_BENCH_SAMPLE_SIZE=$sample_size MIMALLOC_BENCH_MEASUREMENT_SECS=$measurement_secs MIMALLOC_BENCH_WARMUP_SECS=$warmup_secs CARGO_TARGET_DIR=$artifact_dir cargo bench --features bench --bench mimalloc_bench -- mimalloc-bench-native
scripts/run-bench-resources.py --artifact-dir $artifact_dir --artifact-mode default-rust --feature bench --operations $resource_operations --repeats $resource_repeats
scripts/check-bench-thresholds.py --thresholds $thresholds --criterion-dir $artifact_dir/criterion --artifact-mode default-rust --feature bench --report-only
scripts/check-bench-resources.py --results $artifact_dir/resource-results.jsonl --artifact-mode default-rust --feature bench
EOF
  run scripts/write-benchmark-manifest.py \
    --artifact-dir "$artifact_dir" \
    --artifact-mode default-rust \
    --feature bench \
    --commands "$artifact_dir/commands.txt"
  run env \
    CARGO_TARGET_DIR="$artifact_dir" \
    MIMALLOC_HOTPATH_SAMPLE_SIZE="$sample_size" \
    MIMALLOC_HOTPATH_MEASUREMENT_SECS="$measurement_secs" \
    MIMALLOC_HOTPATH_WARMUP_SECS="$warmup_secs" \
    cargo bench --features bench --bench hotpath -- \
      hotpath/paired_alloc_free/mechanical_mimalloc_public/16
  run env \
    CARGO_TARGET_DIR="$artifact_dir" \
    MIMALLOC_BENCH_SCENARIOS="$scenarios" \
    MIMALLOC_BENCH_ALLOCATORS="mechanical_mimalloc_raw,c-mimalloc-3.3.2" \
    MIMALLOC_BENCH_SAMPLE_SIZE="$sample_size" \
    MIMALLOC_BENCH_MEASUREMENT_SECS="$measurement_secs" \
    MIMALLOC_BENCH_WARMUP_SECS="$warmup_secs" \
    cargo bench --features bench --bench mimalloc_bench -- mimalloc-bench-native
  run scripts/check-bench-thresholds.py \
    --thresholds "$thresholds" \
    --criterion-dir "$artifact_dir/criterion" \
    --csv-out "$artifact_dir/threshold-results.csv" \
    --artifact-mode default-rust \
    --feature bench \
    --report-only
  run scripts/run-bench-resources.py \
    --artifact-dir "$artifact_dir" \
    --artifact-mode default-rust \
    --feature bench \
    --operations "$resource_operations" \
    --repeats "$resource_repeats"
  run scripts/check-bench-resources.py \
    --results "$artifact_dir/resource-results.jsonl" \
    --artifact-mode default-rust \
    --feature bench
  run scripts/check-benchmark-manifest.py \
    --artifact-dir "$artifact_dir" \
    --artifact-mode default-rust \
    --feature bench
done

capture_args=()
for artifact_dir in "${runs[@]}"; do
  capture_args+=(--run-dir "$artifact_dir")
done
run scripts/capture-benchmark-baseline.py \
  "${capture_args[@]}" \
  --thresholds "$thresholds" \
  --artifact-mode default-rust \
  --feature bench \
  --host-profile "$host_profile" \
  --output "$base_dir/proposed-thresholds.json"
