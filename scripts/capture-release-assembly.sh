#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

cd_repo_root

out_dir="${MIMALLOC_ASM_OUT_DIR:-target/strict-type-assembly}"
objdump_tool="${MIMALLOC_OBJDUMP:-objdump}"
nm_tool="${MIMALLOC_NM:-nm}"

run cargo build --release --lib

artifact=""
for candidate in \
  target/release/libmechanical_mimalloc.so \
  target/release/libmechanical_mimalloc.dylib \
  target/release/mechanical_mimalloc.dll
do
  if [[ -f "$candidate" ]]; then
    artifact="$candidate"
    break
  fi
done

if [[ -z "$artifact" ]]; then
  printf 'error: release dynamic library artifact not found\n' >&2
  exit 1
fi

mkdir -p "$out_dir"

{
  printf 'artifact=%s\n' "$artifact"
  printf 'git_head=%s\n' "$(git rev-parse --verify HEAD 2>/dev/null || printf unknown)"
  printf 'rustc=%s\n' "$(rustc --version)"
  printf 'cargo=%s\n' "$(cargo --version)"
  printf 'objdump=%s\n' "$("$objdump_tool" --version | sed -n '1p')"
  printf 'nm=%s\n' "$("$nm_tool" --version | sed -n '1p')"
} >"$out_dir/manifest.txt"

resolve_symbol_range() {
  local symbol="$1"

  "$nm_tool" -anC "$artifact" | awk -v symbol="$symbol" '
    /^[[:xdigit:]]+[[:space:]][A-Za-z][[:space:]]/ {
      addr = $1
      type = $2
      name = $0
      sub(/^[^[:space:]]+[[:space:]]+[^[:space:]]+[[:space:]]+/, "", name)

      if (type !~ /[Tt]/) {
        next
      }

      if (found && stop == "" && addr != start) {
        stop = addr
        next
      }

      if (!found && (name == symbol || name ~ ("::" symbol "$"))) {
        start = addr
        found = 1
      }
    }

    END {
      if (found) {
        print start, stop
      }
    }
  '
}

capture_symbol() {
  local symbol="$1"
  local out_file="$out_dir/$symbol.asm"
  local range start stop

  range="$(resolve_symbol_range "$symbol")"
  if [[ -z "$range" ]]; then
    printf 'error: symbol not found: %s\n' "$symbol" >&2
    exit 1
  fi

  read -r start stop <<<"$range"

  {
    printf '# symbol=%s\n' "$symbol"
    printf '# start=0x%s\n' "$start"
    if [[ -n "${stop:-}" ]]; then
      printf '# stop=0x%s\n' "$stop"
      "$objdump_tool" -Cd --start-address="0x$start" --stop-address="0x$stop" "$artifact"
    else
      "$objdump_tool" -Cd --start-address="0x$start" "$artifact"
    fi
  } >"$out_file"

  printf 'captured %s -> %s\n' "$symbol" "$out_file"
}

capture_symbol mi_malloc
capture_symbol mi_malloc_small
capture_symbol mi_free
capture_symbol _mi_theap_malloc_zero_ex

printf 'manifest -> %s\n' "$out_dir/manifest.txt"
