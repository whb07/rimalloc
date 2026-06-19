#!/usr/bin/env python3
"""Check Rust public ABI declarations, layouts, and constants against C headers."""

from __future__ import annotations

import os
import pathlib
import re
import shutil
import subprocess
import sys


ROOT = pathlib.Path(__file__).resolve().parents[1]
BUILD_DIR = ROOT / "target/public-abi"
PUBLIC_HEADERS = [
    ROOT / "mimalloc-3.3.2/include/mimalloc.h",
    ROOT / "mimalloc-3.3.2/include/mimalloc-stats.h",
]

OPTION_NAMES = [
    "mi_option_show_errors",
    "mi_option_show_stats",
    "mi_option_verbose",
    "mi_option_deprecated_eager_commit",
    "mi_option_arena_eager_commit",
    "mi_option_purge_decommits",
    "mi_option_allow_large_os_pages",
    "mi_option_reserve_huge_os_pages",
    "mi_option_reserve_huge_os_pages_at",
    "mi_option_reserve_os_memory",
    "mi_option_deprecated_segment_cache",
    "mi_option_deprecated_page_reset",
    "mi_option_deprecated_abandoned_page_purge",
    "mi_option_deprecated_segment_reset",
    "mi_option_deprecated_eager_commit_delay",
    "mi_option_purge_delay",
    "mi_option_use_numa_nodes",
    "mi_option_disallow_os_alloc",
    "mi_option_os_tag",
    "mi_option_max_errors",
    "mi_option_max_warnings",
    "mi_option_deprecated_max_segment_reclaim",
    "mi_option_destroy_on_exit",
    "mi_option_arena_reserve",
    "mi_option_arena_purge_mult",
    "mi_option_deprecated_purge_extend_delay",
    "mi_option_disallow_arena_alloc",
    "mi_option_retry_on_oom",
    "mi_option_visit_abandoned",
    "mi_option_guarded_min",
    "mi_option_guarded_max",
    "mi_option_guarded_precise",
    "mi_option_guarded_sample_rate",
    "mi_option_guarded_sample_seed",
    "mi_option_generic_collect",
    "mi_option_page_reclaim_on_free",
    "mi_option_page_full_retain",
    "mi_option_page_max_candidates",
    "mi_option_max_vabits",
    "mi_option_pagemap_commit",
    "mi_option_page_commit_on_demand",
    "mi_option_page_max_reclaim",
    "mi_option_page_cross_thread_max_reclaim",
    "mi_option_allow_thp",
    "mi_option_minimal_purge_size",
    "mi_option_arena_max_object_size",
    "mi_option_arena_is_numa_local",
    "_mi_option_last",
    "mi_option_large_os_pages",
    "mi_option_eager_region_commit",
    "mi_option_reset_decommits",
    "mi_option_reset_delay",
    "mi_option_limit_os_alloc",
]

STAT_FIELDS = [
    "size",
    "version",
    "pages",
    "reserved",
    "committed",
    "reset",
    "purged",
    "page_committed",
    "pages_abandoned",
    "threads",
    "malloc_normal",
    "malloc_huge",
    "malloc_requested",
    "mmap_calls",
    "commit_calls",
    "reset_calls",
    "purge_calls",
    "arena_count",
    "malloc_normal_count",
    "malloc_huge_count",
    "malloc_guarded_count",
    "arena_rollback_count",
    "arena_purges",
    "pages_extended",
    "pages_retire",
    "page_searches",
    "page_searches_count",
    "segments",
    "segments_abandoned",
    "segments_cache",
    "_segments_reserved",
    "heaps",
    "theaps",
    "pages_reclaim_on_alloc",
    "pages_reclaim_on_free",
    "pages_reabandon_full",
    "pages_unabandon_busy_wait",
    "heaps_delete_wait",
    "_stat_reserved",
    "_stat_counter_reserved",
    "malloc_bins",
    "page_bins",
    "chunk_bins",
]


def public_header_symbols() -> list[str]:
    decl = re.compile(r"mi_decl(?:_nodiscard)?[^;#]*?\b(mi_[A-Za-z0-9_]+)\s*\(")
    symbols: list[str] = []
    for header in PUBLIC_HEADERS:
        text = header.read_text()
        text = re.sub(r"/\*.*?\*/", "", text, flags=re.S)
        text = re.sub(r"//.*", "", text)
        for match in decl.finditer(text):
            symbol = match.group(1)
            if symbol not in symbols:
                symbols.append(symbol)
    return symbols


def facade_symbols() -> list[str]:
    text = (ROOT / "src/abi.rs").read_text()
    symbols: list[str] = []
    for match in re.finditer(r"pub fn\s+(mi_[A-Za-z0-9_]+)\s*\(", text):
        symbol = match.group(1)
        if symbol not in symbols:
            symbols.append(symbol)
    return symbols


def check_facade() -> None:
    expected = public_header_symbols()
    actual = facade_symbols()
    missing = [symbol for symbol in expected if symbol not in actual]
    extra = [symbol for symbol in actual if symbol not in expected]
    if missing or extra:
        if missing:
            print("facade missing public C declarations:", file=sys.stderr)
            for symbol in missing:
                print(symbol, file=sys.stderr)
        if extra:
            print("facade has non-header public declarations:", file=sys.stderr)
            for symbol in extra:
                print(symbol, file=sys.stderr)
        raise SystemExit(1)


def c_source() -> str:
    stat_offsets = "\n".join(
        f'  printf("offset.mi_stats_t.{field}=%zu\\n", offsetof(mi_stats_t, {field}));'
        for field in STAT_FIELDS
    )
    option_values = "\n".join(
        f'  printf("const.{name}=%d\\n", (int){name});' for name in OPTION_NAMES
    )
    return f"""\
#include <mimalloc.h>
#include <mimalloc-stats.h>

#include <stddef.h>
#include <stdio.h>
#include <string.h>

#define PRINT_SIZE(name, ty) printf("size." name "=%zu\\n", sizeof(ty))
#define PRINT_ALIGN(name, ty) printf("align." name "=%zu\\n", _Alignof(ty))
#define PRINT_OFFSET(ty, field) printf("offset." #ty "." #field "=%zu\\n", offsetof(ty, field))

int main(void) {{
  PRINT_SIZE("mi_heap_area_t", mi_heap_area_t);
  PRINT_ALIGN("mi_heap_area_t", mi_heap_area_t);
  PRINT_OFFSET(mi_heap_area_t, blocks);
  PRINT_OFFSET(mi_heap_area_t, reserved);
  PRINT_OFFSET(mi_heap_area_t, committed);
  PRINT_OFFSET(mi_heap_area_t, used);
  PRINT_OFFSET(mi_heap_area_t, block_size);
  PRINT_OFFSET(mi_heap_area_t, full_block_size);
  PRINT_OFFSET(mi_heap_area_t, reserved1);

  PRINT_SIZE("mi_stat_count_t", mi_stat_count_t);
  PRINT_ALIGN("mi_stat_count_t", mi_stat_count_t);
  PRINT_OFFSET(mi_stat_count_t, total);
  PRINT_OFFSET(mi_stat_count_t, peak);
  PRINT_OFFSET(mi_stat_count_t, current);

  PRINT_SIZE("mi_stat_counter_t", mi_stat_counter_t);
  PRINT_ALIGN("mi_stat_counter_t", mi_stat_counter_t);
  PRINT_OFFSET(mi_stat_counter_t, total);

  PRINT_SIZE("mi_stats_t", mi_stats_t);
  PRINT_ALIGN("mi_stats_t", mi_stats_t);
{stat_offsets}

  PRINT_SIZE("mi_chunkbin_t", mi_chunkbin_t);
  PRINT_ALIGN("mi_chunkbin_t", mi_chunkbin_t);
  PRINT_SIZE("mi_subproc_id_t", mi_subproc_id_t);
  PRINT_ALIGN("mi_subproc_id_t", mi_subproc_id_t);
  PRINT_OFFSET(mi_subproc_id_t, _mi_subproc_id);
  PRINT_SIZE("mi_arena_id_t", mi_arena_id_t);
  PRINT_ALIGN("mi_arena_id_t", mi_arena_id_t);
  PRINT_SIZE("mi_heap_t_ptr", mi_heap_t*);
  PRINT_ALIGN("mi_heap_t_ptr", mi_heap_t*);
  PRINT_SIZE("mi_theap_t_ptr", mi_theap_t*);
  PRINT_ALIGN("mi_theap_t_ptr", mi_theap_t*);

  printf("const.MI_MALLOC_VERSION=%d\\n", MI_MALLOC_VERSION);
  printf("const.MI_SMALL_WSIZE_MAX=%d\\n", MI_SMALL_WSIZE_MAX);
  printf("const.MI_SMALL_SIZE_MAX=%zu\\n", (size_t)MI_SMALL_SIZE_MAX);
  printf("const.MI_STAT_VERSION=%d\\n", MI_STAT_VERSION);
  printf("const.MI_BIN_HUGE=%u\\n", MI_BIN_HUGE);
  printf("const.MI_CBIN_SMALL=%d\\n", (int)MI_CBIN_SMALL);
  printf("const.MI_CBIN_OTHER=%d\\n", (int)MI_CBIN_OTHER);
  printf("const.MI_CBIN_MEDIUM=%d\\n", (int)MI_CBIN_MEDIUM);
  printf("const.MI_CBIN_LARGE=%d\\n", (int)MI_CBIN_LARGE);
  printf("const.MI_CBIN_HUGE=%d\\n", (int)MI_CBIN_HUGE);
  printf("const.MI_CBIN_NONE=%d\\n", (int)MI_CBIN_NONE);
  printf("const.MI_CBIN_COUNT=%d\\n", (int)MI_CBIN_COUNT);
{option_values}

  mi_stats_t stats_header;
  memset(&stats_header, 0, sizeof(stats_header));
  mi_stats_header_init(&stats_header);
  printf("stats_header.size=%zu\\n", stats_header.size);
  printf("stats_header.version=%zu\\n", stats_header.version);

  mi_stats_t stats_init;
  memset(&stats_init, 0, sizeof(stats_init));
  stats_init.pages.total = 123;
  stats_init.committed.current = 456;
  stats_init.malloc_bins[0].peak = 789;
  stats_init.page_bins[MI_BIN_HUGE].current = 321;
  stats_init.chunk_bins[MI_CBIN_HUGE].total = 654;
  mi_stats_init(&stats_init);
  printf("stats_init.size=%zu\\n", stats_init.size);
  printf("stats_init.version=%zu\\n", stats_init.version);
  printf("stats_init.pages.total=%lld\\n", (long long)stats_init.pages.total);
  printf("stats_init.committed.current=%lld\\n", (long long)stats_init.committed.current);
  printf("stats_init.malloc_bins0.peak=%lld\\n", (long long)stats_init.malloc_bins[0].peak);
  printf("stats_init.page_bins_huge.current=%lld\\n", (long long)stats_init.page_bins[MI_BIN_HUGE].current);
  printf("stats_init.chunk_bins_huge.total=%lld\\n", (long long)stats_init.chunk_bins[MI_CBIN_HUGE].total);
  return 0;
}}
"""


def rust_source() -> str:
    stat_offsets = "\n".join(
        f'    println!("offset.mi_stats_t.{field}={{}}", offset_of!(mi_stats_t, {field}));'
        for field in STAT_FIELDS
    )
    option_values = "\n".join(
        f'    println!("const.{name}={{}}", mi_option_t::{name} as i32);'
        for name in OPTION_NAMES[:48]
    )
    legacy_values = "\n".join(
        f'    println!("const.{name}={{}}", {name} as i32);' for name in OPTION_NAMES[48:]
    )
    return f"""\
use std::mem::{{align_of, offset_of, size_of}};

use mechanical_mimalloc::raw::mimalloc::{{
    MI_MALLOC_VERSION, MI_SMALL_SIZE_MAX, MI_SMALL_WSIZE_MAX, mi_arena_id_t, mi_heap_area_t,
    mi_heap_t, mi_subproc_id_t, mi_theap_t,
    MI_BIN_HUGE, MI_CBIN_COUNT, MI_CBIN_HUGE, MI_CBIN_LARGE, MI_CBIN_MEDIUM, MI_CBIN_NONE,
    MI_CBIN_OTHER, MI_CBIN_SMALL, MI_STAT_VERSION, mi_chunkbin_t, mi_stat_count_t,
    mi_stat_counter_t, mi_stats_header_init, mi_stats_init, mi_stats_t,
    {", ".join(OPTION_NAMES[48:])}, mi_option_t,
}};

fn main() {{
    println!("size.mi_heap_area_t={{}}", size_of::<mi_heap_area_t>());
    println!("align.mi_heap_area_t={{}}", align_of::<mi_heap_area_t>());
    println!("offset.mi_heap_area_t.blocks={{}}", offset_of!(mi_heap_area_t, blocks));
    println!("offset.mi_heap_area_t.reserved={{}}", offset_of!(mi_heap_area_t, reserved));
    println!("offset.mi_heap_area_t.committed={{}}", offset_of!(mi_heap_area_t, committed));
    println!("offset.mi_heap_area_t.used={{}}", offset_of!(mi_heap_area_t, used));
    println!("offset.mi_heap_area_t.block_size={{}}", offset_of!(mi_heap_area_t, block_size));
    println!("offset.mi_heap_area_t.full_block_size={{}}", offset_of!(mi_heap_area_t, full_block_size));
    println!("offset.mi_heap_area_t.reserved1={{}}", offset_of!(mi_heap_area_t, reserved1));

    println!("size.mi_stat_count_t={{}}", size_of::<mi_stat_count_t>());
    println!("align.mi_stat_count_t={{}}", align_of::<mi_stat_count_t>());
    println!("offset.mi_stat_count_t.total={{}}", offset_of!(mi_stat_count_t, total));
    println!("offset.mi_stat_count_t.peak={{}}", offset_of!(mi_stat_count_t, peak));
    println!("offset.mi_stat_count_t.current={{}}", offset_of!(mi_stat_count_t, current));

    println!("size.mi_stat_counter_t={{}}", size_of::<mi_stat_counter_t>());
    println!("align.mi_stat_counter_t={{}}", align_of::<mi_stat_counter_t>());
    println!("offset.mi_stat_counter_t.total={{}}", offset_of!(mi_stat_counter_t, total));

    println!("size.mi_stats_t={{}}", size_of::<mi_stats_t>());
    println!("align.mi_stats_t={{}}", align_of::<mi_stats_t>());
{stat_offsets}

    println!("size.mi_chunkbin_t={{}}", size_of::<mi_chunkbin_t>());
    println!("align.mi_chunkbin_t={{}}", align_of::<mi_chunkbin_t>());
    println!("size.mi_subproc_id_t={{}}", size_of::<mi_subproc_id_t>());
    println!("align.mi_subproc_id_t={{}}", align_of::<mi_subproc_id_t>());
    println!("offset.mi_subproc_id_t._mi_subproc_id={{}}", offset_of!(mi_subproc_id_t, _mi_subproc_id));
    println!("size.mi_arena_id_t={{}}", size_of::<mi_arena_id_t>());
    println!("align.mi_arena_id_t={{}}", align_of::<mi_arena_id_t>());
    println!("size.mi_heap_t_ptr={{}}", size_of::<*mut mi_heap_t>());
    println!("align.mi_heap_t_ptr={{}}", align_of::<*mut mi_heap_t>());
    println!("size.mi_theap_t_ptr={{}}", size_of::<*mut mi_theap_t>());
    println!("align.mi_theap_t_ptr={{}}", align_of::<*mut mi_theap_t>());

    println!("const.MI_MALLOC_VERSION={{}}", MI_MALLOC_VERSION);
    println!("const.MI_SMALL_WSIZE_MAX={{}}", MI_SMALL_WSIZE_MAX);
    println!("const.MI_SMALL_SIZE_MAX={{}}", MI_SMALL_SIZE_MAX);
    println!("const.MI_STAT_VERSION={{}}", MI_STAT_VERSION);
    println!("const.MI_BIN_HUGE={{}}", MI_BIN_HUGE);
    println!("const.MI_CBIN_SMALL={{}}", MI_CBIN_SMALL as i32);
    println!("const.MI_CBIN_OTHER={{}}", MI_CBIN_OTHER as i32);
    println!("const.MI_CBIN_MEDIUM={{}}", MI_CBIN_MEDIUM as i32);
    println!("const.MI_CBIN_LARGE={{}}", MI_CBIN_LARGE as i32);
    println!("const.MI_CBIN_HUGE={{}}", MI_CBIN_HUGE as i32);
    println!("const.MI_CBIN_NONE={{}}", MI_CBIN_NONE as i32);
    println!("const.MI_CBIN_COUNT={{}}", MI_CBIN_COUNT as i32);
{option_values}
{legacy_values}

    let mut stats_header = mi_stats_t::default();
    unsafe {{ mi_stats_header_init(&mut stats_header) }};
    println!("stats_header.size={{}}", stats_header.size);
    println!("stats_header.version={{}}", stats_header.version);

    let mut stats_init = mi_stats_t::default();
    stats_init.pages.total = 123;
    stats_init.committed.current = 456;
    stats_init.malloc_bins[0].peak = 789;
    stats_init.page_bins[MI_BIN_HUGE].current = 321;
    stats_init.chunk_bins[MI_CBIN_HUGE as usize].total = 654;
    unsafe {{ mi_stats_init(&mut stats_init) }};
    println!("stats_init.size={{}}", stats_init.size);
    println!("stats_init.version={{}}", stats_init.version);
    println!("stats_init.pages.total={{}}", stats_init.pages.total);
    println!("stats_init.committed.current={{}}", stats_init.committed.current);
    println!("stats_init.malloc_bins0.peak={{}}", stats_init.malloc_bins[0].peak);
    println!("stats_init.page_bins_huge.current={{}}", stats_init.page_bins[MI_BIN_HUGE].current);
    println!("stats_init.chunk_bins_huge.total={{}}", stats_init.chunk_bins[MI_CBIN_HUGE as usize].total);
}}
"""


def run(command: list[str]) -> str:
    completed = subprocess.run(
        command,
        cwd=ROOT,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
    )
    return completed.stdout


def rust_rlib() -> pathlib.Path:
    subprocess.run(["cargo", "build", "--lib"], cwd=ROOT, check=True)
    libs = sorted(
        [
            *(ROOT / "target/debug/deps").glob("libmechanical_mimalloc-*.rlib"),
            *(ROOT / "target/debug/deps").glob("libmechanical_mimalloc.rlib"),
            *(ROOT / "target/debug").glob("libmechanical_mimalloc.rlib"),
        ],
        key=lambda path: path.stat().st_mtime,
        reverse=True,
    )
    if not libs:
        raise SystemExit("no Rust rlib artifact found")
    return libs[0]


def main() -> int:
    check_facade()

    cc = os.environ.get("CC") or shutil.which("cc") or shutil.which("gcc")
    if cc is None:
        raise SystemExit("no C compiler found; set CC")

    BUILD_DIR.mkdir(parents=True, exist_ok=True)
    c_file = BUILD_DIR / "public-abi.c"
    c_bin = BUILD_DIR / "public-abi-c"
    rust_file = BUILD_DIR / "public-abi.rs"
    rust_bin = BUILD_DIR / "public-abi-rust"
    c_file.write_text(c_source())
    rust_file.write_text(rust_source())

    subprocess.run(
        [
            cc,
            "-std=c11",
            "-I",
            str(ROOT / "mimalloc-3.3.2/include"),
            str(c_file),
            "-o",
            str(c_bin),
        ],
        cwd=ROOT,
        check=True,
    )

    rlib = rust_rlib()
    subprocess.run(
        [
            "rustc",
            "--edition=2024",
            "-C",
            "panic=abort",
            str(rust_file),
            "--extern",
            f"mechanical_mimalloc={rlib}",
            "-L",
            str(ROOT / "target/debug/deps"),
            "-o",
            str(rust_bin),
        ],
        cwd=ROOT,
        check=True,
    )

    c_output = run([str(c_bin)])
    rust_output = run([str(rust_bin)])
    if c_output != rust_output:
        print("public ABI layout/constant mismatch", file=sys.stderr)
        print("--- C headers ---", file=sys.stderr)
        print(c_output, file=sys.stderr)
        print("--- Rust ---", file=sys.stderr)
        print(rust_output, file=sys.stderr)
        return 1

    print(
        "ok: facade declarations match public C headers; "
        "public layouts and constants match C"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
