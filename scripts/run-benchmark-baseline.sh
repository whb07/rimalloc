#!/usr/bin/env bash

set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd_repo_root

base_dir="${MIMALLOC_BASELINE_DIR:-target/bench-full/baseline}"
hotpath_sample_size="${MIMALLOC_BASELINE_HOTPATH_SAMPLE_SIZE:-100}"
hotpath_measurement_secs="${MIMALLOC_BASELINE_HOTPATH_MEASUREMENT_SECS:-10}"
hotpath_warmup_secs="${MIMALLOC_BASELINE_HOTPATH_WARMUP_SECS:-1}"
native_sample_size="${MIMALLOC_BASELINE_NATIVE_SAMPLE_SIZE:-20}"
native_measurement_secs="${MIMALLOC_BASELINE_NATIVE_MEASUREMENT_SECS:-5}"
native_warmup_secs="${MIMALLOC_BASELINE_NATIVE_WARMUP_SECS:-2}"
cooldown_secs="${MIMALLOC_BASELINE_COOLDOWN_SECS:-60}"
benchmark_cpu="${MIMALLOC_BASELINE_CPU:-0}"
resource_operations="${MIMALLOC_BASELINE_RESOURCE_OPERATIONS:-20000000}"
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
CARGO_TARGET_DIR=$artifact_dir cargo bench --no-run --features bench --bench hotpath --bench mimalloc_bench
MIMALLOC_HOTPATH_SAMPLE_SIZE=$hotpath_sample_size MIMALLOC_HOTPATH_MEASUREMENT_SECS=$hotpath_measurement_secs MIMALLOC_HOTPATH_WARMUP_SECS=$hotpath_warmup_secs CARGO_TARGET_DIR=$artifact_dir taskset --cpu-list $benchmark_cpu cargo bench --features bench --bench hotpath -- hotpath/paired_alloc_free/rimalloc_public/16
sleep $cooldown_secs
MIMALLOC_BENCH_SCENARIOS=$scenarios MIMALLOC_BENCH_ALLOCATORS=rimalloc_raw,c-mimalloc-3.3.2 MIMALLOC_BENCH_SAMPLE_SIZE=$native_sample_size MIMALLOC_BENCH_MEASUREMENT_SECS=$native_measurement_secs MIMALLOC_BENCH_WARMUP_SECS=$native_warmup_secs CARGO_TARGET_DIR=$artifact_dir cargo bench --features bench --bench mimalloc_bench -- mimalloc-bench-native
scripts/run-bench-resources.py --artifact-dir $artifact_dir --artifact-mode default-rust --feature bench --operations $resource_operations --repeats $resource_repeats
scripts/run-evidence-report.py --artifact-dir $artifact_dir
scripts/check-evidence-report.py --report $artifact_dir/evidence-report.json
scripts/check-bench-thresholds.py --thresholds $thresholds --criterion-dir $artifact_dir/criterion --csv-out $artifact_dir/threshold-results.csv --review-out $artifact_dir/benchmark-review.json --artifact-mode default-rust --feature bench --report-only
scripts/check-bench-resources.py --results $artifact_dir/resource-results.jsonl --artifact-mode default-rust --feature bench
scripts/check-benchmark-manifest.py --artifact-dir $artifact_dir --artifact-mode default-rust --feature bench --allow-hard-failure
EOF
  run env \
    MIMALLOC_HOTPATH_SAMPLE_SIZE="$hotpath_sample_size" \
    MIMALLOC_HOTPATH_MEASUREMENT_SECS="$hotpath_measurement_secs" \
    MIMALLOC_HOTPATH_WARMUP_SECS="$hotpath_warmup_secs" \
    MIMALLOC_BENCH_SAMPLE_SIZE="$native_sample_size" \
    MIMALLOC_BENCH_MEASUREMENT_SECS="$native_measurement_secs" \
    MIMALLOC_BENCH_WARMUP_SECS="$native_warmup_secs" \
    MIMALLOC_BENCH_SCENARIOS="$scenarios" \
    MIMALLOC_BENCH_ALLOCATORS="rimalloc_raw,c-mimalloc-3.3.2" \
    MIMALLOC_BENCH_RESOURCE_OPERATIONS="$resource_operations" \
    MIMALLOC_BENCH_RESOURCE_REPEATS="$resource_repeats" \
    scripts/write-benchmark-manifest.py \
    --artifact-dir "$artifact_dir" \
    --artifact-mode default-rust \
    --feature bench \
    --commands "$artifact_dir/commands.txt"
  run env CARGO_TARGET_DIR="$artifact_dir" \
    cargo bench --no-run --features bench --bench hotpath --bench mimalloc_bench
done

# Keep the five fresh-process microbenchmark replicas adjacent. Spreading them
# across the sustained native and resource workloads turns host phase changes
# into apparent allocator variance.
for artifact_dir in "${runs[@]}"; do
  run env \
    CARGO_TARGET_DIR="$artifact_dir" \
    MIMALLOC_HOTPATH_SAMPLE_SIZE="$hotpath_sample_size" \
    MIMALLOC_HOTPATH_MEASUREMENT_SECS="$hotpath_measurement_secs" \
    MIMALLOC_HOTPATH_WARMUP_SECS="$hotpath_warmup_secs" \
    taskset --cpu-list "$benchmark_cpu" cargo bench --features bench --bench hotpath -- \
      hotpath/paired_alloc_free/rimalloc_public/16
done

for artifact_dir in "${runs[@]}"; do
  run sleep "$cooldown_secs"
  run env \
    CARGO_TARGET_DIR="$artifact_dir" \
    MIMALLOC_BENCH_SCENARIOS="$scenarios" \
    MIMALLOC_BENCH_ALLOCATORS="rimalloc_raw,c-mimalloc-3.3.2" \
    MIMALLOC_BENCH_SAMPLE_SIZE="$native_sample_size" \
    MIMALLOC_BENCH_MEASUREMENT_SECS="$native_measurement_secs" \
    MIMALLOC_BENCH_WARMUP_SECS="$native_warmup_secs" \
    cargo bench --features bench --bench mimalloc_bench -- mimalloc-bench-native
  run scripts/check-bench-thresholds.py \
    --thresholds "$thresholds" \
    --criterion-dir "$artifact_dir/criterion" \
    --csv-out "$artifact_dir/threshold-results.csv" \
    --review-out "$artifact_dir/benchmark-review.json" \
    --artifact-mode default-rust \
    --feature bench \
    --report-only
  run scripts/run-evidence-report.py --artifact-dir "$artifact_dir"
  run scripts/check-evidence-report.py --report "$artifact_dir/evidence-report.json"
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
    --feature bench \
    --allow-hard-failure
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
