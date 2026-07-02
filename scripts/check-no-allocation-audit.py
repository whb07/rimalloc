#!/usr/bin/env python3
"""Check the no-allocation critical path audit and guard test."""

from __future__ import annotations

import json
import pathlib
import re


ROOT = pathlib.Path(__file__).resolve().parents[1]
AUDIT = ROOT / "NO_ALLOCATION_AUDIT.md"
GUARD_TEST = ROOT / "tests/no_allocation_critical_paths.rs"
MANIFEST = ROOT / "NO_ALLOCATION_CRITICAL_PATHS.json"

CRITICAL_FILES = {
    "src/allocator/alloc.rs",
    "src/allocator/aligned.rs",
    "src/allocator/arena.rs",
    "src/allocator/arena_meta.rs",
    "src/internal/bitmap.rs",
    "src/allocator/free.rs",
    "src/allocator/heap.rs",
    "src/allocator/init.rs",
    "src/internal/bitmap/bfield.rs",
    "src/internal/mod.rs",
    "src/platform/os.rs",
    "src/allocator/page/deferred.rs",
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
ALLOCATING_HELPERS = {
    "Box::",
    "CString::",
    "String::",
    "Vec::",
    "alloc_zeroed",
    "format!",
    "std::alloc",
}
PATH_ALIASES = {
    "src/allocator/aligned.rs": ["src/alloc_aligned.rs"],
    "src/allocator/thread_heap.rs": ["src/theap.rs"],
    "src/internal/bitmap/bfield.rs": ["src/bitmap.rs", "src/internal/bitmap.rs"],
    "src/internal/mod.rs": ["src/internal.rs"],
    "src/internal/bitmap.rs": ["src/bitmap.rs"],
    "src/platform/os.rs": ["src/os.rs"],
}

REQUIRED_AUDIT_SNIPPETS = [
    "dynamic TLS storage follows the C baseline",
    "`src/allocator/init.rs`",
    "`src/allocator/threadlocal.rs`",
    "`mi_rezalloc`",
    "`mi_zalloc_aligned`",
    "`tests/no_allocation_critical_paths.rs`",
    "`NO_ALLOCATION_CRITICAL_PATHS.json`",
    "Rust global allocator",
]

REQUIRED_GUARD_SNIPPETS = [
    "#[global_allocator]",
    "DetectingAllocator",
    "GLOBAL_ALLOC_CALLS",
    "mi_malloc_small",
    "mi_malloc_aligned",
    "mi_calloc",
    "mi_rezalloc",
    "mi_zalloc_aligned",
    "mi_heap_new",
]


def require(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


def audit_path_candidates(path: str) -> list[str]:
    candidates = [path]
    candidates.extend(PATH_ALIASES.get(path, []))
    if path.startswith("src/allocator/"):
        candidates.append("src/" + path.removeprefix("src/allocator/"))
    return candidates


def audit_mentions_path(audit_text: str, path: str) -> bool:
    return any(candidate in audit_text for candidate in audit_path_candidates(path))


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


def load_manifest() -> list[dict[str, str]]:
    data = json.loads(MANIFEST.read_text())
    paths = data.get("critical_paths")
    require(isinstance(paths, list) and paths, "manifest must list critical_paths")
    for item in paths:
        require(isinstance(item, dict), "manifest critical path entries must be objects")
        require(isinstance(item.get("entrypoint"), str), "manifest entry missing entrypoint")
        require(isinstance(item.get("file"), str), "manifest entry missing file")
    return paths


def function_body(source: str, name: str) -> str | None:
    match = re.search(
        rf"(?m)^\s*pub\s+unsafe\s+extern\s+\"C\"\s+fn\s+{re.escape(name)}\b[^{{]*\{{",
        source,
    )
    if not match:
        return None

    start = match.end() - 1
    depth = 0
    for index in range(start, len(source)):
        char = source[index]
        if char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                return source[start : index + 1]
    return None


def scan_manifest_entrypoints(paths: list[dict[str, str]]) -> list[str]:
    hits: list[str] = []
    for item in paths:
        relpath = item["file"]
        entrypoint = item["entrypoint"]
        source = (ROOT / relpath).read_text()
        body = function_body(source, entrypoint)
        if body is None:
            hits.append(f"{entrypoint}: not found in {relpath}")
            continue
        for helper in sorted(ALLOCATING_HELPERS):
            if helper in body:
                hits.append(f"{entrypoint} in {relpath} calls allocation helper {helper}")
    return hits


def main() -> int:
    audit_text = AUDIT.read_text()
    guard_text = GUARD_TEST.read_text()
    manifest_paths = load_manifest()
    manifest_files = {item["file"] for item in manifest_paths}
    manifest_entrypoints = {item["entrypoint"] for item in manifest_paths}

    for relpath in sorted(CRITICAL_FILES):
        require(audit_mentions_path(audit_text, relpath), f"{relpath} is not mentioned in {AUDIT.name}")
    for relpath in sorted(manifest_files):
        require(relpath in CRITICAL_FILES, f"{relpath} is in manifest but not CRITICAL_FILES")
    for entrypoint in sorted(manifest_entrypoints):
        require(entrypoint in audit_text, f"{entrypoint} is not mentioned in {AUDIT.name}")

    for snippet in REQUIRED_AUDIT_SNIPPETS:
        require(snippet in audit_text, f"missing audit snippet: {snippet}")

    for snippet in REQUIRED_GUARD_SNIPPETS:
        require(snippet in guard_text, f"missing guard test snippet: {snippet}")

    hits = scan_forbidden_allocators()
    require(not hits, "forbidden Rust allocation constructs in critical files:\n" + "\n".join(hits))
    entrypoint_hits = scan_manifest_entrypoints(manifest_paths)
    require(
        not entrypoint_hits,
        "forbidden allocating helpers in critical entrypoints:\n" + "\n".join(entrypoint_hits),
    )

    require(
        "std::alloc" not in (ROOT / "src/allocator/threadlocal.rs").read_text(),
        "src/allocator/threadlocal.rs must not use std::alloc for dynamic TLS storage",
    )

    print(
        "ok: no-allocation audit covers "
        f"{len(CRITICAL_FILES)} critical files, {len(manifest_paths)} manifest entrypoints, "
        "and the runtime global-allocator guard"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
