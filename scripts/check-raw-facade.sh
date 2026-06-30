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

default_modules = set()
bad_pub_uses = []
bad_impl_modules = []

for line_number, line in enumerate(raw.splitlines(), start=1):
    stripped = line.strip()
    module = re.match(r"pub mod\s+([A-Za-z0-9_]+)\s*\{", line)
    if module:
        name = module.group(1)
        default_modules.add(name)
        if name in impl_only:
            bad_impl_modules.append(f"src/raw/mod.rs:{line_number}: pub mod {name}")
        continue
    pub_use = re.match(r"pub use crate::([A-Za-z0-9_]+)::\*;", stripped)
    if pub_use:
        name = pub_use.group(1)
        if name in impl_only:
            bad_pub_uses.append(f"src/raw/mod.rs:{line_number}: pub use crate::{name}::*")

missing_default = default_allowed - default_modules
extra_default = default_modules - default_allowed

errors = []
if missing_default or extra_default:
    errors.append(
        "default raw modules drift: "
        f"missing={sorted(missing_default)} extra={sorted(extra_default)}"
    )
if bad_impl_modules:
    errors.append("raw implementation modules found:\n" + "\n".join(bad_impl_modules))
if bad_pub_uses:
    errors.append("raw implementation re-exports found:\n" + "\n".join(bad_pub_uses))

if errors:
    raise SystemExit("\n".join(errors))

print(
    "ok: default raw facade exposes "
    f"{sorted(default_modules)}; implementation modules are private"
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
    r"^\s*use\s+(?:mechanical_mimalloc::)?raw::("
    + "|".join(sorted(impl_only))
    + r")\b",
    flags=re.M,
)
group = re.compile(r"^\s*use\s+(?:mechanical_mimalloc::)?raw::\{([^}]*)\}", flags=re.M | re.S)

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
        if uses_impl_raw:
            errors.append(
                f"{path.relative_to(root)} imports removed private raw implementation modules"
            )

if errors:
    raise SystemExit("\n".join(errors))

print("ok: tests, benches, and examples do not import private raw implementation modules")
PY
