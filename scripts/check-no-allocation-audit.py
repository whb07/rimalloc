#!/usr/bin/env python3
"""Check the no-allocation critical path audit and guard test."""

from __future__ import annotations

import pathlib
import re


ROOT = pathlib.Path(__file__).resolve().parents[1]
AUDIT = ROOT / "NO_ALLOCATION_AUDIT.md"
GUARD_TEST = ROOT / "tests/no_allocation_critical_paths.rs"

CRITICAL_FILES = {
    "src/allocator/alloc.rs",
    "src/allocator/aligned.rs",
    "src/allocator/arena.rs",
    "src/allocator/arena_meta.rs",
    "src/bitmap.rs",
    "src/allocator/free.rs",
    "src/allocator/heap.rs",
    "src/allocator/init.rs",
    "src/internal/mod.rs",
    "src/platform/os.rs",
    "src/allocator/page.rs",
    "src/allocator/page_map.rs",
    "src/allocator/thread_heap.rs",
    "src/allocator/threadlocal.rs",
}

FORBIDDEN_PATTERNS = {
    "Box allocation": re.compile(r"\bBox::"),
    "Vec allocation": re.compile(r"\bVec::"),
    "String allocation": re.compile(r"\bString::"),
    "format allocation": re.compile(r"\bformat!"),
    "CString allocation": re.compile(r"\bCString::"),
    "std::alloc import/use": re.compile(r"\bstd::alloc\b"),
    "alloc_zeroed": re.compile(r"\balloc_zeroed\b"),
}

REQUIRED_AUDIT_SNIPPETS = [
    "dynamic TLS storage follows the C baseline",
    "`src/allocator/init.rs`",
    "`src/allocator/threadlocal.rs`",
    "`mi_rezalloc`",
    "`mi_zalloc_aligned`",
    "`tests/no_allocation_critical_paths.rs`",
    "Rust global allocator",
]

REQUIRED_GUARD_SNIPPETS = [
    "#[global_allocator]",
    "DetectingAllocator",
    "GLOBAL_ALLOC_CALLS",
    "mi_malloc_small",
    "mi_malloc_aligned",
    "mi_rezalloc",
    "mi_heap_new",
]


def require(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


def scan_forbidden_allocators() -> list[str]:
    hits: list[str] = []
    for relpath in sorted(CRITICAL_FILES):
        path = ROOT / relpath
        text = path.read_text()
        for label, pattern in FORBIDDEN_PATTERNS.items():
            for match in pattern.finditer(text):
                line = text.count("\n", 0, match.start()) + 1
                hits.append(f"{relpath}:{line}: {label}")
    return hits


def main() -> int:
    audit_text = AUDIT.read_text()
    guard_text = GUARD_TEST.read_text()

    for relpath in sorted(CRITICAL_FILES):
        require(relpath in audit_text, f"{relpath} is not mentioned in {AUDIT.name}")

    for snippet in REQUIRED_AUDIT_SNIPPETS:
        require(snippet in audit_text, f"missing audit snippet: {snippet}")

    for snippet in REQUIRED_GUARD_SNIPPETS:
        require(snippet in guard_text, f"missing guard test snippet: {snippet}")

    hits = scan_forbidden_allocators()
    require(not hits, "forbidden Rust allocation constructs in critical files:\n" + "\n".join(hits))

    require(
        "std::alloc" not in (ROOT / "src/allocator/threadlocal.rs").read_text(),
        "src/allocator/threadlocal.rs must not use std::alloc for dynamic TLS storage",
    )

    print(
        "ok: no-allocation audit covers "
        f"{len(CRITICAL_FILES)} critical files and the runtime global-allocator guard"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
