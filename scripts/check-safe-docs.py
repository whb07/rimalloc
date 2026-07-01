#!/usr/bin/env python3
"""Check that public safe facade docs include required API sections."""

from __future__ import annotations

import pathlib
import re


ROOT = pathlib.Path(__file__).resolve().parents[1]
SAFE_FILES = sorted(
    {
        ROOT / "src/safe/mod.rs",
        *((ROOT / "src/safe").glob("*.rs")),
    }
)
TIER_U_FILES = [ROOT / "src/ffi/callbacks.rs"]
PUBLIC_DOC_FILES = [
    ROOT / "src/lib.rs",
    *SAFE_FILES,
    *TIER_U_FILES,
]
UNSAFE_SECTION_FILES = [
    *SAFE_FILES,
    *TIER_U_FILES,
    *sorted((ROOT / "src/raw").glob("*.rs")),
]

PUBLIC_ITEM = re.compile(
    r"^\s*pub\s+"
    r"(?:const\s+)?(?:async\s+)?(?:unsafe\s+)?"
    r"(?:struct|enum|trait|type|fn|const)\s+[A-Za-z_][A-Za-z0-9_]*\b"
)
PUBLIC_ENUM = re.compile(r"^\s*pub\s+enum\s+[A-Za-z_][A-Za-z0-9_]*\b")
PUBLIC_STRUCT = re.compile(r"^\s*pub\s+struct\s+(?P<name>[A-Za-z_][A-Za-z0-9_]*)\b")
ENUM_VARIANT = re.compile(r"^\s*([A-Z][A-Za-z0-9_]*)\s*(?:[,({=]|$)")
PUBLIC_FN = re.compile(
    r"^\s*pub\s+"
    r"(?P<qualifiers>(?:const\s+)?(?:async\s+)?(?:unsafe\s+)?)"
    r"fn\s+(?P<name>[A-Za-z_][A-Za-z0-9_]*)\b"
)


def has_doc(lines: list[str], index: int) -> bool:
    cursor = index - 1
    while cursor >= 0:
        stripped = lines[cursor].strip()
        if not stripped:
            cursor -= 1
            continue
        if stripped.startswith("#["):
            cursor -= 1
            continue
        return stripped.startswith("///")
    return False


def doc_block(lines: list[str], index: int) -> list[str]:
    cursor = index - 1
    while cursor >= 0 and lines[cursor].strip().startswith("#["):
        cursor -= 1

    docs: list[str] = []
    while cursor >= 0:
        stripped = lines[cursor].strip()
        if not stripped.startswith("///"):
            break
        docs.append(stripped.removeprefix("///").strip())
        cursor -= 1

    docs.reverse()
    return docs


def has_section(docs: list[str], section: str) -> bool:
    return any(line == section for line in docs)


def function_signature(lines: list[str], index: int) -> str:
    signature = lines[index].strip()
    cursor = index
    while not re.search(r"[{;]\s*(?://.*)?$", signature) and cursor + 1 < len(lines):
        cursor += 1
        signature = f"{signature} {lines[cursor].strip()}"
    return signature


def missing_public_enum_variant_docs(rel: str, lines: list[str]) -> list[str]:
    missing: list[str] = []
    in_public_enum = False
    brace_depth = 0

    for index, line in enumerate(lines):
        if not in_public_enum:
            if PUBLIC_ENUM.match(line):
                in_public_enum = True
                brace_depth = line.count("{") - line.count("}")
            continue

        brace_depth += line.count("{") - line.count("}")
        stripped = line.strip()
        if not stripped or stripped.startswith("#[") or stripped.startswith("//"):
            if brace_depth <= 0:
                in_public_enum = False
            continue

        match = ENUM_VARIANT.match(line)
        if match and not has_doc(lines, index):
            missing.append(f"{rel}:{index + 1}: enum variant {match.group(1)}")

        if brace_depth <= 0:
            in_public_enum = False

    return missing


def missing_public_item_docs(rel: str, lines: list[str]) -> list[str]:
    missing: list[str] = []
    for index, line in enumerate(lines):
        if PUBLIC_ITEM.match(line) and not has_doc(lines, index):
            missing.append(f"{rel}:{index + 1}: {line.strip()}")
    return missing


def missing_safe_result_sections(rel: str, lines: list[str]) -> list[str]:
    missing: list[str] = []
    for index, line in enumerate(lines):
        match = PUBLIC_FN.match(line)
        if not match or "unsafe" in match.group("qualifiers").split():
            continue

        signature = function_signature(lines, index)
        if "-> Result<" not in signature:
            continue

        docs = doc_block(lines, index)
        if not has_section(docs, "# Errors"):
            missing.append(f"{rel}:{index + 1}: missing # Errors for {match.group('name')}")
        if not has_section(docs, "# Panics"):
            missing.append(f"{rel}:{index + 1}: missing # Panics for {match.group('name')}")
    return missing


def safe_signature_borrowing_violations(rel: str, lines: list[str]) -> list[str]:
    violations: list[str] = []
    forbidden_patterns = [
        (re.compile(r"&\s*(?:'[_A-Za-z0-9]+\s+)?Vec\s*<"), "&Vec<T>"),
        (re.compile(r"&\s*(?:'[_A-Za-z0-9]+\s+)?String\b"), "&String"),
        (re.compile(r"&\s*(?:'[_A-Za-z0-9]+\s+)?PathBuf\b"), "&PathBuf"),
        (re.compile(r"\bCow\s*<"), "Cow"),
    ]

    for index, line in enumerate(lines):
        match = PUBLIC_FN.match(line)
        if not match or "unsafe" in match.group("qualifiers").split():
            continue

        signature = function_signature(lines, index)
        for pattern, type_name in forbidden_patterns:
            if pattern.search(signature):
                violations.append(
                    f"{rel}:{index + 1}: public safe signature {match.group('name')} uses {type_name}"
                )

    return violations


def safe_non_copy_clone_violations(rel: str, lines: list[str]) -> list[str]:
    violations: list[str] = []
    pending_non_copy_clone = False

    for index, line in enumerate(lines):
        stripped = line.strip()
        if stripped.startswith("#[derive("):
            pending_non_copy_clone = "Clone" in stripped and "Copy" not in stripped
            continue

        if not stripped or stripped.startswith("#["):
            continue

        match = PUBLIC_STRUCT.match(line)
        if pending_non_copy_clone and match:
            docs = "\n".join(doc_block(lines, index))
            if "Cloning" not in docs:
                violations.append(
                    f"{rel}:{index + 1}: public non-Copy Clone {match.group('name')}"
                    " must document clone semantics"
                )
        pending_non_copy_clone = False

    return violations


def missing_unsafe_sections(rel: str, lines: list[str]) -> list[str]:
    missing: list[str] = []
    for index, line in enumerate(lines):
        match = PUBLIC_FN.match(line)
        if not match or "unsafe" not in match.group("qualifiers").split():
            continue

        if not has_section(doc_block(lines, index), "# Safety"):
            missing.append(f"{rel}:{index + 1}: missing # Safety for {match.group('name')}")
    return missing


def main() -> int:
    missing: list[str] = []
    synthetic_item = ["pub struct Undocumented;"]
    if not missing_public_item_docs("synthetic.rs", synthetic_item):
        print("safe-docs regression failed to flag an undocumented public item")
        return 1
    synthetic_type = ["pub type Alias = ();"]
    if not missing_public_item_docs("synthetic.rs", synthetic_type):
        print("safe-docs regression failed to flag an undocumented type alias")
        return 1
    synthetic = ["pub enum Synthetic {", "    Undocumented,", "}"]
    if not missing_public_enum_variant_docs("synthetic.rs", synthetic):
        print("safe-docs regression failed to flag an undocumented enum variant")
        return 1
    synthetic_result = ["/// Docs.", "pub fn fallible() -> Result<(), ()> { Ok(()) }"]
    if len(missing_safe_result_sections("synthetic.rs", synthetic_result)) != 2:
        print("safe-docs regression failed to flag missing fallible method sections")
        return 1
    synthetic_borrowed_vec = ["/// Docs.", "pub fn borrowed(items: &Vec<u8>) {}"]
    if not safe_signature_borrowing_violations("synthetic.rs", synthetic_borrowed_vec):
        print("safe-docs regression failed to flag borrowed concrete collection")
        return 1
    synthetic_cow = ["/// Docs.", "pub fn maybe_owned(value: Cow<'_, str>) {}"]
    if not safe_signature_borrowing_violations("synthetic.rs", synthetic_cow):
        print("safe-docs regression failed to flag Cow in a public safe signature")
        return 1
    synthetic_clone = [
        "/// Docs.",
        "#[derive(Clone, Debug)]",
        "pub struct Snapshot;",
    ]
    if not safe_non_copy_clone_violations("synthetic.rs", synthetic_clone):
        print("safe-docs regression failed to flag undocumented non-Copy Clone")
        return 1
    synthetic_unsafe = ["/// Docs.", "pub unsafe fn raw() {}"]
    if not missing_unsafe_sections("synthetic.rs", synthetic_unsafe):
        print("safe-docs regression failed to flag a missing unsafe method section")
        return 1

    for path in PUBLIC_DOC_FILES:
        rel = path.relative_to(ROOT).as_posix()
        lines = path.read_text().splitlines()
        missing.extend(missing_public_item_docs(rel, lines))
        missing.extend(missing_public_enum_variant_docs(rel, lines))

    for path in SAFE_FILES:
        rel = path.relative_to(ROOT).as_posix()
        lines = path.read_text().splitlines()
        missing.extend(missing_safe_result_sections(rel, lines))
        missing.extend(safe_signature_borrowing_violations(rel, lines))
        missing.extend(safe_non_copy_clone_violations(rel, lines))

    for path in UNSAFE_SECTION_FILES:
        rel = path.relative_to(ROOT).as_posix()
        lines = path.read_text().splitlines()
        missing.extend(missing_unsafe_sections(rel, lines))

    api_inventory = (ROOT / "docs/api-inventory.md").read_text()
    for phrase in [
        "## Safe Error API Policy",
        "Tier S fallible operations return `Result<T, E>`",
        "Expected absence returns `Option<T>` or `Option<&T>`",
        "Resize failures preserve ownership through",
        "Public fallible methods document",
    ]:
        if phrase not in api_inventory:
            missing.append(f"docs/api-inventory.md: missing safe error policy phrase: {phrase}")

    rust_style = (ROOT / "docs/rust-style.md").read_text()
    for phrase in [
        "| `new(...) -> Self` |",
        "| `unsafe fn new_unchecked(...) -> Self` |",
        "| `from_raw(...)` |",
        "| `from_non_null(...)` |",
        "| `as_ptr(...)` or `as_non_null(...)` |",
        "Internal scalar wrappers may use `get()`",
        "safe value wrappers should prefer a domain-specific accessor name",
        "read text as `&str`",
        "read bytes as `&[u8]`",
        "read paths as `&Path`",
        "read sequences as `&[T]` or iterators",
        "return internal sequences as `&[T]`, not `&Vec<T>`",
        "Take ownership only when the API stores, consumes, or transfers ownership",
        "Store owned text, paths, and bytes as `String`, `PathBuf`, `Vec<T>`, or",
    ]:
        if phrase not in rust_style:
            missing.append(f"docs/rust-style.md: missing safe style phrase: {phrase}")

    if missing:
        print("Public API docs missing required rustdoc coverage:")
        print("\n".join(missing))
        return 1

    print(
        "ok: public docs cover Tier S/Tier U items, fallible safe method "
        "sections, and public unsafe safety sections across "
        f"{len(PUBLIC_DOC_FILES)} documented files and {len(UNSAFE_SECTION_FILES)} unsafe files"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
