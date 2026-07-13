#!/usr/bin/env python3
"""Source checks for the strict-type migration contract."""

from __future__ import annotations

import fnmatch
import pathlib
import re
from collections.abc import Iterable
from dataclasses import dataclass


ROOT = pathlib.Path(__file__).resolve().parents[1]
ALLOWLIST = ROOT / "strict-type-allowlist.txt"
STRICT_TYPES_DOC = ROOT / "docs" / "strict-types.md"
REMOVE_ENTRY_CAP = 0

RAW_BOUNDARY_FILES = {
    "src/ffi/abi.rs",
    "src/raw/mod.rs",
    "src/types/constants.rs",
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
DYNAMIC_DISPATCH_RE = re.compile(r"\bdyn\s+[A-Za-z_]")
BARE_DOMAIN_PARAM_RE = re.compile(
    r"\bfn\s+(?P<name>[A-Za-z_][A-Za-z0-9_]*)\s*\([^)]*?"
    r"(?P<param>\b(?:size|newsize|oldsize|reqsize|block_size|bsize|alignment|"
    r"offset|count|newcount|idx|index|bin|capacity|reserved|used|slice_count|"
    r"block_idx|chunk_idx|bufsize|buf_size)\s*:\s*usize\b)",
    re.DOTALL,
)
CORE_TYPES_DEAD_CODE_RE = re.compile(r"#\s*!\s*\[\s*allow\s*\(\s*dead_code\s*\)\s*\]")
LINEAR_CAPABILITY_TYPES = {
    "AbandonedMappedPageType",
    "AbandonedDisposition",
    "AbandonedPageType",
    "ActivePage",
    "AllocatedBlock",
    "AllFreePage",
    "AvailableFreeList",
    "AvailablePage",
    "LeasedBatch",
    "BlockForFree",
    "DestroyedPage",
    "DetachedRemoteList",
    "EncodedFreeList",
    "ForeignPage",
    "FreeBlock",
    "FullPage",
    "GuardedBlock",
    "LocalPage",
    "LocalFreeBatch",
    "LocalFreeList",
    "LocatedBlock",
    "OwnedAbandonedPage",
    "Page",
    "PageAbandonOutcome",
    "PageForAbandon",
    "PageFreeOwner",
    "PointerLocation",
    "PublishedRemoteHead",
    "QueuedPage",
    "RetiredPage",
    "RemotePushResult",
    "RemoteFreeBatch",
    "UnqueuedPage",
    "ValidatedFree",
    "ValidatedFreeParts",
    "ValidPointerLookup",
}
MUST_NOT_BE_COPY_TYPES = {"PointerLookup"}
CAPABILITY_CONSTRUCTOR_TYPES = {
    "AbandonedMappedPageType",
    "AbandonedPageType",
    "ActivePage",
    "AvailablePage",
    "DestroyedPage",
    "ForeignPage",
    "FullPage",
    "LocalPage",
    "OwnedAbandonedPage",
    "RetiredPage",
}
CAPABILITY_CONSTRUCTOR_RE = re.compile(
    rf"\b(?:(?:{'|'.join(sorted(CAPABILITY_CONSTRUCTOR_TYPES))})|Page(?:::\s*<[^>]+>)?)::new_unchecked\s*\("
)
CAPABILITY_CONSTRUCTOR_FILES = {
    "src/types/page_states.rs",
    "src/types/page_queues.rs",
    "src/types/raw_bridge.rs",
}
GUARDED_CONSTRUCTOR_RE = re.compile(r"\bGuardedBlock::after_guarded_detection\s*\(")
GUARDED_CONSTRUCTOR_FILES = {
    "src/allocator/alloc.rs",
    "src/allocator/free.rs",
}
RAW_BRIDGE_CALL_RE = re.compile(
    r"\b(?P<name>assume_(?:uninit_page|active_page|all_free_abandoned|full_page|"
    r"local_page|foreign_page|abandoned_page|owned_abandoned_page))\s*\("
)
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
RAW_CAPABILITY_CONSTRUCTOR_RE = re.compile(
    r"\b(?P<type>PublishedRemoteHead|DetachedRemoteList)::from_raw_compat\s*\("
)
RAW_CAPABILITY_CONSTRUCTOR_CALLERS = {
    "PublishedRemoteHead": {
        "src/allocator/free.rs",
        "src/allocator/page/lifecycle.rs",
    },
    "DetachedRemoteList": {"src/allocator/page/lifecycle.rs"},
}
DERIVED_TRAITS_RE = re.compile(
    r"#\[derive\((?P<traits>[^)]*)\)\](?:\s*#\[[^]]+\])*\s*"
    r"(?:pub(?:\([^)]*\))?\s+)?(?:struct|enum)\s+(?P<name>[A-Za-z_][A-Za-z0-9_]*)"
)
TYPE_DECL_RE = re.compile(
    r"(?P<attrs>(?:#\[[^\n]+\]\s*)*)"
    r"(?:pub(?:\([^)]*\))?\s+)?(?:struct|enum)\s+(?P<name>[A-Za-z_][A-Za-z0-9_]*)"
)
FN_ITEM_RE = re.compile(r"\bfn\s+(?P<name>[A-Za-z_][A-Za-z0-9_]*)\s*(?:<[^>{}]*>)?\(")
STATIC_OR_CONST_ITEM_RE = re.compile(
    r"^\s*(?:(?:pub|pub\([^)]*\)|pub\s*\([^)]*\))\s+)?"
    r"(?:static|const)\s+(?:mut\s+)?(?P<name>[A-Za-z_][A-Za-z0-9_]*)\b"
)
IMPL_START_RE = re.compile(r"^\s*impl\b")
EXTERN_BLOCK_RE = re.compile(r"\bextern(?:\s+\"(?P<abi>[^\"]+)\")?\s*\{")

SKIP_DIRS = {
    ".git",
    "target",
    "benches",
    "examples",
    "fuzz",
    "mimalloc-3.3.2",
    "mimalloc-bench",
    "tests",
}

PATH_ALIASES = {
    "src/internal/bitmap/bfield.rs": {"src/bitmap.rs", "src/internal/bitmap.rs"},
    "src/internal/bitmap.rs": {"src/bitmap.rs"},
    "src/internal/guarded.rs": {"src/types/guarded.rs"},
}

ALLOWED_DIRECT_MEMID = {
    "src/types/memid.rs",
    "src/types/constants.rs",
}

ALLOWED_BLOCK_NEXT = {
    "src/types/freelist_wrappers.rs",
    "src/types/constants.rs",
}

ALLOWED_XTHREAD_ID = {
    "src/types/page.rs",
    "src/types/atomic.rs",
    "src/types/atomic/page_thread.rs",
    "src/types/constants.rs",
}

ALLOWED_XTHREAD_FREE = {
    "src/types/freelist.rs",
    "src/types/atomic.rs",
    "src/types/atomic/thread_free.rs",
    "src/types/constants.rs",
}

ALLOWED_ATOMIC_CAST = {
    "src/types/atomic.rs",
}

ALLOWED_PTR_USIZE = {
    "src/types/ptr.rs",
}


@dataclass(frozen=True)
class AllowEntry:
    kind: str
    path: str
    symbol: str
    domain: str
    reason: str
    disposition: str


@dataclass(frozen=True)
class ItemContext:
    symbols: tuple[str, ...]
    kind: str

    @property
    def primary_symbol(self) -> str:
        return self.symbols[0]


@dataclass(frozen=True)
class ActiveBlock:
    context: ItemContext
    end_depth: int
    impl_type: str | None = None


def source_files() -> list[pathlib.Path]:
    files: list[pathlib.Path] = []
    for path in ROOT.rglob("*.rs"):
        rel_parts = path.relative_to(ROOT).parts
        if rel_parts == ("build.rs",):
            continue
        if any(part in SKIP_DIRS for part in rel_parts):
            continue
        files.append(path)
    return sorted(files)


def strip_line_comment(line: str) -> str:
    return line.split("//", 1)[0]


def brace_delta(line: str) -> int:
    return line.count("{") - line.count("}")


def is_test_cfg_attr(trimmed: str) -> bool:
    return trimmed.startswith("#[cfg(test)]") or trimmed.startswith("#[cfg(all(test,")


def is_module_start(trimmed: str) -> bool:
    return (
        trimmed.startswith("mod ")
        or trimmed.startswith("pub mod ")
        or trimmed.startswith("pub(crate) mod ")
    )


def production_source_text(text: str) -> str:
    lines: list[str] = []
    skip_test_module = False
    pending_test_cfg = False
    brace_depth = 0

    for line in text.splitlines():
        trimmed = line.lstrip()

        if skip_test_module:
            lines.append("")
            brace_depth += line.count("{")
            brace_depth -= line.count("}")
            if brace_depth <= 0:
                skip_test_module = False
                brace_depth = 0
            continue

        if is_test_cfg_attr(trimmed):
            pending_test_cfg = True
            lines.append("")
            continue

        if pending_test_cfg and is_module_start(trimmed):
            skip_test_module = True
            brace_depth = line.count("{") - line.count("}")
            lines.append("")
            if brace_depth <= 0:
                skip_test_module = False
            pending_test_cfg = False
            continue

        pending_test_cfg = False
        lines.append(line)

    return "\n".join(lines)


def strip_angle_groups(value: str) -> str:
    result: list[str] = []
    depth = 0
    for ch in value:
        if ch == "<":
            depth += 1
            continue
        if ch == ">" and depth > 0:
            depth -= 1
            continue
        if depth == 0:
            result.append(ch)
    return "".join(result)


def impl_type_symbol(line: str) -> str:
    stripped = strip_line_comment(line).strip()
    if "{" in stripped:
        stripped = stripped.split("{", 1)[0].strip()
    body = stripped.removeprefix("impl").strip()
    if body.startswith("<"):
        depth = 0
        for index, ch in enumerate(body):
            if ch == "<":
                depth += 1
            elif ch == ">":
                depth -= 1
                if depth == 0:
                    body = body[index + 1 :].strip()
                    break
    if " for " in body:
        body = body.rsplit(" for ", 1)[1].strip()
    if " where " in body:
        body = body.split(" where ", 1)[0].strip()
    body = strip_angle_groups(body).strip()
    if "::" in body:
        body = body.rsplit("::", 1)[1].strip()
    return body or "impl"


def item_context_for(
    *,
    name: str,
    kind: str,
    impl_type: str | None,
) -> ItemContext:
    if impl_type is None:
        return ItemContext((name,), kind)
    return ItemContext((f"{impl_type}::{name}", name), kind)


def has_item_body_after(code: str, start: int) -> bool:
    opening = code.find("{", start)
    return opening != -1 and not has_top_level_semicolon(code, start, opening)


def has_top_level_semicolon(code: str, start: int, end: int | None = None) -> bool:
    square_depth = 0
    for ch in code[start:end]:
        if ch == "[":
            square_depth += 1
        elif ch == "]" and square_depth > 0:
            square_depth -= 1
        elif ch == ";" and square_depth == 0:
            return True
    return False


def static_or_const_initializer_tail(code: str, start: int) -> str:
    initializer = code.find("=", start)
    if initializer == -1:
        return code[start:]
    return code[initializer:]


def item_contexts_by_line(text: str) -> dict[int, ItemContext]:
    contexts: dict[int, ItemContext] = {}
    active_blocks: list[ActiveBlock] = []
    pending_item: ItemContext | None = None
    pending_impl_type: str | None = None
    brace_depth = 0

    for lineno, line in enumerate(text.splitlines(), start=1):
        code = strip_line_comment(line)
        while active_blocks and brace_depth < active_blocks[-1].end_depth:
            active_blocks.pop()

        current_impl = next(
            (
                block.impl_type
                for block in reversed(active_blocks)
                if block.impl_type is not None
            ),
            None,
        )
        context = next(
            (
                block.context
                for block in reversed(active_blocks)
                if block.context.kind != "impl"
            ),
            ItemContext(("*",), "module"),
        )
        opened_item: ItemContext | None = None
        opened_impl_type: str | None = None

        if pending_item is not None:
            context = pending_item
        elif pending_impl_type is not None:
            context = ItemContext((f"impl {pending_impl_type}", "*"), "impl")

        if pending_item is None and pending_impl_type is None:
            fn_match = FN_ITEM_RE.search(code)
            static_match = STATIC_OR_CONST_ITEM_RE.search(code)
            extern_match = EXTERN_BLOCK_RE.search(code)
            if fn_match is not None:
                context = item_context_for(
                    name=fn_match.group("name"),
                    kind="fn",
                    impl_type=current_impl,
                )
                if has_item_body_after(code, fn_match.start()):
                    opened_item = context
                elif not has_top_level_semicolon(code, fn_match.start()):
                    pending_item = context
            elif static_match is not None:
                context = item_context_for(
                    name=static_match.group("name"),
                    kind="static-const",
                    impl_type=current_impl,
                )
                initializer_tail = static_or_const_initializer_tail(
                    code, static_match.start()
                )
                if "{" in initializer_tail:
                    opened_item = context
                elif ";" not in initializer_tail:
                    pending_item = context
            elif extern_match is not None:
                abi = extern_match.group("abi") or "default"
                context = ItemContext((f"extern-{abi}-block", "extern-block", "*"), "extern")
                opened_item = context
            elif IMPL_START_RE.search(code) is not None and "{" in code:
                opened_impl_type = impl_type_symbol(code)
                context = ItemContext((f"impl {opened_impl_type}", "*"), "impl")
            elif IMPL_START_RE.search(code) is not None:
                pending_impl_type = impl_type_symbol(code)
                context = ItemContext((f"impl {pending_impl_type}", "*"), "impl")

        contexts[lineno] = context

        opens_item = opened_item is not None
        opens_impl = opened_impl_type is not None
        if (
            pending_item is not None
            and "{" in code
            and not has_top_level_semicolon(code, 0, code.find("{"))
        ):
            opened_item = pending_item
            opens_item = True
            pending_item = None
        if pending_item is not None and has_top_level_semicolon(code, 0):
            pending_item = None
        if pending_impl_type is not None and "{" in code:
            opened_impl_type = pending_impl_type
            opens_impl = True
            pending_impl_type = None

        previous_depth = brace_depth
        brace_depth += brace_delta(code)
        if opens_item and opened_item is not None and brace_depth > previous_depth:
            active_blocks.append(ActiveBlock(opened_item, previous_depth + 1))
        if opens_impl and opened_impl_type is not None and brace_depth > previous_depth:
            impl_context = ItemContext((f"impl {opened_impl_type}", "*"), "impl")
            active_blocks.append(
                ActiveBlock(impl_context, previous_depth + 1, opened_impl_type)
            )

    return contexts


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
    remove_entries = [
        entry for entry in entries if entry.disposition.startswith("remove:")
    ]
    if len(remove_entries) > REMOVE_ENTRY_CAP:
        raise AssertionError(
            "strict-type allowlist has "
            f"{len(remove_entries)} removable entries, above cap {REMOVE_ENTRY_CAP}"
        )
    verify_wildcard_counts_documented(entries)
    return entries


def wildcard_counts(entries: Iterable[AllowEntry]) -> dict[str, int]:
    counts: dict[str, int] = {}
    for entry in entries:
        if entry.symbol == "*":
            counts[entry.kind] = counts.get(entry.kind, 0) + 1
    return counts


def wildcard_summary(counts: dict[str, int]) -> str:
    total = sum(counts.values())
    category_counts = ", ".join(
        f"{kind}={count}" for kind, count in sorted(counts.items())
    )
    if category_counts:
        return f"Strict-type wildcard guard: total={total}; {category_counts}."
    return "Strict-type wildcard guard: total=0; none."


def verify_wildcard_counts_documented(entries: Iterable[AllowEntry]) -> None:
    expected = wildcard_summary(wildcard_counts(entries))
    try:
        strict_types_doc = STRICT_TYPES_DOC.read_text()
    except FileNotFoundError as err:
        raise AssertionError("missing docs/strict-types.md for strict-type wildcard guard") from err
    if expected not in strict_types_doc:
        raise AssertionError(
            "docs/strict-types.md must record the current strict-type wildcard counts with:\n"
            f"{expected}"
        )


def is_allowed(
    entries: list[AllowEntry],
    *,
    kind: str,
    rel: str,
    symbol: str | Iterable[str],
    domain: str,
) -> bool:
    rel_candidates = {rel, *PATH_ALIASES.get(rel, set())}
    symbols = {symbol} if isinstance(symbol, str) else set(symbol)
    for entry in entries:
        if entry.kind != kind:
            continue
        if not any(fnmatch.fnmatch(candidate, entry.path) for candidate in rel_candidates):
            continue
        if entry.symbol != "*" and entry.symbol not in symbols:
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
    item_contexts = item_contexts_by_line(text)
    for lineno, line in enumerate(text.splitlines(), start=1):
        code = strip_line_comment(line)
        if not pattern.search(code):
            continue
        context = item_contexts.get(lineno, ItemContext(("*",), "module"))
        if is_allowed(entries, kind=kind, rel=rel, symbol=context.symbols, domain=domain):
            continue
        offenders.append(
            line_offender(
                rel,
                lineno,
                line,
                f"{message} in {context.primary_symbol}",
            )
        )


def run_self_tests() -> None:
    text = """
fn free_function(ptr: *const usize) -> usize {
    ptr as usize
}

fn generic_function<T>(ptr: [*const T; 1]) -> usize {
    let p = ptr[0];
    p as usize
}

impl AtomicField {
    pub(crate) unsafe fn load(self) -> usize {
        self.ptr as *const AtomicUsize
    }

    const MASK: usize = {
        let ptr = 0 as *const usize;
        ptr as usize
    };
}

unsafe extern "C" {
    fn imported(ptr: *const usize);
}

static TABLE: [usize; 1] = [{
    let ptr = 0 as *const usize;
    ptr as usize
}];
"""
    contexts = item_contexts_by_line(text)
    assert contexts[3].symbols == ("free_function",)
    assert contexts[8].symbols == ("generic_function",)
    assert contexts[13].symbols == ("AtomicField::load", "load")
    assert contexts[18].symbols == ("AtomicField::MASK", "MASK")
    assert "imported" in contexts[23].symbols
    assert contexts[28].symbols == ("TABLE",)

    bare_contexts = item_contexts_by_line(
        """
struct LayoutRequest;

impl LayoutRequest {
    pub(crate) fn malloc(size: usize) -> Self {
        Self
    }
}
"""
    )
    assert bare_contexts[5].symbols == ("LayoutRequest::malloc", "malloc")
    assert is_allowed(
        [
            AllowEntry(
                "bare-domain-usize",
                "src/types/layout.rs",
                "LayoutRequest::malloc",
                "usize",
                "test",
                "permanent: self-test",
            )
        ],
        kind="bare-domain-usize",
        rel="src/types/layout.rs",
        symbol=bare_contexts[5].symbols,
        domain="usize",
    )

    entries = [
        AllowEntry(
            "pointer-usize-roundtrip",
            "src/example.rs",
            "free_function",
            "provenance",
            "test",
            "permanent: self-test",
        ),
        AllowEntry(
            "pointer-usize-roundtrip",
            "src/example.rs",
            "AtomicField::MASK",
            "provenance",
            "test",
            "permanent: self-test",
        ),
        AllowEntry(
            "pointer-usize-roundtrip",
            "src/example.rs",
            "generic_function",
            "provenance",
            "test",
            "permanent: self-test",
        ),
        AllowEntry(
            "pointer-usize-roundtrip",
            "src/example.rs",
            "TABLE",
            "provenance",
            "test",
            "permanent: self-test",
        ),
        AllowEntry(
            "atomic-cast",
            "src/example.rs",
            "AtomicField::load",
            "atomic-field",
            "test",
            "permanent: self-test",
        ),
    ]
    offenders: list[str] = []
    require_no_pattern(
        offenders,
        kind="pointer-usize-roundtrip",
        rel="src/example.rs",
        text=text,
        allowed_files=set(),
        pattern=PTR_USIZE_RE,
        entries=entries,
        domain="provenance",
        message="pointer/integer round trip outside typed provenance helpers",
    )
    require_no_pattern(
        offenders,
        kind="atomic-cast",
        rel="src/example.rs",
        text=text,
        allowed_files=set(),
        pattern=ATOMIC_CAST_RE,
        entries=entries,
        domain="atomic-field",
        message="pointer-to-atomic cast outside atomic field wrappers",
    )
    if offenders:
        raise AssertionError("strict-type self-test failed:\n" + "\n".join(offenders))


def main() -> int:
    run_self_tests()
    entries = read_allowlist()
    offenders: list[str] = []
    checked_raw_params = 0
    raw_bridge_text = (ROOT / "src/types/raw_bridge.rs").read_text(encoding="utf-8")
    declared_raw_bridges = set(re.findall(r"\b(assume_[A-Za-z0-9_]+)\b", raw_bridge_text))
    inventoried_raw_bridges = set(RAW_BRIDGE_CALLERS)
    if declared_raw_bridges != inventoried_raw_bridges:
        missing = sorted(declared_raw_bridges - inventoried_raw_bridges)
        stale = sorted(inventoried_raw_bridges - declared_raw_bridges)
        offenders.append(
            "raw bridge caller inventory mismatch: "
            f"missing={missing or 'none'}, stale={stale or 'none'}"
        )

    for path in source_files():
        rel = path.relative_to(ROOT).as_posix()
        text = production_source_text(path.read_text())

        if rel.startswith("src/types/") and CORE_TYPES_DEAD_CODE_RE.search(text):
            offenders.append(f"{rel}: crate-level dead_code allowance in core_types module")

        if rel.startswith("src/types/"):
            for match in TYPE_DECL_RE.finditer(text):
                name = match.group("name")
                if name in LINEAR_CAPABILITY_TYPES and "#[must_use" not in match.group("attrs"):
                    offenders.append(
                        f"{rel}: linear capability {name} lacks a must_use annotation"
                    )
            for match in DERIVED_TRAITS_RE.finditer(text):
                name = match.group("name")
                traits = {trait.strip() for trait in match.group("traits").split(",")}
                forbidden = traits.intersection({"Clone", "Copy"})
                if name in LINEAR_CAPABILITY_TYPES | MUST_NOT_BE_COPY_TYPES and forbidden:
                    classification = (
                        "linear capability"
                        if name in LINEAR_CAPABILITY_TYPES
                        else "nonduplicable result"
                    )
                    offenders.append(
                        f"{rel}: {classification} {name} derives "
                        + ", ".join(sorted(forbidden))
                    )
            for name in LINEAR_CAPABILITY_TYPES:
                if re.search(rf"\bimpl\s+(?:Clone|Copy)\s+for\s+{name}\b", text):
                    offenders.append(
                        f"{rel}: linear capability {name} has an explicit Clone/Copy impl"
                    )

            for name in MUST_NOT_BE_COPY_TYPES:
                if re.search(rf"\bimpl\s+(?:Clone|Copy)\s+for\s+{name}\b", text):
                    offenders.append(
                        f"{rel}: nonduplicable result {name} has an explicit Clone/Copy impl"
                    )

            constructor_fixture = rel.startswith("src/types/") and rel.endswith("/tests.rs")
            if rel not in CAPABILITY_CONSTRUCTOR_FILES and not constructor_fixture:
                for match in CAPABILITY_CONSTRUCTOR_RE.finditer(text):
                    line = text.count("\n", 0, match.start()) + 1
                    offenders.append(
                        f"{rel}:{line}: capability new_unchecked construction must use src/types/raw_bridge.rs"
                    )

            if rel != "src/types/atomic/thread_free.rs" and "DetachedRemoteList(" in text:
                for match in re.finditer(r"\bDetachedRemoteList\s*\(", text):
                    line = text.count("\n", 0, match.start()) + 1
                    offenders.append(
                        f"{rel}:{line}: detached remote lists must originate in the atomic ownership module"
                    )

        if rel not in GUARDED_CONSTRUCTOR_FILES and not rel.endswith("/tests.rs"):
            for match in GUARDED_CONSTRUCTOR_RE.finditer(text):
                line = text.count("\n", 0, match.start()) + 1
                offenders.append(
                    f"{rel}:{line}: guarded capability construction is limited to classification modules"
                )

        if rel != "src/types/raw_bridge.rs":
            for match in RAW_BRIDGE_CALL_RE.finditer(text):
                name = match.group("name")
                if rel in RAW_BRIDGE_CALLERS.get(name, set()):
                    continue
                line = text.count("\n", 0, match.start()) + 1
                offenders.append(
                    f"{rel}:{line}: raw bridge {name} is not allowed from this caller family"
                )

        if rel != "src/types/atomic/thread_free.rs" and not rel.endswith("/tests.rs"):
            for match in RAW_CAPABILITY_CONSTRUCTOR_RE.finditer(text):
                capability = match.group("type")
                if rel in RAW_CAPABILITY_CONSTRUCTOR_CALLERS[capability]:
                    continue
                line = text.count("\n", 0, match.start()) + 1
                offenders.append(
                    f"{rel}:{line}: raw {capability} construction is not allowed from this caller family"
                )

        if rel not in RAW_BOUNDARY_FILES:
            item_contexts = item_contexts_by_line(text)
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
                line = text.count("\n", 0, match.start()) + 1
                context = item_contexts.get(line, ItemContext((symbol,), "fn"))
                if is_allowed(
                    entries,
                    kind="bare-domain-usize",
                    rel=rel,
                    symbol=context.symbols,
                    domain="usize",
                ):
                    continue
                offenders.append(
                    f"{rel}:{line}: bare domain usize parameter in {context.primary_symbol} "
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
            allowed_files={"src/types/constants.rs"},
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
            allowed_files={"src/safe/mod.rs", "src/safe/tests.rs"},
            pattern=HEAP_TYPES_RE,
            entries=entries,
            domain="heap-type",
            message="heap-allocating Rust type in allocator internals",
        )
        if rel.startswith(("src/allocator/", "src/internal/", "src/platform/", "src/types/")):
            require_no_pattern(
                offenders,
                kind="dynamic-dispatch",
                rel=rel,
                text=text,
                allowed_files=set(),
                pattern=DYNAMIC_DISPATCH_RE,
                entries=entries,
                domain="dyn",
                message="dynamic dispatch in allocator internals",
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
