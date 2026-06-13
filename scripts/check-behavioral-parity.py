#!/usr/bin/env python3
"""Run the same C behavior smoke scenarios against C mimalloc and Rust mimalloc."""

from __future__ import annotations

import os
import pathlib
import shutil
import subprocess
import sys


ROOT = pathlib.Path(__file__).resolve().parents[1]
BUILD_DIR = ROOT / "target/behavioral-parity"


SCENARIO_C = r"""
#define _GNU_SOURCE
#include <mimalloc.h>
#include <mimalloc-stats.h>

#include <errno.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

static int failures = 0;
static size_t output_callbacks = 0;
static volatile size_t overflow_count = (size_t)-1;

static void output_callback(const char* msg, void* arg) {
  (void)arg;
  if (msg != NULL && msg[0] != '\0') {
    output_callbacks++;
  }
}

static void result_bool(const char* name, int ok) {
  printf("%s=%d\n", name, ok ? 1 : 0);
  if (!ok) failures++;
}

static void result_size(const char* name, size_t value) {
  printf("%s=%zu\n", name, value);
}

static void result_indexed_size(const char* name, size_t index, size_t value) {
  printf("%s[%zu]=%zu\n", name, index, value);
}

static int bytes_equal(const unsigned char* p, unsigned char value, size_t size) {
  for (size_t i = 0; i < size; i++) {
    if (p[i] != value) return 0;
  }
  return 1;
}

static int is_aligned_at(const void* p, size_t alignment, size_t offset) {
  return p != NULL && ((((uintptr_t)p + offset) % alignment) == 0);
}

static void fill_sequence(unsigned char* p, size_t size) {
  for (size_t i = 0; i < size; i++) {
    p[i] = (unsigned char)(i + 1);
  }
}

static int has_sequence_prefix(const unsigned char* p, size_t size) {
  for (size_t i = 0; i < size; i++) {
    if (p[i] != (unsigned char)(i + 1)) return 0;
  }
  return 1;
}

int main(void) {
  mi_option_disable(mi_option_show_errors);

  result_bool("version_30302", mi_version() == 30302);

  errno = 0;
  void* p = mi_malloc(17);
  result_bool("malloc_nonnull", p != NULL);
  result_bool("malloc_owned", p != NULL && mi_is_in_heap_region(p));
  result_bool("malloc_usable_ge_17", p != NULL && mi_usable_size(p) >= 17);
  result_bool("malloc_errno_unchanged", errno == 0);
  mi_free(p);

  unsigned char* c = (unsigned char*)mi_calloc(8, 4);
  result_bool("calloc_nonnull", c != NULL);
  result_bool("calloc_zeroed", c != NULL && bytes_equal(c, 0, 32));
  mi_free(c);

  unsigned char* r = (unsigned char*)mi_malloc(8);
  if (r != NULL) memset(r, 0x5A, 8);
  r = (unsigned char*)mi_realloc(r, 32);
  result_bool("realloc_nonnull", r != NULL);
  result_bool("realloc_preserved_prefix", r != NULL && bytes_equal(r, 0x5A, 8));
  mi_free(r);

  void* aligned = mi_malloc_aligned(64, 32);
  result_bool("aligned_nonnull", aligned != NULL);
  result_bool("aligned_mod_32", is_aligned_at(aligned, 32, 0));
  mi_free(aligned);

  aligned = mi_malloc_aligned_at(65, 32, 8);
  result_bool("aligned_at_offset", is_aligned_at(aligned, 32, 8));
  mi_free(aligned);

  unsigned char* zaligned = (unsigned char*)mi_zalloc_aligned(66, 32);
  result_bool("zalloc_aligned_zeroed", is_aligned_at(zaligned, 32, 0) && bytes_equal(zaligned, 0, 66));
  mi_free(zaligned);

  zaligned = (unsigned char*)mi_zalloc_aligned_at(67, 32, 16);
  result_bool("zalloc_aligned_at_zeroed", is_aligned_at(zaligned, 32, 16) && bytes_equal(zaligned, 0, 67));
  mi_free(zaligned);

  unsigned char* caligned = (unsigned char*)mi_calloc_aligned(9, 7, 32);
  result_bool("calloc_aligned_zeroed", is_aligned_at(caligned, 32, 0) && bytes_equal(caligned, 0, 63));
  mi_free(caligned);

  caligned = (unsigned char*)mi_calloc_aligned_at(9, 7, 32, 24);
  result_bool("calloc_aligned_at_zeroed", is_aligned_at(caligned, 32, 24) && bytes_equal(caligned, 0, 63));
  mi_free(caligned);

  result_bool("calloc_aligned_overflow_null", mi_calloc_aligned(overflow_count, 2, 32) == NULL);

  void* posix = NULL;
  errno = 0;
  int perr = mi_posix_memalign(&posix, 32, 64);
  result_bool("posix_memalign_success", perr == 0 && posix != NULL);
  result_bool("posix_memalign_mod_32", posix != NULL && ((uintptr_t)posix % 32) == 0);
  result_bool("posix_memalign_errno_unchanged", errno == 0);
  mi_free(posix);

  char* dup = mi_strdup("abc");
  result_bool("strdup_content", dup != NULL && strcmp(dup, "abc") == 0);
  mi_free(dup);

  char* ndup = mi_strndup("abcdef", 3);
  result_bool("strndup_content", ndup != NULL && strcmp(ndup, "abc") == 0);
  mi_free(ndup);

  result_bool("mbsdup_null", mi_mbsdup(NULL) == NULL);
  unsigned char* mdup = mi_mbsdup((const unsigned char*)"");
  result_bool("mbsdup_empty", mdup != NULL && mdup[0] == 0);
  mi_free(mdup);
  mdup = mi_mbsdup((const unsigned char*)"multi-byte-\xc3\xa9");
  result_bool("mbsdup_content", mdup != NULL && strcmp((const char*)mdup, "multi-byte-\xc3\xa9") == 0);
  mi_free(mdup);

  mi_heap_t* heap = mi_heap_new();
  result_bool("heap_nonnull", heap != NULL);
  void* hp = heap == NULL ? NULL : mi_heap_malloc(heap, 24);
  result_bool("heap_malloc_nonnull", hp != NULL);
  result_bool("heap_contains", heap != NULL && hp != NULL && mi_heap_contains(heap, hp));
  result_bool("any_heap_contains", hp != NULL && mi_any_heap_contains(hp));
  mi_free(hp);

  void* ha = heap == NULL ? NULL : mi_heap_malloc_aligned(heap, 96, 32);
  result_bool("heap_malloc_aligned", is_aligned_at(ha, 32, 0) && mi_heap_contains(heap, ha));
  mi_free(ha);

  ha = heap == NULL ? NULL : mi_heap_malloc_aligned_at(heap, 96, 32, 8);
  result_bool("heap_malloc_aligned_at", is_aligned_at(ha, 32, 8) && mi_heap_contains(heap, ha));
  mi_free(ha);

  unsigned char* hza = heap == NULL ? NULL : (unsigned char*)mi_heap_zalloc_aligned(heap, 96, 32);
  result_bool("heap_zalloc_aligned", is_aligned_at(hza, 32, 0) && bytes_equal(hza, 0, 96) && mi_heap_contains(heap, hza));
  mi_free(hza);

  hza = heap == NULL ? NULL : (unsigned char*)mi_heap_zalloc_aligned_at(heap, 96, 32, 16);
  result_bool("heap_zalloc_aligned_at", is_aligned_at(hza, 32, 16) && bytes_equal(hza, 0, 96) && mi_heap_contains(heap, hza));
  mi_free(hza);

  unsigned char* hca = heap == NULL ? NULL : (unsigned char*)mi_heap_calloc_aligned(heap, 8, 9, 32);
  result_bool("heap_calloc_aligned", is_aligned_at(hca, 32, 0) && bytes_equal(hca, 0, 72) && mi_heap_contains(heap, hca));
  mi_free(hca);

  hca = heap == NULL ? NULL : (unsigned char*)mi_heap_calloc_aligned_at(heap, 8, 9, 32, 24);
  result_bool("heap_calloc_aligned_at", is_aligned_at(hca, 32, 24) && bytes_equal(hca, 0, 72) && mi_heap_contains(heap, hca));
  mi_free(hca);

  unsigned char* ar = (unsigned char*)mi_realloc_aligned(NULL, 24, 32);
  result_bool("realloc_aligned_null", is_aligned_at(ar, 32, 0));
  fill_sequence(ar, 24);
  ar = (unsigned char*)mi_realloc_aligned(ar, 48, 32);
  result_bool("realloc_aligned_grow_preserve", is_aligned_at(ar, 32, 0) && has_sequence_prefix(ar, 24));
  ar = (unsigned char*)mi_realloc_aligned(ar, 16, 32);
  result_bool("realloc_aligned_shrink_preserve", is_aligned_at(ar, 32, 0) && has_sequence_prefix(ar, 16));
  mi_free(ar);

  ar = (unsigned char*)mi_realloc_aligned_at(NULL, 24, 32, 8);
  result_bool("realloc_aligned_at_null", is_aligned_at(ar, 32, 8));
  fill_sequence(ar, 24);
  ar = (unsigned char*)mi_realloc_aligned_at(ar, 48, 32, 8);
  result_bool("realloc_aligned_at_grow_preserve", is_aligned_at(ar, 32, 8) && has_sequence_prefix(ar, 24));
  mi_free(ar);

  unsigned char* rz = (unsigned char*)mi_rezalloc_aligned(NULL, 24, 32);
  result_bool("rezalloc_aligned_null_zeroed", is_aligned_at(rz, 32, 0) && bytes_equal(rz, 0, 24));
  fill_sequence(rz, 24);
  rz = (unsigned char*)mi_rezalloc_aligned(rz, 48, 32);
  result_bool("rezalloc_aligned_grow_zeroed", is_aligned_at(rz, 32, 0) && has_sequence_prefix(rz, 24) && bytes_equal(rz + 24, 0, 24));
  mi_free(rz);

  rz = (unsigned char*)mi_rezalloc_aligned_at(NULL, 24, 32, 16);
  result_bool("rezalloc_aligned_at_null_zeroed", is_aligned_at(rz, 32, 16) && bytes_equal(rz, 0, 24));
  mi_free(rz);

  unsigned char* rc = (unsigned char*)mi_recalloc_aligned(NULL, 3, 8, 32);
  result_bool("recalloc_aligned_null_zeroed", is_aligned_at(rc, 32, 0) && bytes_equal(rc, 0, 24));
  fill_sequence(rc, 24);
  rc = (unsigned char*)mi_recalloc_aligned(rc, 6, 8, 32);
  result_bool("recalloc_aligned_grow_zeroed", is_aligned_at(rc, 32, 0) && has_sequence_prefix(rc, 24) && bytes_equal(rc + 24, 0, 24));
  mi_free(rc);

  rc = (unsigned char*)mi_recalloc_aligned_at(NULL, 3, 8, 32, 24);
  result_bool("recalloc_aligned_at_null_zeroed", is_aligned_at(rc, 32, 24) && bytes_equal(rc, 0, 24));
  mi_free(rc);
  result_bool("recalloc_aligned_overflow_null", mi_recalloc_aligned(NULL, overflow_count, 2, 32) == NULL);

  unsigned char* hr = heap == NULL ? NULL : (unsigned char*)mi_heap_realloc_aligned(heap, NULL, 24, 32);
  result_bool("heap_realloc_aligned_null", is_aligned_at(hr, 32, 0) && mi_heap_contains(heap, hr));
  fill_sequence(hr, 24);
  hr = (unsigned char*)mi_heap_realloc_aligned(heap, hr, 48, 32);
  result_bool("heap_realloc_aligned_grow_preserve", is_aligned_at(hr, 32, 0) && has_sequence_prefix(hr, 24) && mi_heap_contains(heap, hr));
  hr = (unsigned char*)mi_heap_realloc_aligned(heap, hr, 16, 32);
  result_bool("heap_realloc_aligned_shrink_preserve", is_aligned_at(hr, 32, 0) && has_sequence_prefix(hr, 16) && mi_heap_contains(heap, hr));
  mi_free(hr);

  hr = heap == NULL ? NULL : (unsigned char*)mi_heap_realloc_aligned_at(heap, NULL, 24, 32, 8);
  result_bool("heap_realloc_aligned_at_null", is_aligned_at(hr, 32, 8) && mi_heap_contains(heap, hr));
  fill_sequence(hr, 24);
  hr = (unsigned char*)mi_heap_realloc_aligned_at(heap, hr, 48, 32, 8);
  result_bool("heap_realloc_aligned_at_grow_preserve", is_aligned_at(hr, 32, 8) && has_sequence_prefix(hr, 24) && mi_heap_contains(heap, hr));
  mi_free(hr);

  unsigned char* hrz = heap == NULL ? NULL : (unsigned char*)mi_heap_rezalloc_aligned(heap, NULL, 24, 32);
  result_bool("heap_rezalloc_aligned_null_zeroed", is_aligned_at(hrz, 32, 0) && bytes_equal(hrz, 0, 24) && mi_heap_contains(heap, hrz));
  fill_sequence(hrz, 24);
  hrz = (unsigned char*)mi_heap_rezalloc_aligned(heap, hrz, 48, 32);
  result_bool("heap_rezalloc_aligned_grow_zeroed", is_aligned_at(hrz, 32, 0) && has_sequence_prefix(hrz, 24) && bytes_equal(hrz + 24, 0, 24) && mi_heap_contains(heap, hrz));
  mi_free(hrz);

  hrz = heap == NULL ? NULL : (unsigned char*)mi_heap_rezalloc_aligned_at(heap, NULL, 24, 32, 16);
  result_bool("heap_rezalloc_aligned_at_null_zeroed", is_aligned_at(hrz, 32, 16) && bytes_equal(hrz, 0, 24) && mi_heap_contains(heap, hrz));
  fill_sequence(hrz, 24);
  hrz = (unsigned char*)mi_heap_rezalloc_aligned_at(heap, hrz, 48, 32, 16);
  result_bool("heap_rezalloc_aligned_at_grow_zeroed", is_aligned_at(hrz, 32, 16) && has_sequence_prefix(hrz, 24) && bytes_equal(hrz + 24, 0, 24) && mi_heap_contains(heap, hrz));
  mi_free(hrz);

  unsigned char* hrc = heap == NULL ? NULL : (unsigned char*)mi_heap_recalloc_aligned(heap, NULL, 3, 8, 32);
  result_bool("heap_recalloc_aligned_null_zeroed", is_aligned_at(hrc, 32, 0) && bytes_equal(hrc, 0, 24) && mi_heap_contains(heap, hrc));
  fill_sequence(hrc, 24);
  hrc = (unsigned char*)mi_heap_recalloc_aligned(heap, hrc, 6, 8, 32);
  result_bool("heap_recalloc_aligned_grow_zeroed", is_aligned_at(hrc, 32, 0) && has_sequence_prefix(hrc, 24) && bytes_equal(hrc + 24, 0, 24) && mi_heap_contains(heap, hrc));
  mi_free(hrc);

  hrc = heap == NULL ? NULL : (unsigned char*)mi_heap_recalloc_aligned_at(heap, NULL, 3, 8, 32, 24);
  result_bool("heap_recalloc_aligned_at_null_zeroed", is_aligned_at(hrc, 32, 24) && bytes_equal(hrc, 0, 24) && mi_heap_contains(heap, hrc));
  fill_sequence(hrc, 24);
  hrc = (unsigned char*)mi_heap_recalloc_aligned_at(heap, hrc, 6, 8, 32, 24);
  result_bool("heap_recalloc_aligned_at_grow_zeroed", is_aligned_at(hrc, 32, 24) && has_sequence_prefix(hrc, 24) && bytes_equal(hrc + 24, 0, 24) && mi_heap_contains(heap, hrc));
  mi_free(hrc);

  unsigned char* arc = (unsigned char*)mi_aligned_recalloc(NULL, 3, 8, 32);
  result_bool("aligned_recalloc_null_zeroed", is_aligned_at(arc, 32, 0) && bytes_equal(arc, 0, 24));
  fill_sequence(arc, 24);
  arc = (unsigned char*)mi_aligned_recalloc(arc, 6, 8, 32);
  result_bool("aligned_recalloc_grow_zeroed", is_aligned_at(arc, 32, 0) && has_sequence_prefix(arc, 24) && bytes_equal(arc + 24, 0, 24));
  mi_free(arc);

  arc = (unsigned char*)mi_aligned_offset_recalloc(NULL, 3, 8, 32, 8);
  result_bool("aligned_offset_recalloc_null_zeroed", is_aligned_at(arc, 32, 8) && bytes_equal(arc, 0, 24));
  mi_free(arc);
  result_bool("aligned_recalloc_overflow_null", mi_aligned_recalloc(NULL, overflow_count, 2, 32) == NULL);

  size_t block_size = 0;
  void* up = mi_umalloc_aligned(96, 32, &block_size);
  result_bool("umalloc_aligned_block_size", is_aligned_at(up, 32, 0) && block_size >= 96);
  mi_free(up);

  block_size = 0;
  unsigned char* uzp = (unsigned char*)mi_uzalloc_aligned(96, 32, &block_size);
  result_bool("uzalloc_aligned_block_size_zeroed", is_aligned_at(uzp, 32, 0) && block_size >= 96 && bytes_equal(uzp, 0, 96));
  mi_free(uzp);

  void* newp = mi_heap_alloc_new_n(heap, 4, 16);
  result_bool("heap_alloc_new_n_size_owner", newp != NULL && mi_usable_size(newp) >= 64 && mi_heap_contains(heap, newp));
  mi_free(newp);

  mi_theap_t* theap = heap == NULL ? NULL : mi_heap_theap(heap);
  void* tsp = mi_theap_malloc_small(theap, 32);
  result_bool("theap_malloc_small", tsp != NULL && mi_usable_size(tsp) >= 32);
  mi_free(tsp);
  void* tap = mi_theap_malloc_aligned(theap, 96, 32);
  result_bool("theap_malloc_aligned", is_aligned_at(tap, 32, 0));
  mi_free(tap);

  char* rp = mi_realpath(".", NULL);
  result_bool("realpath_alloc_or_platform_failure", rp != NULL);
  if (rp != NULL) mi_free(rp);

  if (heap != NULL) mi_heap_delete(heap);

  result_size("good_size_17", mi_good_size(17));
  result_size("malloc_good_size_17", mi_malloc_good_size(17));
  result_size("good_size_0", mi_good_size(0));
  result_size("malloc_good_size_0", mi_malloc_good_size(0));
  result_size("good_size_small_max", mi_good_size(MI_SMALL_SIZE_MAX));
  result_size("malloc_good_size_small_max", mi_malloc_good_size(MI_SMALL_SIZE_MAX));
  result_size("good_size_small_max_plus_1", mi_good_size(MI_SMALL_SIZE_MAX + 1));
  result_size("malloc_good_size_small_max_plus_1", mi_malloc_good_size(MI_SMALL_SIZE_MAX + 1));

  for (size_t bin = 0; bin <= MI_BIN_HUGE; bin++) {
    size_t bin_size = mi_stats_get_bin_size(bin);
    result_indexed_size("stats_bin_size", bin, bin_size);
    if (bin_size > 0) {
      result_indexed_size("good_size_bin", bin, mi_good_size(bin_size));
      result_indexed_size("malloc_good_size_bin", bin, mi_malloc_good_size(bin_size));
      if (bin_size > 1) {
        result_indexed_size("good_size_bin_minus_1", bin, mi_good_size(bin_size - 1));
        result_indexed_size("malloc_good_size_bin_minus_1", bin, mi_malloc_good_size(bin_size - 1));
      }
      if (bin_size < ((size_t)-1)) {
        result_indexed_size("good_size_bin_plus_1", bin, mi_good_size(bin_size + 1));
        result_indexed_size("malloc_good_size_bin_plus_1", bin, mi_malloc_good_size(bin_size + 1));
      }
    }
  }
  result_size("stats_bin_size_out_of_range", mi_stats_get_bin_size(MI_BIN_HUGE + 1));

  size_t size_checks[] = { 0, 1, 8, 16, 17, 32, 33, 64, 65, 128, 129, 256, 257, 512, 513,
                           1024, 1025, MI_SMALL_SIZE_MAX - 1, MI_SMALL_SIZE_MAX,
                           MI_SMALL_SIZE_MAX + 1, 8*1024 - 1, 8*1024, 8*1024 + 1,
                           64*1024, 1024*1024, 2*1024*1024 };
  size_t size_check_count = sizeof(size_checks) / sizeof(size_checks[0]);
  for (size_t i = 0; i < size_check_count; i++) {
    size_t requested = size_checks[i];
    void* sized = mi_malloc(requested);
    result_bool("malloc_size_live_nonnull", sized != NULL);
    if (sized != NULL) {
      result_bool("malloc_size_live_ge_request", mi_malloc_size(sized) >= requested);
      result_bool("malloc_usable_size_matches_size", mi_malloc_usable_size(sized) == mi_malloc_size(sized));
      mi_free(sized);
    }
  }
  void* aligned_sized = mi_malloc_aligned(257, 128);
  result_bool("malloc_size_aligned_ge_request", aligned_sized != NULL && mi_malloc_size(aligned_sized) >= 257);
  result_bool("malloc_usable_size_aligned_matches", aligned_sized != NULL && mi_malloc_usable_size(aligned_sized) == mi_malloc_size(aligned_sized));
  mi_free(aligned_sized);

  result_bool("usable_null_zero", mi_usable_size(NULL) == 0);
  result_bool("malloc_size_null_zero", mi_malloc_size(NULL) == 0);

  output_callbacks = 0;
  mi_stats_print_out(output_callback, NULL);
  result_bool("stats_output_callback_nonempty", output_callbacks > 0);

  mi_collect(false);
  return failures == 0 ? 0 : 1;
}
"""


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


def compile_binary(cc: str, output: pathlib.Path, objects: list[pathlib.Path]) -> None:
    subprocess.run(
        [
            cc,
            "-std=c11",
            "-I",
            str(ROOT / "mimalloc-3.3.2/include"),
            "-I",
            str(ROOT / "mimalloc-3.3.2/src"),
            *[str(obj) for obj in objects],
            "-pthread",
            "-ldl",
            "-lm",
            "-o",
            str(output),
        ],
        cwd=ROOT,
        check=True,
    )


def run(binary: pathlib.Path) -> str:
    completed = subprocess.run(
        [str(binary)],
        cwd=ROOT,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
    )
    return completed.stdout


def main() -> int:
    cc = os.environ.get("CC") or shutil.which("cc") or shutil.which("gcc")
    if cc is None:
        raise SystemExit("no C compiler found; set CC")

    BUILD_DIR.mkdir(parents=True, exist_ok=True)
    scenario = BUILD_DIR / "behavioral-parity.c"
    scenario.write_text(SCENARIO_C)

    rust_lib = rust_staticlib()
    c_bin = BUILD_DIR / "behavioral-parity-c"
    rust_bin = BUILD_DIR / "behavioral-parity-rust"

    compile_binary(cc, c_bin, [ROOT / "mimalloc-3.3.2/src/static.c", scenario])
    compile_binary(cc, rust_bin, [scenario, rust_lib])

    c_output = run(c_bin)
    rust_output = run(rust_bin)
    if c_output != rust_output:
        print("behavioral parity mismatch", file=sys.stderr)
        print("--- C mimalloc ---", file=sys.stderr)
        print(c_output, file=sys.stderr)
        print("--- Rust mimalloc ---", file=sys.stderr)
        print(rust_output, file=sys.stderr)
        return 1

    print("ok: C and Rust mimalloc behavior smoke outputs match")
    print(c_output, end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
