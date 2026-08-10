#!/usr/bin/env python3
"""Check architectural boundaries that Rust visibility cannot express."""

from __future__ import annotations

import pathlib
import re


ROOT = pathlib.Path(__file__).resolve().parents[1]

DIRECT_MEMID_RE = re.compile(r"\bmemid\s*\.\s*(mem|arena|os|tag|id)\b")
BLOCK_NEXT_RE = re.compile(r"\(\s*\*\s*(?:block|free|next)\s*\)\.next\b")
XTHREAD_ID_BITS_RE = re.compile(
    r"\bxthread_id\b.*(?:&|\||\^|<<|>>)|(?:&|\||\^|<<|>>).*?\bxthread_id\b"
)
XTHREAD_FREE_BITS_RE = re.compile(
    r"\bxthread_free\b.*(?:&|\||\^|<<|>>)|(?:&|\||\^|<<|>>).*?\bxthread_free\b"
)
ATOMIC_CAST_RE = re.compile(r"\bas\s+\*const\s+Atomic|\bas\s+\*mut\s+Atomic")
DYNAMIC_DISPATCH_RE = re.compile(r"\bdyn\s+[A-Za-z_]")
METADATA_DEREF_RE = re.compile(
    r"\bimpl(?:<[^>]*>)?\s+(?:core::ops::|std::ops::)?Deref(?:Mut)?\s+for\s+"
    r"(?:PagePtr|BlockPtr|HeapPtr|ThreadHeapPtr|TldPtr|ArenaPtr|SubprocPtr|PageQueuePtr)"
)

ALLOWED_DIRECT_MEMID = {
    "src/types/constants.rs",
    "src/types/memid.rs",
}
ALLOWED_BLOCK_NEXT = {
    "src/types/constants.rs",
    "src/types/freelist_wrappers.rs",
}
ALLOWED_XTHREAD_ID = {
    "src/types/atomic.rs",
    "src/types/atomic/page_thread.rs",
    "src/types/constants.rs",
    "src/types/page.rs",
}
ALLOWED_XTHREAD_FREE = {
    "src/types/atomic.rs",
    "src/types/atomic/thread_free.rs",
    "src/types/constants.rs",
    "src/types/freelist.rs",
}

RAW_BRIDGE_CALLERS = {
    "assume_uninit_page": {"src/allocator/page.rs"},
    "assume_active_page": {
        "src/allocator/aligned/thread_heap.rs",
        "src/allocator/alloc.rs",
        "src/allocator/arena.rs",
        "src/allocator/free.rs",
        "src/allocator/page.rs",
        "src/allocator/thread_heap.rs",
    },
    "assume_all_free_abandoned": {"src/allocator/arena.rs"},
    "assume_full_page": {
        "src/allocator/arena.rs",
        "src/allocator/free.rs",
        "src/allocator/page.rs",
        "src/allocator/thread_heap.rs",
    },
    "assume_local_page": {
        "src/allocator/bench_diagnostics.rs",
        "src/allocator/free.rs",
        "src/allocator/page/lifecycle.rs",
        "src/allocator/thread_heap.rs",
    },
    "assume_foreign_page": {"src/allocator/free.rs"},
    "assume_abandoned_page": {
        "src/allocator/arena.rs",
        "src/allocator/arena/abandoned.rs",
        "src/allocator/arena/heap_pages.rs",
        "src/allocator/page.rs",
        "src/allocator/thread_heap.rs",
    },
    "assume_owned_abandoned_page": {
        "src/allocator/arena.rs",
        "src/allocator/arena/abandoned.rs",
        "src/allocator/free.rs",
        "src/allocator/page.rs",
        "src/allocator/page/lifecycle.rs",
        "src/internal/mod.rs",
    },
}
RAW_BRIDGE_CALL_RE = re.compile(
    r"\b(?P<name>assume_(?:uninit_page|active_page|all_free_abandoned|full_page|"
    r"local_page|foreign_page|abandoned_page|owned_abandoned_page))\s*\("
)


def source_files() -> list[pathlib.Path]:
    return sorted((ROOT / "src").rglob("*.rs"))


def is_test_source(path: pathlib.Path) -> bool:
    return (
        path.name == "tests.rs"
        or path.name.endswith("_tests.rs")
        or "page_batch_tests" in path.parts
        or path.name == "concurrency_models.rs"
    )


def line_number(text: str, offset: int) -> int:
    return text.count("\n", 0, offset) + 1


def add_pattern_violations(
    violations: list[str],
    *,
    rel: str,
    text: str,
    pattern: re.Pattern[str],
    allowed_files: set[str],
    message: str,
) -> None:
    if rel in allowed_files:
        return
    for match in pattern.finditer(text):
        violations.append(f"{rel}:{line_number(text, match.start())}: {message}")


def main() -> int:
    violations: list[str] = []
    bridge_source = (ROOT / "src/types/raw_bridge.rs").read_text()
    declared_bridges = set(re.findall(r"\b(assume_[A-Za-z0-9_]+)\b", bridge_source))
    expected_bridges = set(RAW_BRIDGE_CALLERS)
    if declared_bridges != expected_bridges:
        violations.append(
            "src/types/raw_bridge.rs: raw bridge inventory changed; "
            f"added={sorted(declared_bridges - expected_bridges)}, "
            f"removed={sorted(expected_bridges - declared_bridges)}"
        )

    for path in source_files():
        if is_test_source(path):
            continue
        rel = path.relative_to(ROOT).as_posix()
        text = path.read_text().split("#[cfg(test)]", maxsplit=1)[0]

        add_pattern_violations(
            violations,
            rel=rel,
            text=text,
            pattern=DIRECT_MEMID_RE,
            allowed_files=ALLOWED_DIRECT_MEMID,
            message="direct memid union access must use MemId helpers",
        )
        add_pattern_violations(
            violations,
            rel=rel,
            text=text,
            pattern=BLOCK_NEXT_RE,
            allowed_files=ALLOWED_BLOCK_NEXT,
            message="direct block-link access must use free-list helpers",
        )
        add_pattern_violations(
            violations,
            rel=rel,
            text=text,
            pattern=XTHREAD_ID_BITS_RE,
            allowed_files=ALLOWED_XTHREAD_ID,
            message="xthread_id bit operations must use typed page-owner helpers",
        )
        add_pattern_violations(
            violations,
            rel=rel,
            text=text,
            pattern=XTHREAD_FREE_BITS_RE,
            allowed_files=ALLOWED_XTHREAD_FREE,
            message="xthread_free bit operations must use typed free-list helpers",
        )

        if not (
            rel == "src/internal/atomic.rs"
            or rel.startswith("src/types/atomic/")
            or rel == "src/types/atomic.rs"
        ):
            add_pattern_violations(
                violations,
                rel=rel,
                text=text,
                pattern=ATOMIC_CAST_RE,
                allowed_files=set(),
                message="pointer-to-atomic casts must stay in atomic owners",
            )

        if rel.startswith(("src/allocator/", "src/internal/", "src/platform/", "src/types/")):
            add_pattern_violations(
                violations,
                rel=rel,
                text=text,
                pattern=DYNAMIC_DISPATCH_RE,
                allowed_files=set(),
                message="dynamic dispatch is not allowed in allocator internals",
            )

        add_pattern_violations(
            violations,
            rel=rel,
            text=text,
            pattern=METADATA_DEREF_RE,
            allowed_files=set(),
            message="metadata pointer wrappers must not implement Deref",
        )

        if rel != "src/types/raw_bridge.rs":
            for match in RAW_BRIDGE_CALL_RE.finditer(text):
                name = match.group("name")
                if rel not in RAW_BRIDGE_CALLERS[name]:
                    violations.append(
                        f"{rel}:{line_number(text, match.start())}: "
                        f"{name} is outside its reviewed allocator ingress"
                    )

    if violations:
        raise AssertionError("strict-type boundary check failed:\n" + "\n".join(violations))

    print(
        "ok: strict-type boundaries confine union fields, block links, packed "
        "thread words, atomic casts, raw state bridges, Deref, and dynamic dispatch"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
