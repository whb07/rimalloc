#!/usr/bin/env python3
"""Verify high-level source layer boundaries stay deliberate."""

from __future__ import annotations

import pathlib
import re


ROOT = pathlib.Path(__file__).resolve().parents[1]
ALLOWED_SAFE_BACKENDS = {
    "allocator::aligned",
    "allocator::alloc",
    "allocator::heap",
    "allocator::init",
    "allocator::options",
    "allocator::stats",
    "ffi::mimalloc",
    "platform::os",
    "types",
}

BACKEND_MODULES = {
    "alloc",
    "alloc_aligned",
    "alloc_override",
    "alloc_posix",
    "arena",
    "arena_meta",
    "atomic",
    "bitmap",
    "core_types",
    "free",
    "heap",
    "init",
    "internal",
    "memory",
    "mimalloc",
    "options",
    "os",
    "page",
    "page_map",
    "platform",
    "prim",
    "raw",
    "static_c",
    "stats",
    "theap",
    "threadlocal",
    "types",
}

OWNING_BACKEND_ROOTS = {"allocator", "ffi", "internal", "platform", "raw"}


def require(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


def production_source(text: str) -> str:
    output: list[str] = []
    lines = text.splitlines()
    index = 0
    pending_cfg_test = False
    while index < len(lines):
        line = lines[index]
        stripped = line.strip()
        if stripped == "#[cfg(test)]":
            pending_cfg_test = True
            index += 1
            continue

        if pending_cfg_test and re.match(r"(?:pub(?:\([^)]*\))?\s+)?mod\s+\w+\s*(?:;|\{)", stripped):
            if stripped.endswith(";"):
                pending_cfg_test = False
                index += 1
                continue

            depth = line.count("{") - line.count("}")
            index += 1
            while index < len(lines) and depth > 0:
                depth += lines[index].count("{") - lines[index].count("}")
                index += 1
            pending_cfg_test = False
            continue

        pending_cfg_test = False
        output.append(line)
        index += 1

    return "\n".join(output)


def backend_modules_used(text: str) -> set[str]:
    modules = set()
    for match in re.finditer(r"\bcrate::([A-Za-z_][A-Za-z0-9_]*)::", text):
        module = match.group(1)
        if module in OWNING_BACKEND_ROOTS:
            owner = re.match(r"([A-Za-z_][A-Za-z0-9_]*)::", text[match.end() :])
            modules.add(module if owner is None else f"{module}::{owner.group(1)}")
            continue
        if module in BACKEND_MODULES:
            modules.add(module)
    return modules


def main() -> int:
    safe_files = sorted(
        rel
        for path in (ROOT / "src/safe").glob("*.rs")
        if (rel := path.relative_to(ROOT).as_posix()) != "src/safe/tests.rs"
    )

    offenders = []
    for rel in safe_files:
        text = production_source((ROOT / rel).read_text())
        actual = backend_modules_used(text)
        forbidden = actual - ALLOWED_SAFE_BACKENDS
        if forbidden:
            offenders.append(f"{rel}: forbidden={sorted(forbidden)}")

    synthetic = (
        "fn bad() { let _ = crate::internal::MI_DEBUG; "
        "let _ = crate::allocator::options::mi_option_get; "
        "let _ = crate::allocator::unknown_owner::bad; }"
    )
    require(
        backend_modules_used(synthetic)
        == {"internal", "allocator::options", "allocator::unknown_owner"},
        "layer-boundary regression failed to detect synthetic root and owning imports",
    )
    require(
        not offenders,
        "safe facade imports unsupported backend modules: " + "; ".join(offenders),
    )

    print(
        "ok: safe facade imports only the supported backend API roots"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
