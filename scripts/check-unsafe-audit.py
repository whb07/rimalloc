#!/usr/bin/env python3
"""Check that the unsafe audit covers every Rust source file using unsafe."""

from __future__ import annotations

import pathlib
import re


ROOT = pathlib.Path(__file__).resolve().parents[1]
AUDIT = ROOT / "UNSAFE_AUDIT.md"

AUDITED_RUST_FILES = {
    "src/abi.rs",
    "src/alloc.rs",
    "src/alloc_aligned.rs",
    "src/alloc_override.rs",
    "src/alloc_posix.rs",
    "src/arena.rs",
    "src/arena_meta.rs",
    "src/atomic.rs",
    "src/bench_diagnostics.rs",
    "src/bitmap.rs",
    "src/core_types/arena.rs",
    "src/core_types/atomic_wrappers.rs",
    "src/core_types/callbacks.rs",
    "src/core_types/freelist.rs",
    "src/core_types/freelist_wrappers.rs",
    "src/core_types/guarded.rs",
    "src/core_types/heap.rs",
    "src/core_types/layout.rs",
    "src/core_types/memid.rs",
    "src/core_types/padding.rs",
    "src/core_types/page.rs",
    "src/core_types/page_queues.rs",
    "src/core_types/page_states.rs",
    "src/core_types/ptr.rs",
    "src/core_types/scalar.rs",
    "src/core_types/validation.rs",
    "src/core_types/validation_extra.rs",
    "src/free.rs",
    "src/heap.rs",
    "src/init.rs",
    "src/internal.rs",
    "src/lib.rs",
    "src/libc.rs",
    "src/options.rs",
    "src/os.rs",
    "src/page.rs",
    "src/page_map.rs",
    "src/prim.rs",
    "src/prim/emscripten.rs",
    "src/prim/osx.rs",
    "src/prim/unix.rs",
    "src/prim/wasi.rs",
    "src/prim/windows.rs",
    "src/random.rs",
    "src/raw.rs",
    "src/safe.rs",
    "src/stats.rs",
    "src/theap.rs",
    "src/threadlocal.rs",
    "src/track.rs",
    "src/types.rs",
}

REQUIRED_AUDIT_SNIPPETS = [
    "Public `extern \"C\"` APIs accept the same nullable and invalid-pointer inputs",
    "Metadata pointers are obtained from owning allocator structures",
    "Raw pointer arithmetic is limited to C-equivalent block/page/arena layout",
    "Cross-thread metadata mutation goes through the audited atomic helpers",
    "Callback invocation and process/thread lifecycle entry points",
    "`ATOMIC_ORDERING_AUDIT.md`",
    "scripts/check-valgrind-tracking-parity.py",
    "scripts/check-asan-tracking-parity.py",
]


def unsafe_source_files() -> set[str]:
    files: set[str] = set()
    for path in (ROOT / "src").rglob("*.rs"):
        text = path.read_text()
        if re.search(r"\bunsafe\b", text):
            files.add(path.relative_to(ROOT).as_posix())
    return files


def require(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


def main() -> int:
    audit_text = AUDIT.read_text()
    found = unsafe_source_files()
    require(found == AUDITED_RUST_FILES, f"unsafe audit file set mismatch: {sorted(found ^ AUDITED_RUST_FILES)}")

    for path in sorted(AUDITED_RUST_FILES):
        require(path in audit_text, f"{path} is not mentioned in {AUDIT.name}")

    for snippet in REQUIRED_AUDIT_SNIPPETS:
        require(snippet in audit_text, f"missing unsafe audit invariant: {snippet}")

    unsafe_mentions = sum(len(re.findall(r"\bunsafe\b", (ROOT / path).read_text())) for path in found)
    print(f"ok: unsafe audit covers {len(found)} source files and {unsafe_mentions} unsafe mentions")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
