#!/usr/bin/env bash

set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd_repo_root

repo_root="$(git rev-parse --show-toplevel)"
home_dir="${HOME:-}"

if [[ "$#" -eq 0 ]]; then
  echo "usage: $0 TARGET_DIR..." >&2
  exit 2
fi

cleaned=()
for requested in "$@"; do
  if [[ -z "$requested" ]]; then
    echo "refusing to clean an empty path" >&2
    exit 2
  fi
  target="$(realpath -m -- "$requested")"
  case "$target" in
    "$repo_root"/target/bench-smoke/*|"$repo_root"/target/bench-full/*) ;;
    *)
      echo "refusing to clean non-benchmark path: $target" >&2
      exit 2
      ;;
  esac
  if [[ "$target" == "/" || "$target" == "$repo_root" || -n "$home_dir" && "$target" == "$home_dir" ]]; then
    echo "refusing to clean protected path: $target" >&2
    exit 2
  fi
  rm -rf -- "$target"
  mkdir -p -- "$target"
  cleaned+=("$target")
done

python3 - "${cleaned[@]}" <<'PY'
import datetime
import json
import pathlib
import sys

paths = sys.argv[1:]
record = {
    "schema": 1,
    "cleaned_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
    "removed_and_recreated": paths,
}
for value in paths:
    path = pathlib.Path(value) / "clean-manifest.json"
    path.write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")
PY
