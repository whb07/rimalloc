#!/usr/bin/env python3
"""Compare normalized allocation traces from C mimalloc and this Rust port."""

from __future__ import annotations

import difflib
import os
import pathlib
import shutil
import subprocess
import sys


ROOT = pathlib.Path(__file__).resolve().parents[1]
BUILD_DIR = ROOT / "target/differential-trace"

PROBE_C = r"""
#define _GNU_SOURCE
#include <mimalloc.h>

#include <errno.h>
#include <pthread.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define TRACE_SLOTS 96
#define TRACE_STEPS 2048
#define TRACE_PATTERN_LIMIT 192

struct trace_slot {
  void* ptr;
  size_t size;
  size_t pattern;
  int heap_owned;
};

static uint64_t trace_next(uint64_t* state) {
  uint64_t x = *state;
  x ^= x >> 12;
  x ^= x << 25;
  x ^= x >> 27;
  *state = x;
  return x * 2685821657736338717ULL;
}

static uint64_t trace_mix(uint64_t hash, uint64_t value) {
  hash ^= value;
  hash *= 1099511628211ULL;
  return hash;
}

static size_t trace_pattern_len(size_t size) {
  return size < TRACE_PATTERN_LIMIT ? size : TRACE_PATTERN_LIMIT;
}

static unsigned char trace_pattern_byte(size_t pattern, size_t size, size_t index) {
  return (unsigned char)((pattern * 131 + size * 17 + index * 7) & 0xFF);
}

static void trace_fill(void* ptr, size_t size, size_t pattern) {
  unsigned char* bytes = (unsigned char*)ptr;
  for (size_t i = 0; i < trace_pattern_len(size); i++) {
    bytes[i] = trace_pattern_byte(pattern, size, i);
  }
}

static int trace_verify(const void* ptr, size_t size, size_t pattern) {
  const unsigned char* bytes = (const unsigned char*)ptr;
  for (size_t i = 0; i < trace_pattern_len(size); i++) {
    if (bytes[i] != trace_pattern_byte(pattern, size, i)) return 0;
  }
  return 1;
}

static int trace_zeroed(const void* ptr, size_t size) {
  const unsigned char* bytes = (const unsigned char*)ptr;
  for (size_t i = 0; i < trace_pattern_len(size); i++) {
    if (bytes[i] != 0) return 0;
  }
  return 1;
}

static uint64_t trace_hash_bytes(const void* ptr, size_t size) {
  const unsigned char* bytes = (const unsigned char*)ptr;
  uint64_t hash = 1469598103934665603ULL;
  for (size_t i = 0; i < trace_pattern_len(size); i++) {
    hash ^= bytes[i];
    hash *= 1099511628211ULL;
  }
  return hash;
}

static size_t trace_random_size(uint64_t* state) {
  static const size_t interesting[] = {
    0, 1, 2, 3, 7, 8, 15, 16, 17, 24, 31, 32, 33, 48, 63, 64,
    65, 80, 96, 127, 128, 129, 191, 192, 193, 255, 256, 257,
    511, 512, 513, 1023, 1024, 1025, 2047, 2048, 2049, 4095,
    4096, 4097, 8191, 8192, 8193, 16383, 16384, 16385, 32767,
    32768, 32769, 65535, 65536
  };
  uint64_t r = trace_next(state);
  if ((r & 3) == 0) return interesting[(r >> 8) % (sizeof(interesting) / sizeof(interesting[0]))];
  return (size_t)((r >> 8) % 65537);
}

static size_t trace_random_nonzero_size(uint64_t* state) {
  size_t size = trace_random_size(state);
  return size == 0 ? 1 : size;
}

static size_t trace_random_alignment(uint64_t* state) {
  return (size_t)8 << (trace_next(state) % 10);
}

static void trace_reset_slot(struct trace_slot* slot) {
  slot->ptr = NULL;
  slot->size = 0;
  slot->pattern = 0;
  slot->heap_owned = 0;
}

static int trace_fail(uint64_t seed, size_t step, size_t slot, const char* op, const char* reason) {
  printf("@@TRACE_FAIL seed=%llu step=%zu slot=%zu op=%s reason=%s\n",
         (unsigned long long)seed,
         step,
         slot,
         op,
         reason);
  return 0;
}

static int trace_edge_alloc(const char* name, void* ptr, size_t request, size_t alignment, size_t offset, int expect_zero) {
  int present = ptr != NULL;
  size_t usable = present ? mi_usable_size(ptr) : 0;
  int usable_ok = (!present || usable >= request);
  int owned = present ? mi_check_owned(ptr) : 0;
  int align_ok = (!present || alignment == 0 || ((((uintptr_t)ptr + offset) % alignment) == 0));
  int zero_ok = (!present || !expect_zero || trace_zeroed(ptr, request));
  uint64_t hash = 1469598103934665603ULL;

  if (present) {
    trace_fill(ptr, request, request + alignment + offset + 1);
    hash = trace_hash_bytes(ptr, request);
  }

  printf("@@TRACE_EDGE_ALLOC name=%s req=%zu alignment=%zu offset=%zu present=%d usable_ok=%d owned=%d align_ok=%d zero_ok=%d hash=%016llx\n",
         name,
         request,
         alignment,
         offset,
         present,
         usable_ok,
         owned,
         align_ok,
         zero_ok,
         (unsigned long long)hash);

  mi_free(ptr);
  return (!present || (usable_ok && owned && align_ok && zero_ok));
}

static int trace_edges(void) {
  static const size_t sizes[] = {
    0, 1, 2, 3, 7, 8, 15, 16, 17, 31, 32, 33, 63, 64, 65,
    127, 128, 129, 255, 256, 257, 511, 512, 513, 1023, 1024,
    1025, 2047, 2048, 2049, 4095, 4096, 4097, 8191, 8192,
    8193, 65535, 65536, 65537, 1048576
  };
  int ok = 1;

  for (size_t i = 0; i < sizeof(sizes) / sizeof(sizes[0]); i++) {
    printf("@@TRACE_EDGE_GOOD index=%zu size=%zu good=%zu malloc_good=%zu\n",
           i,
           sizes[i],
           mi_good_size(sizes[i]),
           mi_malloc_good_size(sizes[i]));
  }

  ok = trace_edge_alloc("malloc_zero", mi_malloc(0), 0, 0, 0, 0) && ok;
  ok = trace_edge_alloc("zalloc_zero", mi_zalloc(0), 0, 0, 0, 1) && ok;
  ok = trace_edge_alloc("calloc_zero", mi_calloc(0, 64), 0, 0, 0, 1) && ok;
  ok = trace_edge_alloc("aligned_zero", mi_malloc_aligned(0, 64), 0, 64, 0, 0) && ok;
  ok = trace_edge_alloc("aligned_at", mi_malloc_aligned_at(96, 32, 8), 96, 32, 8, 0) && ok;

  void* posix = NULL;
  int posix_err = mi_posix_memalign(&posix, 64, 73);
  printf("@@TRACE_EDGE_POSIX name=valid err=%d present=%d alignment=%d usable_ok=%d\n",
         posix_err,
         posix != NULL,
         posix != NULL && (((uintptr_t)posix % 64) == 0),
         posix != NULL && mi_usable_size(posix) >= 73);
  ok = (posix_err == 0 && posix != NULL && (((uintptr_t)posix % 64) == 0) && mi_usable_size(posix) >= 73) && ok;
  mi_free(posix);

  posix = (void*)0x1;
  posix_err = mi_posix_memalign(&posix, 3, 73);
  printf("@@TRACE_EDGE_POSIX name=invalid err=%d unchanged=%d\n", posix_err, posix == (void*)0x1);
  ok = (posix_err == EINVAL && posix == (void*)0x1) && ok;

  void* huge = mi_malloc((size_t)-1);
  printf("@@TRACE_EDGE_OVERSIZE name=malloc_max present=%d\n", huge != NULL);
  ok = (huge == NULL) && ok;
  mi_free(huge);

  void* overflow = mi_mallocn(((size_t)-1 / 2) + 1, 2);
  printf("@@TRACE_EDGE_OVERSIZE name=mallocn_overflow present=%d\n", overflow != NULL);
  ok = (overflow == NULL) && ok;
  mi_free(overflow);

  overflow = mi_calloc(((size_t)-1 / 2) + 1, 2);
  printf("@@TRACE_EDGE_OVERSIZE name=calloc_overflow present=%d\n", overflow != NULL);
  ok = (overflow == NULL) && ok;
  mi_free(overflow);

  return ok;
}

static int trace_alloc_slot(
    uint64_t seed,
    size_t step,
    size_t slot_index,
    struct trace_slot* slot,
    mi_heap_t* heap,
    uint64_t* state,
    uint64_t* checksum) {
  size_t size = trace_random_size(state);
  size_t kind = (size_t)(trace_next(state) % 9);
  size_t alignment = 0;
  size_t offset = 0;
  int heap_owned = 0;
  int zero_expected = 0;
  void* ptr = NULL;
  const char* op = "malloc";

  switch (kind) {
    case 0:
      ptr = mi_malloc(size);
      op = "malloc";
      break;
    case 1:
      ptr = mi_zalloc(size);
      op = "zalloc";
      zero_expected = 1;
      break;
    case 2: {
      size_t count = 1 + (size_t)(trace_next(state) % 17);
      size_t elem = size / count;
      if (elem == 0) elem = 1;
      size = count * elem;
      ptr = mi_calloc(count, elem);
      op = "calloc";
      zero_expected = 1;
      break;
    }
    case 3: {
      size_t count = 1 + (size_t)(trace_next(state) % 17);
      size_t elem = size / count;
      if (elem == 0) elem = 1;
      size = count * elem;
      ptr = mi_mallocn(count, elem);
      op = "mallocn";
      break;
    }
    case 4:
      alignment = trace_random_alignment(state);
      ptr = mi_malloc_aligned(size, alignment);
      op = "aligned";
      break;
    case 5:
      alignment = trace_random_alignment(state);
      ptr = mi_zalloc_aligned(size, alignment);
      op = "zalloc_aligned";
      zero_expected = 1;
      break;
    case 6:
      ptr = mi_heap_malloc(heap, size);
      op = "heap_malloc";
      heap_owned = 1;
      break;
    case 7:
      ptr = mi_heap_zalloc(heap, size);
      op = "heap_zalloc";
      heap_owned = 1;
      zero_expected = 1;
      break;
    default:
      alignment = 64;
      offset = 16;
      if (size < 34) size = 34;
      ptr = mi_malloc_aligned_at(size, alignment, offset);
      op = "aligned_at";
      break;
  }

  if (ptr == NULL) return trace_fail(seed, step, slot_index, op, "null");

  size_t usable = mi_usable_size(ptr);
  int usable_ok = usable >= size;
  int owned = mi_check_owned(ptr);
  int align_ok = (alignment == 0 || ((((uintptr_t)ptr + offset) % alignment) == 0));
  int zero_ok = (!zero_expected || trace_zeroed(ptr, size));
  int heap_ok = (!heap_owned || mi_heap_contains(heap, ptr));
  if (!usable_ok || !owned || !align_ok || !zero_ok || !heap_ok) {
    mi_free(ptr);
    return trace_fail(seed, step, slot_index, op, "invariant");
  }

  slot->ptr = ptr;
  slot->size = size;
  slot->pattern = seed + step + slot_index + 1;
  slot->heap_owned = heap_owned;
  trace_fill(slot->ptr, slot->size, slot->pattern);
  uint64_t hash = trace_hash_bytes(slot->ptr, slot->size);
  printf("@@TRACE seed=%llu step=%zu slot=%zu op=%s req=%zu alignment=%zu offset=%zu heap=%d usable_ok=%d zero=%d owned=%d hash=%016llx\n",
         (unsigned long long)seed,
         step,
         slot_index,
         op,
         size,
         alignment,
         offset,
         heap_owned,
         usable_ok,
         zero_ok,
         owned,
         (unsigned long long)hash);
  *checksum = trace_mix(*checksum, (uint64_t)step);
  *checksum = trace_mix(*checksum, (uint64_t)slot_index);
  *checksum = trace_mix(*checksum, (uint64_t)kind);
  *checksum = trace_mix(*checksum, (uint64_t)size);
  *checksum = trace_mix(*checksum, (uint64_t)offset);
  *checksum = trace_mix(*checksum, (uint64_t)usable_ok);
  *checksum = trace_mix(*checksum, hash);
  return 1;
}

static int trace_free_slot(
    uint64_t seed,
    size_t step,
    size_t slot_index,
    struct trace_slot* slot,
    const char* op,
    uint64_t* checksum) {
  if (!trace_verify(slot->ptr, slot->size, slot->pattern)) {
    return trace_fail(seed, step, slot_index, op, "pattern");
  }
  size_t usable = mi_usable_size(slot->ptr);
  int usable_ok = usable >= slot->size;
  uint64_t hash = trace_hash_bytes(slot->ptr, slot->size);
  printf("@@TRACE seed=%llu step=%zu slot=%zu op=%s req=%zu heap=%d usable_ok=%d hash=%016llx\n",
         (unsigned long long)seed,
         step,
         slot_index,
         op,
         slot->size,
         slot->heap_owned,
         usable_ok,
         (unsigned long long)hash);
  *checksum = trace_mix(*checksum, (uint64_t)step);
  *checksum = trace_mix(*checksum, (uint64_t)slot_index);
  *checksum = trace_mix(*checksum, slot->size);
  *checksum = trace_mix(*checksum, (uint64_t)usable_ok);
  *checksum = trace_mix(*checksum, hash);
  mi_free(slot->ptr);
  trace_reset_slot(slot);
  return 1;
}

static int trace_check_slot(
    uint64_t seed,
    size_t step,
    size_t slot_index,
    const struct trace_slot* slot,
    mi_heap_t* heap,
    uint64_t* checksum) {
  if (!trace_verify(slot->ptr, slot->size, slot->pattern)) {
    return trace_fail(seed, step, slot_index, "check", "pattern");
  }
  size_t usable = mi_usable_size(slot->ptr);
  int usable_ok = usable >= slot->size;
  int owned = mi_check_owned(slot->ptr);
  int heap_ok = (!slot->heap_owned || mi_heap_contains(heap, slot->ptr));
  uint64_t hash = trace_hash_bytes(slot->ptr, slot->size);
  printf("@@TRACE seed=%llu step=%zu slot=%zu op=check req=%zu heap=%d usable_ok=%d owned=%d heap_ok=%d hash=%016llx\n",
         (unsigned long long)seed,
         step,
         slot_index,
         slot->size,
         slot->heap_owned,
         usable_ok,
         owned,
         heap_ok,
         (unsigned long long)hash);
  *checksum = trace_mix(*checksum, (uint64_t)step);
  *checksum = trace_mix(*checksum, (uint64_t)slot_index);
  *checksum = trace_mix(*checksum, slot->size);
  *checksum = trace_mix(*checksum, (uint64_t)usable_ok);
  *checksum = trace_mix(*checksum, hash);
  return (usable_ok && owned && heap_ok);
}

static int trace_realloc_slot(
    uint64_t seed,
    size_t step,
    size_t slot_index,
    struct trace_slot* slot,
    mi_heap_t* heap,
    uint64_t* state,
    uint64_t* checksum) {
  if (!trace_verify(slot->ptr, slot->size, slot->pattern)) {
    return trace_fail(seed, step, slot_index, "realloc", "pattern");
  }
  size_t old_size = slot->size;
  size_t old_pattern = slot->pattern;
  size_t new_size = trace_random_nonzero_size(state);
  void* resized = slot->heap_owned ? mi_heap_realloc(heap, slot->ptr, new_size) : mi_realloc(slot->ptr, new_size);
  if (resized == NULL) {
    return trace_fail(seed, step, slot_index, "realloc", "null");
  }

  size_t preserved = old_size < new_size ? old_size : new_size;
  size_t checked = trace_pattern_len(preserved);
  const unsigned char* bytes = (const unsigned char*)resized;
  for (size_t i = 0; i < checked; i++) {
    if (bytes[i] != trace_pattern_byte(old_pattern, old_size, i)) {
      mi_free(resized);
      trace_reset_slot(slot);
      return trace_fail(seed, step, slot_index, "realloc", "preserve");
    }
  }

  size_t usable = mi_usable_size(resized);
  int usable_ok = usable >= new_size;
  int owned = mi_check_owned(resized);
  int heap_ok = (!slot->heap_owned || mi_heap_contains(heap, resized));
  if (!usable_ok || !owned || !heap_ok) {
    mi_free(resized);
    trace_reset_slot(slot);
    return trace_fail(seed, step, slot_index, "realloc", "invariant");
  }

  slot->ptr = resized;
  slot->size = new_size;
  slot->pattern = seed + step + slot_index + 1;
  trace_fill(slot->ptr, slot->size, slot->pattern);
  uint64_t hash = trace_hash_bytes(slot->ptr, slot->size);
  printf("@@TRACE seed=%llu step=%zu slot=%zu op=realloc old=%zu new=%zu heap=%d usable_ok=%d owned=%d preserved=%zu hash=%016llx\n",
         (unsigned long long)seed,
         step,
         slot_index,
         old_size,
         new_size,
         slot->heap_owned,
         usable_ok,
         owned,
         checked,
         (unsigned long long)hash);
  *checksum = trace_mix(*checksum, (uint64_t)step);
  *checksum = trace_mix(*checksum, (uint64_t)slot_index);
  *checksum = trace_mix(*checksum, old_size);
  *checksum = trace_mix(*checksum, new_size);
  *checksum = trace_mix(*checksum, (uint64_t)usable_ok);
  *checksum = trace_mix(*checksum, hash);
  return 1;
}

static int trace_seed(uint64_t seed) {
  struct trace_slot slots[TRACE_SLOTS];
  uint64_t state = seed == 0 ? 0x9e3779b97f4a7c15ULL : seed;
  uint64_t checksum = trace_mix(1469598103934665603ULL, seed);
  mi_heap_t* heap = mi_heap_new();
  size_t live = 0;
  int ok = 1;

  if (heap == NULL) return trace_fail(seed, 0, 0, "heap_new", "null");
  for (size_t i = 0; i < TRACE_SLOTS; i++) trace_reset_slot(&slots[i]);

  printf("@@TRACE_BEGIN seed=%llu steps=%d slots=%d\n",
         (unsigned long long)seed,
         TRACE_STEPS,
         TRACE_SLOTS);

  for (size_t step = 0; step < TRACE_STEPS && ok; step++) {
    size_t slot_index = (size_t)(trace_next(&state) % TRACE_SLOTS);
    size_t action = (size_t)(trace_next(&state) % 12);
    struct trace_slot* slot = &slots[slot_index];

    if (slot->ptr == NULL) {
      ok = trace_alloc_slot(seed, step, slot_index, slot, heap, &state, &checksum);
      if (ok) live++;
    }
    else if (action < 4) {
      ok = trace_free_slot(seed, step, slot_index, slot, "free", &checksum);
      if (ok) live--;
    }
    else if (action < 8) {
      ok = trace_realloc_slot(seed, step, slot_index, slot, heap, &state, &checksum);
    }
    else {
      ok = trace_check_slot(seed, step, slot_index, slot, heap, &checksum);
    }

    if ((step % 127) == 0) mi_collect(false);
  }

  for (size_t i = 0; i < TRACE_SLOTS; i++) {
    if (slots[i].ptr != NULL) {
      ok = trace_free_slot(seed, TRACE_STEPS + i, i, &slots[i], "final_free", &checksum) && ok;
      if (ok) live--;
    }
  }

  mi_heap_delete(heap);
  mi_collect(true);

  printf("@@TRACE_SUMMARY seed=%llu live=%zu checksum=%016llx ok=%d\n",
         (unsigned long long)seed,
         live,
         (unsigned long long)checksum,
         ok);
  return ok && live == 0;
}

struct transition_result {
  void* ptr;
  size_t size;
};

static void* trace_owner_exit(void* raw) {
  struct transition_result* result = (struct transition_result*)raw;
  result->ptr = mi_malloc(result->size);
  if (result->ptr != NULL) trace_fill(result->ptr, result->size, 77);
  return NULL;
}

static int trace_transitions(void) {
  struct transition_result result = {NULL, 4096};
  pthread_t owner;
  int started = pthread_create(&owner, NULL, trace_owner_exit, &result) == 0;
  int joined = started && pthread_join(owner, NULL) == 0;
  int remote_ok = joined && result.ptr != NULL &&
                  trace_verify(result.ptr, result.size, 77) &&
                  mi_check_owned(result.ptr);
  printf("@@TRACE_TRANSITION name=owner_exit_remote_free started=%d joined=%d ok=%d\n",
         started, joined, remote_ok);
  mi_free(result.ptr);

  mi_heap_t* heap = mi_heap_new();
  void* heap_ptr = heap == NULL ? NULL : mi_heap_malloc(heap, 8192);
  int destroy_ok = heap != NULL && heap_ptr != NULL && mi_heap_contains(heap, heap_ptr);
  if (heap != NULL) mi_heap_destroy(heap);
  printf("@@TRACE_TRANSITION name=heap_destroy ok=%d\n", destroy_ok);
  mi_collect(true);
  return remote_ok && destroy_ok;
}

int main(void) {
  static const uint64_t seeds[] = {
    0x4d595df4d0f33173ULL,
    0x9e3779b97f4a7c15ULL,
    0xbf58476d1ce4e5b9ULL,
    0x94d049bb133111ebULL,
    0xd1b54a32d192ed03ULL,
    0x123456789abcdef0ULL
  };
  int ok = 1;

  mi_option_set(mi_option_show_errors, 0);
  mi_option_set(mi_option_verbose, 0);

  ok = trace_edges() && ok;
  ok = trace_transitions() && ok;
  for (size_t i = 0; i < sizeof(seeds) / sizeof(seeds[0]); i++) {
    ok = trace_seed(seeds[i]) && ok;
  }
  printf("@@TRACE_DONE seeds=%zu steps=%d slots=%d ok=%d\n",
         sizeof(seeds) / sizeof(seeds[0]),
         TRACE_STEPS,
         TRACE_SLOTS,
         ok);
  return ok ? 0 : 1;
}
"""


def run(
    cmd: list[str],
    *,
    check: bool = True,
    env: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        cmd,
        cwd=ROOT,
        check=check,
        env=env,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )


def rust_staticlib() -> pathlib.Path:
    target_dir = BUILD_DIR / "rust-target"
    env = dict(os.environ)
    env["CARGO_TARGET_DIR"] = str(target_dir)
    run(["cargo", "rustc", "--lib", "--", "--crate-type", "staticlib"], env=env)
    libs = sorted(
        [
            *(target_dir / "debug/deps").glob("librimalloc-*.a"),
            *(target_dir / "debug/deps").glob("librimalloc.a"),
            *(target_dir / "debug").glob("librimalloc.a"),
        ],
        key=lambda path: path.stat().st_mtime,
        reverse=True,
    )
    if not libs:
        raise SystemExit("no Rust staticlib artifact found")
    return libs[0]


def compile_probe(cc: str, output: pathlib.Path, objects: list[pathlib.Path]) -> None:
    run(
        [
            cc,
            "-std=c11",
            "-DMI_MALLOC_OVERRIDE=0",
            "-I",
            str(ROOT / "mimalloc-3.3.2/include"),
            "-I",
            str(ROOT / "mimalloc-3.3.2/src"),
            str(BUILD_DIR / "differential-trace.c"),
            *[str(obj) for obj in objects],
            "-pthread",
            "-ldl",
            "-lm",
            "-o",
            str(output),
        ]
    )


def trace_env() -> dict[str, str]:
    env = {key: value for key, value in os.environ.items() if not key.lower().startswith("mimalloc_")}
    env["MIMALLOC_SHOW_ERRORS"] = "0"
    env["MIMALLOC_VERBOSE"] = "0"
    return env


def run_trace(binary: pathlib.Path) -> str:
    return run([str(binary)], env=trace_env()).stdout


def main() -> int:
    cc = os.environ.get("CC") or shutil.which("cc") or shutil.which("gcc") or shutil.which("clang")
    if cc is None:
        raise SystemExit("no C compiler found; set CC")

    shutil.rmtree(BUILD_DIR, ignore_errors=True)
    BUILD_DIR.mkdir(parents=True, exist_ok=True)
    (BUILD_DIR / "differential-trace.c").write_text(PROBE_C)

    rust_lib = rust_staticlib()
    c_bin = BUILD_DIR / "differential-trace-c"
    rust_bin = BUILD_DIR / "differential-trace-rust"
    compile_probe(cc, c_bin, [ROOT / "mimalloc-3.3.2/src/static.c"])
    compile_probe(cc, rust_bin, [rust_lib])

    c_output = run_trace(c_bin)
    rust_output = run_trace(rust_bin)
    (BUILD_DIR / "c.trace").write_text(c_output)
    (BUILD_DIR / "rust.trace").write_text(rust_output)

    if c_output != rust_output:
        diff = "".join(
            difflib.unified_diff(
                c_output.splitlines(keepends=True),
                rust_output.splitlines(keepends=True),
                fromfile="c.trace",
                tofile="rust.trace",
                n=5,
            )
        )
        sys.stderr.write("differential allocation trace mismatch\n")
        sys.stderr.write(f"wrote {BUILD_DIR / 'c.trace'} and {BUILD_DIR / 'rust.trace'}\n")
        sys.stderr.write(diff)
        return 1

    print("ok: C and Rust mimalloc differential allocation traces match")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except subprocess.CalledProcessError as err:
        sys.stderr.write(err.stdout or "")
        sys.stderr.write(err.stderr or "")
        raise
