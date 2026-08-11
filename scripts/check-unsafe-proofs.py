#!/usr/bin/env python3
"""Audit checked-in unsafe functions and operations for local proof markers.

This is a source audit, not a Rust parser. It uses the repository's
lexical-state-aware masking and balanced item scanner, fails closed on malformed
input, and preserves byte/line offsets. In report mode existing findings are
printed without failing the build; ``--deny`` turns the same findings into a
gate once the checked-in backlog is empty. Marker locality is checked
mechanically; reviewers remain responsible for the proof's semantic accuracy.
"""

from __future__ import annotations

import argparse
import importlib.util
import pathlib
import re
from dataclasses import dataclass


ROOT = pathlib.Path(__file__).resolve().parents[1]
STRICT_TYPES = ROOT / "scripts/check-strict-types.py"
SPEC = importlib.util.spec_from_file_location("check_strict_types", STRICT_TYPES)
if SPEC is None or SPEC.loader is None:
    raise RuntimeError(f"cannot load lexical helpers from {STRICT_TYPES}")
STRICT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(STRICT)

UNSAFE_FN_RE = re.compile(
    # Rust item qualifiers place `const`/`async` before `unsafe`.
    r"\b(?:const\s+)?(?:async\s+)?unsafe\s+(?:extern\s*)?fn\s+"
    r"(?P<name>\$?[A-Za-z_][A-Za-z0-9_]*)"
)
UNSAFE_BLOCK_RE = re.compile(r"\bunsafe\s*\{")
UNSAFE_IMPL_RE = re.compile(r"\bunsafe\s+impl\b")
UNSAFE_EXTERN_RE = re.compile(r"\bunsafe\s+extern\s*\{")
MACRO_RULES_RE = re.compile(r"\bmacro_rules!\s+[A-Za-z_][A-Za-z0-9_]*\s*\{")
SAFETY_MARKER_RE = re.compile(r"\bSAFETY:\s*\S")


@dataclass(frozen=True)
class Finding:
    path: str
    line: int
    kind: str
    item: str
    category: str

    def render(self) -> str:
        item = f" {self.item}" if self.item else ""
        return f"{self.path}:{self.line}: {self.kind}{item} [{self.category}]"


def checked_in_sources() -> list[pathlib.Path]:
    return sorted((ROOT / "src").rglob("*.rs"))


def comment_mask(text: str) -> str:
    """Keep comments only, preserving byte and line offsets."""
    kept = ["\n" if char == "\n" else " " for char in text]
    index = 0
    while index < len(text):
        if text.startswith("//", index):
            end = text.find("\n", index + 2)
            end = len(text) if end < 0 else end
            kept[index:end] = text[index:end]
            index = end
            continue
        if text.startswith("/*", index):
            depth = 1
            end = index + 2
            while end < len(text) and depth:
                if text.startswith("/*", end):
                    depth += 1
                    end += 2
                elif text.startswith("*/", end):
                    depth -= 1
                    end += 2
                else:
                    end += 1
            if depth:
                raise AssertionError(
                    f"unterminated block comment at line {STRICT.line_number(text, index)}"
                )
            kept[index:end] = text[index:end]
            index = end
            continue

        raw = STRICT.RAW_STRING_RE.match(text, index)
        if raw:
            terminator = '"' + raw.group("hashes")
            end = text.find(terminator, raw.end())
            if end < 0:
                raise AssertionError(
                    f"unterminated raw string at line {STRICT.line_number(text, index)}"
                )
            index = end + len(terminator)
            continue

        quote = index
        if text[index] in "bc" and index + 1 < len(text) and text[index + 1] == '"':
            quote += 1
        if text[quote] == '"':
            end = quote + 1
            while end < len(text):
                if text[end] == "\\":
                    end += 2
                elif text[end] == '"':
                    end += 1
                    break
                else:
                    end += 1
            else:
                raise AssertionError(
                    f"unterminated string at line {STRICT.line_number(text, index)}"
                )
            index = end
            continue

        char = STRICT.CHAR_LITERAL_RE.match(text, index)
        if char:
            index = char.end()
            continue
        index += 1
    return "".join(kept)


def validate_delimiters(code: str) -> None:
    pairs = {"(": ")", "[": "]", "{": "}"}
    stack: list[str] = []
    for offset, char in enumerate(code):
        if char in pairs:
            stack.append(pairs[char])
        elif char in ")]}" and (not stack or char != stack.pop()):
            raise AssertionError(
                f"unbalanced source at line {STRICT.line_number(code, offset)}"
            )
    if stack:
        raise AssertionError("unterminated source delimiter")


def attached_safety_docs(text: str, offset: int) -> bool:
    """Return whether the unsafe fn has an attached literal # Safety section."""
    lines = text.splitlines()
    line = STRICT.line_number(text, offset) - 2
    docs: list[str] = []
    attribute_depth = 0
    while line >= 0:
        stripped = lines[line].strip()
        if attribute_depth:
            attribute_depth += stripped.count("]") - stripped.count("[")
            if "# Safety" in stripped:
                docs.append("# Safety")
            line -= 1
            continue
        if not stripped or stripped.startswith("//"):
            if stripped.startswith("///"):
                docs.append(stripped.removeprefix("///").strip())
            line -= 1
            continue
        if stripped.endswith("]") or stripped.startswith("#["):
            attribute_depth = stripped.count("]") - stripped.count("[")
            if "# Safety" in stripped:
                docs.append("# Safety")
            line -= 1
            continue
        break
    return "# Safety" in docs


def macro_spans(code: str) -> list[tuple[int, int]]:
    spans: list[tuple[int, int]] = []
    for match in MACRO_RULES_RE.finditer(code):
        brace = code.find("{", match.start(), match.end())
        spans.append((match.start(), STRICT.matching_delimiter(code, brace)))
    return spans


def category(rel: str, offset: int, spans: list[tuple[int, int]], line: int) -> str:
    if any(start <= offset < end for start, end in spans):
        return "macro-template"
    if rel == "src/platform/windows.rs" and line >= 1800:
        return "translated-etw"
    return "handwritten"


def statement_regions(code: str, offsets: list[int]) -> dict[int, int]:
    """Map unsafe offsets to the start of their containing statement/body."""
    wanted = set(offsets)
    result: dict[int, int] = {}
    brace_depth = 0
    paren_depth = 0
    bracket_depth = 0
    bases: list[tuple[int, int]] = [(0, 0)]
    last_boundary: dict[int, int] = {0: -1}
    for index, char in enumerate(code):
        if index in wanted:
            result[index] = last_boundary.get(brace_depth, -1) + 1
        if char == "(":
            paren_depth += 1
        elif char == ")":
            paren_depth -= 1
        elif char == "[":
            bracket_depth += 1
        elif char == "]":
            bracket_depth -= 1
        elif char == "{":
            brace_depth += 1
            bases.append((paren_depth, bracket_depth))
            last_boundary[brace_depth] = index
        elif char == "}":
            last_boundary.pop(brace_depth, None)
            bases.pop()
            brace_depth -= 1
            last_boundary[brace_depth] = index
        elif char == ";" and (paren_depth, bracket_depth) == bases[-1]:
            last_boundary[brace_depth] = index
    missing = wanted - result.keys()
    if missing:
        raise AssertionError(f"unsafe offsets were not scanned: {sorted(missing)}")
    return result


def safety_markers(comments: str, start: int, end: int) -> list[int]:
    return [start + match.start() for match in SAFETY_MARKER_RE.finditer(comments[start:end])]


def first_inside_comment(
    code: str, comments: str, brace: int, block_end: int
) -> tuple[int, int] | None:
    """Return the first comment when only trivia precedes it inside a block."""
    first_code = brace + 1
    while first_code < block_end and code[first_code].isspace():
        first_code += 1
    first_comment = brace + 1
    while first_comment < first_code and comments[first_comment].isspace():
        first_comment += 1
    if first_comment >= first_code:
        return None
    if comments.startswith("//", first_comment):
        end = comments.find("\n", first_comment + 2, first_code)
        return first_comment, first_code if end < 0 else end
    if comments.startswith("/*", first_comment):
        depth = 1
        end = first_comment + 2
        while end < first_code and depth:
            if comments.startswith("/*", end):
                depth += 1
                end += 2
            elif comments.startswith("*/", end):
                depth -= 1
                end += 2
            else:
                end += 1
        if depth:
            raise AssertionError("comment mask lost a balanced block-comment boundary")
        return first_comment, end
    return None


def scan_source(rel: str, source: str) -> list[Finding]:
    code = STRICT.mask_non_code(source)
    validate_delimiters(code)
    comments = comment_mask(source)
    spans = macro_spans(code)
    findings: list[Finding] = []

    for match in UNSAFE_FN_RE.finditer(code):
        line = STRICT.line_number(source, match.start())
        if not attached_safety_docs(source, match.start()):
            findings.append(
                Finding(
                    rel,
                    line,
                    "unsafe fn missing # Safety docs",
                    match.group("name"),
                    category(rel, match.start(), spans, line),
                )
            )

    operations = [
        ("unsafe block missing adjacent SAFETY marker", match.start(), match.end() - 1)
        for match in UNSAFE_BLOCK_RE.finditer(code)
    ] + [
        ("unsafe impl missing adjacent SAFETY marker", match.start(), None)
        for match in UNSAFE_IMPL_RE.finditer(code)
    ] + [
        ("unsafe extern block missing adjacent SAFETY marker", match.start(), None)
        for match in UNSAFE_EXTERN_RE.finditer(code)
    ]
    regions = statement_regions(code, [offset for _, offset, _ in operations])
    used_markers: set[int] = set()
    for kind, offset, brace in sorted(operations, key=lambda operation: operation[1]):
        region = regions[offset]
        before = safety_markers(comments, region, offset)
        available_before = [marker for marker in before if marker not in used_markers]
        matched: list[int] = []
        if available_before:
            # All leading markers belong to the immediately following operation;
            # none may drift forward to a sibling unsafe operation.
            matched = available_before
        if brace is not None:
            block_end = STRICT.matching_delimiter(code, brace)
            leading_comment = first_inside_comment(code, comments, brace, block_end)
            if not matched and leading_comment is not None:
                inside = safety_markers(comments, *leading_comment)
                matched = [marker for marker in inside if marker not in used_markers]
        if not matched:
            line = STRICT.line_number(source, offset)
            findings.append(
                Finding(rel, line, kind, "", category(rel, offset, spans, line))
            )
        used_markers.update(matched)
    return findings


def synthetic_findings(source: str) -> list[Finding]:
    return scan_source("src/synthetic.rs", source)


def self_test() -> None:
    accepted = r'''/// # Safety
/// Caller keeps `ptr` live.
#[cfg(all(
    feature = "synthetic",
    not(test),
))]
unsafe fn documented(ptr: *mut u8) {
    // SAFETY: `ptr` is live and aligned for this one-byte write.
    unsafe { ptr.write(1) };
}

// SAFETY: the marker explains why the synthetic type is Send.
unsafe impl Send for Synthetic {}

// SAFETY: these declarations exactly match the synthetic platform ABI.
unsafe extern "C" {
    fn operation();
}

/// # Safety
/// Caller proves the const constructor invariant.
const unsafe fn const_constructor() {}

/// # Safety
/// Caller proves the asynchronous operation invariant.
async unsafe fn async_operation() {}

fn leading_inside_marker() {
    unsafe {
        // SAFETY: this is the first comment and proves this exact block.
        operation()
    }
}

macro_rules! generated {
    () => {
        /// # Safety
        /// Caller proves the generated pointer contract.
        unsafe fn generated() {
            // SAFETY: the generated operation uses its caller-proven pointer.
            unsafe { operation() };
        }
    };
}

#[cfg(test)]
mod tests {
    /// # Safety
    /// Test caller proves the operation contract.
    unsafe fn checked_too() {
        // SAFETY: the test caller proved this exact operation contract.
        unsafe { operation() }
    }
}

mod nested {
    #[cfg(all(feature = "synthetic", test))]
    /// # Safety
    /// Test caller proves the nested operation contract.
    unsafe fn checked_too() {
        // SAFETY: the test caller proved this exact nested operation contract.
        unsafe { operation() }
    }
}
'''
    assert not synthetic_findings(accepted)

    missing = r'''unsafe fn missing_docs() {
    unsafe { missing_marker() };
    unsafe { another_missing_marker() };
}

unsafe impl Sync for Missing {}

unsafe extern "C" { static MISSING: u8; }

const unsafe fn missing_const_docs() {}

async unsafe fn missing_async_docs() {}

const
unsafe extern "C"
fn missing_multiline_abi_docs() {}

/// # Safety
/// Caller proves both operation contracts.
unsafe fn marker_is_not_reused() {
    // SAFETY: this proof applies only to the first operation.
    unsafe { first_operation() };
    unsafe { second_operation() };
}

macro_rules! generated {
    ($name:ident) => {
        const unsafe fn $name() {
            unsafe { missing_marker() };
        }
    };
}
'''
    found = synthetic_findings(missing)
    assert sum("unsafe fn" in item.kind for item in found) == 5
    assert {
        "missing_const_docs",
        "missing_async_docs",
        "missing_multiline_abi_docs",
    } <= {item.item for item in found}
    assert sum("unsafe block" in item.kind for item in found) == 4
    assert sum("unsafe impl" in item.kind for item in found) == 1
    assert sum("unsafe extern" in item.kind for item in found) == 1
    assert any(item.category == "macro-template" for item in found)

    attachment = r'''fn siblings() {
    // SAFETY: this marker belongs to the first tuple element only.
    let _ = (unsafe { first() }, unsafe { second() });
}

fn nested() {
    // SAFETY: this marker belongs to the outer block only.
    unsafe {
        unsafe { nested_operation() }
    }
}

fn late_inside_comment() {
    unsafe {
        operation();
        // SAFETY: too late to prove the enclosing block.
    }
}
'''
    found = synthetic_findings(attachment)
    block_lines = [item.line for item in found if "unsafe block" in item.kind]
    assert block_lines == [3, 9, 14]

    decoys = r'''const TEXT: &str = r###"
unsafe fn fake() { unsafe { fake() } }
// SAFETY: fake
"###;
// unsafe fn fake_comment() { unsafe { fake() } }
type UnsafePointer = unsafe fn(*mut u8);
const UNSAFE_POINTER: unsafe fn(*mut u8) = pointer_target;
#[cfg(any(test, feature = "synthetic"))]
unsafe fn visible() { unsafe { missing_marker() } }

#[cfg(test)]
unsafe fn test_code_is_visible() { unsafe { missing_marker() } }
'''
    found = synthetic_findings(decoys)
    assert sum("unsafe fn" in item.kind for item in found) == 2
    assert sum("unsafe block" in item.kind for item in found) == 2

    malformed = [
        "const TEXT: &str = \"unterminated",
        "/* unterminated",
        "fn broken() { unsafe { operation(); }",
    ]
    for source in malformed:
        try:
            synthetic_findings(source)
        except AssertionError:
            pass
        else:
            raise AssertionError("malformed synthetic source was silently accepted")


def main() -> int:
    parser = argparse.ArgumentParser()
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--report", action="store_true", help="report findings and exit successfully")
    mode.add_argument("--deny", action="store_true", help="fail when any finding is present")
    parser.add_argument("--verbose", action="store_true", help="print every finding")
    args = parser.parse_args()

    self_test()
    findings: list[Finding] = []
    checked = {
        "unsafe fn": 0,
        "unsafe block": 0,
        "unsafe impl": 0,
        "unsafe extern block": 0,
    }
    for path in checked_in_sources():
        rel = path.relative_to(ROOT).as_posix()
        source = path.read_text()
        code = STRICT.mask_non_code(source)
        checked["unsafe fn"] += len(UNSAFE_FN_RE.findall(code))
        checked["unsafe block"] += len(UNSAFE_BLOCK_RE.findall(code))
        checked["unsafe impl"] += len(UNSAFE_IMPL_RE.findall(code))
        checked["unsafe extern block"] += len(UNSAFE_EXTERN_RE.findall(code))
        findings.extend(scan_source(rel, source))

    by_kind = {
        "unsafe fn missing # Safety docs": 0,
        "unsafe block missing adjacent SAFETY marker": 0,
        "unsafe impl missing adjacent SAFETY marker": 0,
        "unsafe extern block missing adjacent SAFETY marker": 0,
    }
    by_category: dict[str, int] = {}
    by_file: dict[str, int] = {}
    for finding in findings:
        by_kind[finding.kind] = by_kind.get(finding.kind, 0) + 1
        by_category[finding.category] = by_category.get(finding.category, 0) + 1
        by_file[finding.path] = by_file.get(finding.path, 0) + 1

    print("checked-in unsafe proof audit")
    print("checked:")
    for kind, count in checked.items():
        print(f"  {count:4}  {kind}")
    print(f"findings: {len(findings)}")
    for kind, count in sorted(by_kind.items()):
        print(f"  {count:4}  {kind}")
    print("categories:")
    for name, count in sorted(by_category.items()):
        print(f"  {count:4}  {name}")
    print("largest files:")
    for path, count in sorted(by_file.items(), key=lambda row: (-row[1], row[0]))[:20]:
        print(f"  {count:4}  {path}")
    if args.verbose:
        print("details:")
        for finding in findings:
            print(f"  {finding.render()}")

    if findings and args.deny:
        print("unsafe proof audit failed")
        return 1
    if findings:
        print("report-only: use --deny once the checked-in backlog is empty")
    else:
        print("ok: every checked-in unsafe function and operation has a local proof marker")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
