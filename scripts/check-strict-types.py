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

RAW_STRING_RE = re.compile(r'(?:b|c)?r(?P<hashes>#{0,255})"')
CHAR_LITERAL_RE = re.compile(r"'(?:\\.|[^'\\\r\n])'")


def source_files() -> list[pathlib.Path]:
    return sorted((ROOT / "src").rglob("*.rs"))


def is_test_source(path: pathlib.Path) -> bool:
    return (
        path.name == "tests.rs"
        or path.name.endswith("_tests.rs")
        or path.name == "concurrency_models.rs"
    )


def line_number(text: str, offset: int) -> int:
    return text.count("\n", 0, offset) + 1


def mask_non_code(text: str) -> str:
    """Blank comments and string literals while retaining byte and line offsets."""
    masked = list(text)

    def blank(start: int, end: int) -> None:
        for offset in range(start, end):
            if masked[offset] not in "\r\n":
                masked[offset] = " "

    index = 0
    while index < len(text):
        if text.startswith("//", index):
            end = text.find("\n", index + 2)
            end = len(text) if end < 0 else end
            blank(index, end)
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
                line = line_number(text, index)
                raise AssertionError(f"unterminated block comment at line {line}")
            blank(index, end)
            index = end
            continue

        raw = RAW_STRING_RE.match(text, index)
        if raw:
            terminator = '"' + raw.group("hashes")
            end = text.find(terminator, raw.end())
            if end < 0:
                raise AssertionError(f"unterminated raw string at line {line_number(text, index)}")
            end += len(terminator)
            blank(index, end)
            index = end
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
                raise AssertionError(f"unterminated string at line {line_number(text, index)}")
            blank(index, end)
            index = end
            continue

        char = CHAR_LITERAL_RE.match(text, index)
        if char:
            end = char.end()
            blank(index, end)
            index = end
            continue
        index += 1

    return "".join(masked)


def matching_delimiter(text: str, start: int) -> int:
    pairs = {"(": ")", "[": "]", "{": "}"}
    stack = [pairs[text[start]]]
    index = start + 1
    while index < len(text) and stack:
        char = text[index]
        if char in pairs:
            stack.append(pairs[char])
        elif char in ")]}":
            if char != stack.pop():
                raise AssertionError(f"unbalanced delimiter at line {line_number(text, index)}")
        index += 1
    if stack:
        raise AssertionError(f"unterminated delimiter at line {line_number(text, start)}")
    return index


def split_cfg_arguments(arguments: str) -> list[str]:
    parts: list[str] = []
    start = 0
    stack: list[str] = []
    pairs = {"(": ")", "[": "]", "{": "}"}
    for index, char in enumerate(arguments):
        if char in pairs:
            stack.append(pairs[char])
        elif char in ")]}":
            if not stack or char != stack.pop():
                return []
        elif char == "," and not stack:
            parts.append(arguments[start:index])
            start = index + 1
    parts.append(arguments[start:])
    return parts


def is_test_cfg_attribute(attribute: str) -> bool:
    compact = re.sub(r"\s+", "", attribute)
    if compact == "#[cfg(test)]":
        return True
    prefix = "#[cfg(all("
    suffix = "))]"
    if not compact.startswith(prefix) or not compact.endswith(suffix):
        return False
    arguments = compact[len(prefix) : -len(suffix)]
    return "test" in split_cfg_arguments(arguments)


def item_requires_semicolon(code: str, start: int) -> bool:
    item = code[start:].lstrip()
    item = re.sub(r"^pub(?:\s*\([^)]*\))?\s+", "", item, count=1)
    if re.match(r"^(?:static(?:\s+mut)?|type|use)\b", item):
        return True
    if not item.startswith("const "):
        return False
    const_fn = re.match(r"^const\s+(?:(?:async|unsafe|extern)\s+)*fn\b", item)
    return const_fn is None


def attributed_item_end(code: str, start: int) -> int:
    index = start
    stack: list[str] = []
    pairs = {"(": ")", "[": "]", "{": "}"}
    requires_semicolon = item_requires_semicolon(code, start)
    while index < len(code):
        char = code[index]
        if char in pairs:
            stack.append(pairs[char])
        elif char in ")]}":
            if not stack or char != stack.pop():
                raise AssertionError(f"unbalanced item at line {line_number(code, index)}")
            if not stack and char == "}" and not requires_semicolon:
                end = index + 1
                while end < len(code) and code[end].isspace():
                    end += 1
                return end + 1 if end < len(code) and code[end] == ";" else index + 1
        elif char == ";" and not stack:
            return index + 1
        index += 1
    raise AssertionError(f"unterminated cfg(test) item at line {line_number(code, start)}")


def mask_top_level_test_items(text: str) -> str:
    """Mask top-level test-only items without changing source locations."""
    code = mask_non_code(text)
    masked = list(text)
    stack: list[str] = []
    pairs = {"(": ")", "[": "]", "{": "}"}
    index = 0
    while index < len(code):
        if not stack and code.startswith("#[", index):
            attribute_end = matching_delimiter(code, index + 1)
            attribute = code[index:attribute_end]
            if is_test_cfg_attribute(attribute):
                item_start = attribute_end
                while True:
                    while item_start < len(code) and code[item_start].isspace():
                        item_start += 1
                    if not code.startswith("#[", item_start):
                        break
                    item_start = matching_delimiter(code, item_start + 1)
                item_end = attributed_item_end(code, item_start)
                for offset in range(index, item_end):
                    if masked[offset] not in "\r\n":
                        masked[offset] = " "
                index = item_end
                continue
        char = code[index]
        if char in pairs:
            stack.append(pairs[char])
        elif char in ")]}":
            if not stack or char != stack.pop():
                raise AssertionError(f"unbalanced source at line {line_number(code, index)}")
        index += 1
    if stack:
        raise AssertionError("unterminated source delimiter")
    return "".join(masked)


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

    for path in source_files():
        if is_test_source(path):
            continue
        rel = path.relative_to(ROOT).as_posix()
        text = mask_top_level_test_items(path.read_text())

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

    if violations:
        raise AssertionError("strict-type boundary check failed:\n" + "\n".join(violations))

    print(
        "ok: strict-type boundaries confine union fields, block links, packed "
        "thread words, atomic casts, Deref, and dynamic dispatch"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
