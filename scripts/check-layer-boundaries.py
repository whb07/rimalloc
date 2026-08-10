#!/usr/bin/env python3
"""Verify high-level source layer boundaries stay deliberate."""

from __future__ import annotations

import pathlib
import re


ROOT = pathlib.Path(__file__).resolve().parents[1]
TEST_ONLY_SAFE_FILES = {"src/safe/tests.rs"}

SAFE_BACKEND_IMPORT_ALLOWLIST = {
    "src/safe/allocation.rs": set(),
    "src/safe/error.rs": set(),
    "src/safe/heap.rs": {
        "allocator::aligned",
        "allocator::alloc",
        "allocator::heap",
        "allocator::init",
        "ffi::mimalloc",
        "types",
    },
    "src/safe/mod.rs": set(),
    "src/safe/options.rs": {"allocator::options"},
    "src/safe/stats.rs": {"allocator::init", "allocator::stats", "platform::os"},
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
        if (rel := path.relative_to(ROOT).as_posix()) not in TEST_ONLY_SAFE_FILES
    )
    expected_files = sorted(SAFE_BACKEND_IMPORT_ALLOWLIST)
    require(
        safe_files == expected_files,
        "safe facade file inventory drift: "
        f"missing={sorted(set(expected_files) - set(safe_files))} "
        f"extra={sorted(set(safe_files) - set(expected_files))}",
    )

    offenders = []
    for rel in safe_files:
        text = production_source((ROOT / rel).read_text())
        actual = backend_modules_used(text)
        allowed = SAFE_BACKEND_IMPORT_ALLOWLIST[rel]
        if actual != allowed:
            offenders.append(
                f"{rel}: missing={sorted(allowed - actual)} extra={sorted(actual - allowed)}"
            )

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
        "safe facade backend import allowlist drift: " + "; ".join(offenders),
    )

    print(
        "ok: safe facade backend imports match the layer-boundary allowlist "
        f"for {len(safe_files)} files"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
