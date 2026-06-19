#!/usr/bin/env bash

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd_repo_root

feature_sets=(
  ""
  "secure"
  "guarded"
  "track-valgrind"
  "track-asan"
)

for features in "${feature_sets[@]}"; do
  if [[ -z "${features}" ]]; then
    echo "+ ABI/layout feature set: default"
    run scripts/check-public-abi.py
    run scripts/check-public-symbols.py
  elif [[ "${features}" == "track-asan" ]]; then
    echo "+ track-asan uses ASan runtime symbols, so the executable layout probe"
    echo "+ is covered by the non-ASan feature sets; checking compilation and exports"
    run cargo check --features track-asan
    run env MIMALLOC_CHECK_FEATURES="${features}" scripts/check-public-symbols.py
  else
    echo "+ ABI/layout feature set: ${features}"
    run env MIMALLOC_CHECK_FEATURES="${features}" scripts/check-public-abi.py
    run env MIMALLOC_CHECK_FEATURES="${features}" scripts/check-public-symbols.py
  fi
done
