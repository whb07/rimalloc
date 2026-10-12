#!/usr/bin/env bash

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd_repo_root

test_feature_sets=(
  ""
  "std"
  "secure"
  "guarded"
  "track-valgrind"
)

for features in "${test_feature_sets[@]}"; do
  if [[ -z "${features}" ]]; then
    echo "+ safe API tests feature set: default"
    run cargo test --quiet --lib safe::tests
  else
    echo "+ safe API tests feature set: ${features}"
    run cargo test --quiet --lib safe::tests --features "${features}"
  fi
done

echo "+ safe API feature set: track-asan"
echo "+ track-asan requires ASan runtime symbols for executable tests; checking compilation"
run cargo check --lib --features track-asan
