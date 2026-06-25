#!/usr/bin/env bash

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd_repo_root

toolchain="${MIRI_TOOLCHAIN:-nightly}"

if ! cargo +"${toolchain}" miri --version >/dev/null 2>&1; then
  echo "cargo-miri is not installed for toolchain '${toolchain}'" >&2
  echo "install it with: rustup +${toolchain} component add miri" >&2
  exit 2
fi

filters=(
  core_types::arena::
  core_types::atomic_wrappers::
  core_types::callbacks::
  core_types::freelist::
  core_types::freelist_wrappers::
  core_types::layout::
  core_types::layout_tests::
  core_types::memid::
  core_types::padding::
  core_types::page::
  core_types::page_queues::
  core_types::page_states::
  core_types::ptr::
  core_types::scalar::
  core_types::track::
  core_types::validation::
)

for filter in "${filters[@]}"; do
  run cargo +"${toolchain}" miri test --features std --lib "${filter}"
done
