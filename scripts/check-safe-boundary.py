#!/usr/bin/env python3
"""Verify the safe Rust facade keeps unsafe boundaries classified."""

from __future__ import annotations

import pathlib
import re


ROOT = pathlib.Path(__file__).resolve().parents[1]
TEST_ONLY_SAFE_FILES = {"src/safe/tests.rs"}
SAFE_BOUNDARY_FILES = [
    "src/lib.rs",
    "src/safe/mod.rs",
    *sorted(
        rel
        for path in (ROOT / "src/safe").glob("*.rs")
        if (rel := path.relative_to(ROOT).as_posix()) not in TEST_ONLY_SAFE_FILES
    ),
]
ALLOWED_PUBLIC_UNSAFE = {
    "src/safe/allocation.rs": {"assume_initialized"},
    "src/safe/heap.rs": {"assume_initialized"},
}
CALLBACK_GUARDS = {"DeferredFreeCallbackGuard", "ErrorCallbackGuard"}
STRICT_MUST_USE_POLICY = "#![deny(clippy::must_use_candidate)]"
STRICT_LINT_POLICY_FILES = {
    "src/safe/mod.rs",
    "src/ffi/callbacks.rs",
    "src/raw/mod.rs",
}
SAFE_FACING_LINT_FILES = [
    *SAFE_BOUNDARY_FILES,
    "src/ffi/callbacks.rs",
    "src/raw/mod.rs",
]


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


def callback_guard_safe_surface_mentions(rel: str, text: str) -> list[str]:
    source = text.split("#[cfg(test)]", maxsplit=1)[0]
    return [
        guard
        for guard in CALLBACK_GUARDS
        if guard in source and rel != "src/lib.rs"
    ]


def missing_strict_lint_policy(rel: str, text: str) -> bool:
    return rel in STRICT_LINT_POLICY_FILES and STRICT_MUST_USE_POLICY not in text


def local_clippy_allowances(rel: str, text: str) -> list[str]:
    source = text.split("#[cfg(test)]", maxsplit=1)[0]
    return [
        f"line {lineno}: {line.strip()}"
        for lineno, line in enumerate(source.splitlines(), start=1)
        if "allow(clippy::" in line
    ]


def main() -> int:
    all_public_unsafe: list[str] = []
    callback_guard_mentions: list[str] = []
    missing_policy: list[str] = []
    clippy_allowances: list[str] = []

    synthetic = "pub unsafe fn synthetic_safe_escape() {}"
    require(
        public_unsafe_items("src/safe/synthetic.rs", synthetic),
        "safe-boundary regression check failed to flag a synthetic public unsafe fn",
    )

    for rel in SAFE_BOUNDARY_FILES:
        text = (ROOT / rel).read_text()
        all_public_unsafe.extend(f"{rel}:{item}" for item in public_unsafe_items(rel, text))
        callback_guard_mentions.extend(
            f"{rel}:{guard}" for guard in callback_guard_safe_surface_mentions(rel, text)
        )

    for rel in SAFE_FACING_LINT_FILES:
        text = (ROOT / rel).read_text()
        if missing_strict_lint_policy(rel, text):
            missing_policy.append(rel)
        clippy_allowances.extend(
            f"{rel}:{item}" for item in local_clippy_allowances(rel, text)
        )

    lib_rs = (ROOT / "src/lib.rs").read_text()

    require(
        not all_public_unsafe,
        "public unsafe Rust items outside raw facade: " + "; ".join(all_public_unsafe),
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
    require(
        not missing_policy,
        "safe-facing modules must locally deny clippy::must_use_candidate: "
        + "; ".join(missing_policy),
    )
    require(
        not clippy_allowances,
        "safe-facing modules must not add local Clippy allowances without updating the audited policy: "
        + "; ".join(clippy_allowances),
    )

    print(
        "ok: safe facade unsafe boundary has only allowlisted public unsafe items, "
        "callback guards remain namespaced, and safe-facing lint policy is local"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
