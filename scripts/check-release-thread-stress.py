#!/usr/bin/env python3
"""Run sustained release-thread stress against the c-abi artifact."""

from __future__ import annotations

import os
import pathlib
import shutil
import subprocess
import sys


ROOT = pathlib.Path(__file__).resolve().parents[1]
BUILD_DIR = ROOT / "target/release-thread-stress"

PROBE = r"""
#include <mimalloc.h>
#include <mimalloc-stats.h>

#include <pthread.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

#define STRESS_THREADS 8
#define STRESS_WAVES 5
#define STRESS_SLOTS 128
#define STRESS_INCOMING 32
#define STRESS_ROUNDS 4096
#define ABANDONED_BLOCKS 256

struct stress_arg {
  void* incoming[STRESS_INCOMING];
  size_t id;
  int use_threadpool;
  int ok;
};

struct abandoned_arg {
  mi_heap_t* heap;
  void* blocks[ABANDONED_BLOCKS];
  int allocated;
};

struct visit_counts {
  size_t areas;
  size_t blocks;
  int fields_ok;
};

static int stats_output_calls = 0;

static int64_t stats_alloc_current(const mi_stats_t* stats) {
  return stats->malloc_normal.current + stats->malloc_huge.current;
}

static int64_t stats_alloc_peak(const mi_stats_t* stats) {
  return stats->malloc_normal.peak + stats->malloc_huge.peak;
}

static int64_t stats_alloc_total(const mi_stats_t* stats) {
  return stats->malloc_normal.total + stats->malloc_huge.total;
}

static int check_block(void* p, size_t size, size_t alignment) {
  if (p == NULL) return 0;
  if (!mi_check_owned(p)) return 0;
  if (mi_usable_size(p) < size) return 0;
  if (alignment != 0 && ((uintptr_t)p % alignment) != 0) return 0;
  return 1;
}

static int churn_allocations(size_t id, size_t rounds) {
  void* slots[STRESS_SLOTS];
  memset(slots, 0, sizeof(slots));
  int ok = 1;

  for (size_t round = 0; round < rounds; round++) {
    size_t slot = (round * 19 + id * 7) % STRESS_SLOTS;
    if (slots[slot] != NULL) {
      if ((round % 4) == 0) {
        size_t new_size = 1 + ((round * 31 + id * 17) % 16384);
        void* resized = mi_realloc(slots[slot], new_size);
        if (!check_block(resized, new_size, 0)) {
          ok = 0;
          mi_free(resized);
          slots[slot] = NULL;
        }
        else {
          slots[slot] = resized;
        }
      }
      else {
        mi_free(slots[slot]);
        slots[slot] = NULL;
      }
    }
    else {
      size_t size = 1 + ((round * 41 + id * 29) % 32768);
      void* p = NULL;
      size_t alignment = 0;
      switch (round % 6) {
        case 0:
          p = mi_malloc(size);
          break;
        case 1:
          p = mi_zalloc(size);
          break;
        case 2:
          alignment = (size_t)16 << ((round + id) % 4);
          p = mi_malloc_aligned(size, alignment);
          break;
        case 3:
          p = mi_realloc(NULL, size);
          break;
        case 4: {
          size_t count = 1 + (round % 11);
          size_t elem_size = 1 + (size / count);
          size = count * elem_size;
          p = mi_mallocn(count, elem_size);
          break;
        }
        default:
          p = mi_zalloc_aligned(size, 64);
          alignment = 64;
          break;
      }
      if (!check_block(p, size, alignment)) {
        ok = 0;
        mi_free(p);
      }
      else {
        slots[slot] = p;
      }
    }

    if ((round % 257) == 0) {
      mi_collect(false);
    }
  }

  for (size_t i = 0; i < STRESS_SLOTS; i++) {
    mi_free(slots[i]);
  }
  mi_collect(true);
  return ok;
}

static void* stress_worker(void* arg) {
  struct stress_arg* result = (struct stress_arg*)arg;
  result->ok = 1;

  mi_thread_init();
  if (result->use_threadpool) {
    mi_thread_set_in_threadpool();
  }

  for (size_t i = 0; i < STRESS_INCOMING; i++) {
    if (result->incoming[i] == NULL || !mi_is_in_heap_region(result->incoming[i])) {
      result->ok = 0;
    }
    mi_free(result->incoming[i]);
    result->incoming[i] = NULL;
  }

  if (!churn_allocations(result->id, STRESS_ROUNDS)) {
    result->ok = 0;
  }

  mi_thread_done();
  mi_thread_init();
  void* p = mi_malloc(128 + result->id);
  if (!check_block(p, 128 + result->id, 0)) {
    result->ok = 0;
  }
  mi_free(p);
  mi_thread_done();
  return NULL;
}

static bool visit_abandoned(
    const mi_heap_t* heap,
    const mi_heap_area_t* area,
    void* block,
    size_t block_size,
    void* arg) {
  (void)heap;
  struct visit_counts* counts = (struct visit_counts*)arg;
  if (area == NULL || area->blocks == NULL || area->reserved < area->committed ||
      area->full_block_size < area->block_size) {
    counts->fields_ok = 0;
  }
  if (block == NULL) {
    counts->areas++;
  }
  else {
    counts->blocks++;
    if (block_size == 0 || block_size > area->full_block_size) {
      counts->fields_ok = 0;
    }
  }
  return true;
}

static void* abandoned_worker(void* arg) {
  struct abandoned_arg* result = (struct abandoned_arg*)arg;
  result->allocated = 1;
  mi_thread_init();
  for (size_t i = 0; i < ABANDONED_BLOCKS; i++) {
    size_t size = 64 + ((i % 8) * 32);
    result->blocks[i] = mi_heap_malloc(result->heap, size);
    if (result->blocks[i] == NULL ||
        !mi_heap_contains(result->heap, result->blocks[i])) {
      result->allocated = 0;
    }
  }
  mi_thread_done();
  return NULL;
}

static int run_same_thread_stress(void) {
  return churn_allocations(1009, STRESS_ROUNDS * 2);
}

static int run_cross_thread_and_lifecycle_stress(void) {
  int all_started = 1;
  int all_workers = 1;

  for (size_t wave = 0; wave < STRESS_WAVES; wave++) {
    pthread_t threads[STRESS_THREADS];
    struct stress_arg args[STRESS_THREADS];
    size_t started = 0;
    memset(threads, 0, sizeof(threads));
    memset(args, 0, sizeof(args));

    for (size_t t = 0; t < STRESS_THREADS; t++) {
      args[t].id = t + (wave * STRESS_THREADS);
      args[t].use_threadpool = ((t + wave) % 2) == 0;
      for (size_t i = 0; i < STRESS_INCOMING; i++) {
        size_t size = 1 + (((wave + 1) * (t + 3) * (i + 5)) % 4096);
        args[t].incoming[i] = mi_malloc(size);
        if (!check_block(args[t].incoming[i], size, 0)) {
          all_workers = 0;
        }
      }
      if (pthread_create(&threads[t], NULL, stress_worker, &args[t]) == 0) {
        started++;
      }
      else {
        all_started = 0;
        all_workers = 0;
        for (size_t i = 0; i < STRESS_INCOMING; i++) {
          mi_free(args[t].incoming[i]);
        }
      }
    }

    if (started != STRESS_THREADS) {
      all_started = 0;
    }
    for (size_t t = 0; t < started; t++) {
      if (pthread_join(threads[t], NULL) != 0 || !args[t].ok) {
        all_workers = 0;
      }
    }
    mi_collect(true);
  }

  return all_started && all_workers;
}

static int run_abandoned_reclaim_stress(void) {
  long old_visit_abandoned = mi_option_get(mi_option_visit_abandoned);
  mi_option_enable(mi_option_visit_abandoned);

  mi_heap_t* heap = mi_heap_new();
  if (heap == NULL) {
    mi_option_set(mi_option_visit_abandoned, old_visit_abandoned);
    return 0;
  }

  struct abandoned_arg abandoned;
  memset(&abandoned, 0, sizeof(abandoned));
  abandoned.heap = heap;
  pthread_t thread;
  int started = pthread_create(&thread, NULL, abandoned_worker, &abandoned) == 0;
  int joined = started && pthread_join(thread, NULL) == 0;

  struct visit_counts counts = {0, 0, 1};
  bool visited = mi_heap_visit_abandoned_blocks(heap, true, visit_abandoned, &counts);

  mi_stats_t before;
  mi_stats_t after;
  mi_stats_init(&before);
  mi_stats_init(&after);
  int before_ok = mi_heap_stats_get(heap, &before);
  void* reclaimed = mi_heap_malloc(heap, 96);
  int reclaim_ok = reclaimed != NULL && mi_heap_contains(heap, reclaimed);
  int after_ok = mi_heap_stats_get(heap, &after);

  for (size_t i = 0; i < ABANDONED_BLOCKS; i++) {
    mi_free(abandoned.blocks[i]);
  }
  mi_free(reclaimed);
  mi_heap_delete(heap);
  mi_option_set(mi_option_visit_abandoned, old_visit_abandoned);

  int ok = started && joined && abandoned.allocated && visited &&
           counts.fields_ok && counts.areas > 0 &&
           before_ok && after_ok && reclaim_ok &&
           after.pages_reclaim_on_alloc.total >= before.pages_reclaim_on_alloc.total;
  if (!ok) {
    fprintf(stderr,
            "abandoned stress failed: started=%d joined=%d allocated=%d visited=%d fields=%d areas=%zu blocks=%zu before=%d after=%d reclaim=%d before_reclaim=%ld after_reclaim=%ld\n",
            started,
            joined,
            abandoned.allocated,
            visited ? 1 : 0,
            counts.fields_ok,
            counts.areas,
            counts.blocks,
            before_ok,
            after_ok,
            reclaim_ok,
            (long)before.pages_reclaim_on_alloc.total,
            (long)after.pages_reclaim_on_alloc.total);
  }
  return ok;
}

static void stats_output(const char* msg, void* arg) {
  if (arg == (void*)0x51A75 && msg != NULL) {
    stats_output_calls++;
  }
}

static int run_shutdown_stats(void) {
  mi_stats_t before;
  mi_stats_t after;
  mi_stats_init(&before);
  mi_stats_init(&after);
  int before_ok = mi_stats_get(&before);

  void* p = mi_malloc(8192);
  if (!check_block(p, 8192, 0)) {
    mi_free(p);
    return 0;
  }
  mi_free(p);
  mi_collect(true);

  int after_ok = mi_stats_get(&after);
  mi_stats_print_out(stats_output, (void*)0x51A75);
  int stats_ok = before_ok && after_ok && stats_output_calls > 0 &&
                 stats_alloc_current(&after) >= 0 &&
                 stats_alloc_peak(&after) >= stats_alloc_current(&after) &&
                 stats_alloc_total(&after) >= stats_alloc_total(&before);
  mi_process_done();
  return stats_ok;
}

int main(void) {
  int same_thread = run_same_thread_stress();
  int cross_thread = run_cross_thread_and_lifecycle_stress();
  int abandoned = run_abandoned_reclaim_stress();
  int shutdown = run_shutdown_stats();
  if (!(same_thread && cross_thread && abandoned && shutdown)) {
    fprintf(stderr,
            "release thread stress failed: same_thread=%d cross_thread=%d abandoned=%d shutdown=%d\n",
            same_thread,
            cross_thread,
            abandoned,
            shutdown);
    return 1;
  }
  return 0;
}
"""


def run(
    cmd: list[str],
    *,
    cwd: pathlib.Path = ROOT,
    env: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        cmd,
        cwd=cwd,
        env=env,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )


def cc() -> str:
    compiler = os.environ.get("CC") or shutil.which("cc") or shutil.which("gcc") or shutil.which("clang")
    if compiler is None:
        raise SystemExit("no C compiler found; set CC")
    return compiler


def build_c_abi_cdylib() -> pathlib.Path:
    target_dir = BUILD_DIR / "rust-target"
    env = dict(os.environ)
    env["CARGO_TARGET_DIR"] = str(target_dir)
    run(
        [
            "cargo",
            "rustc",
            "--features",
            "c-abi",
            "--lib",
            "--release",
            "--",
            "--crate-type",
            "cdylib",
        ],
        env=env,
    )

    candidates = sorted(
        list((target_dir / "release").glob("libmechanical_mimalloc*.so"))
        + list((target_dir / "release/deps").glob("libmechanical_mimalloc*.so")),
        key=lambda path: path.stat().st_mtime,
        reverse=True,
    )
    if not candidates:
        raise SystemExit("no release c-abi cdylib artifact found")
    return candidates[0]


def compile_probe(compiler: str, cdylib: pathlib.Path) -> pathlib.Path:
    probe_dir = BUILD_DIR / "probe"
    probe_dir.mkdir(parents=True, exist_ok=True)
    source = probe_dir / "release_thread_stress.c"
    binary = probe_dir / "release_thread_stress"
    source.write_text(PROBE)
    run(
        [
            compiler,
            "-std=c11",
            "-DMI_MALLOC_OVERRIDE=0",
            "-I",
            str(ROOT / "mimalloc-3.3.2/include"),
            str(source),
            str(cdylib),
            "-Wl,-rpath," + str(cdylib.parent),
            "-pthread",
            "-ldl",
            "-lm",
            "-o",
            str(binary),
        ]
    )
    return binary


def run_probe(binary: pathlib.Path, cdylib: pathlib.Path) -> None:
    env = dict(os.environ)
    ld_library_path = str(cdylib.parent)
    if env.get("LD_LIBRARY_PATH"):
        ld_library_path = ld_library_path + os.pathsep + env["LD_LIBRARY_PATH"]
    env["LD_LIBRARY_PATH"] = ld_library_path
    run([str(binary)], env=env)


def main() -> int:
    if sys.platform != "linux":
        raise SystemExit("release thread stress currently requires Linux")

    shutil.rmtree(BUILD_DIR, ignore_errors=True)
    BUILD_DIR.mkdir(parents=True, exist_ok=True)

    cdylib = build_c_abi_cdylib()
    probe = compile_probe(cc(), cdylib)
    run_probe(probe, cdylib)
    print("ok: release c-abi thread stress passed")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except subprocess.CalledProcessError as err:
        sys.stderr.write(err.stdout or "")
        sys.stderr.write(err.stderr or "")
        raise
