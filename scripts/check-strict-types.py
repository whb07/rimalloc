#!/usr/bin/env python3
"""Source checks for the strict-type migration contract."""

from __future__ import annotations

import fnmatch
import pathlib
import re
from dataclasses import dataclass


ROOT = pathlib.Path(__file__).resolve().parents[1]
ALLOWLIST = ROOT / "strict-type-allowlist.txt"

RAW_BOUNDARY_FILES = {
    "src/abi.rs",
    "src/raw.rs",
    "src/types.rs",
}

RAW_POINTER_TYPES = (
    "mi_page_t",
    "mi_block_t",
    "mi_heap_t",
    "mi_theap_t",
    "mi_arena_t",
    "mi_subproc_t",
)

RAW_POINTER_PARAM_RE = re.compile(
    r"\bfn\s+(?P<name>[A-Za-z_][A-Za-z0-9_]*)\s*\([^)]*?\*(?:mut|const)\s+"
    r"(?P<domain>mi_(?:page|block|heap|theap|arena|subproc)_t)\b",
    re.DOTALL,
)
DIRECT_MEMID_RE = re.compile(r"\bmemid\s*\.\s*(mem|arena|os|tag|id)\b")
BLOCK_NEXT_RE = re.compile(r"\(\s*\*\s*(?:block|free|next)\s*\)\s*\.next\b")
XTHREAD_ID_BITS_RE = re.compile(r"\bxthread_id\b.*(?:&|\||\^|<<|>>)|(?:&|\||\^|<<|>>).*?\bxthread_id\b")
XTHREAD_FREE_BITS_RE = re.compile(r"\bxthread_free\b.*(?:&|\||\^|<<|>>)|(?:&|\||\^|<<|>>).*?\bxthread_free\b")
ATOMIC_CAST_RE = re.compile(r"\bas\s+\*const\s+Atomic|\bas\s+\*mut\s+Atomic")
METADATA_REF_RE = re.compile(r"&\s*(?:mut\s*)?\*(?!\s*(?:mut|const)\b)\s*[A-Za-z_][A-Za-z0-9_]*")
PTR_USIZE_RE = re.compile(
    r"(?:ptr|page|block|base|start|aligned_p|padding_ptr|p)\s+as\s+usize\b"
)
HEAP_TYPES_RE = re.compile(r"\b(Vec|String|Box|Arc|Mutex|OnceLock)\b")
BARE_DOMAIN_PARAM_RE = re.compile(
    r"\bfn\s+(?P<name>[A-Za-z_][A-Za-z0-9_]*)\s*\([^)]*?"
    r"(?P<param>\b(?:size|newsize|oldsize|reqsize|block_size|bsize|alignment|"
    r"offset|count|newcount|idx|index|bin|capacity|reserved|used|slice_count|"
    r"block_idx|chunk_idx|bufsize|buf_size)\s*:\s*usize\b)",
    re.DOTALL,
)
CORE_TYPES_DEAD_CODE_RE = re.compile(r"#\s*!\s*\[\s*allow\s*\(\s*dead_code\s*\)\s*\]")

SKIP_DIRS = {
    ".git",
    "target",
    "benches",
    "examples",
    "mimalloc-3.3.2",
    "mimalloc-bench",
    "tests",
}

ALLOWED_DIRECT_MEMID = {
    "src/core_types/memid.rs",
    "src/types.rs",
}

ALLOWED_BLOCK_NEXT = {
    "src/core_types/freelist_wrappers.rs",
    "src/types.rs",
}

ALLOWED_XTHREAD_ID = {
    "src/core_types/page.rs",
    "src/core_types/atomic_wrappers.rs",
    "src/types.rs",
}

ALLOWED_XTHREAD_FREE = {
    "src/core_types/freelist.rs",
    "src/core_types/atomic_wrappers.rs",
    "src/types.rs",
}

ALLOWED_ATOMIC_CAST = {
    "src/core_types/atomic_wrappers.rs",
}

ALLOWED_PTR_USIZE = {
    "src/core_types/ptr.rs",
}


@dataclass(frozen=True)
class AllowEntry:
    kind: str
    path: str
    symbol: str
    domain: str
    reason: str
    disposition: str


def source_files() -> list[pathlib.Path]:
    files: list[pathlib.Path] = []
    for path in ROOT.rglob("*.rs"):
        rel_parts = path.relative_to(ROOT).parts
        if any(part in SKIP_DIRS for part in rel_parts):
            continue
        files.append(path)
    return sorted(files)


def strip_line_comment(line: str) -> str:
    return line.split("//", 1)[0]


def read_allowlist() -> list[AllowEntry]:
    entries: list[AllowEntry] = []
    if not ALLOWLIST.exists():
        raise AssertionError("missing strict-type allowlist: strict-type-allowlist.txt")
    for lineno, raw_line in enumerate(ALLOWLIST.read_text().splitlines(), start=1):
        line = raw_line.strip()
        if not line or line.startswith("#"):
            continue
        parts = line.split("|", 5)
        if len(parts) != 6:
            raise AssertionError(f"invalid allowlist line {lineno}: expected 6 fields")
        entry = AllowEntry(*[part.strip() for part in parts])
        if not entry.reason:
            raise AssertionError(f"invalid allowlist line {lineno}: missing reason")
        if not (
            entry.disposition.startswith("permanent:")
            or entry.disposition.startswith("remove:")
        ):
            raise AssertionError(
                f"invalid allowlist line {lineno}: disposition must start with "
                "'permanent:' or 'remove:'"
            )
        entries.append(entry)
    return entries


def is_allowed(
    entries: list[AllowEntry],
    *,
    kind: str,
    rel: str,
    symbol: str,
    domain: str,
) -> bool:
    for entry in entries:
        if entry.kind != kind:
            continue
        if not fnmatch.fnmatch(rel, entry.path):
            continue
        if entry.symbol not in {"*", symbol}:
            continue
        if kind == "raw-pointer-parameter" and (
            entry.domain != "*"
            and entry.domain != domain
            and domain not in entry.domain
            and entry.domain not in domain
        ):
            continue
        return True
    return False


def is_raw_compat_function(text: str, start: int) -> bool:
    prefix = text[max(0, start - 180) : start]
    return (
        'extern "C"' in prefix
        or "#[unsafe(no_mangle)]" in prefix
        or "#[unsafe(export_name" in prefix
    )


def line_offender(rel: str, lineno: int, line: str, message: str) -> str:
    return f"{rel}:{lineno}: {message}: {line.strip()}"


def require_no_pattern(
    offenders: list[str],
    *,
    kind: str,
    rel: str,
    text: str,
    allowed_files: set[str],
    pattern: re.Pattern[str],
    entries: list[AllowEntry],
    domain: str,
    message: str,
) -> None:
    if rel in allowed_files:
        return
    for lineno, line in enumerate(text.splitlines(), start=1):
        code = strip_line_comment(line)
        if not pattern.search(code):
            continue
        if is_allowed(entries, kind=kind, rel=rel, symbol="*", domain=domain):
            continue
        offenders.append(line_offender(rel, lineno, line, message))


def main() -> int:
    entries = read_allowlist()
    offenders: list[str] = []
    checked_raw_params = 0

    for path in source_files():
        rel = path.relative_to(ROOT).as_posix()
        text = path.read_text()

        if rel.startswith("src/core_types/") and CORE_TYPES_DEAD_CODE_RE.search(text):
            offenders.append(f"{rel}: crate-level dead_code allowance in core_types module")

        if rel not in RAW_BOUNDARY_FILES:
            for match in RAW_POINTER_PARAM_RE.finditer(text):
                symbol = match.group("name")
                domain = match.group("domain")
                checked_raw_params += 1
                if not is_allowed(
                    entries,
                    kind="raw-pointer-parameter",
                    rel=rel,
                    symbol=symbol,
                    domain=domain,
                ):
                    line = text.count("\n", 0, match.start()) + 1
                    offenders.append(
                        f"{rel}:{line}: raw {domain} parameter in {symbol} lacks strict-type allowlist entry"
                    )

            for match in BARE_DOMAIN_PARAM_RE.finditer(text):
                if is_raw_compat_function(text, match.start()):
                    continue
                symbol = match.group("name")
                if is_allowed(
                    entries,
                    kind="bare-domain-usize",
                    rel=rel,
                    symbol=symbol,
                    domain="usize",
                ):
                    continue
                line = text.count("\n", 0, match.start()) + 1
                offenders.append(
                    f"{rel}:{line}: bare domain usize parameter in {symbol} "
                    "lacks strict-type allowlist entry"
                )

        require_no_pattern(
            offenders,
            kind="direct-memid-read",
            rel=rel,
            text=text,
            allowed_files=ALLOWED_DIRECT_MEMID,
            pattern=DIRECT_MEMID_RE,
            entries=entries,
            domain="memid",
            message="direct memid union read outside MemId helpers",
        )
        require_no_pattern(
            offenders,
            kind="block-next-access",
            rel=rel,
            text=text,
            allowed_files=ALLOWED_BLOCK_NEXT,
            pattern=BLOCK_NEXT_RE,
            entries=entries,
            domain="mi_block_s.next",
            message="direct mi_block_s.next access outside free-list wrappers",
        )
        require_no_pattern(
            offenders,
            kind="xthread-id-bitop",
            rel=rel,
            text=text,
            allowed_files=ALLOWED_XTHREAD_ID,
            pattern=XTHREAD_ID_BITS_RE,
            entries=entries,
            domain="xthread_id",
            message="direct xthread_id bit operation outside typed helpers",
        )
        require_no_pattern(
            offenders,
            kind="xthread-free-bitop",
            rel=rel,
            text=text,
            allowed_files=ALLOWED_XTHREAD_FREE,
            pattern=XTHREAD_FREE_BITS_RE,
            entries=entries,
            domain="xthread_free",
            message="direct xthread_free bit operation outside typed helpers",
        )
        require_no_pattern(
            offenders,
            kind="atomic-cast",
            rel=rel,
            text=text,
            allowed_files=ALLOWED_ATOMIC_CAST,
            pattern=ATOMIC_CAST_RE,
            entries=entries,
            domain="atomic-field",
            message="pointer-to-atomic cast outside atomic field wrappers",
        )
        require_no_pattern(
            offenders,
            kind="metadata-reference",
            rel=rel,
            text=text,
            allowed_files={"src/types.rs"},
            pattern=METADATA_REF_RE,
            entries=entries,
            domain="metadata",
            message="broad raw metadata reference outside typed field helpers",
        )
        require_no_pattern(
            offenders,
            kind="pointer-usize-roundtrip",
            rel=rel,
            text=text,
            allowed_files=ALLOWED_PTR_USIZE,
            pattern=PTR_USIZE_RE,
            entries=entries,
            domain="provenance",
            message="pointer/integer round trip outside typed provenance helpers",
        )
        require_no_pattern(
            offenders,
            kind="heap-allocating-type",
            rel=rel,
            text=text,
            allowed_files={"src/safe.rs"},
            pattern=HEAP_TYPES_RE,
            entries=entries,
            domain="heap-type",
            message="heap-allocating Rust type in allocator internals",
        )

    if offenders:
        raise AssertionError(
            "strict-type source check failed:\n" + "\n".join(offenders[:200])
        )

    print(
        "ok: strict-type checks passed; "
        f"{checked_raw_params} raw pointer parameter sites are either boundary or allowlisted"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
