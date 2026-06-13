#!/usr/bin/env python3
"""Link and run a C smoke program against every public mimalloc API symbol."""

from __future__ import annotations

import os
import pathlib
import re
import shutil
import subprocess
import sys


ROOT = pathlib.Path(__file__).resolve().parents[1]
PUBLIC_HEADERS = [
    ROOT / "mimalloc-3.3.2/include/mimalloc.h",
    ROOT / "mimalloc-3.3.2/include/mimalloc-stats.h",
]
BUILD_DIR = ROOT / "target/public-api-link"


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


def rust_staticlib() -> pathlib.Path:
    subprocess.run(
        ["cargo", "rustc", "--lib", "--", "--crate-type", "staticlib"],
        cwd=ROOT,
        check=True,
    )
    libs = sorted(
        (ROOT / "target/debug/deps").glob("libmechanical_mimalloc-*.a"),
        key=lambda path: path.stat().st_mtime,
        reverse=True,
    )
    if not libs:
        raise SystemExit("no Rust staticlib artifact found")
    return libs[0]


def c_source(symbols: list[str]) -> str:
    refs = "\n".join(f"  (void (*)(void)){symbol}," for symbol in symbols)
    return f"""\
#define _GNU_SOURCE
#include <mimalloc.h>
#include <mimalloc-stats.h>

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>
#include <wchar.h>

static void out(const char* msg, void* arg) {{
  (void)msg;
  (void)arg;
}}

static void err(int error, void* arg) {{
  (void)error;
  (void)arg;
}}

static bool block_visitor(const mi_heap_t* heap, const mi_heap_area_t* area, void* block, size_t block_size, void* arg) {{
  (void)heap;
  (void)area;
  (void)block;
  (void)block_size;
  (void)arg;
  return true;
}}

static bool heap_visitor(mi_heap_t* heap, void* arg) {{
  (void)heap;
  (void)arg;
  return true;
}}

static bool commit_fun(bool commit, void* start, size_t size, bool* is_zero, void* arg) {{
  (void)commit;
  (void)start;
  (void)size;
  (void)arg;
  if (is_zero != NULL) {{
    *is_zero = false;
  }}
  return true;
}}

static void (*volatile public_symbol_refs[])(void) = {{
{refs}
}};

static int expect_ptr(void* p) {{
  return (p == NULL ? 1 : 0);
}}

int main(void) {{
  (void)public_symbol_refs[0];

  mi_process_init();
  mi_thread_init();
  mi_register_output(out, NULL);
  mi_register_error(err, NULL);

  if (mi_version() <= 0) return 10;
  if (mi_good_size(17) < 17) return 11;
  if (mi_malloc_good_size(17) < 17) return 12;

  void* p = mi_malloc(16);
  if (expect_ptr(p)) return 20;
  memset(p, 0xA5, 16);
  if (!mi_is_in_heap_region(p)) return 21;
  if (mi_usable_size(p) < 16) return 22;
  if (mi_malloc_size(p) < 16) return 23;
  if (mi_malloc_usable_size(p) < 16) return 24;
  p = mi_realloc(p, 32);
  if (expect_ptr(p)) return 25;
  mi_free(p);

  void* z = mi_zalloc(24);
  if (expect_ptr(z)) return 30;
  mi_free_size(z, 24);

  void* c = mi_calloc(4, 8);
  if (expect_ptr(c)) return 31;
  mi_free(c);

  void* a = mi_malloc_aligned(64, 32);
  if (expect_ptr(a) || (((uintptr_t)a % 32) != 0)) return 32;
  mi_free_aligned(a, 32);

  void* pa = NULL;
  if (mi_posix_memalign(&pa, 32, 64) != 0 || pa == NULL) return 33;
  mi_free(pa);

  char* dup = mi_strdup("abc");
  if (expect_ptr(dup) || strcmp(dup, "abc") != 0) return 40;
  mi_free(dup);

  char* ndup = mi_strndup("abcdef", 3);
  if (expect_ptr(ndup) || strcmp(ndup, "abc") != 0) return 41;
  mi_free(ndup);

  wchar_t* wdup = mi_wcsdup(L"abc");
  if (expect_ptr(wdup) || wcscmp(wdup, L"abc") != 0) return 42;
  mi_free(wdup);

  unsigned char* mdup = mi_mbsdup((const unsigned char*)"abc");
  if (expect_ptr(mdup) || strcmp((const char*)mdup, "abc") != 0) return 43;
  mi_free(mdup);

  char* rp = mi_realpath(".", NULL);
  if (rp != NULL) mi_free(rp);

  mi_heap_t* heap = mi_heap_new();
  if (heap == NULL) return 50;
  void* hp = mi_heap_malloc(heap, 48);
  if (expect_ptr(hp) || !mi_heap_contains(heap, hp)) return 51;
  if (!mi_any_heap_contains(hp)) return 52;
  if (mi_heap_of(hp) == NULL) return 53;
  hp = mi_heap_realloc(heap, hp, 96);
  if (expect_ptr(hp)) return 54;
  mi_free(hp);
  if (!mi_heap_visit_blocks(heap, false, block_visitor, NULL)) return 55;
  mi_heap_collect(heap, true);

  mi_theap_t* theap = mi_heap_theap(heap);
  if (theap == NULL) return 60;
  void* tp = mi_theap_malloc(theap, 32);
  if (expect_ptr(tp)) return 61;
  mi_free(tp);
  mi_theap_collect(theap, false);
  mi_theap_guarded_set_sample_rate(theap, 0, 0);
  mi_theap_guarded_set_size_bound(theap, 0, 0);

  mi_stats_t stats;
  mi_stats_init(&stats);
  (void)mi_heap_stats_get(heap, &stats);
  (void)mi_stats_get(&stats);
  (void)mi_stats_get_bin_size(0);
  char json_buf[4096];
  char* json = mi_stats_get_json(sizeof(json_buf), json_buf);
  if (json != NULL && json != json_buf) mi_free(json);
  json = mi_heap_stats_get_json(heap, sizeof(json_buf), json_buf);
  if (json != NULL && json != json_buf) mi_free(json);
  json = mi_stats_as_json(&stats, sizeof(json_buf), json_buf);
  if (json != NULL && json != json_buf) mi_free(json);

  mi_subproc_id_t subproc = mi_subproc_current();
  (void)mi_subproc_main();
  (void)mi_subproc_stats_get(subproc, &stats);
  (void)mi_subproc_stats_get_exclusive(subproc, &stats);
  if (!mi_subproc_visit_heaps(subproc, heap_visitor, NULL)) return 70;

  size_t arena_size = 0;
  (void)mi_arena_min_alignment();
  (void)mi_arena_min_size();
  (void)mi_arena_area(NULL, &arena_size);
  (void)mi_arena_contains(NULL, NULL);

  bool zero = false;
  (void)commit_fun(false, NULL, 0, &zero, NULL);

  mi_option_set_enabled(mi_option_verbose, false);
  volatile bool option_enabled = mi_option_is_enabled(mi_option_verbose);
  volatile long option_value = mi_option_get(mi_option_verbose);
  volatile long option_clamped = mi_option_get_clamp(mi_option_verbose, 0, 1);
  volatile size_t option_size = mi_option_get_size(mi_option_verbose);
  (void)option_enabled;
  (void)option_value;
  (void)option_clamped;
  (void)option_size;

  mi_options_print_out(out, NULL);
  mi_process_info_print_out(out, NULL);
  mi_stats_print_out(out, NULL);
  mi_heap_stats_print_out(heap, out, NULL);
  mi_subproc_stats_print_out(subproc, out, NULL);
  mi_subproc_heap_stats_print_out(subproc, out, NULL);

  mi_heap_delete(heap);
  mi_collect(false);
  mi_thread_done();
  return 0;
}}
"""


def main() -> int:
    cc = os.environ.get("CC") or shutil.which("cc") or shutil.which("gcc")
    if cc is None:
        raise SystemExit("no C compiler found; set CC")

    symbols = public_header_symbols()
    staticlib = rust_staticlib()
    BUILD_DIR.mkdir(parents=True, exist_ok=True)
    source = BUILD_DIR / "public-api-link-smoke.c"
    binary = BUILD_DIR / "public-api-link-smoke"
    source.write_text(c_source(symbols))

    subprocess.run(
        [
            cc,
            "-std=c11",
            "-I",
            str(ROOT / "mimalloc-3.3.2/include"),
            str(source),
            str(staticlib),
            "-pthread",
            "-ldl",
            "-lm",
            "-o",
            str(binary),
        ],
        cwd=ROOT,
        check=True,
    )
    subprocess.run([str(binary)], cwd=ROOT, check=True)
    print(f"ok: C smoke linked {len(symbols)} public API symbols and exited successfully")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
