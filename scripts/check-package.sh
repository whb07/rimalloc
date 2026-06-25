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
run cargo package --allow-dirty

version="$(
  cargo metadata --no-deps --format-version 1 |
    python3 -c 'import json, sys; print(json.load(sys.stdin)["packages"][0]["version"])'
)"
crate_file="target/package/mechanical_mimalloc-${version}.crate"

if [[ ! -f "$crate_file" ]]; then
  echo "expected package artifact $crate_file" >&2
  exit 1
fi

consumer_dir="$(mktemp -d)"
trap 'rm -rf "$consumer_dir"' EXIT

mkdir -p "$consumer_dir/vendor"
tar -xzf "$crate_file" -C "$consumer_dir/vendor"

cat >"$consumer_dir/Cargo.toml" <<EOF
[package]
name = "mechanical_mimalloc_package_consumer"
version = "0.0.0"
edition = "2024"

[dependencies]
mechanical_mimalloc = { path = "vendor/mechanical_mimalloc-${version}", features = ["std"] }
EOF

mkdir -p "$consumer_dir/src"
cat >"$consumer_dir/src/main.rs" <<'EOF'
use core::alloc::Layout;
use mechanical_mimalloc::{Allocation, MiMalloc};

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn main() {
    let layout = Layout::from_size_align(64, 8).unwrap();
    let allocation = Allocation::zeroed(layout).unwrap();
    assert_eq!(allocation.len(), 64);
    let value = Box::new(42usize);
    assert_eq!(*value, 42);
}
EOF

run cargo run --quiet --manifest-path "$consumer_dir/Cargo.toml"
