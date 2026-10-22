#!/usr/bin/env bash

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd_repo_root

toolchain="${SANITIZER_TOOLCHAIN:-nightly}"
sanitizer="${SANITIZER:-address}"
target="${SANITIZER_TARGET:-x86_64-unknown-linux-gnu}"

case "$sanitizer" in
  address)
    rustflags="${RUSTFLAGS:-} --cfg mi_sanitizer_runtime -Zsanitizer=address -Zunstable-options -Cpanic=immediate-abort"
    test_args=(
      --features std
      --test allocation_boundary_cases
      --test aligned_exports
      --test oversize_allocations
      --test fuzz_alloc_sequences
      --test thread_stats_regression
      --test thread_lifecycle
      --test mimalloc_global_alloc
      --test mimalloc_global_allocator_std
    )
    ;;
  thread)
    rustflags="${RUSTFLAGS:-} --cfg mi_sanitizer_runtime -Zsanitizer=thread -Zunstable-options -Cpanic=immediate-abort"
    test_args=(
      --features std
      --test original_test_stress
      --test thread_stats_regression
      --test thread_lifecycle
      --test fuzz_alloc_sequences
    )
    ;;
  *)
    echo "unsupported SANITIZER=$sanitizer; expected address or thread" >&2
    exit 1
    ;;
esac

run env RUSTFLAGS="${rustflags# }" cargo +"${toolchain}" test \
  -Zbuild-std \
  -Zpanic-abort-tests \
  --target-dir "target/sanitizer-${sanitizer}" \
  --target "$target" \
  "${test_args[@]}"
