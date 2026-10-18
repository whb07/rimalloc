#!/usr/bin/env bash

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd_repo_root

toolchain="${MIRI_TOOLCHAIN:-nightly}"
borrow_model="${MIRI_BORROW_MODEL:-stacked}"

if ! cargo +"${toolchain}" miri --version >/dev/null 2>&1; then
  echo "cargo-miri is not installed for toolchain '${toolchain}'" >&2
  echo "install it with: rustup +${toolchain} component add miri" >&2
  exit 2
fi

effective_miriflags="${MIRIFLAGS:-}"

append_default_miri_flag() {
  local flag="$1"
  case " ${effective_miriflags} " in
    *" ${flag} "*) ;;
    *) effective_miriflags="${flag}${effective_miriflags:+ ${effective_miriflags}}" ;;
  esac
}

if [[ "${MIRI_SYMBOLIC_ALIGNMENT_CHECK:-1}" != "0" ]]; then
  append_default_miri_flag "-Zmiri-symbolic-alignment-check"
fi

if [[ "${MIRI_STRICT_PROVENANCE:-0}" != "0" ]]; then
  append_default_miri_flag "-Zmiri-strict-provenance"
fi

case "${borrow_model}" in
  stacked | "") ;;
  tree | tree-borrows)
    append_default_miri_flag "-Zmiri-tree-borrows"
    ;;
  *)
    echo "unsupported MIRI_BORROW_MODEL='${borrow_model}'" >&2
    echo "supported values: stacked, tree-borrows" >&2
    exit 2
    ;;
esac

export MIRIFLAGS="${effective_miriflags}"

if (($# > 0)); then
  filters=("$@")
else
  filters=(
    allocator::alloc::tests::
    types::arena::
    types::atomic::
    types::callbacks::
    types::freelist::
    types::freelist_wrappers::
    types::layout::
    types::layout_tests::
    types::memid::
    types::padding::
    types::page::
    types::page_queues::
    types::page_states::
    types::ptr::
    types::scalar::
    types::validation::
  )
fi

test_list="$(
  env MIRIFLAGS="${MIRIFLAGS}" \
    cargo +"${toolchain}" miri test --quiet --features std --lib -- --list
)"

for filter in "${filters[@]}"; do
  selected_count="$(
    printf '%s\n' "${test_list}" \
      | awk -v filter="${filter}" 'index($0, filter) && /: test$/ { count++ } END { print count + 0 }'
  )"
  if [[ "${selected_count}" == "0" ]]; then
    printf 'error: Miri filter selected no tests: %s\n' "${filter}" >&2
    exit 1
  fi
  printf 'selected %s Miri tests for %s\n' "${selected_count}" "${filter}"
  run env MIRIFLAGS="${MIRIFLAGS}" cargo +"${toolchain}" miri test --features std --lib "${filter}"
done
