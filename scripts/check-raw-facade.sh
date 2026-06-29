#!/usr/bin/env bash

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd_repo_root

run scripts/check-api-inventory.py

python3 - <<'PY'
from __future__ import annotations

import pathlib
import re

root = pathlib.Path.cwd()
raw = (root / "src/raw/mod.rs").read_text()

default_allowed = {"abi", "headers", "mimalloc"}
impl_only = {
    "alloc",
    "alloc_aligned",
    "alloc_override",
    "alloc_posix",
    "arena",
    "arena_meta",
    "atomic",
    "bitmap",
    "bits",
    "free",
    "heap",
    "init",
    "internal",
    "libc",
    "memory",
    "options",
    "os",
    "page",
    "page_map",
    "prim",
    "random",
    "static_c",
    "stats",
    "theap",
    "threadlocal",
    "track",
    "types",
}

pending_unstable = False
pending_attr = False
unstable_depth = 0
default_modules = set()
unstable_modules = set()
bad_pub_uses = []

for line_number, line in enumerate(raw.splitlines(), start=1):
    stripped = line.strip()
    if unstable_depth > 0:
        unstable_depth += line.count("{") - line.count("}")
        continue
    if stripped == '#[cfg(feature = "unstable-raw-internals")]':
        pending_unstable = True
        pending_attr = False
        continue
    if pending_attr:
        if stripped.endswith("]"):
            pending_attr = False
        continue
    if stripped.startswith("#["):
        if not stripped.endswith("]"):
            pending_attr = True
        continue
    module = re.match(r"pub mod\s+([A-Za-z0-9_]+)\s*\{", line)
    if module:
        name = module.group(1)
        if pending_unstable:
            unstable_modules.add(name)
            unstable_depth = line.count("{") - line.count("}")
        else:
            default_modules.add(name)
        pending_unstable = False
        continue
    pub_use = re.match(r"pub use crate::([A-Za-z0-9_]+)::\*;", stripped)
    if pub_use and not pending_unstable:
        name = pub_use.group(1)
        if name in impl_only:
            bad_pub_uses.append(f"src/raw/mod.rs:{line_number}: default pub use crate::{name}::*")
    if stripped:
        pending_unstable = False

missing_default = default_allowed - default_modules
extra_default = default_modules - default_allowed
missing_unstable = impl_only - unstable_modules
extra_unstable = unstable_modules - impl_only

errors = []
if missing_default or extra_default:
    errors.append(
        "default raw modules drift: "
        f"missing={sorted(missing_default)} extra={sorted(extra_default)}"
    )
if missing_unstable or extra_unstable:
    errors.append(
        "unstable raw-internals modules drift: "
        f"missing={sorted(missing_unstable)} extra={sorted(extra_unstable)}"
    )
if bad_pub_uses:
    errors.append("default raw implementation re-exports found:\n" + "\n".join(bad_pub_uses))

if errors:
    raise SystemExit("\n".join(errors))

print(
    "ok: default raw facade exposes "
    f"{sorted(default_modules)}; {len(unstable_modules)} implementation modules are feature-gated"
)
PY

python3 - <<'PY'
from __future__ import annotations

import pathlib
import re

root = pathlib.Path.cwd()
impl_only = {
    "alloc",
    "alloc_aligned",
    "alloc_override",
    "alloc_posix",
    "arena",
    "arena_meta",
    "atomic",
    "bitmap",
    "bits",
    "free",
    "heap",
    "init",
    "internal",
    "libc",
    "memory",
    "options",
    "os",
    "page",
    "page_map",
    "prim",
    "random",
    "static_c",
    "stats",
    "theap",
    "threadlocal",
    "track",
    "types",
}
path_roots = [root / "tests", root / "benches", root / "examples"]
direct = re.compile(
    r"(?:mechanical_mimalloc::)?raw::(" + "|".join(sorted(impl_only)) + r")\b"
)
group = re.compile(r"(?:mechanical_mimalloc::)?raw::\{([^}]*)\}", flags=re.S)

errors: list[str] = []
for path_root in path_roots:
    if not path_root.exists():
        continue
    for path in path_root.rglob("*.rs"):
        text = path.read_text()
        uses_impl_raw = False
        if direct.search(text):
            uses_impl_raw = True
        for match in group.finditer(text):
            names = [
                item.strip().split(" as ")[0].split("::", 1)[0].strip()
                for item in match.group(1).split(",")
            ]
            if any(name in impl_only for name in names):
                uses_impl_raw = True
                break
        if not uses_impl_raw:
            continue
        if '#![cfg(feature = "unstable-raw-internals")]' not in text:
            errors.append(
                f"{path.relative_to(root)} uses gated raw internals without "
                'feature-level cfg(feature = "unstable-raw-internals")'
            )

if errors:
    raise SystemExit("\n".join(errors))

print("ok: tests, benches, and examples do not use raw internals without unstable gating")
PY
