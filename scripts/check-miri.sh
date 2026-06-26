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
fi

for filter in "${filters[@]}"; do
  run env MIRIFLAGS="${MIRIFLAGS}" cargo +"${toolchain}" miri test --features std --lib "${filter}"
done
