#!/usr/bin/env python3
"""Verify the safe Rust facade keeps unsafe boundaries classified."""

from __future__ import annotations

import pathlib
import re


ROOT = pathlib.Path(__file__).resolve().parents[1]
SAFE_BOUNDARY_FILES = [
    "src/lib.rs",
    "src/safe/mod.rs",
    *sorted(path.relative_to(ROOT).as_posix() for path in (ROOT / "src/safe").glob("*.rs")),
]
AUDIT_TAG = "SAFETY[AUDIT:"
ALLOWED_PUBLIC_UNSAFE = {
    "src/safe/allocation.rs": {"assume_initialized"},
    "src/safe/heap.rs": {"assume_initialized"},
}
CALLBACK_GUARDS = {"DeferredFreeCallbackGuard", "ErrorCallbackGuard"}


def require(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


def public_unsafe_items(rel: str, text: str) -> list[str]:
    offenders: list[str] = []
    for lineno, line in enumerate(text.splitlines(), start=1):
        stripped = line.strip()
        match = re.search(
            r"\bpub\s+unsafe\s+(?P<kind>fn|extern|trait|impl)\s+(?P<name>[A-Za-z_][A-Za-z0-9_]*)?",
            stripped,
        )
        if not match:
            continue
        name = match.group("name") or ""
        if name in ALLOWED_PUBLIC_UNSAFE.get(rel, set()):
            continue
        if match:
            offenders.append(f"line {lineno}: {stripped}")
    return offenders


def has_recent_audit_tag(lines: list[str], index: int) -> bool:
    start = max(0, index - 4)
    return any(AUDIT_TAG in lines[i] for i in range(start, index))


def untagged_boundary_unsafe(text: str) -> list[str]:
    source = text.split("#[cfg(test)]", maxsplit=1)[0]
    lines = source.splitlines()
    offenders: list[str] = []
    patterns = [
        re.compile(r"^\s*unsafe\s*\{"),
        re.compile(r"^\s*unsafe\s+impl\b"),
        re.compile(r"^\s*unsafe\s+extern\b"),
    ]
    for index, line in enumerate(lines):
        if any(pattern.search(line) for pattern in patterns) and not has_recent_audit_tag(
            lines, index
        ):
            offenders.append(f"line {index + 1}: {line.strip()}")
    return offenders


def callback_guard_safe_surface_mentions(rel: str, text: str) -> list[str]:
    source = text.split("#[cfg(test)]", maxsplit=1)[0]
    return [
        guard
        for guard in CALLBACK_GUARDS
        if guard in source and rel != "src/lib.rs"
    ]


def main() -> int:
    all_public_unsafe: list[str] = []
    all_untagged: list[str] = []
    callback_guard_mentions: list[str] = []

    synthetic = "pub unsafe fn synthetic_safe_escape() {}"
    require(
        public_unsafe_items("src/safe/synthetic.rs", synthetic),
        "safe-boundary regression check failed to flag a synthetic public unsafe fn",
    )

    for rel in SAFE_BOUNDARY_FILES:
        text = (ROOT / rel).read_text()
        all_public_unsafe.extend(f"{rel}:{item}" for item in public_unsafe_items(rel, text))
        all_untagged.extend(f"{rel}:{item}" for item in untagged_boundary_unsafe(text))
        callback_guard_mentions.extend(
            f"{rel}:{guard}" for guard in callback_guard_safe_surface_mentions(rel, text)
        )

    lib_rs = (ROOT / "src/lib.rs").read_text()

    require(
        not all_public_unsafe,
        "public unsafe Rust items outside raw facade: " + "; ".join(all_public_unsafe),
    )
    require(
        not all_untagged,
        "safe facade unsafe boundary lacks SAFETY[AUDIT:*] tag: "
        + "; ".join(all_untagged),
    )
    require(
        "pub use ffi::callbacks as unsafe_callbacks;" in lib_rs,
        "unsafe callback guards must stay namespaced under unsafe_callbacks",
    )
    require(
        "DeferredFreeCallbackGuard, ErrorCallbackGuard" not in lib_rs
        and "ErrorCallbackGuard" not in lib_rs.split("pub use ffi::callbacks as unsafe_callbacks;", 1)[0],
        "callback guards must not be re-exported as crate-root Tier S items",
    )
    require(
        not callback_guard_mentions,
        "callback guards must not be referenced from Tier S safe facade source outside tests: "
        + "; ".join(callback_guard_mentions),
    )

    print(
        "ok: safe facade unsafe boundary has only allowlisted public unsafe items "
        "and all unsafe blocks are audit-tagged"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
