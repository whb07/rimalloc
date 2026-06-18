#!/usr/bin/env bash

set -euo pipefail

repo_root() {
  cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd
}

cd_repo_root() {
  cd "$(repo_root)"
}

run() {
  printf '+'
  printf ' %q' "$@"
  printf '\n'
  "$@"
}
