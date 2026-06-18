#!/usr/bin/env bash

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd_repo_root

package_list="$(mktemp)"
run cargo package --list --allow-dirty >"$package_list"
cat "$package_list"

if grep -E '(^|/)__pycache__/|\.pyc$|\.pyo$' "$package_list" >/dev/null; then
  echo "generated Python cache files must not be packaged" >&2
  rm -f "$package_list"
  exit 1
fi

rm -f "$package_list"
run cargo package --allow-dirty --no-verify
