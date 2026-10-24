#!/usr/bin/env bash

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd_repo_root

toolchain="${MIRI_TOOLCHAIN:-$(cat scripts/miri-toolchain)}"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-target/miri-${toolchain}}"
borrow_model="${MIRI_BORROW_MODEL:-stacked}"
features="${MIRI_FEATURES:-std}"
profile_args=()
case "${MIRI_PROFILE:-debug}" in
  debug) ;;
  release) profile_args=(--release) ;;
  *)
    echo "unsupported MIRI_PROFILE; expected debug or release" >&2
    exit 2
    ;;
esac

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

if [[ -n "${MIRI_SEEDS:-}" ]]; then
  append_default_miri_flag "-Zmiri-many-seeds=${MIRI_SEEDS}"
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
  # Run new library tests automatically. Individual OS-dependent tests explain
  # their Miri exclusions with cfg_attr(miri, ignore = "...").
  filters=("")
fi

test_list="$(
  env MIRIFLAGS="${MIRIFLAGS}" \
    cargo +"${toolchain}" miri test --quiet "${profile_args[@]}" --features "${features}" --lib -- --list
)"

for filter in "${filters[@]}"; do
  selected_count="$(
    printf '%s\n' "${test_list}" \
      | awk -v filter="${filter}" '(filter == "" || index($0, filter)) && /: test$/ { count++ } END { print count + 0 }'
  )"
  if [[ "${selected_count}" == "0" ]]; then
    printf 'error: Miri filter selected no tests: %s\n' "${filter}" >&2
    exit 1
  fi
  printf 'selected %s Miri tests for %s\n' "${selected_count}" "${filter:-all library tests}"
  run env MIRIFLAGS="${MIRIFLAGS}" cargo +"${toolchain}" miri test "${profile_args[@]}" --features "${features}" --lib "${filter}"
done
