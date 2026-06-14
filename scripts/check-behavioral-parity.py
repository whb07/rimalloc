#!/usr/bin/env python3
"""Run the same C behavior smoke scenarios against C mimalloc and Rust mimalloc."""

from __future__ import annotations

import os
import json
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
#include <mimalloc/types.h>

#include <dlfcn.h>
#include <errno.h>
#include <limits.h>
#include <sys/mman.h>
#include <sys/wait.h>
#include <pthread.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <wchar.h>

static int failures = 0;
static size_t output_callbacks = 0;
static size_t registered_output_a = 0;
static size_t registered_output_b = 0;
static size_t registered_error_a = 0;
static size_t registered_error_b = 0;
static int registered_error_a_last = 0;
static int registered_error_b_last = 0;
static size_t reentrant_output_calls = 0;
static int reentrant_output_alloc_ok = 1;
static size_t reentrant_error_calls = 0;
static int reentrant_error_alloc_ok = 1;
static int reentrant_error_last = 0;
static size_t reentrant_deferred_calls = 0;
static int reentrant_deferred_alloc_ok = 1;
static size_t deferred_calls = 0;
static size_t deferred_force_true = 0;
static size_t deferred_force_false = 0;
static int deferred_arg_seen = 0;
static int deferred_heartbeat_monotonic = 1;
static unsigned long long deferred_last_heartbeat = 0;
static size_t arena_output_callbacks = 0;
static int arena_output_seen_arena = 0;
static int arena_output_seen_chunks = 0;
static int arena_output_seen_total = 0;
static size_t stats_output_callbacks = 0;
static int stats_output_seen_heap = 0;
static int stats_output_seen_subproc = 0;
static int stats_output_seen_blocks = 0;
static int stats_output_seen_pages = 0;
static int stats_output_seen_process = 0;
static int stats_output_seen_elapsed = 0;
static int stats_output_seen_default_arg = 0;
static int stats_output_seen_version = 0;
static int stats_output_seen_option = 0;
static int stats_output_seen_debug = 0;
static int stats_output_seen_secure = 0;
static int stats_output_seen_tracking = 0;
static int option_table_seen[_mi_option_last];
static size_t option_table_callbacks = 0;
static size_t option_table_seen_count = 0;
static size_t option_table_kib_count = 0;
static int option_table_arg_seen = 0;
static volatile size_t overflow_count = (size_t)-1;

static const char* option_table_names[_mi_option_last] = {
  "show_errors",
  "show_stats",
  "verbose",
  "deprecated_eager_commit",
  "arena_eager_commit",
  "purge_decommits",
  "allow_large_os_pages",
  "reserve_huge_os_pages",
  "reserve_huge_os_pages_at",
  "reserve_os_memory",
  "deprecated_segment_cache",
  "deprecated_page_reset",
  "deprecated_abandoned_page_purge",
  "deprecated_segment_reset",
  "deprecated_eager_commit_delay",
  "purge_delay",
  "use_numa_nodes",
  "disallow_os_alloc",
  "os_tag",
  "max_errors",
  "max_warnings",
  "deprecated_max_segment_reclaim",
  "destroy_on_exit",
  "arena_reserve",
  "arena_purge_mult",
  "deprecated_purge_extend_delay",
  "disallow_arena_alloc",
  "retry_on_oom",
  "visit_abandoned",
  "guarded_min",
  "guarded_max",
  "guarded_precise",
  "guarded_sample_rate",
  "guarded_sample_seed",
  "generic_collect",
  "page_reclaim_on_free",
  "page_full_retain",
  "page_max_candidates",
  "max_vabits",
  "pagemap_commit",
  "page_commit_on_demand",
  "page_max_reclaim",
  "page_cross_thread_max_reclaim",
  "allow_thp",
  "minimal_purge_size",
  "arena_max_object_size",
  "arena_is_numa_local"
};

typedef void* (*system_malloc_fun)(size_t);
typedef void (*system_free_fun)(void*);

static void output_callback(const char* msg, void* arg) {
  (void)arg;
  if (msg != NULL && msg[0] != '\0') {
    output_callbacks++;
  }
}

static int callback_alloc_roundtrip(size_t size) {
  void* p = mi_malloc(size);
  int ok = (p != NULL && mi_check_owned(p) && mi_usable_size(p) >= size);
  mi_free(p);
  return ok;
}

static void registered_output_callback_a(const char* msg, void* arg) {
  if (msg != NULL && msg[0] != '\0' && arg == (void*)0xA11CE) {
    registered_output_a++;
  }
}

static void registered_output_callback_b(const char* msg, void* arg) {
  if (msg != NULL && msg[0] != '\0' && arg == (void*)0xB0B) {
    registered_output_b++;
  }
}

static void reentrant_output_callback(const char* msg, void* arg) {
  if (msg == NULL || msg[0] == '\0' || arg != (void*)0xCA11) return;
  reentrant_output_calls++;
  if (!callback_alloc_roundtrip(31)) reentrant_output_alloc_ok = 0;
}

static void registered_error_callback_a(int err, void* arg) {
  if (err != 0 && arg == (void*)0xE11A) {
    registered_error_a++;
    registered_error_a_last = err;
  }
}

static void registered_error_callback_b(int err, void* arg) {
  if (err != 0 && arg == (void*)0xE22B) {
    registered_error_b++;
    registered_error_b_last = err;
  }
}

static void reentrant_error_callback(int err, void* arg) {
  if (err == 0 || arg != (void*)0xECA11) return;
  reentrant_error_calls++;
  reentrant_error_last = err;
  if (!callback_alloc_roundtrip(37)) reentrant_error_alloc_ok = 0;
}

static void registered_deferred_callback(bool force, unsigned long long heartbeat, void* arg) {
  deferred_calls++;
  if (force) deferred_force_true++;
  else deferred_force_false++;
  if (arg == (void*)0xDEF) deferred_arg_seen = 1;
  if (heartbeat <= deferred_last_heartbeat) deferred_heartbeat_monotonic = 0;
  deferred_last_heartbeat = heartbeat;
}

static void reentrant_deferred_callback(bool force, unsigned long long heartbeat, void* arg) {
  (void)force;
  (void)heartbeat;
  if (arg != (void*)0xDCA11) return;
  reentrant_deferred_calls++;
  if (!callback_alloc_roundtrip(43)) reentrant_deferred_alloc_ok = 0;
}

static void arena_output_callback(const char* msg, void* arg) {
  if (arg != (void*)0xA4E4A || msg == NULL || msg[0] == '\0') return;
  arena_output_callbacks++;
  if (strstr(msg, "arena ") != NULL && strstr(msg, "slices") != NULL) arena_output_seen_arena = 1;
  if (strstr(msg, "chunks (p:page") != NULL) arena_output_seen_chunks = 1;
  if (strstr(msg, "total pages in arenas:") != NULL) arena_output_seen_total = 1;
}

static void stats_output_reset(void) {
  stats_output_callbacks = 0;
  stats_output_seen_heap = 0;
  stats_output_seen_subproc = 0;
  stats_output_seen_blocks = 0;
  stats_output_seen_pages = 0;
  stats_output_seen_process = 0;
  stats_output_seen_elapsed = 0;
  stats_output_seen_default_arg = 0;
  stats_output_seen_version = 0;
  stats_output_seen_option = 0;
  stats_output_seen_debug = 0;
  stats_output_seen_secure = 0;
  stats_output_seen_tracking = 0;
}

static void stats_output_callback(const char* msg, void* arg) {
  if (msg == NULL || msg[0] == '\0') return;
  stats_output_callbacks++;
  if (arg == (void*)0x57A75) stats_output_seen_default_arg = 1;
  if (strncmp(msg, "heap ", 5) == 0) stats_output_seen_heap = 1;
  if (strncmp(msg, "subproc ", 8) == 0) stats_output_seen_subproc = 1;
  if (strstr(msg, "blocks") != NULL) stats_output_seen_blocks = 1;
  if (strstr(msg, "pages") != NULL) stats_output_seen_pages = 1;
  if (strstr(msg, "process") != NULL) stats_output_seen_process = 1;
  if (strstr(msg, "elapsed") != NULL) stats_output_seen_elapsed = 1;
  if (msg[0] == 'v' && strstr(msg, "3.3.2") != NULL) stats_output_seen_version = 1;
  if (strstr(msg, "option '") != NULL) stats_output_seen_option = 1;
  if (strstr(msg, "debug level") != NULL) stats_output_seen_debug = 1;
  if (strstr(msg, "secure level") != NULL) stats_output_seen_secure = 1;
  if (strstr(msg, "mem tracking") != NULL) stats_output_seen_tracking = 1;
}

static void option_table_output_callback(const char* msg, void* arg) {
  if (msg == NULL || msg[0] == '\0') return;
  option_table_callbacks++;
  if (arg == (void*)0x04710A) option_table_arg_seen = 1;
  if (strncmp(msg, "option '", 8) != 0) return;
  if (strstr(msg, "KiB") != NULL) option_table_kib_count++;
  for (int i = 0; i < _mi_option_last; i++) {
    char expected[96];
    snprintf(expected, sizeof(expected), "option '%s':", option_table_names[i]);
    if (strstr(msg, expected) != NULL) {
      if (!option_table_seen[i]) {
        option_table_seen[i] = 1;
        option_table_seen_count++;
      }
      return;
    }
  }
}

struct managed_commit_result {
  size_t commit_calls;
  size_t decommit_calls;
  size_t commit_bytes;
  int arg_seen;
  int zero_state_seen;
  size_t alloc_calls;
  int alloc_ok;
};

static bool managed_commit_callback(bool commit, void* start, size_t size, bool* is_zero, void* user_arg) {
  struct managed_commit_result* result = (struct managed_commit_result*)user_arg;
  if (result != NULL) {
    if (result->alloc_calls == 0) {
      result->alloc_calls++;
      result->alloc_ok = callback_alloc_roundtrip(53);
    }
    if (commit) {
      result->commit_calls++;
      result->commit_bytes += size;
    }
    else {
      result->decommit_calls++;
    }
    if (user_arg == (void*)result) result->arg_seen = 1;
    if (is_zero != NULL) {
      result->zero_state_seen = 1;
      *is_zero = false;
    }
  }
  (void)start;
  return true;
}

static bool managed_commit_fail_callback(bool commit, void* start, size_t size, bool* is_zero, void* user_arg) {
  struct managed_commit_result* result = (struct managed_commit_result*)user_arg;
  if (result != NULL) {
    if (commit) result->commit_calls++;
    else result->decommit_calls++;
    if (is_zero != NULL) result->zero_state_seen = 1;
  }
  (void)start;
  (void)size;
  (void)is_zero;
  return false;
}

static void* map_aligned_arena_memory(size_t size) {
  size_t alignment = mi_arena_min_alignment();
  void* mapping = mmap(NULL, size + alignment, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
  if (mapping == MAP_FAILED) return MAP_FAILED;
  return (void*)(((uintptr_t)mapping + alignment - 1) & ~(uintptr_t)(alignment - 1));
}

static void result_bool(const char* name, int ok) {
  printf("%s=%d\n", name, ok ? 1 : 0);
  if (!ok) failures++;
}

static int64_t stats_alloc_current(const mi_stats_t* stats) {
  return stats->malloc_normal.current + stats->malloc_huge.current;
}

static int64_t stats_alloc_peak(const mi_stats_t* stats) {
  return stats->malloc_normal.peak + stats->malloc_huge.peak;
}

static int64_t stats_alloc_total(const mi_stats_t* stats) {
  return stats->malloc_normal.total + stats->malloc_huge.total;
}

static int64_t stats_count_current_sum(const mi_stat_count_t* stats, size_t count) {
  int64_t total = 0;
  for (size_t i = 0; i < count; i++) total += stats[i].current;
  return total;
}

static int64_t stats_count_peak_sum(const mi_stat_count_t* stats, size_t count) {
  int64_t total = 0;
  for (size_t i = 0; i < count; i++) total += stats[i].peak;
  return total;
}

static int64_t stats_count_total_sum(const mi_stat_count_t* stats, size_t count) {
  int64_t total = 0;
  for (size_t i = 0; i < count; i++) total += stats[i].total;
  return total;
}

static void result_size(const char* name, size_t value) {
  printf("%s=%zu\n", name, value);
}

static void result_long(const char* name, long value) {
  printf("%s=%ld\n", name, value);
}

static void result_indexed_long(const char* name, size_t index, long value) {
  printf("%s[%zu]=%ld\n", name, index, value);
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

static size_t count_substrings(const char* text, const char* needle) {
  size_t count = 0;
  const char* p = text;
  while (p != NULL && *p != '\0') {
    p = strstr(p, needle);
    if (p == NULL) break;
    count++;
    p += strlen(needle);
  }
  return count;
}

static int json_tail_ok(const char* json) {
  if (json == NULL) return 0;
  size_t len = strlen(json);
  return len >= 3 && json[len - 2] == '}' && json[len - 1] == '\n';
}

static int stats_json_schema_ok(const char* json) {
  if (json == NULL || json[0] != '{' || !json_tail_ok(json)) return 0;
  return strstr(json, "\"stat_version\": 5") != NULL &&
         strstr(json, "\"mimalloc_version\": 30302") != NULL &&
         strstr(json, "\"process\": {") != NULL &&
         strstr(json, "\"elapsed_msecs\"") != NULL &&
         strstr(json, "\"page_faults\"") != NULL &&
         strstr(json, "\"rss_current\"") != NULL &&
         strstr(json, "\"commit_peak\"") != NULL &&
         strstr(json, "\"pages\": {") != NULL &&
         strstr(json, "\"malloc_requested\": {") != NULL &&
         strstr(json, "\"malloc_bins\": [") != NULL &&
         strstr(json, "\"page_bins\": [") != NULL &&
         strstr(json, "\"chunk_bins\": [") != NULL &&
         count_substrings(json, "\"block_size\"") == 2 * (MI_BIN_HUGE + 1) &&
         count_substrings(json, "\"page_size\"") == 2 * (MI_BIN_HUGE + 1) &&
         count_substrings(json, "\"bin\"") == MI_CBIN_COUNT;
}

struct thread_lifecycle_result {
  int init_alloc_owned;
  int threadpool_alloc_owned;
  int after_done_alloc_owned;
};

struct abandoned_pointer_result {
  void* ptr;
};

struct heap_cross_thread_free_result {
  void* ptr;
  int owned_before_free;
};

struct heap_abandoned_result {
  mi_heap_t* heap;
  void* a;
  void* b;
  void* c;
  int allocated;
};

#define STRESS_THREADS 8
#define STRESS_SLOTS 64
#define STRESS_INCOMING 16
#define STRESS_ROUNDS 512
#define STRESS_WAVES 3

struct stress_worker_arg {
  void* incoming[STRESS_INCOMING];
  size_t incoming_count;
  size_t id;
  int use_threadpool;
  int ok;
};

struct theap_wrong_thread_result {
  mi_theap_t* theap;
  int op;
  int ok;
};

struct heap_visit_result {
  const mi_heap_t* expected_heap;
  size_t areas;
  size_t blocks;
  int heap_matches;
  int area_fields_ok;
  int block_fields_ok;
};

struct heap_visit_alloc_result {
  const mi_heap_t* expected_heap;
  size_t calls;
  int heap_matches;
  int alloc_ok;
};

static bool heap_visit_counting_visitor(const mi_heap_t* heap, const mi_heap_area_t* area, void* block, size_t block_size, void* arg) {
  struct heap_visit_result* result = (struct heap_visit_result*)arg;
  if (heap != result->expected_heap) result->heap_matches = 0;
  if (area == NULL || area->blocks == NULL || area->reserved < area->committed || area->full_block_size < area->block_size) {
    result->area_fields_ok = 0;
  }
  if (block == NULL) {
    result->areas++;
  } else {
    result->blocks++;
    if (block_size == 0 || area == NULL || block_size > area->full_block_size) {
      result->block_fields_ok = 0;
    }
  }
  return true;
}

static bool heap_visit_stop_visitor(const mi_heap_t* heap, const mi_heap_area_t* area, void* block, size_t block_size, void* arg) {
  (void)heap;
  (void)area;
  (void)block;
  (void)block_size;
  struct heap_visit_result* result = (struct heap_visit_result*)arg;
  result->areas++;
  return false;
}

static bool heap_visit_allocating_visitor(const mi_heap_t* heap, const mi_heap_area_t* area, void* block, size_t block_size, void* arg) {
  (void)area;
  (void)block;
  (void)block_size;
  struct heap_visit_alloc_result* result = (struct heap_visit_alloc_result*)arg;
  result->calls++;
  if (heap != result->expected_heap) result->heap_matches = 0;
  if (!callback_alloc_roundtrip(47)) result->alloc_ok = 0;
  return true;
}

static void* thread_lifecycle_worker(void* arg) {
  struct thread_lifecycle_result* result = (struct thread_lifecycle_result*)arg;
  mi_thread_init();
  void* p = mi_malloc(32);
  result->init_alloc_owned = (p != NULL && mi_check_owned(p));
  mi_free(p);

  mi_thread_set_in_threadpool();
  void* q = mi_malloc(33);
  result->threadpool_alloc_owned = (q != NULL && mi_check_owned(q));
  mi_free(q);

  mi_thread_done();
  mi_thread_init();
  void* r = mi_malloc(34);
  result->after_done_alloc_owned = (r != NULL && mi_check_owned(r));
  mi_free(r);
  mi_thread_done();
  return NULL;
}

static void* abandoned_pointer_worker(void* arg) {
  struct abandoned_pointer_result* result = (struct abandoned_pointer_result*)arg;
  mi_thread_init();
  result->ptr = mi_malloc(37);
  mi_thread_done();
  return NULL;
}

static void* heap_cross_thread_free_worker(void* arg) {
  struct heap_cross_thread_free_result* result = (struct heap_cross_thread_free_result*)arg;
  mi_thread_init();
  result->owned_before_free = (result->ptr != NULL && mi_check_owned(result->ptr));
  mi_free(result->ptr);
  result->ptr = NULL;
  mi_thread_done();
  return NULL;
}

static void* heap_abandoned_worker(void* arg) {
  struct heap_abandoned_result* result = (struct heap_abandoned_result*)arg;
  mi_thread_init();
  result->a = mi_heap_malloc(result->heap, 80);
  result->b = mi_heap_malloc(result->heap, 160);
  result->c = mi_heap_zalloc(result->heap, 240);
  result->allocated = (result->a != NULL && result->b != NULL && result->c != NULL &&
                       mi_heap_contains(result->heap, result->a) &&
                       mi_heap_contains(result->heap, result->b) &&
                       mi_heap_contains(result->heap, result->c));
  mi_thread_done();
  return NULL;
}

static void* stress_worker(void* arg) {
  struct stress_worker_arg* result = (struct stress_worker_arg*)arg;
  void* slots[STRESS_SLOTS];
  memset(slots, 0, sizeof(slots));
  result->ok = 1;

  mi_thread_init();
  if (result->use_threadpool) {
    mi_thread_set_in_threadpool();
  }

  for (size_t i = 0; i < result->incoming_count; i++) {
    if (result->incoming[i] == NULL || !mi_is_in_heap_region(result->incoming[i])) {
      result->ok = 0;
    }
    mi_free(result->incoming[i]);
    result->incoming[i] = NULL;
  }

  for (size_t round = 0; round < STRESS_ROUNDS; round++) {
    size_t slot = (round * 17 + result->id * 5) % STRESS_SLOTS;
    if (slots[slot] != NULL) {
      if ((round % 3) == 0) {
        size_t new_size = 1 + ((round * 29 + result->id * 13) % 4096);
        void* resized = mi_realloc(slots[slot], new_size);
        if (resized == NULL || !mi_check_owned(resized) || mi_usable_size(resized) < new_size) {
          result->ok = 0;
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
      size_t size = 1 + ((round * 37 + result->id * 19) % 8192);
      void* p = NULL;
      switch (round % 5) {
        case 0:
          p = mi_malloc(size);
          break;
        case 1:
          p = mi_zalloc(size);
          break;
        case 2: {
          size_t alignment = (size_t)16 << ((round + result->id) % 3);
          p = mi_malloc_aligned(size, alignment);
          if (p != NULL && (((uintptr_t)p % alignment) != 0)) {
            result->ok = 0;
          }
          break;
        }
        case 3:
          p = mi_realloc(NULL, size);
          break;
        default: {
          size_t count = 1 + (round % 7);
          size_t elem_size = 1 + (size / count);
          size = count * elem_size;
          p = mi_mallocn(count, elem_size);
          break;
        }
      }
      if (p == NULL || !mi_check_owned(p) || mi_usable_size(p) < size) {
        result->ok = 0;
        mi_free(p);
      }
      else {
        slots[slot] = p;
      }
    }

    if ((round % 97) == 0) {
      mi_collect(false);
    }
  }

  for (size_t i = 0; i < STRESS_SLOTS; i++) {
    mi_free(slots[i]);
  }
  mi_collect(true);
  mi_thread_done();
  return NULL;
}

static void* theap_wrong_thread_worker(void* arg) {
  struct theap_wrong_thread_result* result = (struct theap_wrong_thread_result*)arg;
  mi_thread_init();
  void* p = NULL;
  unsigned char* bytes = NULL;
  switch (result->op) {
    case 0:
      p = mi_theap_malloc(result->theap, 40);
      result->ok = (p != NULL);
      break;
    case 1:
      bytes = (unsigned char*)mi_theap_zalloc(result->theap, 41);
      p = bytes;
      result->ok = (bytes != NULL && bytes_equal(bytes, 0, 41));
      break;
    case 2:
      bytes = (unsigned char*)mi_theap_calloc(result->theap, 3, 17);
      p = bytes;
      result->ok = (bytes != NULL && bytes_equal(bytes, 0, 51));
      break;
    case 3:
      p = mi_theap_malloc_small(result->theap, 42);
      result->ok = (p != NULL);
      break;
    case 4:
      bytes = (unsigned char*)mi_theap_zalloc_small(result->theap, 43);
      p = bytes;
      result->ok = (bytes != NULL && bytes_equal(bytes, 0, 43));
      break;
    case 5:
      p = mi_theap_malloc_aligned(result->theap, 96, 32);
      result->ok = is_aligned_at(p, 32, 0);
      break;
    case 6:
      p = mi_theap_realloc(result->theap, NULL, 44);
      result->ok = (p != NULL);
      break;
    default:
      result->ok = 0;
      break;
  }
  mi_free(p);
  mi_thread_done();
  return NULL;
}

static int theap_wrong_thread_case_status(mi_theap_t* theap, int op) {
  fflush(stdout);
  pid_t pid = fork();
  if (pid < 0) return -1;
  if (pid == 0) {
    FILE* devnull = fopen("/dev/null", "w");
    if (devnull != NULL) {
      (void)dup2(fileno(devnull), fileno(stderr));
    }
    struct theap_wrong_thread_result result = { theap, op, 0 };
    pthread_t thread;
    int started = (pthread_create(&thread, NULL, theap_wrong_thread_worker, &result) == 0);
    int joined = started && pthread_join(thread, NULL) == 0;
    _exit((joined && result.ok) ? 0 : 7);
  }
  int status = 0;
  if (waitpid(pid, &status, 0) < 0) return -2;
  if (WIFSIGNALED(status)) return 128 + WTERMSIG(status);
  if (WIFEXITED(status)) return WEXITSTATUS(status);
  return -3;
}

static int fork_allocation_case_status(void) {
  mi_stats_t parent_before;
  mi_stats_init(&parent_before);
  int parent_before_ok = mi_stats_get(&parent_before);
  void* parent_before_ptr = mi_malloc(128);
  if (!parent_before_ok || parent_before_ptr == NULL || !mi_check_owned(parent_before_ptr)) return 10;

  fflush(stdout);
  pid_t pid = fork();
  if (pid < 0) {
    mi_free(parent_before_ptr);
    return 11;
  }
  if (pid == 0) {
    mi_stats_t child_before;
    mi_stats_init(&child_before);
    int child_before_ok = mi_stats_get(&child_before);
    void* child_ptr = mi_malloc(256);
    int child_alloc_ok = (child_ptr != NULL && mi_check_owned(child_ptr));
    mi_free(child_ptr);
    mi_collect(true);
    mi_stats_t child_after;
    mi_stats_init(&child_after);
    int child_after_ok = mi_stats_get(&child_after);
    void* child_after_ptr = mi_malloc(64);
    int child_after_alloc_ok = (child_after_ptr != NULL && mi_check_owned(child_after_ptr));
    mi_free(child_after_ptr);
    mi_collect(true);
    _exit(child_before_ok && child_alloc_ok && child_after_ok && child_after_alloc_ok ? 0 : 12);
  }

  int status = 0;
  if (waitpid(pid, &status, 0) < 0) {
    mi_free(parent_before_ptr);
    return 13;
  }
  void* parent_after_ptr = mi_malloc(512);
  int parent_after_alloc_ok = (parent_after_ptr != NULL && mi_check_owned(parent_after_ptr));
  mi_free(parent_after_ptr);
  mi_free(parent_before_ptr);
  mi_collect(true);
  mi_stats_t parent_after;
  mi_stats_init(&parent_after);
  int parent_after_ok = mi_stats_get(&parent_after);
  if (WIFSIGNALED(status)) return 128 + WTERMSIG(status);
  if (!WIFEXITED(status) || WEXITSTATUS(status) != 0) return WEXITSTATUS(status);
  return parent_after_alloc_ok && parent_after_ok ? 0 : 14;
}

static int generic_collect_case_status(long threshold, size_t expected_deferred_calls) {
  fflush(stdout);
  pid_t pid = fork();
  if (pid < 0) return -1;
  if (pid == 0) {
    void* ptrs[1500];
    size_t count = 0;
    deferred_calls = 0;
    deferred_force_true = 0;
    deferred_force_false = 0;
    deferred_arg_seen = 0;
    deferred_heartbeat_monotonic = 1;
    deferred_last_heartbeat = 0;
    mi_option_set(mi_option_generic_collect, threshold);
    mi_register_deferred_free(registered_deferred_callback, (void*)0xDEF);

    for (size_t i = 0; i < 1500; i++) {
      ptrs[count] = mi_malloc(65536 + ((i % 8) * 4096));
      if (ptrs[count] == NULL) break;
      count++;
    }
    mi_stats_t live_stats;
    mi_stats_init(&live_stats);
    int stats_ok = (mi_stats_get(&live_stats) &&
                    stats_alloc_current(&live_stats) > 0 &&
                    stats_alloc_peak(&live_stats) >= stats_alloc_current(&live_stats));
    for (size_t i = 0; i < count; i++) {
      mi_free(ptrs[i]);
    }
    mi_register_deferred_free(NULL, NULL);

    int ok = (count == 1500 &&
              stats_ok &&
              deferred_calls == expected_deferred_calls &&
              deferred_force_false == expected_deferred_calls &&
              deferred_force_true == 0 &&
              deferred_arg_seen &&
              deferred_heartbeat_monotonic &&
              deferred_last_heartbeat >= deferred_calls);
    _exit(ok ? 0 : 15);
  }
  int status = 0;
  if (waitpid(pid, &status, 0) < 0) return -2;
  if (WIFSIGNALED(status)) return 128 + WTERMSIG(status);
  if (WIFEXITED(status)) return WEXITSTATUS(status);
  return -3;
}

static int heap_lifecycle_case_status(int op) {
  fflush(stdout);
  pid_t pid = fork();
  if (pid < 0) return -1;
  if (pid == 0) {
    FILE* devnull = fopen("/dev/null", "w");
    if (devnull != NULL) {
      (void)dup2(fileno(devnull), fileno(stderr));
    }
    mi_heap_t* heap = mi_heap_new();
    if (heap == NULL) _exit(31);
    void* p = NULL;
    struct heap_cross_thread_free_result free_result = { NULL, 0 };
    pthread_t thread;
    int started = 0;
    int joined = 0;
    switch (op) {
      case 0:
        p = mi_heap_malloc(heap, 77);
        if (p == NULL || !mi_heap_contains(heap, p)) _exit(32);
        mi_heap_delete(heap);
        if (!mi_check_owned(p) || !mi_heap_contains(mi_heap_main(), p)) _exit(33);
        mi_free(p);
        _exit(0);
      case 1:
        p = mi_heap_malloc(heap, 78);
        if (p == NULL || !mi_heap_contains(heap, p)) _exit(34);
        mi_heap_destroy(heap);
        mi_free(p);
        _exit(0);
      case 2:
        free_result.ptr = mi_heap_malloc(heap, 81);
        if (free_result.ptr == NULL || !mi_heap_contains(heap, free_result.ptr)) _exit(35);
        started = (pthread_create(&thread, NULL, heap_cross_thread_free_worker, &free_result) == 0);
        joined = started && pthread_join(thread, NULL) == 0;
        if (!joined || !free_result.owned_before_free) _exit(36);
        mi_heap_delete(heap);
        p = mi_malloc(82);
        if (p == NULL || !mi_check_owned(p)) _exit(37);
        mi_free(p);
        _exit(0);
      case 3:
        free_result.ptr = mi_heap_malloc(heap, 83);
        if (free_result.ptr == NULL || !mi_heap_contains(heap, free_result.ptr)) _exit(38);
        started = (pthread_create(&thread, NULL, heap_cross_thread_free_worker, &free_result) == 0);
        joined = started && pthread_join(thread, NULL) == 0;
        if (!joined || !free_result.owned_before_free) _exit(39);
        mi_heap_destroy(heap);
        p = mi_malloc(84);
        if (p == NULL || !mi_check_owned(p)) _exit(40);
        mi_free(p);
        _exit(0);
      default:
        mi_heap_delete(heap);
        _exit(41);
    }
  }
  int status = 0;
  if (waitpid(pid, &status, 0) < 0) return -2;
  if (WIFSIGNALED(status)) return 128 + WTERMSIG(status);
  if (WIFEXITED(status)) return WEXITSTATUS(status);
  return -3;
}

static int option_invalid_case_status(int op) {
  fflush(stdout);
  pid_t pid = fork();
  if (pid < 0) return -1;
  if (pid == 0) {
    FILE* devnull = fopen("/dev/null", "w");
    if (devnull != NULL) {
      (void)dup2(fileno(devnull), fileno(stderr));
    }
    mi_option_t invalid_option = (mi_option_t)-1;
    switch (op) {
      case 0:
        { volatile long value = mi_option_get(invalid_option); (void)value; }
        _exit(0);
      case 1:
        mi_option_set(invalid_option, 123);
        _exit(0);
      case 2:
        mi_option_set_default(invalid_option, 456);
        _exit(0);
      case 3:
        { volatile long value = mi_option_get_clamp(invalid_option, 3, 7); (void)value; }
        _exit(0);
      case 4:
        { volatile size_t value = mi_option_get_size(invalid_option); (void)value; }
        _exit(0);
      case 5:
        { volatile bool value = mi_option_is_enabled(invalid_option); (void)value; }
        _exit(0);
      case 6:
        mi_option_enable(invalid_option);
        _exit(0);
      case 7:
        mi_option_disable(invalid_option);
        _exit(0);
      case 8:
        mi_option_set_enabled(invalid_option, true);
        _exit(0);
      case 9:
        mi_option_set_enabled_default(invalid_option, true);
        _exit(0);
      default:
        _exit(41);
    }
  }
  int status = 0;
  if (waitpid(pid, &status, 0) < 0) return -2;
  if (WIFSIGNALED(status)) return 128 + WTERMSIG(status);
  if (WIFEXITED(status)) return WEXITSTATUS(status);
  return -3;
}

static int foreign_size_case_status(int op) {
  fflush(stdout);
  pid_t pid = fork();
  if (pid < 0) return -1;
  if (pid == 0) {
    FILE* devnull = fopen("/dev/null", "w");
    if (devnull != NULL) {
      (void)dup2(fileno(devnull), fileno(stderr));
    }
    int stack_value = 42;
    switch (op) {
      case 0:
        { volatile size_t value = mi_usable_size(&stack_value); (void)value; }
        _exit(0);
      case 1:
        { volatile size_t value = mi_malloc_usable_size(&stack_value); (void)value; }
        _exit(0);
      case 2:
        { volatile size_t value = mi_malloc_size(&stack_value); (void)value; }
        _exit(0);
      default:
        _exit(41);
    }
  }
  int status = 0;
  if (waitpid(pid, &status, 0) < 0) return -2;
  if (WIFSIGNALED(status)) return 128 + WTERMSIG(status);
  if (WIFEXITED(status)) return WEXITSTATUS(status);
  return -3;
}

static int free_small_case_status(int op) {
  fflush(stdout);
  pid_t pid = fork();
  if (pid < 0) return -1;
  if (pid == 0) {
    FILE* devnull = fopen("/dev/null", "w");
    if (devnull != NULL) {
      (void)dup2(fileno(devnull), fileno(stderr));
    }
    switch (op) {
      case 0:
        mi_free_small(NULL);
        _exit(0);
      case 1: {
        void* p = mi_malloc(MI_SMALL_SIZE_MAX + 1);
        if (p == NULL) _exit(10);
        mi_free_small(p);
        _exit(0);
      }
      case 2: {
        void* p = mi_malloc_aligned(64, 64);
        if (p == NULL) _exit(11);
        mi_free_small(p);
        _exit(0);
      }
      default:
        _exit(41);
    }
  }
  int status = 0;
  if (waitpid(pid, &status, 0) < 0) return -2;
  if (WIFSIGNALED(status)) return 128 + WTERMSIG(status);
  if (WIFEXITED(status)) return WEXITSTATUS(status);
  return -3;
}

static int free_hint_case_status(int op) {
  fflush(stdout);
  pid_t pid = fork();
  if (pid < 0) return -1;
  if (pid == 0) {
    FILE* devnull = fopen("/dev/null", "w");
    if (devnull != NULL) {
      (void)dup2(fileno(devnull), fileno(stderr));
    }
    switch (op) {
      case 0: {
        void* p = mi_malloc(32);
        if (p == NULL) _exit(10);
        mi_free_size(p, 1024 * 1024);
        _exit(0);
      }
      case 1: {
        void* p = mi_malloc_aligned_at(32, 64, 32);
        if (p == NULL) _exit(11);
        mi_free_size_aligned(p, 32, 64);
        _exit(0);
      }
      case 2: {
        void* p = mi_malloc_aligned_at(32, 64, 32);
        if (p == NULL) _exit(12);
        mi_free_aligned(p, 64);
        _exit(0);
      }
      default:
        _exit(41);
    }
  }
  int status = 0;
  if (waitpid(pid, &status, 0) < 0) return -2;
  if (WIFSIGNALED(status)) return 128 + WTERMSIG(status);
  if (WIFEXITED(status)) return WEXITSTATUS(status);
  return -3;
}

static int realloc_failure_case_status(int op) {
  fflush(stdout);
  pid_t pid = fork();
  if (pid < 0) return -1;
  if (pid == 0) {
    FILE* devnull = fopen("/dev/null", "w");
    if (devnull != NULL) {
      (void)dup2(fileno(devnull), fileno(stderr));
    }
    unsigned char* p = (unsigned char*)mi_malloc(64);
    if (p == NULL) _exit(10);
    fill_sequence(p, 64);
    switch (op) {
      case 0: {
        void* q = mi_realloc(p, overflow_count);
        if (q != NULL || !has_sequence_prefix(p, 64)) _exit(11);
        void* replacement = mi_malloc(64);
        if (replacement == NULL || replacement == p) _exit(13);
        mi_free(replacement);
        mi_free(p);
        _exit(0);
      }
      case 1: {
        void* q = mi_reallocf(p, overflow_count);
        if (q != NULL) _exit(12);
        volatile size_t old_size = mi_malloc_size(p);
        (void)old_size;
        _exit(0);
      }
      default:
        _exit(41);
    }
  }
  int status = 0;
  if (waitpid(pid, &status, 0) < 0) return -2;
  if (WIFSIGNALED(status)) return 128 + WTERMSIG(status);
  if (WIFEXITED(status)) return WEXITSTATUS(status);
  return -3;
}

static void options_print_with_stderr_suppressed(void) {
  FILE* devnull = fopen("/dev/null", "w");
  if (devnull == NULL) return;
  fflush(stderr);
  int saved = dup(fileno(stderr));
  if (saved < 0) {
    fclose(devnull);
    return;
  }
  if (dup2(fileno(devnull), fileno(stderr)) >= 0) {
    mi_options_print();
    fflush(stderr);
    (void)dup2(saved, fileno(stderr));
  }
  close(saved);
  fclose(devnull);
}

int main(void) {
  mi_option_set_default(mi_option_os_tag, 1234);
  mi_process_init();
  result_long("option_preinit_default_survives_process_init", mi_option_get(mi_option_os_tag));
  result_long("option_preinit_default_get", mi_option_get(mi_option_os_tag));
  mi_option_set(mi_option_os_tag, 5678);
  mi_thread_init();
  result_long("option_postinit_set_survives_thread_init", mi_option_get(mi_option_os_tag));
  mi_option_set_default(mi_option_os_tag, 4321);
  mi_process_init();
  result_long("option_postinit_default_does_not_override", mi_option_get(mi_option_os_tag));
  mi_option_set(mi_option_os_tag, 8765);
  result_long("option_postinit_set_overrides_current", mi_option_get(mi_option_os_tag));
  mi_option_set_default(mi_option_os_tag, 100);
  result_long("option_postinit_default_keeps_current", mi_option_get(mi_option_os_tag));
  mi_option_set(mi_option_os_tag, 100);
  mi_option_disable(mi_option_show_errors);

  result_bool("version_30302", mi_version() == 30302);

  struct thread_lifecycle_result thread_result = {0, 0, 0};
  pthread_t thread;
  int thread_started = (pthread_create(&thread, NULL, thread_lifecycle_worker, &thread_result) == 0);
  result_bool("thread_lifecycle_started", thread_started);
  int thread_joined = thread_started && pthread_join(thread, NULL) == 0;
  result_bool("thread_lifecycle_joined", thread_joined);
  result_bool("thread_init_alloc_owned", thread_joined && thread_result.init_alloc_owned);
  result_bool("threadpool_alloc_owned", thread_joined && thread_result.threadpool_alloc_owned);
  result_bool("thread_after_done_alloc_owned", thread_joined && thread_result.after_done_alloc_owned);

  mi_stats_t stress_before;
  mi_stats_t stress_after;
  mi_stats_t stress_final;
  mi_stats_init(&stress_before);
  mi_stats_init(&stress_after);
  mi_stats_init(&stress_final);
  result_bool("stress_before_ok", mi_stats_get(&stress_before));
  int stress_all_ok = 1;
  for (size_t wave = 0; wave < STRESS_WAVES; wave++) {
    pthread_t stress_threads[STRESS_THREADS];
    struct stress_worker_arg stress_args[STRESS_THREADS];
    size_t stress_started = 0;
    memset(stress_threads, 0, sizeof(stress_threads));
    memset(stress_args, 0, sizeof(stress_args));
    for (size_t t = 0; t < STRESS_THREADS; t++) {
      stress_args[t].id = t + (wave * STRESS_THREADS);
      stress_args[t].incoming_count = STRESS_INCOMING;
      stress_args[t].use_threadpool = ((t + wave) % 2) == 0;
      stress_args[t].ok = 0;
      for (size_t i = 0; i < STRESS_INCOMING; i++) {
        size_t size = 1 + (((wave + 1) * (t + 3) * (i + 5)) % 2048);
        stress_args[t].incoming[i] = mi_malloc(size);
        if (stress_args[t].incoming[i] == NULL || !mi_check_owned(stress_args[t].incoming[i])) {
          stress_all_ok = 0;
        }
      }
      if (pthread_create(&stress_threads[t], NULL, stress_worker, &stress_args[t]) == 0) {
        stress_started++;
      }
      else {
        stress_all_ok = 0;
        for (size_t i = 0; i < stress_args[t].incoming_count; i++) {
          mi_free(stress_args[t].incoming[i]);
          stress_args[t].incoming[i] = NULL;
        }
      }
    }
    result_bool("stress_wave_threads_started", stress_started == STRESS_THREADS);
    for (size_t t = 0; t < stress_started; t++) {
      if (pthread_join(stress_threads[t], NULL) != 0 || !stress_args[t].ok) {
        stress_all_ok = 0;
      }
    }
    result_bool("stress_wave_workers_ok", stress_all_ok);
    mi_collect(true);
  }
  result_bool("stress_after_ok", mi_stats_get(&stress_after));
  mi_collect(true);
  result_bool("stress_final_ok", mi_stats_get(&stress_final));
  result_bool("stress_workers_ok", stress_all_ok);
  result_bool("stress_current_nonnegative",
              stats_alloc_current(&stress_after) >= 0 &&
              stats_alloc_current(&stress_final) >= 0);
  result_bool("stress_peak_consistent",
              stats_alloc_peak(&stress_after) >= stats_alloc_current(&stress_after) &&
              stats_alloc_peak(&stress_final) >= stats_alloc_current(&stress_final));
  result_bool("stress_total_monotonic",
              stats_alloc_total(&stress_after) >= stats_alloc_total(&stress_before) &&
              stats_alloc_total(&stress_final) >= stats_alloc_total(&stress_after));
  result_bool("stress_current_not_increased_after_collect",
              stats_alloc_current(&stress_final) <= stats_alloc_current(&stress_after));

  result_bool("fork_alloc_parent_child_status", fork_allocation_case_status() == 0);

  result_bool("option_alias_large_os_pages", mi_option_large_os_pages == mi_option_allow_large_os_pages);
  result_bool("option_alias_eager_region_commit", mi_option_eager_region_commit == mi_option_arena_eager_commit);
  result_bool("option_alias_reset_decommits", mi_option_reset_decommits == mi_option_purge_decommits);
  result_bool("option_alias_reset_delay", mi_option_reset_delay == mi_option_purge_delay);
  result_bool("option_alias_limit_os_alloc", mi_option_limit_os_alloc == mi_option_disallow_os_alloc);
  for (int i = 0; i < _mi_option_last; i++) {
    mi_option_t option = (mi_option_t)i;
    long value = mi_option_get(option);
    result_indexed_long("option_default", (size_t)i, value);
    result_indexed_size("option_default_size", (size_t)i, mi_option_get_size(option));
    result_bool("option_enabled_matches_default", mi_option_is_enabled(option) == (value != 0));
  }
  memset(option_table_seen, 0, sizeof(option_table_seen));
  option_table_callbacks = 0;
  option_table_seen_count = 0;
  option_table_kib_count = 0;
  option_table_arg_seen = 0;
  mi_options_print_out(option_table_output_callback, (void*)0x04710A);
  result_bool("option_table_output_arg", option_table_arg_seen);
  result_bool("option_table_output_all_names", option_table_seen_count == (size_t)_mi_option_last);
  result_bool("option_table_output_kib_options", option_table_kib_count == 4);
  result_bool("option_table_output_has_extra_lines", option_table_callbacks > option_table_seen_count);

  long original_verbose = mi_option_get(mi_option_verbose);
  mi_option_disable(mi_option_verbose);
  result_bool("option_disable", !mi_option_is_enabled(mi_option_verbose));
  result_size("option_get_clamp_disabled", (size_t)mi_option_get_clamp(mi_option_verbose, 3, 7));
  mi_option_enable(mi_option_verbose);
  result_bool("option_enable", mi_option_is_enabled(mi_option_verbose));
  mi_option_set(mi_option_verbose, 5);
  result_size("option_get_set", (size_t)mi_option_get(mi_option_verbose));
  result_size("option_get_clamp_mid", (size_t)mi_option_get_clamp(mi_option_verbose, 3, 7));
  result_size("option_get_clamp_min", (size_t)mi_option_get_clamp(mi_option_verbose, 6, 7));
  result_size("option_get_size", mi_option_get_size(mi_option_verbose));
  mi_option_set_enabled(mi_option_verbose, false);
  result_size("option_set_enabled_false", (size_t)mi_option_get(mi_option_verbose));
  mi_option_set_enabled_default(mi_option_verbose, true);
  mi_option_set_default(mi_option_verbose, original_verbose);
  mi_option_set(mi_option_verbose, original_verbose);

  long original_purge_delay = mi_option_get(mi_option_purge_delay);
  mi_option_set(mi_option_purge_delay, -1);
  result_long("option_get_clamp_negative_to_min", mi_option_get_clamp(mi_option_purge_delay, 0, 100));
  result_long("option_get_clamp_negative_in_range", mi_option_get_clamp(mi_option_purge_delay, -5, 100));
  result_size("option_get_size_negative_zero", mi_option_get_size(mi_option_purge_delay));
  mi_option_set(mi_option_purge_delay, 1000);
  result_long("option_get_clamp_max", mi_option_get_clamp(mi_option_purge_delay, 0, 250));
  result_size("option_get_size_plain_bytes", mi_option_get_size(mi_option_purge_delay));
  mi_option_set(mi_option_purge_delay, original_purge_delay);

  long original_reserve_os_memory = mi_option_get(mi_option_reserve_os_memory);
  mi_option_set(mi_option_reserve_os_memory, 3);
  result_size("option_get_size_kib_bytes", mi_option_get_size(mi_option_reserve_os_memory));
  mi_option_set(mi_option_reserve_os_memory, -7);
  result_size("option_get_size_kib_negative_zero", mi_option_get_size(mi_option_reserve_os_memory));
  mi_option_set(mi_option_reserve_os_memory, LONG_MAX);
  result_size("option_get_size_kib_large_clamped", mi_option_get_size(mi_option_reserve_os_memory));
  mi_option_set(mi_option_reserve_os_memory, original_reserve_os_memory);

  result_size("option_invalid_get_status", (size_t)option_invalid_case_status(0));
  result_size("option_invalid_set_status", (size_t)option_invalid_case_status(1));
  result_size("option_invalid_set_default_status", (size_t)option_invalid_case_status(2));
  result_size("option_invalid_get_clamp_status", (size_t)option_invalid_case_status(3));
  result_size("option_invalid_get_size_status", (size_t)option_invalid_case_status(4));
  result_size("option_invalid_is_enabled_status", (size_t)option_invalid_case_status(5));
  result_size("option_invalid_enable_status", (size_t)option_invalid_case_status(6));
  result_size("option_invalid_disable_status", (size_t)option_invalid_case_status(7));
  result_size("option_invalid_set_enabled_status", (size_t)option_invalid_case_status(8));
  result_size("option_invalid_set_enabled_default_status", (size_t)option_invalid_case_status(9));

  registered_output_a = 0;
  registered_output_b = 0;
  mi_register_output(registered_output_callback_a, (void*)0xA11CE);
  mi_options_print();
  result_bool("register_output_a_called", registered_output_a > 0);
  mi_register_output(registered_output_callback_b, (void*)0xB0B);
  mi_options_print();
  result_bool("register_output_b_called", registered_output_b > 0);
  mi_register_output(NULL, NULL);
  size_t output_after_clear = registered_output_b;
  options_print_with_stderr_suppressed();
  result_bool("register_output_clear", registered_output_b == output_after_clear);

  stats_output_reset();
  mi_options_print_out(stats_output_callback, (void*)0x57A75);
  result_bool("options_print_out_callback_nonempty", stats_output_callbacks > 0);
  result_bool("options_print_out_callback_arg", stats_output_seen_default_arg);
  result_bool("options_print_out_structure", stats_output_seen_version && stats_output_seen_option && stats_output_seen_debug && stats_output_seen_secure && stats_output_seen_tracking);
  result_bool("options_print_out_chunking", stats_output_callbacks > 4);

  stats_output_reset();
  mi_register_output(stats_output_callback, (void*)0x57A75);
  mi_options_print();
  result_bool("options_print_registered_output_nonempty", stats_output_callbacks > 0);
  result_bool("options_print_registered_output_arg", stats_output_seen_default_arg);
  result_bool("options_print_registered_output_structure", stats_output_seen_version && stats_output_seen_option && stats_output_seen_debug && stats_output_seen_secure && stats_output_seen_tracking);
  mi_register_output(NULL, NULL);

  reentrant_output_calls = 0;
  reentrant_output_alloc_ok = 1;
  mi_register_output(reentrant_output_callback, (void*)0xCA11);
  mi_options_print();
  result_bool("output_callback_alloc_called", reentrant_output_calls > 0);
  result_bool("output_callback_alloc_ok", reentrant_output_alloc_ok);
  mi_register_output(NULL, NULL);

  registered_error_a = 0;
  registered_error_b = 0;
  mi_register_error(registered_error_callback_a, (void*)0xE11A);
  volatile void* err_alloc = mi_calloc(overflow_count, 2);
  (void)err_alloc;
  result_bool("register_error_a_called", registered_error_a > 0);
  result_size("register_error_a_code", (size_t)registered_error_a_last);
  mi_register_error(registered_error_callback_b, (void*)0xE22B);
  err_alloc = mi_calloc(overflow_count, 2);
  (void)err_alloc;
  result_bool("register_error_b_called", registered_error_b > 0);
  result_size("register_error_b_code", (size_t)registered_error_b_last);
  mi_register_error(NULL, NULL);
  size_t error_after_clear = registered_error_b;
  int error_code_after_clear = registered_error_b_last;
  err_alloc = mi_calloc(overflow_count, 2);
  (void)err_alloc;
  result_bool("register_error_clear", registered_error_b == error_after_clear);
  result_bool("register_error_clear_code", registered_error_b_last == error_code_after_clear);

  reentrant_error_calls = 0;
  reentrant_error_alloc_ok = 1;
  mi_register_error(reentrant_error_callback, (void*)0xECA11);
  err_alloc = mi_calloc(overflow_count, 2);
  (void)err_alloc;
  result_bool("error_callback_alloc_called", reentrant_error_calls > 0);
  result_bool("error_callback_alloc_ok", reentrant_error_alloc_ok);
  result_size("error_callback_code", (size_t)reentrant_error_last);
  mi_register_error(NULL, NULL);

  deferred_calls = 0;
  deferred_force_true = 0;
  deferred_force_false = 0;
  deferred_arg_seen = 0;
  deferred_heartbeat_monotonic = 1;
  deferred_last_heartbeat = 0;
  mi_register_deferred_free(registered_deferred_callback, (void*)0xDEF);
  mi_collect(false);
  mi_collect(true);
  result_bool("deferred_free_called", deferred_calls >= 2);
  result_bool("deferred_free_force_false", deferred_force_false > 0);
  result_bool("deferred_free_force_true", deferred_force_true > 0);
  result_bool("deferred_free_arg", deferred_arg_seen);
  result_bool("deferred_free_heartbeat", deferred_heartbeat_monotonic && deferred_last_heartbeat >= deferred_calls);
  mi_register_deferred_free(NULL, NULL);
  size_t deferred_after_clear = deferred_calls;
  mi_collect(true);
  result_bool("deferred_free_clear", deferred_calls == deferred_after_clear);

  reentrant_deferred_calls = 0;
  reentrant_deferred_alloc_ok = 1;
  mi_register_deferred_free(reentrant_deferred_callback, (void*)0xDCA11);
  mi_collect(false);
  mi_collect(true);
  result_bool("deferred_callback_alloc_called", reentrant_deferred_calls > 0);
  result_bool("deferred_callback_alloc_ok", reentrant_deferred_alloc_ok);
  mi_register_deferred_free(NULL, NULL);

  result_long("generic_collect_threshold_1000_status", generic_collect_case_status(1000, 2));
  result_long("generic_collect_threshold_2000_status", generic_collect_case_status(2000, 1));

  errno = 0;
  void* p = mi_malloc(17);
  result_bool("malloc_nonnull", p != NULL);
  result_bool("malloc_owned", p != NULL && mi_is_in_heap_region(p));
  result_bool("malloc_usable_ge_17", p != NULL && mi_usable_size(p) >= 17);
  result_bool("malloc_errno_unchanged", errno == 0);
  result_bool("owned_live_check_owned", p != NULL && mi_check_owned(p));
  result_bool("owned_live_any_heap_contains", p != NULL && mi_any_heap_contains(p));
  result_bool("owned_live_heap_of_nonnull", p != NULL && mi_heap_of(p) != NULL);
  result_bool("owned_live_heap_contains", p != NULL && mi_heap_contains(mi_heap_of(p), p));
  mi_free(p);
  result_bool("owned_freed_check_owned", mi_check_owned(p));
  result_bool("owned_freed_any_heap_contains", mi_any_heap_contains(p));

  mi_stats_t free_plain_before;
  mi_stats_t free_plain_live;
  mi_stats_t free_plain_after;
  mi_stats_init(&free_plain_before);
  mi_stats_init(&free_plain_live);
  mi_stats_init(&free_plain_after);
  result_bool("free_plain_before_ok", mi_stats_get(&free_plain_before));
  mi_free(NULL);
  result_bool("free_plain_null_ok", 1);
  void* free_plain = mi_malloc(211);
  size_t free_plain_usable = free_plain == NULL ? 0 : mi_usable_size(free_plain);
  result_bool("free_plain_allocated", free_plain != NULL && free_plain_usable >= 211);
  result_bool("free_plain_live_ok", mi_stats_get(&free_plain_live));
  result_bool("free_plain_live_increased",
              stats_alloc_current(&free_plain_live) >= stats_alloc_current(&free_plain_before) &&
              stats_alloc_total(&free_plain_live) >= stats_alloc_total(&free_plain_before));
  mi_free(free_plain);
  result_bool("free_plain_after_ok", mi_stats_get(&free_plain_after));
  result_bool("free_plain_current_restored",
              stats_alloc_current(&free_plain_after) <= stats_alloc_current(&free_plain_live));
  result_bool("free_plain_totals_persistent",
              stats_alloc_total(&free_plain_after) >= stats_alloc_total(&free_plain_live));

  mi_stats_t free_family_before;
  mi_stats_t free_family_live;
  mi_stats_t free_family_after;
  mi_stats_init(&free_family_before);
  mi_stats_init(&free_family_live);
  mi_stats_init(&free_family_after);
  result_bool("free_family_before_ok", mi_stats_get(&free_family_before));
  long free_family_page_size_long = sysconf(_SC_PAGESIZE);
  size_t free_family_page_size = free_family_page_size_long > 0 ? (size_t)free_family_page_size_long : 4096;
  void* free_family[16];
  size_t free_family_requested[16];
  size_t free_family_count = 0;
  free_family_requested[free_family_count] = 23;
  free_family[free_family_count++] = mi_malloc(23);
  free_family_requested[free_family_count] = 24;
  free_family[free_family_count++] = mi_zalloc(24);
  free_family_requested[free_family_count] = 25;
  free_family[free_family_count++] = mi_calloc(5, 5);
  free_family_requested[free_family_count] = 27;
  free_family[free_family_count++] = mi_mallocn(3, 9);
  free_family_requested[free_family_count] = 29;
  free_family[free_family_count++] = mi_realloc(NULL, 29);
  free_family_requested[free_family_count] = 31;
  free_family[free_family_count++] = mi_rezalloc(NULL, 31);
  free_family_requested[free_family_count] = 32;
  free_family[free_family_count++] = mi_recalloc(NULL, 4, 8);
  free_family_requested[free_family_count] = 33;
  free_family[free_family_count++] = mi_malloc_aligned(33, 32);
  free_family_requested[free_family_count] = 34;
  free_family[free_family_count++] = mi_malloc_aligned_at(34, 64, 16);
  free_family_requested[free_family_count] = 35;
  free_family[free_family_count++] = mi_zalloc_aligned(35, 32);
  free_family_requested[free_family_count] = 36;
  free_family[free_family_count++] = mi_calloc_aligned(6, 6, 64);
  void* free_family_posix = NULL;
  result_bool("free_family_posix_memalign_ok", mi_posix_memalign(&free_family_posix, 64, 37) == 0);
  free_family_requested[free_family_count] = 37;
  free_family[free_family_count++] = free_family_posix;
  free_family_requested[free_family_count] = 38;
  free_family[free_family_count++] = mi_memalign(128, 38);
  free_family_requested[free_family_count] = 39;
  free_family[free_family_count++] = mi_valloc(39);
  free_family_requested[free_family_count] = free_family_page_size + 1;
  free_family[free_family_count++] = mi_pvalloc(free_family_page_size + 1);
  free_family_requested[free_family_count] = 64;
  free_family[free_family_count++] = mi_aligned_alloc(64, 64);
  for (size_t i = 0; i < free_family_count; i++) {
    result_bool("free_family_allocated", free_family[i] != NULL && mi_check_owned(free_family[i]));
    result_bool("free_family_usable_ge_request",
                free_family[i] != NULL && mi_usable_size(free_family[i]) >= free_family_requested[i]);
  }
  result_bool("free_family_live_ok", mi_stats_get(&free_family_live));
  result_bool("free_family_live_increased",
              stats_alloc_current(&free_family_live) >= stats_alloc_current(&free_family_before) &&
              stats_alloc_total(&free_family_live) >= stats_alloc_total(&free_family_before));
  for (size_t i = 0; i < free_family_count; i++) {
    mi_free(free_family[i]);
  }
  result_bool("free_family_after_ok", mi_stats_get(&free_family_after));
  result_bool("free_family_current_restored",
              stats_alloc_current(&free_family_after) <= stats_alloc_current(&free_family_live));
  result_bool("free_family_totals_persistent",
              stats_alloc_total(&free_family_after) >= stats_alloc_total(&free_family_live));

  int stack_value = 42;
  result_bool("owned_stack_check_owned", !mi_check_owned(&stack_value));
  result_bool("owned_stack_any_heap_contains", !mi_any_heap_contains(&stack_value));
  result_bool("owned_stack_heap_of_null", mi_heap_of(&stack_value) == NULL);
  result_bool("owned_stack_not_in_region", !mi_is_in_heap_region(&stack_value));

  system_malloc_fun system_malloc = (system_malloc_fun)dlsym(RTLD_NEXT, "malloc");
  system_free_fun system_free = (system_free_fun)dlsym(RTLD_NEXT, "free");
  void* foreign_malloc = (system_malloc == NULL ? NULL : system_malloc(41));
  result_bool("owned_foreign_malloc_allocated", foreign_malloc != NULL && system_free != NULL);
  if (foreign_malloc != NULL && system_free != NULL) {
    result_bool("owned_foreign_malloc_check_owned", !mi_check_owned(foreign_malloc));
    result_bool("owned_foreign_malloc_not_in_region", !mi_is_in_heap_region(foreign_malloc));
    system_free(foreign_malloc);
  } else {
    result_bool("owned_foreign_malloc_check_owned", 0);
    result_bool("owned_foreign_malloc_not_in_region", 0);
  }

  mi_cfree(NULL);
  result_bool("cfree_null_ok", 1);
  void* cfree_owned = mi_malloc(19);
  result_bool("cfree_owned_region", cfree_owned != NULL && mi_is_in_heap_region(cfree_owned));
  mi_cfree(cfree_owned);
  void* cfree_foreign = mmap(NULL, 4096, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
  result_bool("cfree_foreign_allocated", cfree_foreign != MAP_FAILED);
  if (cfree_foreign != MAP_FAILED) {
    result_bool("cfree_foreign_not_in_heap", !mi_is_in_heap_region(cfree_foreign));
    mi_cfree(cfree_foreign);
    result_bool("cfree_foreign_unmap_ok", munmap(cfree_foreign, 4096) == 0);
  } else {
    result_bool("cfree_foreign_not_in_heap", 0);
    result_bool("cfree_foreign_unmap_ok", 0);
  }

  void* owned_aligned = mi_malloc_aligned(55, 64);
  result_bool("owned_aligned_check_owned", owned_aligned != NULL && mi_check_owned(owned_aligned));
  result_bool("owned_aligned_any_heap_contains", owned_aligned != NULL && mi_any_heap_contains(owned_aligned));
  result_bool("owned_aligned_heap_of_nonnull", owned_aligned != NULL && mi_heap_of(owned_aligned) != NULL);
  mi_free(owned_aligned);

  mi_free_size(NULL, 0);
  mi_free_size_aligned(NULL, 0, 1);
  mi_free_aligned(NULL, 1);
  result_bool("free_hint_null_ok", 1);
  mi_stats_t free_hint_before;
  mi_stats_init(&free_hint_before);
  int free_hint_before_ok = mi_stats_get(&free_hint_before);
  void* free_sized = mi_malloc(73);
  size_t free_sized_usable = (free_sized == NULL ? 0 : mi_usable_size(free_sized));
  result_bool("free_size_allocated", free_sized != NULL && free_sized_usable >= 73);
  mi_free_size(free_sized, 73);
  void* free_aligned = mi_malloc_aligned(74, 64);
  size_t free_aligned_usable = (free_aligned == NULL ? 0 : mi_usable_size(free_aligned));
  result_bool("free_aligned_allocated", is_aligned_at(free_aligned, 64, 0) && free_aligned_usable >= 74);
  mi_free_aligned(free_aligned, 64);
  void* free_size_aligned = mi_malloc_aligned(75, 128);
  size_t free_size_aligned_usable = (free_size_aligned == NULL ? 0 : mi_usable_size(free_size_aligned));
  result_bool("free_size_aligned_allocated", is_aligned_at(free_size_aligned, 128, 0) && free_size_aligned_usable >= 75);
  mi_free_size_aligned(free_size_aligned, 75, 128);
  result_size("free_hint_oversize_status", (size_t)free_hint_case_status(0));
  result_size("free_hint_size_aligned_wrong_alignment_status", (size_t)free_hint_case_status(1));
  result_size("free_hint_aligned_wrong_alignment_status", (size_t)free_hint_case_status(2));
  mi_collect(true);
  mi_stats_t free_hint_after;
  mi_stats_init(&free_hint_after);
  int free_hint_after_ok = mi_stats_get(&free_hint_after);
  result_bool("free_hint_stats_success", free_hint_before_ok && free_hint_after_ok);
  result_bool("free_hint_stats_current_restored",
              !free_hint_before_ok || !free_hint_after_ok ||
              stats_alloc_current(&free_hint_after) <= stats_alloc_current(&free_hint_before));

  struct abandoned_pointer_result abandoned_result = {NULL};
  pthread_t abandoned_thread;
  int abandoned_started = (pthread_create(&abandoned_thread, NULL, abandoned_pointer_worker, &abandoned_result) == 0);
  int abandoned_joined = abandoned_started && pthread_join(abandoned_thread, NULL) == 0;
  result_bool("owned_abandoned_thread_joined", abandoned_joined);
  result_bool("owned_abandoned_check_owned", abandoned_joined && abandoned_result.ptr != NULL && mi_check_owned(abandoned_result.ptr));
  result_bool("owned_abandoned_in_region", abandoned_joined && abandoned_result.ptr != NULL && mi_is_in_heap_region(abandoned_result.ptr));
  mi_free(abandoned_result.ptr);

  unsigned char* c = (unsigned char*)mi_calloc(8, 4);
  result_bool("calloc_nonnull", c != NULL);
  result_bool("calloc_zeroed", c != NULL && bytes_equal(c, 0, 32));
  result_bool("calloc_usable_size", c != NULL && mi_usable_size(c) >= 32);
  mi_free(c);

  unsigned char* c_zero_count = (unsigned char*)mi_calloc(0, 32);
  result_bool("calloc_zero_count_nonnull", c_zero_count != NULL);
  result_size("calloc_zero_count_usable", c_zero_count == NULL ? 0 : mi_usable_size(c_zero_count));
  mi_free(c_zero_count);

  unsigned char* c_zero_size = (unsigned char*)mi_calloc(32, 0);
  result_bool("calloc_zero_size_nonnull", c_zero_size != NULL);
  result_size("calloc_zero_size_usable", c_zero_size == NULL ? 0 : mi_usable_size(c_zero_size));
  mi_free(c_zero_size);

  result_bool("calloc_overflow_null", mi_calloc(overflow_count, 2) == NULL);

  unsigned char* c_large = (unsigned char*)mi_calloc(1024, 1024);
  result_bool("calloc_large_nonnull", c_large != NULL);
  result_bool("calloc_large_zeroed", c_large != NULL && bytes_equal(c_large, 0, 1024 * 1024));
  result_bool("calloc_large_usable_size", c_large != NULL && mi_usable_size(c_large) >= 1024 * 1024);
  mi_free(c_large);

  size_t zalloc_sizes[] = { 0, 1, 17, MI_SMALL_SIZE_MAX - 1, MI_SMALL_SIZE_MAX, MI_SMALL_SIZE_MAX + 1 };
  size_t zalloc_size_count = sizeof(zalloc_sizes) / sizeof(zalloc_sizes[0]);
  for (size_t i = 0; i < zalloc_size_count; i++) {
    size_t requested = zalloc_sizes[i];
    unsigned char* zp = (unsigned char*)mi_zalloc(requested);
    result_bool("zalloc_nonnull", zp != NULL);
    result_bool("zalloc_zeroed", zp != NULL && bytes_equal(zp, 0, requested));
    result_indexed_size("zalloc_usable_size", i, zp == NULL ? 0 : mi_usable_size(zp));
    mi_free(zp);
  }

  size_t zalloc_small_sizes[] = { 0, 1, MI_SMALL_SIZE_MAX - 1, MI_SMALL_SIZE_MAX };
  size_t zalloc_small_size_count = sizeof(zalloc_small_sizes) / sizeof(zalloc_small_sizes[0]);
  for (size_t i = 0; i < zalloc_small_size_count; i++) {
    size_t requested = zalloc_small_sizes[i];
    unsigned char* zp = (unsigned char*)mi_zalloc_small(requested);
    result_bool("zalloc_small_nonnull", zp != NULL);
    result_bool("zalloc_small_zeroed", zp != NULL && bytes_equal(zp, 0, requested));
    result_bool("zalloc_small_usable_ge_request", zp != NULL && mi_usable_size(zp) >= requested);
    result_indexed_size("zalloc_small_usable_size", i, zp == NULL ? 0 : mi_usable_size(zp));
    mi_free(zp);
  }

  mi_stats_t small_stats_before;
  mi_stats_t small_stats_live;
  mi_stats_t small_stats_after;
  mi_stats_init(&small_stats_before);
  mi_stats_init(&small_stats_live);
  mi_stats_init(&small_stats_after);
  result_bool("malloc_small_stats_before_ok", mi_stats_get(&small_stats_before));

  void* small_ptrs[MI_BIN_HUGE + 1];
  size_t small_requests[MI_BIN_HUGE + 1];
  size_t small_bin_count = 0;
  for (size_t bin = 0; bin <= MI_BIN_HUGE; bin++) {
    size_t bin_size = mi_stats_get_bin_size(bin);
    if (bin_size == 0 || bin_size > MI_SMALL_SIZE_MAX) continue;
    void* sp = mi_malloc_small(bin_size);
    small_ptrs[small_bin_count] = sp;
    small_requests[small_bin_count] = bin_size;
    result_bool("malloc_small_bin_nonnull", sp != NULL);
    result_bool("malloc_small_bin_usable", sp != NULL && mi_usable_size(sp) >= bin_size);
    result_bool("malloc_small_bin_owned", sp != NULL && mi_check_owned(sp));
    result_indexed_size("malloc_small_bin_request", small_bin_count, bin_size);
    result_indexed_size("malloc_small_bin_usable_size", small_bin_count, sp == NULL ? 0 : mi_usable_size(sp));
    small_bin_count++;
  }
  result_size("malloc_small_bin_count", small_bin_count);
  result_bool("malloc_small_stats_live_ok", mi_stats_get(&small_stats_live));
  result_bool("malloc_small_stats_live_increased",
              small_stats_live.malloc_normal.current >= small_stats_before.malloc_normal.current &&
              small_stats_live.malloc_normal.peak >= small_stats_live.malloc_normal.current &&
              small_stats_live.malloc_normal.total >= small_stats_before.malloc_normal.total);

  for (size_t i = 0; i < small_bin_count; i++) {
    if (small_ptrs[i] != NULL) {
      memset(small_ptrs[i], 0xB6, small_requests[i]);
    }
    mi_free_small(small_ptrs[i]);
  }
  result_bool("malloc_small_stats_after_ok", mi_stats_get(&small_stats_after));
  result_bool("malloc_small_stats_after_decreased",
              small_stats_after.malloc_normal.current <= small_stats_live.malloc_normal.current &&
              small_stats_after.malloc_normal.peak >= small_stats_live.malloc_normal.current &&
              small_stats_after.malloc_normal.total >= small_stats_live.malloc_normal.total);
  result_size("free_small_null_status", (size_t)free_small_case_status(0));
  result_size("free_small_oversize_status", (size_t)free_small_case_status(1));
  result_size("free_small_aligned_status", (size_t)free_small_case_status(2));

  unsigned char* mn = (unsigned char*)mi_mallocn(5, 9);
  result_bool("mallocn_nonnull", mn != NULL);
  result_bool("mallocn_size", mn != NULL && mi_usable_size(mn) >= 45);
  mi_free(mn);
  result_bool("mallocn_overflow_null", mi_mallocn(overflow_count, 2) == NULL);

  unsigned char* r = (unsigned char*)mi_malloc(8);
  if (r != NULL) memset(r, 0x5A, 8);
  r = (unsigned char*)mi_realloc(r, 32);
  result_bool("realloc_nonnull", r != NULL);
  result_bool("realloc_preserved_prefix", r != NULL && bytes_equal(r, 0x5A, 8));
  mi_free(r);
  unsigned char* rnz = (unsigned char*)mi_realloc(NULL, 40);
  result_bool("realloc_null_allocates", rnz != NULL && mi_usable_size(rnz) >= 40);
  mi_free(rnz);
  unsigned char* rg = (unsigned char*)mi_malloc(33);
  fill_sequence(rg, 33);
  size_t rg_usable = mi_usable_size(rg);
  unsigned char* rg2 = (unsigned char*)mi_realloc(rg, rg_usable);
  result_bool("realloc_fit_in_place", rg2 == rg && has_sequence_prefix(rg2, 33) && mi_usable_size(rg2) >= rg_usable);
  mi_free(rg2);
  unsigned char* rs = (unsigned char*)mi_malloc(64);
  fill_sequence(rs, 64);
  unsigned char* rs2 = (unsigned char*)mi_realloc(rs, 48);
  result_bool("realloc_shrink_in_place", rs2 == rs && has_sequence_prefix(rs2, 48) && mi_usable_size(rs2) >= 48);
  mi_free(rs2);
  void* realloc_zero = mi_realloc(NULL, 0);
  result_bool("realloc_null_zero_allocates", realloc_zero != NULL);
  result_bool("realloc_null_zero_usable", realloc_zero != NULL && mi_usable_size(realloc_zero) == mi_malloc_size(realloc_zero));
  mi_free(realloc_zero);
  unsigned char* rz_nonnull = (unsigned char*)mi_malloc(16);
  fill_sequence(rz_nonnull, 16);
  rz_nonnull = (unsigned char*)mi_realloc(rz_nonnull, 0);
  result_bool("realloc_nonnull_zero_allocates", rz_nonnull != NULL && mi_usable_size(rz_nonnull) == mi_malloc_size(rz_nonnull));
  mi_free(rz_nonnull);
  unsigned char* rh = (unsigned char*)mi_malloc(1024);
  fill_sequence(rh, 1024);
  unsigned char* rh_old = rh;
  rh = (unsigned char*)mi_realloc(rh, 3*1024*1024);
  result_bool("realloc_huge_grow_preserve", rh != NULL && has_sequence_prefix(rh, 1024) && mi_usable_size(rh) >= 3*1024*1024);
  result_bool("realloc_huge_grow_moves", rh != NULL && rh != rh_old);
  rh = (unsigned char*)mi_realloc(rh, 512);
  result_bool("realloc_huge_shrink_preserve", rh != NULL && has_sequence_prefix(rh, 512) && mi_usable_size(rh) >= 512);
  mi_free(rh);
  unsigned char* rf = (unsigned char*)mi_malloc(64);
  fill_sequence(rf, 64);
  errno = 0;
  void* rf_failed = mi_realloc(rf, overflow_count);
  result_bool("realloc_failure_null", rf_failed == NULL);
  result_size("realloc_failure_errno", (size_t)errno);
  result_bool("realloc_failure_original_owned", mi_check_owned(rf) && has_sequence_prefix(rf, 64));
  mi_free(rf);
  result_size("realloc_failure_retains_original_status", (size_t)realloc_failure_case_status(0));
  result_size("reallocf_failure_invalidates_original_status", (size_t)realloc_failure_case_status(1));

  result_bool("expand_null_fails", mi_expand(NULL, 16) == NULL);
  errno = 0;
  result_bool("mi__expand_null_fails", mi__expand(NULL, 16) == NULL && errno == ENOMEM);
  unsigned char* ex = (unsigned char*)mi_malloc(64);
  fill_sequence(ex, 64);
  size_t ex_usable = mi_usable_size(ex);
  void* ex_same = mi_expand(ex, ex_usable);
  result_size("expand_same_size_in_place", ex_same == ex ? 1 : 0);
  result_bool("expand_same_size_original_valid", (ex_same == ex || ex_same == NULL) && mi_check_owned(ex) && has_sequence_prefix(ex, 64));
  void* ex_shrink = mi_expand(ex, 32);
  result_size("expand_shrink_in_place", ex_shrink == ex ? 1 : 0);
  result_bool("expand_shrink_original_valid", (ex_shrink == ex || ex_shrink == NULL) && mi_check_owned(ex) && has_sequence_prefix(ex, 32));
  void* ex_too_large = mi_expand(ex, ex_usable + 1);
  result_bool("expand_too_large_null", ex_too_large == NULL);
  result_bool("expand_too_large_original_owned", mi_check_owned(ex) && has_sequence_prefix(ex, 32));
  errno = 0;
  void* ex_compat_too_large = mi__expand(ex, ex_usable + 1);
  result_bool("mi__expand_too_large_errno", ex_compat_too_large == NULL && errno == ENOMEM);
  result_bool("mi__expand_too_large_original_owned", mi_check_owned(ex) && has_sequence_prefix(ex, 32));
  mi_free(ex);

  unsigned char* rn = (unsigned char*)mi_reallocn(NULL, 4, 8);
  result_bool("reallocn_null_nonnull", rn != NULL);
  result_bool("reallocn_null_size", rn != NULL && mi_usable_size(rn) >= 32);
  fill_sequence(rn, 32);
  rn = (unsigned char*)mi_reallocn(rn, 8, 8);
  result_bool("reallocn_grow_preserve", rn != NULL && has_sequence_prefix(rn, 32));
  void* rn_overflow = mi_reallocn(rn, overflow_count, 2);
  result_bool("reallocn_overflow_null", rn_overflow == NULL);
  result_bool("reallocn_overflow_original_owned", mi_check_owned(rn));
  mi_free(rn);

  unsigned char* rz_plain = (unsigned char*)mi_rezalloc(NULL, 24);
  result_bool("rezalloc_null_zeroed", rz_plain != NULL && bytes_equal(rz_plain, 0, 24));
  size_t rz_plain_usable = mi_usable_size(rz_plain);
  memset(rz_plain, 0, rz_plain_usable);
  fill_sequence(rz_plain, 24);
  rz_plain = (unsigned char*)mi_rezalloc(rz_plain, rz_plain_usable + 32);
  result_bool("rezalloc_grow_zeroed", rz_plain != NULL && has_sequence_prefix(rz_plain, 24) && bytes_equal(rz_plain + 24, 0, rz_plain_usable + 8));
  rz_plain = (unsigned char*)mi_rezalloc(rz_plain, 16);
  result_bool("rezalloc_shrink_preserve", rz_plain != NULL && has_sequence_prefix(rz_plain, 16));
  mi_free(rz_plain);

  unsigned char* rc_plain = (unsigned char*)mi_recalloc(NULL, 3, 8);
  result_bool("recalloc_null_zeroed", rc_plain != NULL && bytes_equal(rc_plain, 0, 24));
  size_t rc_plain_usable = mi_usable_size(rc_plain);
  memset(rc_plain, 0, rc_plain_usable);
  fill_sequence(rc_plain, 24);
  rc_plain = (unsigned char*)mi_recalloc(rc_plain, 1, rc_plain_usable + 32);
  result_bool("recalloc_grow_zeroed", rc_plain != NULL && has_sequence_prefix(rc_plain, 24) && bytes_equal(rc_plain + 24, 0, rc_plain_usable + 8));
  rc_plain = (unsigned char*)mi_recalloc(rc_plain, 1, 16);
  result_bool("recalloc_shrink_preserve", rc_plain != NULL && has_sequence_prefix(rc_plain, 16));
  void* rc_plain_overflow = mi_recalloc(rc_plain, overflow_count, 2);
  result_bool("recalloc_overflow_null", rc_plain_overflow == NULL);
  result_bool("recalloc_overflow_original_owned", mi_check_owned(rc_plain));
  mi_free(rc_plain);

  unsigned char* ra = (unsigned char*)mi_reallocarray(NULL, 4, 8);
  result_bool("reallocarray_null_nonnull", ra != NULL);
  result_bool("reallocarray_null_size", ra != NULL && mi_usable_size(ra) >= 32);
  fill_sequence(ra, 32);
  ra = (unsigned char*)mi_reallocarray(ra, 8, 8);
  result_bool("reallocarray_grow_preserve", ra != NULL && has_sequence_prefix(ra, 32));
  errno = 0;
  void* ra_overflow = mi_reallocarray(ra, overflow_count, 2);
  result_bool("reallocarray_overflow_null", ra_overflow == NULL);
  result_bool("reallocarray_overflow_errno", errno == EOVERFLOW);
  result_bool("reallocarray_overflow_original_owned", mi_check_owned(ra));
  mi_free(ra);

  unsigned char* rar = (unsigned char*)mi_malloc(16);
  fill_sequence(rar, 16);
  void* rar_ptr = rar;
  errno = 0;
  int rar_err = mi_reallocarr(&rar_ptr, 4, 8);
  result_bool("reallocarr_grow_success", rar_err == 0 && rar_ptr != NULL);
  result_bool("reallocarr_grow_preserve", rar_ptr != NULL && has_sequence_prefix((const unsigned char*)rar_ptr, 16));
  errno = 0;
  void* rar_before_overflow = rar_ptr;
  rar_err = mi_reallocarr(&rar_ptr, overflow_count, 2);
  result_bool("reallocarr_overflow_error", rar_err == EOVERFLOW && errno == EOVERFLOW);
  result_bool("reallocarr_overflow_preserve_ptr", rar_ptr == rar_before_overflow);
  mi_free(rar_ptr);
  rar_ptr = NULL;
  rar_err = mi_reallocarr(&rar_ptr, 3, 8);
  result_bool("reallocarr_null_source_success", rar_err == 0 && rar_ptr != NULL && mi_usable_size(rar_ptr) >= 24);
  mi_free(rar_ptr);

  void* aligned = mi_malloc_aligned(64, 32);
  result_bool("aligned_nonnull", aligned != NULL);
  result_bool("aligned_mod_32", is_aligned_at(aligned, 32, 0));
  result_bool("aligned_size", aligned != NULL && mi_usable_size(aligned) >= 64);
  mi_free(aligned);

  size_t malloc_aligned_sizes[] = { 0, 1, 17, MI_SMALL_SIZE_MAX, MI_SMALL_SIZE_MAX + 1 };
  size_t malloc_aligned_alignments[] = { 1, 2, 8, 64, 4096 };
  size_t malloc_aligned_count = sizeof(malloc_aligned_sizes) / sizeof(malloc_aligned_sizes[0]);
  for (size_t i = 0; i < malloc_aligned_count; i++) {
    size_t requested = malloc_aligned_sizes[i];
    size_t alignment = malloc_aligned_alignments[i];
    aligned = mi_malloc_aligned(requested, alignment);
    result_bool("malloc_aligned_matrix_nonnull", aligned != NULL);
    result_bool("malloc_aligned_matrix_alignment", is_aligned_at(aligned, alignment, 0));
    result_bool("malloc_aligned_matrix_usable", aligned != NULL && mi_usable_size(aligned) >= requested);
    mi_free(aligned);
  }

  errno = 0;
  result_bool("malloc_aligned_invalid_zero_alignment", mi_malloc_aligned(64, 0) == NULL);
  result_size("malloc_aligned_invalid_zero_alignment_errno", (size_t)errno);
  errno = 0;
  result_bool("malloc_aligned_invalid_non_power_two", mi_malloc_aligned(64, 3) == NULL);
  result_size("malloc_aligned_invalid_non_power_two_errno", (size_t)errno);
  errno = 0;
  result_bool("malloc_aligned_oversize_null", mi_malloc_aligned(overflow_count, 64) == NULL);
  result_size("malloc_aligned_oversize_errno", (size_t)errno);

  aligned = mi_malloc_aligned_at(24, 32, 8);
  result_bool("aligned_at_offset", is_aligned_at(aligned, 32, 8));
  result_bool("aligned_at_size", aligned != NULL && mi_usable_size(aligned) >= 24);
  mi_free(aligned);

  size_t malloc_aligned_at_sizes[] = { 0, 24, 32, 64, 512 };
  size_t malloc_aligned_at_alignments[] = { 8, 16, 32, 64, 4096 };
  size_t malloc_aligned_at_offsets[] = { 0, 8, 16, 24, 128 };
  size_t malloc_aligned_at_count = sizeof(malloc_aligned_at_sizes) / sizeof(malloc_aligned_at_sizes[0]);
  for (size_t i = 0; i < malloc_aligned_at_count; i++) {
    size_t requested = malloc_aligned_at_sizes[i];
    size_t alignment = malloc_aligned_at_alignments[i];
    size_t offset = malloc_aligned_at_offsets[i];
    aligned = mi_malloc_aligned_at(requested, alignment, offset);
    result_bool("malloc_aligned_at_matrix_nonnull", aligned != NULL);
    result_bool("malloc_aligned_at_matrix_offset", is_aligned_at(aligned, alignment, offset));
    result_bool("malloc_aligned_at_matrix_usable", aligned != NULL && mi_usable_size(aligned) >= requested);
    mi_free(aligned);
  }

  result_bool("malloc_aligned_at_invalid_zero_alignment", mi_malloc_aligned_at(64, 0, 0) == NULL);
  result_bool("malloc_aligned_at_invalid_non_power_two", mi_malloc_aligned_at(64, 3, 1) == NULL);
  result_bool("malloc_aligned_at_large_alignment_offset_null", mi_malloc_aligned_at(64, 1024 * 1024 * 4, 1) == NULL);

  unsigned char* zaligned = (unsigned char*)mi_zalloc_aligned(66, 32);
  result_bool("zalloc_aligned_zeroed", is_aligned_at(zaligned, 32, 0) && bytes_equal(zaligned, 0, 66));
  result_bool("zalloc_aligned_size", zaligned != NULL && mi_usable_size(zaligned) >= 66);
  mi_free(zaligned);

  size_t zalloc_aligned_sizes[] = { 0, 1, 17, MI_SMALL_SIZE_MAX, MI_SMALL_SIZE_MAX + 1 };
  size_t zalloc_aligned_alignments[] = { 8, 16, 32, 64, 4096 };
  size_t zalloc_aligned_count = sizeof(zalloc_aligned_sizes) / sizeof(zalloc_aligned_sizes[0]);
  for (size_t i = 0; i < zalloc_aligned_count; i++) {
    size_t requested = zalloc_aligned_sizes[i];
    size_t alignment = zalloc_aligned_alignments[i];
    zaligned = (unsigned char*)mi_zalloc_aligned(requested, alignment);
    result_bool("zalloc_aligned_matrix_nonnull", zaligned != NULL);
    result_bool("zalloc_aligned_matrix_alignment", is_aligned_at(zaligned, alignment, 0));
    result_bool("zalloc_aligned_matrix_zeroed", zaligned != NULL && bytes_equal(zaligned, 0, requested));
    result_bool("zalloc_aligned_matrix_usable", zaligned != NULL && mi_usable_size(zaligned) >= requested);
    mi_free(zaligned);
  }

  zaligned = (unsigned char*)mi_zalloc_aligned_at(67, 32, 16);
  result_bool("zalloc_aligned_at_zeroed", is_aligned_at(zaligned, 32, 16) && bytes_equal(zaligned, 0, 67));
  result_bool("zalloc_aligned_at_size", zaligned != NULL && mi_usable_size(zaligned) >= 67);
  mi_free(zaligned);

  size_t zalloc_aligned_at_sizes[] = { 0, 24, 32, 64, 512 };
  size_t zalloc_aligned_at_alignments[] = { 8, 16, 32, 64, 4096 };
  size_t zalloc_aligned_at_offsets[] = { 0, 8, 16, 24, 128 };
  size_t zalloc_aligned_at_count = sizeof(zalloc_aligned_at_sizes) / sizeof(zalloc_aligned_at_sizes[0]);
  for (size_t i = 0; i < zalloc_aligned_at_count; i++) {
    size_t requested = zalloc_aligned_at_sizes[i];
    size_t alignment = zalloc_aligned_at_alignments[i];
    size_t offset = zalloc_aligned_at_offsets[i];
    zaligned = (unsigned char*)mi_zalloc_aligned_at(requested, alignment, offset);
    result_bool("zalloc_aligned_at_matrix_nonnull", zaligned != NULL);
    result_bool("zalloc_aligned_at_matrix_offset", is_aligned_at(zaligned, alignment, offset));
    result_bool("zalloc_aligned_at_matrix_zeroed", zaligned != NULL && bytes_equal(zaligned, 0, requested));
    result_bool("zalloc_aligned_at_matrix_usable", zaligned != NULL && mi_usable_size(zaligned) >= requested);
    mi_free(zaligned);
  }

  unsigned char* caligned = (unsigned char*)mi_calloc_aligned(9, 7, 32);
  result_bool("calloc_aligned_zeroed", is_aligned_at(caligned, 32, 0) && bytes_equal(caligned, 0, 63));
  result_bool("calloc_aligned_size", caligned != NULL && mi_usable_size(caligned) >= 63);
  mi_free(caligned);

  caligned = (unsigned char*)mi_calloc_aligned(0, 64, 32);
  result_bool("calloc_aligned_zero_count", is_aligned_at(caligned, 32, 0));
  mi_free(caligned);

  caligned = (unsigned char*)mi_calloc_aligned(64, 0, 32);
  result_bool("calloc_aligned_zero_size", is_aligned_at(caligned, 32, 0));
  mi_free(caligned);

  size_t calloc_aligned_counts[] = { 1, 3, 5, 1 };
  size_t calloc_aligned_sizes[] = { 0, 8, 17, MI_SMALL_SIZE_MAX + 1 };
  size_t calloc_aligned_alignments[] = { 8, 16, 64, 4096 };
  size_t calloc_aligned_count = sizeof(calloc_aligned_counts) / sizeof(calloc_aligned_counts[0]);
  for (size_t i = 0; i < calloc_aligned_count; i++) {
    size_t count = calloc_aligned_counts[i];
    size_t size = calloc_aligned_sizes[i];
    size_t total = count * size;
    size_t alignment = calloc_aligned_alignments[i];
    caligned = (unsigned char*)mi_calloc_aligned(count, size, alignment);
    result_bool("calloc_aligned_matrix_nonnull", caligned != NULL);
    result_bool("calloc_aligned_matrix_alignment", is_aligned_at(caligned, alignment, 0));
    result_bool("calloc_aligned_matrix_zeroed", caligned != NULL && bytes_equal(caligned, 0, total));
    result_bool("calloc_aligned_matrix_usable", caligned != NULL && mi_usable_size(caligned) >= total);
    mi_free(caligned);
  }

  caligned = (unsigned char*)mi_calloc_aligned_at(9, 7, 32, 24);
  result_bool("calloc_aligned_at_zeroed", is_aligned_at(caligned, 32, 24) && bytes_equal(caligned, 0, 63));
  result_bool("calloc_aligned_at_size", caligned != NULL && mi_usable_size(caligned) >= 63);
  mi_free(caligned);

  size_t calloc_aligned_at_counts[] = { 1, 3, 4, 1 };
  size_t calloc_aligned_at_sizes[] = { 0, 8, 16, 512 };
  size_t calloc_aligned_at_alignments[] = { 8, 16, 64, 4096 };
  size_t calloc_aligned_at_offsets[] = { 0, 8, 24, 128 };
  size_t calloc_aligned_at_count = sizeof(calloc_aligned_at_counts) / sizeof(calloc_aligned_at_counts[0]);
  for (size_t i = 0; i < calloc_aligned_at_count; i++) {
    size_t count = calloc_aligned_at_counts[i];
    size_t size = calloc_aligned_at_sizes[i];
    size_t total = count * size;
    size_t alignment = calloc_aligned_at_alignments[i];
    size_t offset = calloc_aligned_at_offsets[i];
    caligned = (unsigned char*)mi_calloc_aligned_at(count, size, alignment, offset);
    result_bool("calloc_aligned_at_matrix_nonnull", caligned != NULL);
    result_bool("calloc_aligned_at_matrix_offset", is_aligned_at(caligned, alignment, offset));
    result_bool("calloc_aligned_at_matrix_zeroed", caligned != NULL && bytes_equal(caligned, 0, total));
    result_bool("calloc_aligned_at_matrix_usable", caligned != NULL && mi_usable_size(caligned) >= total);
    mi_free(caligned);
  }

  result_bool("calloc_aligned_overflow_null", mi_calloc_aligned(overflow_count, 2, 32) == NULL);
  result_bool("calloc_aligned_at_overflow_null", mi_calloc_aligned_at(overflow_count, 2, 32, 8) == NULL);

  void* posix = NULL;
  errno = 0;
  int perr = mi_posix_memalign(&posix, 32, 64);
  result_bool("posix_memalign_success", perr == 0 && posix != NULL);
  result_bool("posix_memalign_mod_32", posix != NULL && ((uintptr_t)posix % 32) == 0);
  result_bool("posix_memalign_errno_unchanged", errno == 0);
  mi_free(posix);

  posix = (void*)0xBAD;
  errno = 0;
  perr = mi_posix_memalign(&posix, 3, 64);
  result_bool("posix_memalign_invalid_alignment_error", perr == EINVAL);
  result_bool("posix_memalign_invalid_alignment_preserve", posix == (void*)0xBAD);
  result_bool("posix_memalign_invalid_alignment_errno", errno == 0);

  posix = (void*)0xBAD;
  perr = mi_posix_memalign(&posix, sizeof(void*) / 2, 64);
  result_bool("posix_memalign_small_alignment_error", perr == EINVAL);
  result_bool("posix_memalign_small_alignment_preserve", posix == (void*)0xBAD);

  perr = mi_posix_memalign(NULL, 32, 64);
  result_bool("posix_memalign_null_out_error", perr == EINVAL);

  posix = (void*)0xBAD;
  perr = mi_posix_memalign(&posix, 64, 0);
  result_bool("posix_memalign_zero_size_success", perr == 0);
  result_bool("posix_memalign_zero_size_alignment", posix == NULL || ((uintptr_t)posix % 64) == 0);
  mi_free(posix);

  long sys_page_size = sysconf(_SC_PAGESIZE);
  size_t page_size = (sys_page_size > 0 ? (size_t)sys_page_size : (size_t)4096);
  void* memaligned = mi_memalign(128, 77);
  result_bool("memalign_aligned", memaligned != NULL && ((uintptr_t)memaligned % 128) == 0);
  result_bool("memalign_size", memaligned != NULL && mi_usable_size(memaligned) >= 77);
  mi_free(memaligned);

  void* vallocated = mi_valloc(79);
  result_bool("valloc_page_aligned", vallocated != NULL && ((uintptr_t)vallocated % page_size) == 0);
  result_bool("valloc_size", vallocated != NULL && mi_usable_size(vallocated) >= 79);
  mi_free(vallocated);

  void* pvallocated = mi_pvalloc(page_size + 17);
  result_bool("pvalloc_page_aligned", pvallocated != NULL && ((uintptr_t)pvallocated % page_size) == 0);
  result_bool("pvalloc_rounded_size", pvallocated != NULL && mi_usable_size(pvallocated) >= (page_size * 2));
  mi_free(pvallocated);
  result_bool("pvalloc_overflow_null", mi_pvalloc(overflow_count) == NULL);

  void* c11aligned = mi_aligned_alloc(64, 96);
  result_bool("aligned_alloc_aligned", c11aligned != NULL && ((uintptr_t)c11aligned % 64) == 0);
  result_bool("aligned_alloc_non_multiple_size", c11aligned != NULL && mi_usable_size(c11aligned) >= 96);
  mi_free(c11aligned);

  char* dup = mi_strdup("abc");
  result_bool("strdup_content", dup != NULL && strcmp(dup, "abc") == 0);
  mi_free(dup);

  char* ndup = mi_strndup("abcdef", 3);
  result_bool("strndup_content", ndup != NULL && strcmp(ndup, "abc") == 0);
  mi_free(ndup);

  result_bool("strdup_null", mi_strdup(NULL) == NULL);
  char* full_ndup = mi_strndup("abc", 99);
  result_bool("strndup_full_content", full_ndup != NULL && strcmp(full_ndup, "abc") == 0);
  mi_free(full_ndup);
  result_bool("strndup_null", mi_strndup(NULL, 3) == NULL);

  result_bool("mbsdup_null", mi_mbsdup(NULL) == NULL);
  unsigned char* mdup = mi_mbsdup((const unsigned char*)"");
  result_bool("mbsdup_empty", mdup != NULL && mdup[0] == 0);
  mi_free(mdup);
  mdup = mi_mbsdup((const unsigned char*)"multi-byte-\xc3\xa9");
  result_bool("mbsdup_content", mdup != NULL && strcmp((const char*)mdup, "multi-byte-\xc3\xa9") == 0);
  mi_free(mdup);

  result_bool("wcsdup_null", mi_wcsdup(NULL) == NULL);
  wchar_t* wdup = mi_wcsdup(L"wide-\u03bb");
  result_bool("wcsdup_content", wdup != NULL && wcscmp(wdup, L"wide-\u03bb") == 0);
  mi_free(wdup);

  char* envbuf = (char*)0xBAD;
  size_t envsize = 123;
  result_bool("dupenv_setenv", setenv("MI_PARITY_ENV", "value", 1) == 0);
  int env_err = mi_dupenv_s(&envbuf, &envsize, "MI_PARITY_ENV");
  result_bool("dupenv_present_success", env_err == 0 && envbuf != NULL);
  result_bool("dupenv_present_content", envbuf != NULL && strcmp(envbuf, "value") == 0);
  result_bool("dupenv_present_size", envsize == 6);
  result_bool("dupenv_present_owned", envbuf != NULL && mi_check_owned(envbuf));
  mi_free(envbuf);

  envbuf = (char*)0xBAD;
  envsize = 123;
  result_bool("dupenv_unsetenv", unsetenv("MI_PARITY_ENV") == 0);
  env_err = mi_dupenv_s(&envbuf, &envsize, "MI_PARITY_ENV");
  result_bool("dupenv_missing_success", env_err == 0 && envbuf == NULL && envsize == 0);

  envbuf = (char*)0xBAD;
  env_err = mi_dupenv_s(&envbuf, NULL, "MI_PARITY_ENV");
  result_bool("dupenv_missing_null_size", env_err == 0 && envbuf == NULL);

  envsize = 123;
  env_err = mi_dupenv_s(NULL, &envsize, "MI_PARITY_ENV");
  result_bool("dupenv_null_buf_error", env_err == EINVAL && envsize == 0);

  envbuf = (char*)0xBAD;
  envsize = 123;
  env_err = mi_dupenv_s(&envbuf, &envsize, NULL);
  result_bool("dupenv_null_name_error", env_err == EINVAL && envbuf == (char*)0xBAD && envsize == 0);

  static const unsigned char nonutf8_value[] = { 'b', 'y', 't', 'e', '-', 0xFF, 0 };
  result_bool("dupenv_nonutf8_setenv", setenv("MI_PARITY_ENV_BYTES", (const char*)nonutf8_value, 1) == 0);
  envbuf = NULL;
  envsize = 0;
  env_err = mi_dupenv_s(&envbuf, &envsize, "MI_PARITY_ENV_BYTES");
  result_bool("dupenv_nonutf8_success", env_err == 0 && envbuf != NULL);
  result_bool("dupenv_nonutf8_content", envbuf != NULL && memcmp(envbuf, nonutf8_value, sizeof(nonutf8_value)) == 0);
  result_bool("dupenv_nonutf8_size", envsize == sizeof(nonutf8_value));
  mi_free(envbuf);
  (void)unsetenv("MI_PARITY_ENV_BYTES");

  wchar_t* wenvbuf = (wchar_t*)0xBAD;
  size_t wenvsize = 123;
  int wenv_err = mi_wdupenv_s(&wenvbuf, &wenvsize, L"MI_PARITY_ENV");
  result_bool("wdupenv_unsupported_error", wenv_err == EINVAL && wenvbuf == NULL && wenvsize == 0);
  wenvsize = 123;
  wenv_err = mi_wdupenv_s(NULL, &wenvsize, L"MI_PARITY_ENV");
  result_bool("wdupenv_null_buf_error", wenv_err == EINVAL && wenvsize == 0);
  wenvbuf = (wchar_t*)0xBAD;
  wenvsize = 123;
  wenv_err = mi_wdupenv_s(&wenvbuf, &wenvsize, NULL);
  result_bool("wdupenv_null_name_error", wenv_err == EINVAL && wenvbuf == (wchar_t*)0xBAD && wenvsize == 0);

  mi_heap_t* main_heap = mi_heap_main();
  mi_heap_t* main_heap_again = mi_heap_main();
  result_bool("heap_main_nonnull", main_heap != NULL);
  result_bool("heap_main_stable", main_heap != NULL && main_heap == main_heap_again);
  void* main_alloc = mi_malloc(24);
  result_bool("heap_main_contains_malloc", main_heap != NULL && main_alloc != NULL && mi_heap_contains(main_heap, main_alloc));
  result_bool("heap_main_of_malloc", main_heap != NULL && main_alloc != NULL && mi_heap_of(main_alloc) == main_heap);
  mi_free(main_alloc);

  mi_heap_t* heap = mi_heap_new();
  result_bool("heap_nonnull", heap != NULL);
  mi_heap_t* heap_delete_migrate = mi_heap_new();
  void* heap_delete_live = heap_delete_migrate == NULL ? NULL : mi_heap_malloc(heap_delete_migrate, 77);
  result_bool("heap_delete_live_owner", heap_delete_migrate != NULL && heap_delete_live != NULL && mi_heap_contains(heap_delete_migrate, heap_delete_live));
  if (heap_delete_migrate != NULL) mi_heap_delete(heap_delete_migrate);
  result_bool("heap_delete_live_migrated", heap_delete_live != NULL && mi_check_owned(heap_delete_live) && mi_heap_contains(mi_heap_main(), heap_delete_live));
  mi_free(heap_delete_live);
  result_size("heap_lifecycle_delete_live_status", (size_t)heap_lifecycle_case_status(0));
  result_size("heap_lifecycle_destroy_live_free_status", (size_t)heap_lifecycle_case_status(1));
  result_size("heap_lifecycle_delete_delayed_free_status", (size_t)heap_lifecycle_case_status(2));
  result_size("heap_lifecycle_destroy_delayed_free_status", (size_t)heap_lifecycle_case_status(3));
  char* hdup = heap == NULL ? NULL : mi_heap_strdup(heap, "heap-string");
  result_bool("heap_strdup_content_owner", hdup != NULL && strcmp(hdup, "heap-string") == 0 && mi_heap_contains(heap, hdup));
  mi_free(hdup);
  char* hndup = heap == NULL ? NULL : mi_heap_strndup(heap, "heap-prefix", 4);
  result_bool("heap_strndup_content_owner", hndup != NULL && strcmp(hndup, "heap") == 0 && mi_heap_contains(heap, hndup));
  mi_free(hndup);
  char heap_resolved[4096];
  char* hrp_stack = heap == NULL ? NULL : mi_heap_realpath(heap, ".", heap_resolved);
  result_bool("heap_realpath_stack_success", hrp_stack == heap_resolved && heap_resolved[0] == '/');
  char* hrp = heap == NULL ? NULL : mi_heap_realpath(heap, ".", NULL);
  result_bool("heap_realpath_alloc_content_owner", hrp != NULL && strcmp(hrp, heap_resolved) == 0 && mi_heap_contains(heap, hrp));
  mi_free(hrp);
  errno = 0;
  hrp = heap == NULL ? NULL : mi_heap_realpath(heap, "/definitely/not/a/mechanical_mimalloc/path", NULL);
  result_bool("heap_realpath_missing_errno", hrp == NULL && errno == ENOENT);
  void* hp = heap == NULL ? NULL : mi_heap_malloc(heap, 24);
  result_bool("heap_malloc_nonnull", hp != NULL);
  result_bool("heap_contains", heap != NULL && hp != NULL && mi_heap_contains(heap, hp));
  result_bool("any_heap_contains", hp != NULL && mi_any_heap_contains(hp));
  mi_free(hp);

  size_t heap_alloc_sizes[] = { 0, 1, 17, MI_SMALL_SIZE_MAX - 1, MI_SMALL_SIZE_MAX, MI_SMALL_SIZE_MAX + 1 };
  size_t heap_alloc_size_count = sizeof(heap_alloc_sizes) / sizeof(heap_alloc_sizes[0]);
  for (size_t i = 0; i < heap_alloc_size_count; i++) {
    size_t requested = heap_alloc_sizes[i];
    unsigned char* hmp = heap == NULL ? NULL : (unsigned char*)mi_heap_malloc(heap, requested);
    result_bool("heap_malloc_owner", hmp != NULL && mi_heap_contains(heap, hmp));
    result_bool("heap_malloc_usable_ge_request", hmp != NULL && mi_usable_size(hmp) >= requested);
    result_indexed_size("heap_malloc_usable_size", i, hmp == NULL ? 0 : mi_usable_size(hmp));
    mi_free(hmp);
  }

  unsigned char* hzp = heap == NULL ? NULL : (unsigned char*)mi_heap_zalloc(heap, 48);
  result_bool("heap_zalloc_zeroed_owner", hzp != NULL && bytes_equal(hzp, 0, 48) && mi_heap_contains(heap, hzp));
  mi_free(hzp);

  unsigned char* hcp = heap == NULL ? NULL : (unsigned char*)mi_heap_calloc(heap, 6, 7);
  result_bool("heap_calloc_zeroed_owner", hcp != NULL && bytes_equal(hcp, 0, 42) && mi_heap_contains(heap, hcp));
  mi_free(hcp);
  result_bool("heap_calloc_overflow_null", heap != NULL && mi_heap_calloc(heap, overflow_count, 2) == NULL);

  unsigned char* hmnp = heap == NULL ? NULL : (unsigned char*)mi_heap_mallocn(heap, 5, 9);
  result_bool("heap_mallocn_owner", hmnp != NULL && mi_heap_contains(heap, hmnp) && mi_usable_size(hmnp) >= 45);
  mi_free(hmnp);
  result_bool("heap_mallocn_overflow_null", heap != NULL && mi_heap_mallocn(heap, overflow_count, 2) == NULL);

  size_t heap_small_sizes[] = { 0, 1, MI_SMALL_SIZE_MAX - 1, MI_SMALL_SIZE_MAX };
  size_t heap_small_size_count = sizeof(heap_small_sizes) / sizeof(heap_small_sizes[0]);
  for (size_t i = 0; i < heap_small_size_count; i++) {
    size_t requested = heap_small_sizes[i];
    unsigned char* hsp = heap == NULL ? NULL : (unsigned char*)mi_heap_malloc_small(heap, requested);
    result_bool("heap_malloc_small_owner", hsp != NULL && mi_heap_contains(heap, hsp));
    result_bool("heap_malloc_small_usable_ge_request", hsp != NULL && mi_usable_size(hsp) >= requested);
    result_indexed_size("heap_malloc_small_usable_size", i, hsp == NULL ? 0 : mi_usable_size(hsp));
    mi_free(hsp);

    unsigned char* hzsp = heap == NULL ? NULL : (unsigned char*)mi_heap_zalloc_small(heap, requested);
    result_bool("heap_zalloc_small_zeroed_owner", hzsp != NULL && bytes_equal(hzsp, 0, requested) && mi_heap_contains(heap, hzsp));
    result_bool("heap_zalloc_small_usable_ge_request", hzsp != NULL && mi_usable_size(hzsp) >= requested);
    result_indexed_size("heap_zalloc_small_usable_size", i, hzsp == NULL ? 0 : mi_usable_size(hzsp));
    mi_free(hzsp);
  }

  struct heap_cross_thread_free_result heap_thread_result = { heap == NULL ? NULL : mi_heap_malloc(heap, 64), 0 };
  result_bool("heap_cross_thread_alloc_owner", heap_thread_result.ptr != NULL && mi_heap_contains(heap, heap_thread_result.ptr));
  pthread_t heap_free_thread;
  int heap_free_started = (pthread_create(&heap_free_thread, NULL, heap_cross_thread_free_worker, &heap_thread_result) == 0);
  result_bool("heap_cross_thread_free_started", heap_free_started);
  int heap_free_joined = heap_free_started && pthread_join(heap_free_thread, NULL) == 0;
  result_bool("heap_cross_thread_free_joined", heap_free_joined);
  result_bool("heap_cross_thread_owned_before_free", heap_free_joined && heap_thread_result.owned_before_free);
  mi_heap_collect(heap, false);
  void* heap_delayed_reuse = heap == NULL ? NULL : mi_heap_malloc(heap, 64);
  result_bool("heap_collect_delayed_nonforced_reuse", heap_delayed_reuse != NULL && mi_heap_contains(heap, heap_delayed_reuse));
  mi_free(heap_delayed_reuse);
  mi_heap_collect(heap, true);
  heap_delayed_reuse = heap == NULL ? NULL : mi_heap_malloc(heap, 64);
  result_bool("heap_collect_delayed_forced_reuse", heap_delayed_reuse != NULL && mi_heap_contains(heap, heap_delayed_reuse));
  mi_free(heap_delayed_reuse);

  void* heap_collect_live_a = heap == NULL ? NULL : mi_heap_malloc(heap, 72);
  void* heap_collect_live_b = heap == NULL ? NULL : mi_heap_malloc(heap, 144);
  result_bool("heap_collect_live_owner", heap_collect_live_a != NULL && heap_collect_live_b != NULL && mi_heap_contains(heap, heap_collect_live_a) && mi_heap_contains(heap, heap_collect_live_b));
  mi_heap_collect(heap, false);
  result_bool("heap_collect_nonforced_keeps_live", heap_collect_live_a != NULL && heap_collect_live_b != NULL && mi_heap_contains(heap, heap_collect_live_a) && mi_heap_contains(heap, heap_collect_live_b));
  mi_heap_collect(heap, true);
  result_bool("heap_collect_forced_keeps_live", heap_collect_live_a != NULL && heap_collect_live_b != NULL && mi_heap_contains(heap, heap_collect_live_a) && mi_heap_contains(heap, heap_collect_live_b));
  mi_free(heap_collect_live_a);
  mi_free(heap_collect_live_b);

  void* heap_retire_blocks[96];
  size_t heap_retire_allocated = 0;
  for (size_t i = 0; i < 96; i++) {
    heap_retire_blocks[i] = heap == NULL ? NULL : mi_heap_malloc(heap, 1024);
    if (heap_retire_blocks[i] != NULL && mi_heap_contains(heap, heap_retire_blocks[i])) heap_retire_allocated++;
  }
  result_bool("heap_collect_retire_allocated", heap_retire_allocated == 96);
  for (size_t i = 0; i < heap_retire_allocated; i++) {
    mi_free(heap_retire_blocks[i]);
  }
  mi_stats_t_decl(heap_collect_stats_before);
  bool heap_collect_stats_before_ok = heap != NULL && mi_heap_stats_get(heap, &heap_collect_stats_before);
  result_bool("heap_collect_stats_before_success", heap_collect_stats_before_ok);
  mi_heap_collect(heap, false);
  mi_heap_collect(heap, true);
  mi_stats_t_decl(heap_collect_stats_after);
  bool heap_collect_stats_after_ok = heap != NULL && mi_heap_stats_get(heap, &heap_collect_stats_after);
  result_bool("heap_collect_stats_after_success", heap_collect_stats_after_ok);
  result_bool("heap_collect_stats_pages_nonnegative", heap_collect_stats_after_ok && heap_collect_stats_after.pages.current >= 0);
  result_bool("heap_collect_stats_page_peak_consistent", heap_collect_stats_after_ok && heap_collect_stats_after.pages.peak >= heap_collect_stats_after.pages.current);
  result_bool("heap_collect_stats_malloc_peak_consistent", heap_collect_stats_after_ok && heap_collect_stats_after.malloc_normal.peak >= heap_collect_stats_after.malloc_normal.current);
  void* heap_retire_reuse = heap == NULL ? NULL : mi_heap_malloc(heap, 1024);
  result_bool("heap_collect_retire_reuse", heap_retire_reuse != NULL && mi_heap_contains(heap, heap_retire_reuse));
  mi_free(heap_retire_reuse);

  void* visit_a = heap == NULL ? NULL : mi_heap_malloc(heap, 80);
  void* visit_b = heap == NULL ? NULL : mi_heap_malloc(heap, 160);
  result_bool("heap_visit_allocations_owner", visit_a != NULL && visit_b != NULL && mi_heap_contains(heap, visit_a) && mi_heap_contains(heap, visit_b));
  struct heap_visit_result visit_areas = { heap, 0, 0, 1, 1, 1 };
  bool visit_areas_ok = heap != NULL && mi_heap_visit_blocks(heap, false, heap_visit_counting_visitor, &visit_areas);
  result_bool("heap_visit_areas_success", visit_areas_ok);
  result_bool("heap_visit_areas_only", visit_areas_ok && visit_areas.areas > 0 && visit_areas.blocks == 0);
  result_bool("heap_visit_area_fields", visit_areas.heap_matches && visit_areas.area_fields_ok);
  struct heap_visit_result visit_blocks = { heap, 0, 0, 1, 1, 1 };
  bool visit_blocks_ok = heap != NULL && mi_heap_visit_blocks(heap, true, heap_visit_counting_visitor, &visit_blocks);
  result_bool("heap_visit_blocks_success", visit_blocks_ok);
  result_bool("heap_visit_blocks_seen", visit_blocks_ok && visit_blocks.blocks > 0);
  result_bool("heap_visit_block_fields", visit_blocks.heap_matches && visit_blocks.area_fields_ok && visit_blocks.block_fields_ok);
  struct heap_visit_result visit_stop = { heap, 0, 0, 1, 1, 1 };
  bool visit_stop_ok = heap != NULL && mi_heap_visit_blocks(heap, true, heap_visit_stop_visitor, &visit_stop);
  result_bool("heap_visit_stop_returns_false", !visit_stop_ok && visit_stop.areas == 1);
  struct heap_visit_alloc_result visit_alloc = { heap, 0, 1, 1 };
  bool visit_alloc_ok = heap != NULL && mi_heap_visit_blocks(heap, true, heap_visit_allocating_visitor, &visit_alloc);
  result_bool("heap_visit_callback_alloc_success", visit_alloc_ok);
  result_bool("heap_visit_callback_alloc_called", visit_alloc_ok && visit_alloc.calls > 0);
  result_bool("heap_visit_callback_alloc_ok", visit_alloc.heap_matches && visit_alloc.alloc_ok);
  mi_free(visit_a);
  mi_free(visit_b);

  unsigned char* hre = heap == NULL ? NULL : (unsigned char*)mi_heap_realloc(heap, NULL, 24);
  result_bool("heap_realloc_null_owner", hre != NULL && mi_heap_contains(heap, hre) && mi_usable_size(hre) >= 24);
  fill_sequence(hre, 24);
  hre = (unsigned char*)mi_heap_realloc(heap, hre, 96);
  result_bool("heap_realloc_grow_preserve", hre != NULL && mi_heap_contains(heap, hre) && has_sequence_prefix(hre, 24) && mi_usable_size(hre) >= 96);
  hre = (unsigned char*)mi_heap_realloc(heap, hre, 16);
  result_bool("heap_realloc_shrink_preserve", hre != NULL && mi_heap_contains(heap, hre) && has_sequence_prefix(hre, 16) && mi_usable_size(hre) >= 16);
  mi_free(hre);

  unsigned char* hrn = heap == NULL ? NULL : (unsigned char*)mi_heap_reallocn(heap, NULL, 4, 8);
  result_bool("heap_reallocn_null_owner", hrn != NULL && mi_heap_contains(heap, hrn) && mi_usable_size(hrn) >= 32);
  fill_sequence(hrn, 32);
  hrn = (unsigned char*)mi_heap_reallocn(heap, hrn, 8, 8);
  result_bool("heap_reallocn_grow_preserve", hrn != NULL && mi_heap_contains(heap, hrn) && has_sequence_prefix(hrn, 32) && mi_usable_size(hrn) >= 64);
  void* hrn_overflow = heap == NULL ? NULL : mi_heap_reallocn(heap, hrn, overflow_count, 2);
  result_bool("heap_reallocn_overflow_null", hrn_overflow == NULL);
  result_bool("heap_reallocn_overflow_keeps_old", hrn != NULL && has_sequence_prefix(hrn, 32));
  mi_free(hrn);

  unsigned char* hrez = heap == NULL ? NULL : (unsigned char*)mi_heap_rezalloc(heap, NULL, 24);
  result_bool("heap_rezalloc_null_zeroed", hrez != NULL && mi_heap_contains(heap, hrez) && bytes_equal(hrez, 0, 24));
  fill_sequence(hrez, 24);
  size_t hrez_usable = mi_usable_size(hrez);
  hrez = (unsigned char*)mi_heap_rezalloc(heap, hrez, hrez_usable + 32);
  result_bool("heap_rezalloc_grow_zeroed", hrez != NULL && mi_heap_contains(heap, hrez) && has_sequence_prefix(hrez, 24) && bytes_equal(hrez + hrez_usable, 0, 32));
  hrez = (unsigned char*)mi_heap_rezalloc(heap, hrez, 16);
  result_bool("heap_rezalloc_shrink_preserve", hrez != NULL && mi_heap_contains(heap, hrez) && has_sequence_prefix(hrez, 16));
  mi_free(hrez);

  unsigned char* hrec = heap == NULL ? NULL : (unsigned char*)mi_heap_recalloc(heap, NULL, 3, 8);
  result_bool("heap_recalloc_null_zeroed", hrec != NULL && mi_heap_contains(heap, hrec) && bytes_equal(hrec, 0, 24));
  fill_sequence(hrec, 24);
  size_t hrec_usable = mi_usable_size(hrec);
  hrec = (unsigned char*)mi_heap_recalloc(heap, hrec, 1, hrec_usable + 32);
  result_bool("heap_recalloc_grow_zeroed", hrec != NULL && mi_heap_contains(heap, hrec) && has_sequence_prefix(hrec, 24) && bytes_equal(hrec + hrec_usable, 0, 32));
  hrec = (unsigned char*)mi_heap_recalloc(heap, hrec, 1, 16);
  result_bool("heap_recalloc_shrink_preserve", hrec != NULL && mi_heap_contains(heap, hrec) && has_sequence_prefix(hrec, 16));
  void* hrec_overflow = heap == NULL ? NULL : mi_heap_recalloc(heap, hrec, overflow_count, 2);
  result_bool("heap_recalloc_overflow_null", hrec_overflow == NULL);
  result_bool("heap_recalloc_overflow_keeps_old", hrec != NULL && has_sequence_prefix(hrec, 16));
  mi_free(hrec);

  unsigned char* hrf = heap == NULL ? NULL : (unsigned char*)mi_heap_malloc(heap, 32);
  result_bool("heap_reallocf_source_owner", hrf != NULL && mi_heap_contains(heap, hrf));
  void* hrf_failed = heap == NULL ? NULL : mi_heap_reallocf(heap, hrf, overflow_count);
  result_bool("heap_reallocf_failure_null", hrf_failed == NULL);

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
  ar = (unsigned char*)mi_realloc_aligned(ar, 131072, 32);
  result_bool("realloc_aligned_large_grow_preserve", is_aligned_at(ar, 32, 0) && has_sequence_prefix(ar, 24));
  ar = (unsigned char*)mi_realloc_aligned(ar, 16, 32);
  result_bool("realloc_aligned_shrink_preserve", is_aligned_at(ar, 32, 0) && has_sequence_prefix(ar, 16));
  mi_free(ar);
  ar = (unsigned char*)mi_realloc_aligned(NULL, 24, 3);
  result_bool("realloc_aligned_non_power_two_alignment_accepted", ar != NULL);
  mi_free(ar);

  ar = (unsigned char*)mi_realloc_aligned_at(NULL, 24, 32, 8);
  result_bool("realloc_aligned_at_null", is_aligned_at(ar, 32, 8));
  fill_sequence(ar, 24);
  ar = (unsigned char*)mi_realloc_aligned_at(ar, 48, 32, 8);
  result_bool("realloc_aligned_at_grow_preserve", is_aligned_at(ar, 32, 8) && has_sequence_prefix(ar, 24));
  ar = (unsigned char*)mi_realloc_aligned_at(ar, 16, 32, 8);
  result_bool("realloc_aligned_at_shrink_preserve", is_aligned_at(ar, 32, 8) && has_sequence_prefix(ar, 16));
  mi_free(ar);
  ar = (unsigned char*)mi_realloc_aligned_at(NULL, 24, 3, 0);
  result_bool("realloc_aligned_at_non_power_two_alignment_accepted", ar != NULL);
  mi_free(ar);

  unsigned char* rz = (unsigned char*)mi_rezalloc_aligned(NULL, 24, 32);
  result_bool("rezalloc_aligned_null_zeroed", is_aligned_at(rz, 32, 0) && bytes_equal(rz, 0, 24));
  fill_sequence(rz, 24);
  rz = (unsigned char*)mi_rezalloc_aligned(rz, 48, 32);
  result_bool("rezalloc_aligned_grow_zeroed", is_aligned_at(rz, 32, 0) && has_sequence_prefix(rz, 24) && bytes_equal(rz + 24, 0, 24));
  rz = (unsigned char*)mi_rezalloc_aligned(rz, 16, 32);
  result_bool("rezalloc_aligned_shrink_preserve", is_aligned_at(rz, 32, 0) && has_sequence_prefix(rz, 16));
  mi_free(rz);
  result_bool("rezalloc_aligned_overflow_null", mi_rezalloc_aligned(NULL, overflow_count, 32) == NULL);

  rz = (unsigned char*)mi_rezalloc_aligned_at(NULL, 24, 32, 16);
  result_bool("rezalloc_aligned_at_null_zeroed", is_aligned_at(rz, 32, 16) && bytes_equal(rz, 0, 24));
  fill_sequence(rz, 24);
  rz = (unsigned char*)mi_rezalloc_aligned_at(rz, 48, 32, 16);
  result_bool("rezalloc_aligned_at_grow_zeroed", is_aligned_at(rz, 32, 16) && has_sequence_prefix(rz, 24) && bytes_equal(rz + 24, 0, 24));
  mi_free(rz);
  result_bool("rezalloc_aligned_at_overflow_null", mi_rezalloc_aligned_at(NULL, overflow_count, 32, 16) == NULL);

  unsigned char* rc = (unsigned char*)mi_recalloc_aligned(NULL, 3, 8, 32);
  result_bool("recalloc_aligned_null_zeroed", is_aligned_at(rc, 32, 0) && bytes_equal(rc, 0, 24));
  fill_sequence(rc, 24);
  rc = (unsigned char*)mi_recalloc_aligned(rc, 6, 8, 32);
  result_bool("recalloc_aligned_grow_zeroed", is_aligned_at(rc, 32, 0) && has_sequence_prefix(rc, 24) && bytes_equal(rc + 24, 0, 24));
  mi_free(rc);

  rc = (unsigned char*)mi_recalloc_aligned_at(NULL, 3, 8, 32, 24);
  result_bool("recalloc_aligned_at_null_zeroed", is_aligned_at(rc, 32, 24) && bytes_equal(rc, 0, 24));
  fill_sequence(rc, 24);
  rc = (unsigned char*)mi_recalloc_aligned_at(rc, 6, 8, 32, 24);
  result_bool("recalloc_aligned_at_grow_zeroed", is_aligned_at(rc, 32, 24) && has_sequence_prefix(rc, 24) && bytes_equal(rc + 24, 0, 24));
  mi_free(rc);
  result_bool("recalloc_aligned_overflow_null", mi_recalloc_aligned(NULL, overflow_count, 2, 32) == NULL);
  result_bool("recalloc_aligned_at_overflow_null", mi_recalloc_aligned_at(NULL, overflow_count, 2, 32, 24) == NULL);

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
  void* us = mi_umalloc_small(32, &block_size);
  result_bool("umalloc_small_block_size", us != NULL && block_size >= mi_usable_size(us) && mi_usable_size(us) >= 32);
  mi_free(us);

  block_size = 0;
  void* um = mi_umalloc(333, &block_size);
  result_bool("umalloc_block_size", um != NULL && block_size >= mi_usable_size(um) && mi_usable_size(um) >= 333);
  mi_free(um);

  block_size = 0;
  unsigned char* uc = (unsigned char*)mi_ucalloc(7, 33, &block_size);
  result_bool("ucalloc_block_size_zeroed", uc != NULL && block_size >= mi_usable_size(uc) && mi_usable_size(uc) >= 231 && bytes_equal(uc, 0, 231));
  mi_free(uc);

  block_size = 123;
  result_bool("ucalloc_overflow_null_preserves_block_size", mi_ucalloc(overflow_count, 2, &block_size) == NULL && block_size == 123);

  block_size = 0;
  void* up = mi_umalloc_aligned(96, 32, &block_size);
  result_bool("umalloc_aligned_block_size", is_aligned_at(up, 32, 0) && block_size >= mi_usable_size(up) && mi_usable_size(up) >= 96);
  mi_free(up);

  block_size = 0;
  unsigned char* uzp = (unsigned char*)mi_uzalloc_aligned(96, 32, &block_size);
  result_bool("uzalloc_aligned_block_size_zeroed", is_aligned_at(uzp, 32, 0) && block_size >= mi_usable_size(uzp) && mi_usable_size(uzp) >= 96 && bytes_equal(uzp, 0, 96));
  mi_free(uzp);

  void* newp = mi_heap_alloc_new_n(heap, 4, 16);
  result_bool("heap_alloc_new_n_size_owner", newp != NULL && mi_usable_size(newp) >= 64 && mi_heap_contains(heap, newp));
  mi_free(newp);

  mi_theap_t* theap = heap == NULL ? NULL : mi_heap_theap(heap);
  unsigned char* tm = (unsigned char*)mi_theap_malloc(theap, 70);
  result_bool("theap_malloc_owner", tm != NULL && mi_heap_contains(heap, tm) && mi_usable_size(tm) >= 70);
  mi_free(tm);

  unsigned char* tz = (unsigned char*)mi_theap_zalloc(theap, 71);
  result_bool("theap_zalloc_zeroed_owner", tz != NULL && mi_heap_contains(heap, tz) && mi_usable_size(tz) >= 71 && bytes_equal(tz, 0, 71));
  mi_free(tz);

  unsigned char* tc = (unsigned char*)mi_theap_calloc(theap, 5, 17);
  result_bool("theap_calloc_zeroed_owner", tc != NULL && mi_heap_contains(heap, tc) && mi_usable_size(tc) >= 85 && bytes_equal(tc, 0, 85));
  mi_free(tc);
  result_bool("theap_calloc_overflow_null", mi_theap_calloc(theap, overflow_count, 2) == NULL);

  void* tsp = mi_theap_malloc_small(theap, 32);
  result_bool("theap_malloc_small_owner", tsp != NULL && mi_heap_contains(heap, tsp) && mi_usable_size(tsp) >= 32);
  mi_free(tsp);

  unsigned char* tzsp = (unsigned char*)mi_theap_zalloc_small(theap, 33);
  result_bool("theap_zalloc_small_zeroed_owner", tzsp != NULL && mi_heap_contains(heap, tzsp) && mi_usable_size(tzsp) >= 33 && bytes_equal(tzsp, 0, 33));
  mi_free(tzsp);

  void* tap = mi_theap_malloc_aligned(theap, 96, 32);
  result_bool("theap_malloc_aligned_owner", is_aligned_at(tap, 32, 0) && mi_heap_contains(heap, tap) && mi_usable_size(tap) >= 96);
  mi_free(tap);

  unsigned char* tr = (unsigned char*)mi_theap_malloc(theap, 48);
  if (tr != NULL) fill_sequence(tr, 48);
  unsigned char* trg = (unsigned char*)mi_theap_realloc(theap, tr, 112);
  result_bool("theap_realloc_grow_owner_preserves", trg != NULL && mi_heap_contains(heap, trg) && mi_usable_size(trg) >= 112 && has_sequence_prefix(trg, 48));
  unsigned char* trs = (unsigned char*)mi_theap_realloc(theap, trg, 24);
  result_bool("theap_realloc_shrink_preserves", trs != NULL && mi_heap_contains(heap, trs) && mi_usable_size(trs) >= 24 && has_sequence_prefix(trs, 24));
  mi_free(trs);
  void* trn = mi_theap_realloc(theap, NULL, 52);
  result_bool("theap_realloc_null_allocates", trn != NULL && mi_heap_contains(heap, trn) && mi_usable_size(trn) >= 52);
  mi_free(trn);

  result_size("theap_wrong_thread_malloc_status", (size_t)theap_wrong_thread_case_status(theap, 0));
  result_size("theap_wrong_thread_zalloc_status", (size_t)theap_wrong_thread_case_status(theap, 1));
  result_size("theap_wrong_thread_calloc_status", (size_t)theap_wrong_thread_case_status(theap, 2));
  result_size("theap_wrong_thread_malloc_small_status", (size_t)theap_wrong_thread_case_status(theap, 3));
  result_size("theap_wrong_thread_zalloc_small_status", (size_t)theap_wrong_thread_case_status(theap, 4));
  result_size("theap_wrong_thread_malloc_aligned_status", (size_t)theap_wrong_thread_case_status(theap, 5));
  result_size("theap_wrong_thread_realloc_null_status", (size_t)theap_wrong_thread_case_status(theap, 6));

  void* theap_visit_a = theap == NULL ? NULL : mi_theap_malloc(theap, 72);
  void* theap_visit_b = theap == NULL ? NULL : mi_theap_zalloc(theap, 144);
  result_bool("theap_collect_allocations_owner", theap_visit_a != NULL && theap_visit_b != NULL && mi_heap_contains(heap, theap_visit_a) && mi_heap_contains(heap, theap_visit_b));
  if (theap != NULL) mi_theap_collect(theap, false);
  result_bool("theap_collect_nonforced_keeps_live", theap_visit_a != NULL && theap_visit_b != NULL && mi_heap_contains(heap, theap_visit_a) && mi_heap_contains(heap, theap_visit_b));
  if (theap != NULL) mi_theap_collect(theap, true);
  result_bool("theap_collect_forced_keeps_live", theap_visit_a != NULL && theap_visit_b != NULL && mi_heap_contains(heap, theap_visit_a) && mi_heap_contains(heap, theap_visit_b));
  struct heap_visit_result theap_visit_areas = { heap, 0, 0, 1, 1, 1 };
  bool theap_visit_areas_ok = theap != NULL && mi_theap_visit_blocks(theap, false, heap_visit_counting_visitor, &theap_visit_areas);
  result_bool("theap_visit_areas_success", theap_visit_areas_ok);
  result_bool("theap_visit_areas_only", theap_visit_areas_ok && theap_visit_areas.areas > 0 && theap_visit_areas.blocks == 0);
  result_bool("theap_visit_area_fields", theap_visit_areas.heap_matches && theap_visit_areas.area_fields_ok);
  struct heap_visit_result theap_visit_blocks = { heap, 0, 0, 1, 1, 1 };
  bool theap_visit_blocks_ok = theap != NULL && mi_theap_visit_blocks(theap, true, heap_visit_counting_visitor, &theap_visit_blocks);
  result_bool("theap_visit_blocks_success", theap_visit_blocks_ok);
  result_bool("theap_visit_blocks_seen", theap_visit_blocks_ok && theap_visit_blocks.blocks > 0);
  result_bool("theap_visit_block_fields", theap_visit_blocks.heap_matches && theap_visit_blocks.area_fields_ok && theap_visit_blocks.block_fields_ok);
  struct heap_visit_result theap_visit_stop = { heap, 0, 0, 1, 1, 1 };
  bool theap_visit_stop_ok = theap != NULL && mi_theap_visit_blocks(theap, true, heap_visit_stop_visitor, &theap_visit_stop);
  result_bool("theap_visit_stop_returns_false", !theap_visit_stop_ok && theap_visit_stop.areas == 1);
  mi_free(theap_visit_a);
  mi_free(theap_visit_b);

  char resolved[4096];
  char* rp_stack = mi_realpath(".", resolved);
  result_bool("realpath_stack_success", rp_stack == resolved && resolved[0] == '/');
  char* rp = mi_realpath(".", NULL);
  result_bool("realpath_alloc_success", rp != NULL && strcmp(rp, resolved) == 0);
  result_bool("realpath_alloc_owned", rp != NULL && mi_check_owned(rp));
  if (rp != NULL) mi_free(rp);
  errno = 0;
  rp = mi_realpath("/definitely/not/a/mechanical_mimalloc/path", NULL);
  result_bool("realpath_missing_errno", rp == NULL && errno == ENOENT);
  errno = 0;
  rp = mi_realpath("/definitely/not/a/mechanical_mimalloc/path", resolved);
  result_bool("realpath_stack_missing_errno", rp == NULL && errno == ENOENT);

  long original_visit_abandoned = mi_option_get(mi_option_visit_abandoned);
  mi_option_enable(mi_option_visit_abandoned);
  struct heap_abandoned_result abandoned_collect = { heap, NULL, NULL, NULL, 0 };
  pthread_t abandoned_collect_thread;
  int abandoned_collect_started = (pthread_create(&abandoned_collect_thread, NULL, heap_abandoned_worker, &abandoned_collect) == 0);
  result_bool("heap_collect_abandoned_started", abandoned_collect_started);
  int abandoned_collect_joined = abandoned_collect_started && pthread_join(abandoned_collect_thread, NULL) == 0;
  result_bool("heap_collect_abandoned_joined", abandoned_collect_joined);
  result_bool("heap_collect_abandoned_allocated", abandoned_collect_joined && abandoned_collect.allocated);
  mi_heap_collect(heap, false);
  mi_heap_collect(heap, true);
  struct heap_visit_result abandoned_collect_visit = { heap, 0, 0, 1, 1, 1 };
  bool abandoned_collect_ok = mi_heap_visit_abandoned_blocks(heap, true, heap_visit_counting_visitor, &abandoned_collect_visit);
  result_bool("heap_collect_abandoned_visit_success", abandoned_collect_ok);
  result_bool("heap_collect_abandoned_visit_seen", abandoned_collect_ok && abandoned_collect_visit.areas > 0 && abandoned_collect_visit.blocks > 0);
  result_bool("heap_collect_abandoned_visit_fields", abandoned_collect_visit.heap_matches && abandoned_collect_visit.area_fields_ok && abandoned_collect_visit.block_fields_ok);
  mi_free(abandoned_collect.a);
  mi_free(abandoned_collect.b);
  mi_free(abandoned_collect.c);

  mi_option_disable(mi_option_visit_abandoned);
  struct heap_abandoned_result abandoned_disabled = { heap, NULL, NULL, NULL, 0 };
  pthread_t abandoned_disabled_thread;
  int abandoned_disabled_started = (pthread_create(&abandoned_disabled_thread, NULL, heap_abandoned_worker, &abandoned_disabled) == 0);
  result_bool("heap_visit_abandoned_disabled_started", abandoned_disabled_started);
  int abandoned_disabled_joined = abandoned_disabled_started && pthread_join(abandoned_disabled_thread, NULL) == 0;
  result_bool("heap_visit_abandoned_disabled_joined", abandoned_disabled_joined);
  result_bool("heap_visit_abandoned_disabled_allocated", abandoned_disabled_joined && abandoned_disabled.allocated);
  struct heap_visit_result abandoned_disabled_visit = { heap, 0, 0, 1, 1, 1 };
  bool abandoned_disabled_ok = mi_heap_visit_abandoned_blocks(heap, true, heap_visit_counting_visitor, &abandoned_disabled_visit);
  result_bool("heap_visit_abandoned_disabled_success", abandoned_disabled_ok);
  result_bool("heap_visit_abandoned_disabled_seen", abandoned_disabled_ok && abandoned_disabled_visit.areas > 0 && abandoned_disabled_visit.blocks > 0);
  result_bool("heap_visit_abandoned_disabled_fields", abandoned_disabled_visit.heap_matches && abandoned_disabled_visit.area_fields_ok && abandoned_disabled_visit.block_fields_ok);
  mi_free(abandoned_disabled.a);
  mi_free(abandoned_disabled.b);
  mi_free(abandoned_disabled.c);

  mi_option_enable(mi_option_visit_abandoned);
  struct heap_abandoned_result abandoned_enabled = { heap, NULL, NULL, NULL, 0 };
  pthread_t abandoned_enabled_thread;
  int abandoned_enabled_started = (pthread_create(&abandoned_enabled_thread, NULL, heap_abandoned_worker, &abandoned_enabled) == 0);
  result_bool("heap_visit_abandoned_enabled_started", abandoned_enabled_started);
  int abandoned_enabled_joined = abandoned_enabled_started && pthread_join(abandoned_enabled_thread, NULL) == 0;
  result_bool("heap_visit_abandoned_enabled_joined", abandoned_enabled_joined);
  result_bool("heap_visit_abandoned_enabled_allocated", abandoned_enabled_joined && abandoned_enabled.allocated);
  struct heap_visit_result abandoned_areas = { heap, 0, 0, 1, 1, 1 };
  bool abandoned_areas_ok = mi_heap_visit_abandoned_blocks(heap, false, heap_visit_counting_visitor, &abandoned_areas);
  result_bool("heap_visit_abandoned_areas_success", abandoned_areas_ok);
  result_bool("heap_visit_abandoned_areas_only", abandoned_areas_ok && abandoned_areas.areas > 0 && abandoned_areas.blocks == 0);
  result_bool("heap_visit_abandoned_area_fields", abandoned_areas.heap_matches && abandoned_areas.area_fields_ok);
  struct heap_visit_result abandoned_blocks = { heap, 0, 0, 1, 1, 1 };
  bool abandoned_blocks_ok = mi_heap_visit_abandoned_blocks(heap, true, heap_visit_counting_visitor, &abandoned_blocks);
  result_bool("heap_visit_abandoned_blocks_success", abandoned_blocks_ok);
  result_bool("heap_visit_abandoned_blocks_seen", abandoned_blocks_ok && abandoned_blocks.blocks > 0);
  result_bool("heap_visit_abandoned_block_fields", abandoned_blocks.heap_matches && abandoned_blocks.area_fields_ok && abandoned_blocks.block_fields_ok);
  struct heap_visit_result abandoned_stop = { heap, 0, 0, 1, 1, 1 };
  bool abandoned_stop_ok = mi_heap_visit_abandoned_blocks(heap, true, heap_visit_stop_visitor, &abandoned_stop);
  result_bool("heap_visit_abandoned_stop_returns_false", !abandoned_stop_ok && abandoned_stop.areas == 1);
  mi_free(abandoned_enabled.a);
  mi_free(abandoned_enabled.b);
  mi_free(abandoned_enabled.c);
  mi_option_set(mi_option_visit_abandoned, original_visit_abandoned);

  if (heap != NULL) mi_heap_delete(heap);

  result_size("good_size_17", mi_good_size(17));
  result_size("malloc_good_size_17", mi_malloc_good_size(17));
  result_size("good_size_0", mi_good_size(0));
  result_size("malloc_good_size_0", mi_malloc_good_size(0));
  result_size("good_size_small_max", mi_good_size(MI_SMALL_SIZE_MAX));
  result_size("malloc_good_size_small_max", mi_malloc_good_size(MI_SMALL_SIZE_MAX));
  result_size("good_size_small_max_plus_1", mi_good_size(MI_SMALL_SIZE_MAX + 1));
  result_size("malloc_good_size_small_max_plus_1", mi_malloc_good_size(MI_SMALL_SIZE_MAX + 1));
  result_size("good_size_size_max_minus_1", mi_good_size(overflow_count - 1));
  result_size("malloc_good_size_size_max_minus_1", mi_malloc_good_size(overflow_count - 1));
  result_size("good_size_size_max", mi_good_size(overflow_count));
  result_size("malloc_good_size_size_max", mi_malloc_good_size(overflow_count));

  size_t threshold_sizes[] = {
    MI_SMALL_MAX_OBJ_SIZE - 1,
    MI_SMALL_MAX_OBJ_SIZE,
    MI_SMALL_MAX_OBJ_SIZE + 1,
    MI_MEDIUM_MAX_OBJ_SIZE - 1,
    MI_MEDIUM_MAX_OBJ_SIZE,
    MI_MEDIUM_MAX_OBJ_SIZE + 1,
    MI_LARGE_MAX_OBJ_SIZE - 1,
    MI_LARGE_MAX_OBJ_SIZE,
    MI_LARGE_MAX_OBJ_SIZE + 1,
    MI_SMALL_PAGE_SIZE - 1,
    MI_SMALL_PAGE_SIZE,
    MI_SMALL_PAGE_SIZE + 1,
    MI_MEDIUM_PAGE_SIZE - 1,
    MI_MEDIUM_PAGE_SIZE,
    MI_MEDIUM_PAGE_SIZE + 1,
    MI_LARGE_PAGE_SIZE - 1,
    MI_LARGE_PAGE_SIZE,
    MI_LARGE_PAGE_SIZE + 1,
    MI_ARENA_MAX_CHUNK_OBJ_SIZE - 1,
    MI_ARENA_MAX_CHUNK_OBJ_SIZE,
    MI_ARENA_MAX_CHUNK_OBJ_SIZE + 1,
  };
  size_t threshold_count = sizeof(threshold_sizes) / sizeof(threshold_sizes[0]);
  for (size_t i = 0; i < threshold_count; i++) {
    size_t requested = threshold_sizes[i];
    result_indexed_size("threshold_request", i, requested);
    result_indexed_size("threshold_good_size", i, mi_good_size(requested));
    result_indexed_size("threshold_malloc_good_size", i, mi_malloc_good_size(requested));
    void* threshold_ptr = mi_malloc(requested);
    result_bool("threshold_malloc_nonnull", threshold_ptr != NULL);
    result_indexed_size("threshold_malloc_usable_size", i, threshold_ptr == NULL ? 0 : mi_usable_size(threshold_ptr));
    mi_free(threshold_ptr);
  }

  size_t max_alloc_boundary_sizes[] = {
    ((size_t)MI_MAX_ALLOC_SIZE) - 1,
    (size_t)MI_MAX_ALLOC_SIZE,
    ((size_t)MI_MAX_ALLOC_SIZE) + 1,
    overflow_count,
  };
  size_t max_alloc_boundary_count = sizeof(max_alloc_boundary_sizes) / sizeof(max_alloc_boundary_sizes[0]);
  for (size_t i = 0; i < max_alloc_boundary_count; i++) {
    errno = 0;
    void* max_ptr = mi_malloc(max_alloc_boundary_sizes[i]);
    result_bool("max_alloc_boundary_null", max_ptr == NULL);
    result_indexed_size("max_alloc_boundary_errno", i, (size_t)errno);
    mi_free(max_ptr);
  }

  size_t alignment_boundary_values[] = {
    MI_MAX_ALIGN_SIZE / 2,
    MI_MAX_ALIGN_SIZE,
    MI_MAX_ALIGN_SIZE * 2,
    MI_SMALL_PAGE_SIZE / 2,
    MI_SMALL_PAGE_SIZE,
    MI_SMALL_PAGE_SIZE * 2,
  };
  size_t alignment_boundary_count = sizeof(alignment_boundary_values) / sizeof(alignment_boundary_values[0]);
  for (size_t i = 0; i < alignment_boundary_count; i++) {
    size_t alignment = alignment_boundary_values[i];
    void* aligned_ptr = mi_malloc_aligned(257, alignment);
    result_bool("alignment_boundary_nonnull", aligned_ptr != NULL);
    result_bool("alignment_boundary_aligned", is_aligned_at(aligned_ptr, alignment, 0));
    result_indexed_size("alignment_boundary_usable_size", i, aligned_ptr == NULL ? 0 : mi_usable_size(aligned_ptr));
    mi_free(aligned_ptr);
  }

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

  mi_stats_t malloc_stats_before;
  mi_stats_t malloc_stats_live;
  mi_stats_t malloc_stats_after;
  mi_stats_init(&malloc_stats_before);
  mi_stats_init(&malloc_stats_live);
  mi_stats_init(&malloc_stats_after);
  result_bool("malloc_stats_before_ok", mi_stats_get(&malloc_stats_before));
  size_t malloc_family_sizes[] = { 0, 1, MI_SMALL_SIZE_MAX, MI_SMALL_SIZE_MAX + 1,
                                   64*1024, 1024*1024, 2*1024*1024, 4*1024*1024 };
  void* malloc_family_ptrs[sizeof(malloc_family_sizes) / sizeof(malloc_family_sizes[0])];
  size_t malloc_family_count = sizeof(malloc_family_sizes) / sizeof(malloc_family_sizes[0]);
  for (size_t i = 0; i < malloc_family_count; i++) {
    size_t requested = malloc_family_sizes[i];
    errno = 0;
    void* mp = mi_malloc(requested);
    malloc_family_ptrs[i] = mp;
    result_bool("malloc_family_nonnull", mp != NULL);
    result_bool("malloc_family_errno_unchanged", errno == 0);
    result_bool("malloc_family_usable", mp != NULL && mi_usable_size(mp) >= requested);
    result_bool("malloc_family_size_matches_usable", mp != NULL && mi_malloc_size(mp) == mi_malloc_usable_size(mp));
    result_indexed_size("malloc_family_usable_size", i, mp == NULL ? 0 : mi_usable_size(mp));
  }
  result_bool("malloc_stats_live_ok", mi_stats_get(&malloc_stats_live));
  result_bool("malloc_stats_live_increased",
              malloc_stats_live.malloc_normal.current + malloc_stats_live.malloc_huge.current >=
              malloc_stats_before.malloc_normal.current + malloc_stats_before.malloc_huge.current &&
              malloc_stats_live.malloc_normal.peak + malloc_stats_live.malloc_huge.peak >=
              malloc_stats_live.malloc_normal.current + malloc_stats_live.malloc_huge.current &&
              malloc_stats_live.malloc_normal.total + malloc_stats_live.malloc_huge.total >=
              malloc_stats_before.malloc_normal.total + malloc_stats_before.malloc_huge.total);
  for (size_t i = 0; i < malloc_family_count; i++) {
    mi_free(malloc_family_ptrs[i]);
  }
  result_bool("malloc_stats_after_ok", mi_stats_get(&malloc_stats_after));
  result_bool("malloc_stats_after_decreased",
              malloc_stats_after.malloc_normal.current + malloc_stats_after.malloc_huge.current <=
              malloc_stats_live.malloc_normal.current + malloc_stats_live.malloc_huge.current &&
              malloc_stats_after.malloc_normal.peak + malloc_stats_after.malloc_huge.peak >=
              malloc_stats_live.malloc_normal.current + malloc_stats_live.malloc_huge.current &&
              malloc_stats_after.malloc_normal.total + malloc_stats_after.malloc_huge.total >=
              malloc_stats_live.malloc_normal.total + malloc_stats_live.malloc_huge.total);
  errno = 0;
  result_bool("malloc_oom_null", mi_malloc(overflow_count) == NULL);
  result_size("malloc_oom_errno", (size_t)errno);

  mi_stats_t reset_exclusive_before;
  mi_stats_t reset_live;
  mi_stats_t reset_exclusive_after;
  mi_stats_t reset_aggregate_after;
  mi_stats_t reset_after_free;
  mi_stats_t reset_exclusive_final;
  mi_stats_init(&reset_exclusive_before);
  mi_stats_init(&reset_live);
  mi_stats_init(&reset_exclusive_after);
  mi_stats_init(&reset_aggregate_after);
  mi_stats_init(&reset_after_free);
  mi_stats_init(&reset_exclusive_final);
  result_bool("stats_reset_exclusive_before_ok", mi_subproc_stats_get_exclusive(mi_subproc_current(), &reset_exclusive_before));
  void* reset_a = mi_malloc(4096);
  void* reset_b = mi_malloc(1024 * 1024);
  result_bool("stats_reset_allocated", reset_a != NULL && reset_b != NULL);
  result_bool("stats_reset_live_ok", mi_stats_get(&reset_live));
  result_bool("stats_reset_live_increased",
              stats_alloc_current(&reset_live) >= stats_alloc_current(&reset_exclusive_before) &&
              stats_alloc_total(&reset_live) >= stats_alloc_total(&reset_exclusive_before));
  mi_stats_reset();
  result_bool("stats_reset_exclusive_after_ok", mi_subproc_stats_get_exclusive(mi_subproc_current(), &reset_exclusive_after));
  result_bool("stats_reset_aggregate_after_ok", mi_stats_get(&reset_aggregate_after));
  result_bool("stats_reset_exclusive_total_persistent",
              stats_alloc_total(&reset_exclusive_after) >= stats_alloc_total(&reset_exclusive_before));
  result_bool("stats_reset_exclusive_current_merged",
              stats_alloc_current(&reset_exclusive_after) >= stats_alloc_current(&reset_exclusive_before));
  result_bool("stats_reset_aggregate_keeps_live_current",
              stats_alloc_current(&reset_aggregate_after) >= stats_alloc_current(&reset_live));
  mi_free(reset_a);
  mi_free(reset_b);
  result_bool("stats_reset_after_free_ok", mi_stats_get(&reset_after_free));
  result_bool("stats_reset_after_free_decreased",
              stats_alloc_current(&reset_after_free) <= stats_alloc_current(&reset_aggregate_after));
  mi_stats_reset();
  result_bool("stats_reset_exclusive_final_ok", mi_subproc_stats_get_exclusive(mi_subproc_current(), &reset_exclusive_final));
  result_bool("stats_reset_final_current_decreased",
              stats_alloc_current(&reset_exclusive_final) <= stats_alloc_current(&reset_exclusive_after));
  result_bool("stats_reset_final_total_persistent",
              stats_alloc_total(&reset_exclusive_final) >= stats_alloc_total(&reset_exclusive_after));
  result_bool("stats_reset_peak_persistent",
              stats_alloc_peak(&reset_exclusive_final) >= stats_alloc_peak(&reset_exclusive_after));

  mi_stats_reset();
  mi_stats_t normal_stats_before;
  mi_stats_t normal_stats_live;
  mi_stats_t normal_stats_after;
  mi_stats_init(&normal_stats_before);
  mi_stats_init(&normal_stats_live);
  mi_stats_init(&normal_stats_after);
  result_bool("stats_normal_before_ok", mi_stats_get(&normal_stats_before));
  size_t normal_stat_sizes[] = { 16, 128, 4096, 65536 };
  size_t normal_stat_count = sizeof(normal_stat_sizes) / sizeof(normal_stat_sizes[0]);
  void* normal_stat_ptrs[sizeof(normal_stat_sizes) / sizeof(normal_stat_sizes[0])];
  size_t normal_stat_requested = 0;
  for (size_t i = 0; i < normal_stat_count; i++) {
    normal_stat_requested += normal_stat_sizes[i];
    normal_stat_ptrs[i] = mi_malloc(normal_stat_sizes[i]);
    result_bool("stats_normal_allocated", normal_stat_ptrs[i] != NULL);
  }
  result_bool("stats_normal_live_ok", mi_stats_get(&normal_stats_live));
  int64_t normal_current_delta = normal_stats_live.malloc_normal.current - normal_stats_before.malloc_normal.current;
  int64_t normal_requested_delta = normal_stats_live.malloc_requested.current - normal_stats_before.malloc_requested.current;
  int64_t normal_count_delta = normal_stats_live.malloc_normal_count.total - normal_stats_before.malloc_normal_count.total;
  int64_t normal_bins_before = stats_count_current_sum(normal_stats_before.malloc_bins, MI_BIN_HUGE + 1);
  int64_t normal_bins_live = stats_count_current_sum(normal_stats_live.malloc_bins, MI_BIN_HUGE + 1);
  int64_t normal_page_bins_before_total = stats_count_total_sum(normal_stats_before.page_bins, MI_BIN_HUGE + 1);
  int64_t normal_page_bins_live_total = stats_count_total_sum(normal_stats_live.page_bins, MI_BIN_HUGE + 1);
  result_bool("stats_normal_current_delta",
              normal_current_delta >= (int64_t)normal_stat_requested);
  result_bool("stats_normal_requested_delta",
              normal_requested_delta >= (int64_t)normal_stat_requested &&
              normal_requested_delta <= normal_current_delta);
  result_bool("stats_normal_count_delta", normal_count_delta >= (int64_t)normal_stat_count);
  result_bool("stats_normal_bin_current_delta", normal_bins_live - normal_bins_before >= (int64_t)normal_stat_count);
  result_bool("stats_normal_page_bin_total_monotonic", normal_page_bins_live_total >= normal_page_bins_before_total);
  result_bool("stats_normal_peak_consistent",
              normal_stats_live.malloc_normal.peak >= normal_stats_live.malloc_normal.current &&
              normal_stats_live.malloc_requested.peak >= normal_stats_live.malloc_requested.current &&
              stats_count_peak_sum(normal_stats_live.malloc_bins, MI_BIN_HUGE + 1) >=
              stats_count_current_sum(normal_stats_live.malloc_bins, MI_BIN_HUGE + 1) &&
              stats_count_peak_sum(normal_stats_live.page_bins, MI_BIN_HUGE + 1) >=
              stats_count_current_sum(normal_stats_live.page_bins, MI_BIN_HUGE + 1));
  for (size_t i = 0; i < normal_stat_count; i++) {
    mi_free(normal_stat_ptrs[i]);
  }
  result_bool("stats_normal_after_ok", mi_stats_get(&normal_stats_after));
  result_bool("stats_normal_current_after_free",
              normal_stats_after.malloc_normal.current <= normal_stats_live.malloc_normal.current &&
              normal_stats_after.malloc_requested.current <= normal_stats_live.malloc_requested.current);
  result_bool("stats_normal_bin_current_after_free",
              stats_count_current_sum(normal_stats_after.malloc_bins, MI_BIN_HUGE + 1) <=
              stats_count_current_sum(normal_stats_live.malloc_bins, MI_BIN_HUGE + 1));
  result_bool("stats_normal_totals_persistent",
              normal_stats_after.malloc_normal.total >= normal_stats_live.malloc_normal.total &&
              normal_stats_after.malloc_requested.total >= normal_stats_live.malloc_requested.total &&
              normal_stats_after.malloc_normal_count.total >= normal_stats_live.malloc_normal_count.total &&
              stats_count_total_sum(normal_stats_after.malloc_bins, MI_BIN_HUGE + 1) >=
              stats_count_total_sum(normal_stats_live.malloc_bins, MI_BIN_HUGE + 1));

  size_t large_threshold_sizes[] = {
    MI_LARGE_MAX_OBJ_SIZE - 1,
    MI_LARGE_MAX_OBJ_SIZE,
    MI_LARGE_MAX_OBJ_SIZE + 1,
  };
  size_t large_threshold_count = sizeof(large_threshold_sizes) / sizeof(large_threshold_sizes[0]);
  for (size_t i = 0; i < large_threshold_count; i++) {
    size_t requested = large_threshold_sizes[i];
    mi_stats_t large_before;
    mi_stats_t large_live;
    mi_stats_t large_after;
    mi_stats_init(&large_before);
    mi_stats_init(&large_live);
    mi_stats_init(&large_after);
    mi_stats_reset();
    result_bool("large_threshold_before_ok", mi_stats_get(&large_before));
    void* large_ptr = mi_malloc(requested);
    size_t large_usable = large_ptr == NULL ? 0 : mi_usable_size(large_ptr);
    result_bool("large_threshold_nonnull", large_ptr != NULL);
    result_bool("large_threshold_usable", large_ptr != NULL && large_usable >= requested);
    result_bool("large_threshold_live_ok", mi_stats_get(&large_live));
    int64_t normal_current_delta = large_live.malloc_normal.current - large_before.malloc_normal.current;
    int64_t huge_current_delta = large_live.malloc_huge.current - large_before.malloc_huge.current;
    int64_t normal_count_delta = large_live.malloc_normal_count.total - large_before.malloc_normal_count.total;
    int64_t huge_count_delta = large_live.malloc_huge_count.total - large_before.malloc_huge_count.total;
    result_bool("large_threshold_accounted", normal_current_delta > 0 || huge_current_delta > 0);
    result_indexed_size("large_threshold_normal_current_delta", i, (size_t)normal_current_delta);
    result_indexed_size("large_threshold_huge_current_delta", i, (size_t)huge_current_delta);
    result_indexed_size("large_threshold_normal_count_delta", i, (size_t)normal_count_delta);
    result_indexed_size("large_threshold_huge_count_delta", i, (size_t)huge_count_delta);
    result_indexed_size("large_threshold_usable_size", i, large_usable);
    mi_free(large_ptr);
    result_bool("large_threshold_after_ok", mi_stats_get(&large_after));
    result_bool("large_threshold_current_after_free",
                stats_alloc_current(&large_after) <= stats_alloc_current(&large_live));
  }

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
  result_bool("malloc_usable_size_null_zero", mi_malloc_usable_size(NULL) == 0);
  result_size("usable_size_foreign_status", (size_t)foreign_size_case_status(0));
  result_size("malloc_usable_size_foreign_status", (size_t)foreign_size_case_status(1));
  result_size("malloc_size_foreign_status", (size_t)foreign_size_case_status(2));

  size_t arena_invalid_size = 123;
  result_bool("arena_area_invalid_null", mi_arena_area(NULL, &arena_invalid_size) == NULL && arena_invalid_size == 0);
  result_bool("arena_area_invalid_no_size", mi_arena_area(NULL, NULL) == NULL);
  result_bool("arena_contains_invalid_false", !mi_arena_contains(NULL, &arena_invalid_size));
  result_size("arena_min_alignment", mi_arena_min_alignment());
  result_size("arena_min_size", mi_arena_min_size());
  result_bool("arena_min_alignment_power2", mi_arena_min_alignment() != 0 && (mi_arena_min_alignment() & (mi_arena_min_alignment() - 1)) == 0);
  result_bool("arena_min_size_aligned", mi_arena_min_size() != 0 && (mi_arena_min_size() % mi_arena_min_alignment()) == 0);

  int reserve_basic_err = mi_reserve_os_memory(mi_arena_min_size(), false, false);
  result_bool("reserve_os_memory_basic_uncommitted", reserve_basic_err == 0);

  mi_arena_id_t reserve_uncommitted_id = NULL;
  int reserve_uncommitted_err = mi_reserve_os_memory_ex(mi_arena_min_size(), false, false, false, &reserve_uncommitted_id);
  result_bool("reserve_os_memory_ex_uncommitted_nonexclusive", reserve_uncommitted_err == 0 && reserve_uncommitted_id != NULL);
  mi_heap_t* reserve_uncommitted_heap = reserve_uncommitted_id != NULL ? mi_heap_new_in_arena(reserve_uncommitted_id) : NULL;
  void* reserve_uncommitted_alloc = reserve_uncommitted_heap == NULL ? NULL : mi_heap_malloc(reserve_uncommitted_heap, 512);
  result_bool("reserve_os_memory_ex_uncommitted_alloc", reserve_uncommitted_alloc != NULL && mi_arena_contains(reserve_uncommitted_id, reserve_uncommitted_alloc));
  mi_free(reserve_uncommitted_alloc);
  if (reserve_uncommitted_heap != NULL) mi_heap_delete(reserve_uncommitted_heap);

  mi_arena_id_t reserve_large_allowed_id = NULL;
  int reserve_large_allowed_err = mi_reserve_os_memory_ex(mi_arena_min_size(), true, true, false, &reserve_large_allowed_id);
  result_bool("reserve_os_memory_ex_large_allowed", reserve_large_allowed_err == 0 && reserve_large_allowed_id != NULL);
  mi_heap_t* reserve_large_allowed_heap = reserve_large_allowed_id != NULL ? mi_heap_new_in_arena(reserve_large_allowed_id) : NULL;
  void* reserve_large_allowed_alloc = reserve_large_allowed_heap == NULL ? NULL : mi_heap_malloc(reserve_large_allowed_heap, 512);
  result_bool("reserve_os_memory_ex_large_allowed_alloc", reserve_large_allowed_alloc != NULL && mi_arena_contains(reserve_large_allowed_id, reserve_large_allowed_alloc));
  mi_free(reserve_large_allowed_alloc);
  if (reserve_large_allowed_heap != NULL) mi_heap_delete(reserve_large_allowed_heap);

  size_t manage_os_size = mi_arena_min_size();
  void* manage_os_basic_mem = map_aligned_arena_memory(manage_os_size);
  bool manage_os_basic_ok = manage_os_basic_mem != MAP_FAILED && mi_manage_os_memory(manage_os_basic_mem, manage_os_size, true, false, true, -1);
  result_bool("manage_os_memory_basic_registered", manage_os_basic_ok);

  void* manage_os_committed_mem = map_aligned_arena_memory(manage_os_size);
  mi_arena_id_t manage_os_committed_id = NULL;
  bool manage_os_committed_ok = manage_os_committed_mem != MAP_FAILED && mi_manage_os_memory_ex(manage_os_committed_mem, manage_os_size, true, false, true, 0, true, &manage_os_committed_id);
  result_bool("manage_os_memory_ex_committed_zero_numa", manage_os_committed_ok && manage_os_committed_id != NULL);
  size_t manage_os_committed_area_size = 0;
  void* manage_os_committed_area = manage_os_committed_ok ? mi_arena_area(manage_os_committed_id, &manage_os_committed_area_size) : NULL;
  result_bool("manage_os_memory_ex_committed_area", manage_os_committed_area == manage_os_committed_mem && manage_os_committed_area_size == manage_os_size);
  mi_heap_t* manage_os_committed_heap = manage_os_committed_id != NULL ? mi_heap_new_in_arena(manage_os_committed_id) : NULL;
  void* manage_os_committed_alloc = manage_os_committed_heap == NULL ? NULL : mi_heap_malloc(manage_os_committed_heap, 2048);
  result_bool("manage_os_memory_ex_committed_alloc", manage_os_committed_alloc != NULL && mi_arena_contains(manage_os_committed_id, manage_os_committed_alloc));
  mi_free(manage_os_committed_alloc);
  if (manage_os_committed_heap != NULL) mi_heap_collect(manage_os_committed_heap, true);
  mi_collect(true);
  void* manage_os_committed_reuse = manage_os_committed_heap == NULL ? NULL : mi_heap_malloc(manage_os_committed_heap, 2048);
  result_bool("manage_os_memory_ex_committed_reuse_after_collect", manage_os_committed_reuse != NULL && mi_arena_contains(manage_os_committed_id, manage_os_committed_reuse));
  mi_free(manage_os_committed_reuse);
  if (manage_os_committed_heap != NULL) mi_heap_delete(manage_os_committed_heap);

  void* manage_os_uncommitted_mem = map_aligned_arena_memory(manage_os_size);
  mi_arena_id_t manage_os_uncommitted_id = NULL;
  bool manage_os_uncommitted_ok = manage_os_uncommitted_mem != MAP_FAILED && mi_manage_os_memory_ex(manage_os_uncommitted_mem, manage_os_size, false, false, false, -1, true, &manage_os_uncommitted_id);
  result_bool("manage_os_memory_ex_uncommitted", manage_os_uncommitted_ok && manage_os_uncommitted_id != NULL);
  mi_heap_t* manage_os_uncommitted_heap = manage_os_uncommitted_id != NULL ? mi_heap_new_in_arena(manage_os_uncommitted_id) : NULL;
  void* manage_os_uncommitted_alloc = manage_os_uncommitted_heap == NULL ? NULL : mi_heap_malloc(manage_os_uncommitted_heap, 2048);
  result_bool("manage_os_memory_ex_uncommitted_alloc", manage_os_uncommitted_alloc != NULL && mi_arena_contains(manage_os_uncommitted_id, manage_os_uncommitted_alloc));
  mi_free(manage_os_uncommitted_alloc);
  if (manage_os_uncommitted_heap != NULL) mi_heap_collect(manage_os_uncommitted_heap, true);
  mi_collect(true);
  void* manage_os_uncommitted_reuse = manage_os_uncommitted_heap == NULL ? NULL : mi_heap_malloc(manage_os_uncommitted_heap, 2048);
  result_bool("manage_os_memory_ex_uncommitted_reuse_after_collect", manage_os_uncommitted_reuse != NULL && mi_arena_contains(manage_os_uncommitted_id, manage_os_uncommitted_reuse));
  mi_free(manage_os_uncommitted_reuse);
  if (manage_os_uncommitted_heap != NULL) mi_heap_delete(manage_os_uncommitted_heap);

  void* manage_os_pinned_mem = map_aligned_arena_memory(manage_os_size);
  mi_arena_id_t manage_os_pinned_id = NULL;
  bool manage_os_pinned_ok = manage_os_pinned_mem != MAP_FAILED && mi_manage_os_memory_ex(manage_os_pinned_mem, manage_os_size, true, true, true, -1, true, &manage_os_pinned_id);
  result_bool("manage_os_memory_ex_pinned", manage_os_pinned_ok && manage_os_pinned_id != NULL);
  mi_heap_t* manage_os_pinned_heap = manage_os_pinned_id != NULL ? mi_heap_new_in_arena(manage_os_pinned_id) : NULL;
  void* manage_os_pinned_alloc = manage_os_pinned_heap == NULL ? NULL : mi_heap_malloc(manage_os_pinned_heap, 2048);
  result_bool("manage_os_memory_ex_pinned_alloc", manage_os_pinned_alloc != NULL && mi_arena_contains(manage_os_pinned_id, manage_os_pinned_alloc));
  mi_free(manage_os_pinned_alloc);
  if (manage_os_pinned_heap != NULL) mi_heap_collect(manage_os_pinned_heap, true);
  mi_collect(true);
  void* manage_os_pinned_reuse = manage_os_pinned_heap == NULL ? NULL : mi_heap_malloc(manage_os_pinned_heap, 2048);
  result_bool("manage_os_memory_ex_pinned_reuse_after_collect", manage_os_pinned_reuse != NULL && mi_arena_contains(manage_os_pinned_id, manage_os_pinned_reuse));
  mi_free(manage_os_pinned_reuse);
  if (manage_os_pinned_heap != NULL) mi_heap_delete(manage_os_pinned_heap);

  mi_arena_id_t arena_id = NULL;
  int arena_err = mi_reserve_os_memory_ex(mi_arena_min_size(), true, false, true, &arena_id);
  result_bool("arena_reserve_for_inspection", arena_err == 0 && arena_id != NULL);
  size_t arena_size = 0;
  void* arena_area = (arena_err == 0 ? mi_arena_area(arena_id, &arena_size) : NULL);
  result_bool("arena_area_valid_nonnull", arena_area != NULL && arena_size >= mi_arena_min_size());
  result_bool("arena_area_valid_aligned", arena_area != NULL && ((uintptr_t)arena_area % mi_arena_min_alignment()) == 0);
  result_bool("arena_area_size_aligned", arena_size != 0 && (arena_size % mi_arena_min_alignment()) == 0);
  result_bool("arena_contains_start", arena_area != NULL && mi_arena_contains(arena_id, arena_area));
  result_bool("arena_contains_inside", arena_area != NULL && mi_arena_contains(arena_id, (char*)arena_area + mi_arena_min_alignment()));
  result_bool("arena_contains_end_false", arena_area != NULL && !mi_arena_contains(arena_id, (char*)arena_area + arena_size));
  int arena_stack_value = 0;
  result_bool("arena_contains_stack_false", !mi_arena_contains(arena_id, &arena_stack_value));
  mi_heap_t* arena_heap = arena_id != NULL ? mi_heap_new_in_arena(arena_id) : NULL;
  void* arena_heap_alloc = arena_heap == NULL ? NULL : mi_heap_malloc(arena_heap, 1024);
  result_bool("heap_new_in_arena_alloc_contained", arena_heap_alloc != NULL && mi_heap_contains(arena_heap, arena_heap_alloc) && mi_arena_contains(arena_id, arena_heap_alloc));
  void* arena_normal_alloc = mi_malloc(1024);
  result_bool("heap_new_in_arena_normal_isolated", arena_normal_alloc != NULL && !mi_arena_contains(arena_id, arena_normal_alloc));
  void* arena_heap_oversize = arena_heap == NULL ? NULL : mi_heap_malloc(arena_heap, arena_size + mi_arena_min_size());
  result_bool("heap_new_in_arena_oversize_fails", arena_heap_oversize == NULL);
  mi_free(arena_heap_oversize);
  mi_free(arena_normal_alloc);
  mi_free(arena_heap_alloc);
  if (arena_heap != NULL) mi_heap_delete(arena_heap);

  long managed_original_purge_delay = mi_option_get(mi_option_purge_delay);
  long managed_original_purge_decommits = mi_option_get(mi_option_purge_decommits);
  mi_option_set(mi_option_purge_delay, 0);
  mi_option_set(mi_option_purge_decommits, 1);

  size_t managed_size = mi_arena_min_size();
  size_t managed_map_size = managed_size + mi_arena_min_alignment();
  void* managed_map = mmap(NULL, managed_map_size, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
  void* managed_mem = managed_map == MAP_FAILED ? MAP_FAILED : (void*)(((uintptr_t)managed_map + mi_arena_min_alignment() - 1) & ~(uintptr_t)(mi_arena_min_alignment() - 1));
  struct managed_commit_result managed_result = { 0, 0, 0, 0, 0, 0, 1 };
  mi_arena_id_t managed_arena_id = NULL;
  bool managed_ok = managed_mem != MAP_FAILED && mi_manage_memory(
    managed_mem,
    managed_size,
    false,
    false,
    false,
    -1,
    true,
    managed_commit_callback,
    &managed_result,
    &managed_arena_id);
  result_bool("manage_memory_callback_registered_arena", managed_ok && managed_arena_id != NULL);
  result_bool("manage_memory_callback_invoked", managed_result.commit_calls > 0 && managed_result.commit_bytes > 0);
  result_bool("manage_memory_callback_arg_seen", managed_result.arg_seen);
  result_bool("manage_memory_callback_alloc_called", managed_result.alloc_calls > 0);
  result_bool("manage_memory_callback_alloc_ok", managed_result.alloc_ok);
  result_size("manage_memory_commit_calls_after_register", managed_result.commit_calls);
  result_size("manage_memory_decommit_calls_after_register", managed_result.decommit_calls);
  size_t managed_area_size = 0;
  void* managed_area = managed_ok ? mi_arena_area(managed_arena_id, &managed_area_size) : NULL;
  result_bool("manage_memory_area_matches", managed_area == managed_mem && managed_area_size == managed_size);
  mi_heap_t* managed_heap = managed_ok ? mi_heap_new_in_arena(managed_arena_id) : NULL;
  void* managed_alloc = managed_heap == NULL ? NULL : mi_heap_malloc(managed_heap, 256);
  result_bool("manage_memory_exclusive_heap_alloc", managed_alloc != NULL && mi_heap_contains(managed_heap, managed_alloc) && mi_arena_contains(managed_arena_id, managed_alloc));
  mi_free(managed_alloc);
  void* managed_large = managed_heap == NULL ? NULL : mi_heap_malloc(managed_heap, 4 * 1024 * 1024);
  result_bool("manage_memory_large_commit_zero_state", managed_large != NULL && mi_arena_contains(managed_arena_id, managed_large) && managed_result.zero_state_seen);
  result_size("manage_memory_commit_calls_after_large", managed_result.commit_calls);
  mi_free(managed_large);
  if (managed_heap != NULL) mi_heap_collect(managed_heap, true);
  mi_collect(true);
  result_bool("manage_memory_decommit_callback", managed_result.decommit_calls > 0);
  result_size("manage_memory_decommit_calls_after_collect", managed_result.decommit_calls);
  if (managed_heap != NULL) mi_heap_delete(managed_heap);

  void* fail_map = mmap(NULL, managed_map_size, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
  void* fail_mem = fail_map == MAP_FAILED ? MAP_FAILED : (void*)(((uintptr_t)fail_map + mi_arena_min_alignment() - 1) & ~(uintptr_t)(mi_arena_min_alignment() - 1));
  struct managed_commit_result fail_result = { 0, 0, 0, 0, 0, 0, 1 };
  mi_arena_id_t fail_arena_id = NULL;
  bool fail_ok = fail_mem != MAP_FAILED && mi_manage_memory(
    fail_mem,
    managed_size,
    false,
    false,
    false,
    -1,
    true,
    managed_commit_fail_callback,
    &fail_result,
    &fail_arena_id);
  result_bool("manage_memory_failure_rollback", !fail_ok && fail_arena_id == NULL && fail_result.commit_calls > 0 && !mi_arena_contains(fail_arena_id, fail_mem));

  mi_option_set(mi_option_purge_delay, managed_original_purge_delay);
  mi_option_set(mi_option_purge_decommits, managed_original_purge_decommits);

  arena_output_callbacks = 0;
  arena_output_seen_arena = 0;
  arena_output_seen_chunks = 0;
  arena_output_seen_total = 0;
  mi_register_output(arena_output_callback, (void*)0xA4E4A);
  mi_debug_show_arenas();
  result_bool("debug_show_arenas_output_nonempty", arena_output_callbacks > 0);
  result_bool("debug_show_arenas_output_structure", arena_output_seen_arena && arena_output_seen_chunks && arena_output_seen_total);
  size_t debug_arena_output_callbacks = arena_output_callbacks;
  arena_output_seen_arena = 0;
  arena_output_seen_chunks = 0;
  arena_output_seen_total = 0;
  mi_arenas_print();
  result_bool("arenas_print_output_nonempty", arena_output_callbacks > debug_arena_output_callbacks);
  result_bool("arenas_print_output_structure", arena_output_seen_arena && arena_output_seen_chunks && arena_output_seen_total);
  mi_register_output(NULL, NULL);

  char json_buf[65536];
  memset(json_buf, 0xA5, sizeof(json_buf));
  char* json = mi_stats_get_json(sizeof(json_buf), json_buf);
  result_bool("stats_get_json_provided_buffer", json == json_buf);
  result_bool("stats_get_json_schema", stats_json_schema_ok(json));

  mi_stats_t json_stats;
  mi_stats_init(&json_stats);
  result_bool("stats_get_for_json", mi_stats_get(&json_stats));
  memset(json_buf, 0xA5, sizeof(json_buf));
  json = mi_stats_as_json(&json_stats, sizeof(json_buf), json_buf);
  result_bool("stats_as_json_provided_buffer", json == json_buf);
  result_bool("stats_as_json_schema", stats_json_schema_ok(json));

  mi_stats_t invalid_json_stats;
  mi_stats_init(&invalid_json_stats);
  invalid_json_stats.version = 0;
  result_bool("stats_as_json_invalid_null", mi_stats_as_json(&invalid_json_stats, sizeof(json_buf), json_buf) == NULL);
  result_bool("stats_as_json_null_stats_null", mi_stats_as_json(NULL, sizeof(json_buf), json_buf) == NULL);

  mi_heap_t* json_heap = mi_heap_new();
  memset(json_buf, 0xA5, sizeof(json_buf));
  json = json_heap == NULL ? NULL : mi_heap_stats_get_json(json_heap, sizeof(json_buf), json_buf);
  result_bool("heap_stats_get_json_provided_buffer", json_heap != NULL && json == json_buf);
  result_bool("heap_stats_get_json_schema", stats_json_schema_ok(json));
  if (json_heap != NULL) mi_heap_delete(json_heap);

  memset(json_buf, 0xA5, sizeof(json_buf));
  json = mi_subproc_stats_get_json(mi_subproc_current(), sizeof(json_buf), json_buf);
  result_bool("subproc_stats_get_json_provided_buffer", json == json_buf);
  result_bool("subproc_stats_get_json_schema", stats_json_schema_ok(json));

  mi_stats_t invalid_stats;
  mi_stats_init(&invalid_stats);
  invalid_stats.version = 0;
  result_bool("heap_stats_get_rejects_null_output", !mi_heap_stats_get(NULL, NULL));
  result_bool("heap_stats_get_rejects_bad_header", !mi_heap_stats_get(NULL, &invalid_stats));
  result_bool("subproc_stats_get_rejects_null_output", !mi_subproc_stats_get(mi_subproc_current(), NULL));
  result_bool("subproc_stats_get_exclusive_rejects_null_output", !mi_subproc_stats_get_exclusive(mi_subproc_current(), NULL));

  mi_heap_t* stats_heap = mi_heap_new();
  void* stats_heap_block_a = stats_heap == NULL ? NULL : mi_heap_malloc(stats_heap, 2048);
  void* stats_heap_block_b = stats_heap == NULL ? NULL : mi_heap_malloc(stats_heap, 4096);
  result_bool("heap_stats_merge_probe_alloc", stats_heap != NULL && stats_heap_block_a != NULL && stats_heap_block_b != NULL);

  mi_stats_t stats_heap_before;
  mi_stats_init(&stats_heap_before);
  int heap_stats_before_ok = stats_heap != NULL && mi_heap_stats_get(stats_heap, &stats_heap_before);
  result_bool("heap_stats_get_before_merge_success", heap_stats_before_ok);
  result_bool("heap_stats_get_before_merge_live", heap_stats_before_ok && stats_alloc_current(&stats_heap_before) >= 6144);
  result_bool("heap_stats_get_before_merge_peak", heap_stats_before_ok && stats_alloc_peak(&stats_heap_before) >= stats_alloc_current(&stats_heap_before));
  result_bool("heap_stats_get_before_merge_total", heap_stats_before_ok && stats_alloc_total(&stats_heap_before) >= stats_alloc_current(&stats_heap_before));

  mi_stats_t subproc_exclusive_before;
  mi_stats_init(&subproc_exclusive_before);
  result_bool("subproc_stats_get_exclusive_before_success", mi_subproc_stats_get_exclusive(mi_subproc_current(), &subproc_exclusive_before));

  mi_stats_t subproc_aggregate_before;
  mi_stats_init(&subproc_aggregate_before);
  result_bool("subproc_stats_get_before_success", mi_subproc_stats_get(mi_subproc_current(), &subproc_aggregate_before));
  result_bool("subproc_stats_get_before_includes_heap", stats_alloc_current(&subproc_aggregate_before) >= stats_alloc_current(&subproc_exclusive_before) + stats_alloc_current(&stats_heap_before));

  mi_heap_stats_merge_to_subproc(stats_heap);

  mi_stats_t stats_heap_after_merge;
  mi_stats_init(&stats_heap_after_merge);
  int heap_stats_after_merge_ok = stats_heap != NULL && mi_heap_stats_get(stats_heap, &stats_heap_after_merge);
  result_bool("heap_stats_get_after_merge_cleared_header", !heap_stats_after_merge_ok);

  mi_stats_t subproc_exclusive_after;
  mi_stats_init(&subproc_exclusive_after);
  result_bool("subproc_stats_get_exclusive_after_success", mi_subproc_stats_get_exclusive(mi_subproc_current(), &subproc_exclusive_after));
  result_bool("subproc_stats_get_exclusive_after_includes_heap", stats_alloc_current(&subproc_exclusive_after) >= stats_alloc_current(&subproc_exclusive_before) + stats_alloc_current(&stats_heap_before));

  mi_stats_t subproc_aggregate_after;
  mi_stats_init(&subproc_aggregate_after);
  result_bool("subproc_stats_get_after_success", mi_subproc_stats_get(mi_subproc_current(), &subproc_aggregate_after));
  result_bool("subproc_stats_get_after_consistent", stats_alloc_current(&subproc_aggregate_after) >= stats_alloc_current(&subproc_exclusive_after));

  mi_free(stats_heap_block_a);
  mi_free(stats_heap_block_b);
  if (stats_heap != NULL) mi_heap_delete(stats_heap);

  char small_json[32];
  memset(small_json, 0xCC, sizeof(small_json));
  json = mi_stats_get_json(sizeof(small_json), small_json);
  result_bool("stats_get_json_small_buffer_returned_buffer", json == small_json);
  result_bool("stats_get_json_small_buffer_terminated", memchr(small_json, '\0', sizeof(small_json)) != NULL);
  result_bool("stats_get_json_small_buffer_truncated", !stats_json_schema_ok(small_json));

  json = mi_stats_get_json(0, NULL);
  result_bool("stats_get_json_allocated_schema", stats_json_schema_ok(json));
  mi_free(json);

  void* stats_probe = mi_malloc(4096);
  result_bool("stats_print_probe_alloc", stats_probe != NULL);

  stats_output_reset();
  mi_stats_print_out(stats_output_callback, NULL);
  result_bool("stats_print_out_callback_nonempty", stats_output_callbacks > 0);
  result_bool("stats_print_out_structure", stats_output_seen_subproc && stats_output_seen_blocks && stats_output_seen_pages);

  stats_output_reset();
  mi_stats_print((void*)stats_output_callback);
  result_bool("stats_print_deprecated_callback_nonempty", stats_output_callbacks > 0);
  result_bool("stats_print_deprecated_callback_structure", stats_output_seen_subproc && stats_output_seen_blocks);

  stats_output_reset();
  mi_thread_stats_print_out(stats_output_callback, NULL);
  result_bool("thread_stats_print_out_callback_nonempty", stats_output_callbacks > 0);
  result_bool("thread_stats_print_out_structure", stats_output_seen_heap);

  stats_output_reset();
  mi_process_info_print_out(stats_output_callback, NULL);
  result_bool("process_info_print_out_callback_nonempty", stats_output_callbacks > 0);
  result_bool("process_info_print_out_structure", stats_output_seen_process && stats_output_seen_elapsed);

  mi_register_output(stats_output_callback, (void*)0x57A75);
  stats_output_reset();
  mi_stats_print(NULL);
  result_bool("stats_print_null_uses_registered_output", stats_output_callbacks > 0 && stats_output_seen_default_arg);
  stats_output_reset();
  mi_process_info_print();
  result_bool("process_info_print_null_uses_registered_output", stats_output_callbacks > 0 && stats_output_seen_default_arg);
  mi_register_output(NULL, NULL);

  mi_free(stats_probe);

  mi_collect(false);
  return failures == 0 ? 0 : 1;
}
"""

JSON_CAPTURE_C = r"""
#define _GNU_SOURCE
#include <mimalloc.h>
#include <mimalloc-stats.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#ifdef RUST_HELPER_CAPTURE
extern char* mi_stats_get_json_from(const mi_stats_t* stats, size_t buf_size, char* buf);
#endif

static int emit_json(const char* name, char* json, int owned) {
  if (json == NULL) return 1;
  size_t len = strlen(json);
  printf("@@JSON %s %zu\n", name, len);
  fwrite(json, 1, len, stdout);
  printf("\n@@END\n");
  if (owned) mi_free(json);
  return 0;
}

int main(void) {
  char buf[65536];
  int failures = 0;

  memset(buf, 0xA5, sizeof(buf));
  failures += emit_json("stats_get_json_buffer", mi_stats_get_json(sizeof(buf), buf), 0);

  mi_stats_t stats;
  mi_stats_init(&stats);
  if (!mi_stats_get(&stats)) failures++;
  memset(buf, 0xA5, sizeof(buf));
  failures += emit_json("stats_as_json_buffer", mi_stats_as_json(&stats, sizeof(buf), buf), 0);

  mi_heap_t* heap = mi_heap_new();
  memset(buf, 0xA5, sizeof(buf));
  failures += emit_json("heap_stats_get_json_buffer", heap == NULL ? NULL : mi_heap_stats_get_json(heap, sizeof(buf), buf), 0);
  if (heap != NULL) mi_heap_delete(heap);

  memset(buf, 0xA5, sizeof(buf));
  failures += emit_json("subproc_stats_get_json_buffer", mi_subproc_stats_get_json(mi_subproc_current(), sizeof(buf), buf), 0);

  failures += emit_json("stats_get_json_allocated", mi_stats_get_json(0, NULL), 1);

#ifdef RUST_HELPER_CAPTURE
  mi_stats_t helper_stats;
  mi_stats_init(&helper_stats);
  if (!mi_stats_get(&helper_stats)) failures++;
  memset(buf, 0xA5, sizeof(buf));
  failures += emit_json("stats_get_json_from_buffer", mi_stats_get_json_from(&helper_stats, sizeof(buf), buf), 0);
#endif

  return failures == 0 ? 0 : 1;
}
"""

CXX_NEW_HANDLER_CAPTURE = r"""
#include <mimalloc.h>

#include <new>
#include <stdio.h>
#include <stdint.h>

static int handler_calls = 0;

static void counting_new_handler(void) {
  handler_calls++;
}

int main(void) {
  std::set_new_handler(counting_new_handler);
  mi_heap_t* heap = mi_heap_new();
  void* p = (heap == NULL ? NULL : mi_heap_alloc_new_n(heap, (size_t)-1, 2));
  int overflow_null = (p == NULL ? 1 : 0);
  int calls_after_overflow = handler_calls;
  void* q = mi_new(64);
  int new_success = (q != NULL && mi_usable_size(q) >= 64 ? 1 : 0);
  mi_free(q);
  if (heap != NULL) mi_heap_delete(heap);
  std::set_new_handler(NULL);
  printf("@@CXX_NEW overflow_null=%d handler_calls=%d new_success=%d\n",
         overflow_null, calls_after_overflow, new_success);
  return (overflow_null == 1 && calls_after_overflow == 1 && new_success == 1 ? 0 : 1);
}
"""

CXX_OVERRIDE_CAPTURE = r"""
#include <mimalloc.h>

#include <new>
#include <stdint.h>
#include <stdio.h>

struct alignas(64) AlignedBlock {
  unsigned char bytes[64];
};

static int pointer_aligned(void* p, size_t alignment) {
  return p != NULL && (((uintptr_t)p % alignment) == 0);
}

int main(void) {
  int* scalar = new int(42);
  int scalar_owned = (scalar != NULL && *scalar == 42 && mi_check_owned(scalar));
  delete scalar;

  int* array = new int[8];
  int array_owned = (array != NULL && mi_check_owned(array));
  delete[] array;

  char* nothrow_array = new (std::nothrow) char[33];
  int nothrow_owned = (nothrow_array != NULL && mi_check_owned(nothrow_array));
  delete[] nothrow_array;

  AlignedBlock* aligned_object = new AlignedBlock();
  int aligned_owned = (aligned_object != NULL &&
                       pointer_aligned(aligned_object, alignof(AlignedBlock)) &&
                       mi_check_owned(aligned_object));
  delete aligned_object;

  void* raw_aligned = ::operator new(96, std::align_val_t(64));
  int raw_aligned_owned = (pointer_aligned(raw_aligned, 64) && mi_check_owned(raw_aligned));
  ::operator delete(raw_aligned, std::align_val_t(64));

  void* raw_aligned_nothrow = ::operator new[](128, std::align_val_t(64), std::nothrow);
  int raw_aligned_nothrow_owned = (pointer_aligned(raw_aligned_nothrow, 64) &&
                                   mi_check_owned(raw_aligned_nothrow));
  ::operator delete[](raw_aligned_nothrow, std::align_val_t(64), std::nothrow);

  printf("@@CXX_OVERRIDE scalar=%d array=%d nothrow=%d aligned=%d raw_aligned=%d raw_aligned_nothrow=%d\n",
         scalar_owned,
         array_owned,
         nothrow_owned,
         aligned_owned,
         raw_aligned_owned,
         raw_aligned_nothrow_owned);
  return (scalar_owned == 1 &&
          array_owned == 1 &&
          nothrow_owned == 1 &&
          aligned_owned == 1 &&
          raw_aligned_owned == 1 &&
          raw_aligned_nothrow_owned == 1 ? 0 : 1);
}
"""

CXX_STL_ALLOCATOR_CAPTURE = r"""
#include <mimalloc.h>

#include <memory>
#include <stdint.h>
#include <stdio.h>
#include <stdexcept>
#include <type_traits>
#include <vector>

struct Counted {
  static int constructed;
  static int destroyed;
  static bool throw_on_value_13;
  int value;

  explicit Counted(int v = 0) : value(v) {
    if (throw_on_value_13 && v == 13) throw std::runtime_error("expected");
    constructed++;
  }
  Counted(const Counted& other) : value(other.value) { constructed++; }
  ~Counted() { destroyed++; }
};

int Counted::constructed = 0;
int Counted::destroyed = 0;
bool Counted::throw_on_value_13 = false;

static int pointer_owned(const void* p) {
  return p != NULL && mi_check_owned(p);
}

int main(void) {
  int trait_ok =
      std::is_same<mi_stl_allocator<int>::rebind<double>::other, mi_stl_allocator<double>>::value &&
      std::allocator_traits<mi_stl_allocator<int>>::propagate_on_container_copy_assignment::value &&
      std::allocator_traits<mi_stl_allocator<int>>::propagate_on_container_move_assignment::value &&
      std::allocator_traits<mi_stl_allocator<int>>::propagate_on_container_swap::value &&
      std::allocator_traits<mi_stl_allocator<int>>::is_always_equal::value &&
      !std::allocator_traits<mi_heap_stl_allocator<int>>::is_always_equal::value &&
      !std::allocator_traits<mi_heap_destroy_stl_allocator<int>>::is_always_equal::value;

  mi_stl_allocator<int> plain_alloc;
  int* plain = plain_alloc.allocate(8);
  int plain_owned = pointer_owned(plain) && plain_alloc.max_size() >= 8;
  plain_alloc.deallocate(plain, 8);

  std::vector<int, mi_stl_allocator<int>> plain_vec;
  for (int i = 0; i < 16; i++) plain_vec.push_back(i);
  int plain_vector_ok = (plain_vec.size() == 16 &&
                         plain_vec.front() == 0 &&
                         plain_vec.back() == 15 &&
                         pointer_owned(plain_vec.data()));

  mi_stl_allocator<Counted> counted_alloc;
  Counted::constructed = 0;
  Counted::destroyed = 0;
  Counted* counted = counted_alloc.allocate(1);
  std::allocator_traits<mi_stl_allocator<Counted>>::construct(counted_alloc, counted, 42);
  int construct_ok = (Counted::constructed == 1 && counted->value == 42);
  std::allocator_traits<mi_stl_allocator<Counted>>::destroy(counted_alloc, counted);
  int destroy_ok = (Counted::destroyed == 1);
  counted_alloc.deallocate(counted, 1);

  int exception_ok = 0;
  {
    std::vector<Counted, mi_stl_allocator<Counted>> counted_vec;
    counted_vec.emplace_back(7);
    Counted::throw_on_value_13 = true;
    try {
      counted_vec.emplace_back(13);
    } catch (const std::runtime_error&) {
      exception_ok = (counted_vec.size() == 1 && counted_vec[0].value == 7 && pointer_owned(counted_vec.data()));
    }
    Counted::throw_on_value_13 = false;
  }

  mi_heap_t* heap = mi_heap_new();
  mi_heap_stl_allocator<int> heap_alloc(heap);
  int* heap_ptr = heap_alloc.allocate(4);
  int heap_owned = (heap != NULL && pointer_owned(heap_ptr) && mi_heap_contains(heap, heap_ptr));
  heap_alloc.deallocate(heap_ptr, 4);

  int heap_vector_ok = 0;
  {
    std::vector<int, mi_heap_stl_allocator<int>> heap_vec(heap_alloc);
    for (int i = 0; i < 8; i++) heap_vec.push_back(i * 3);
    heap_vector_ok = (heap != NULL &&
                      heap_vec.size() == 8 &&
                      heap_vec.back() == 21 &&
                      pointer_owned(heap_vec.data()) &&
                      mi_heap_contains(heap, heap_vec.data()));
  }

  mi_heap_stl_allocator<double> heap_alloc_rebound(heap_alloc);
  int heap_eq_ok = (heap_alloc == heap_alloc_rebound);
  mi_heap_t* other_heap = mi_heap_new();
  mi_heap_stl_allocator<int> other_heap_alloc(other_heap);
  int heap_ne_ok = (heap_alloc != other_heap_alloc);

  mi_heap_t* destroy_heap = mi_heap_new();
  mi_heap_destroy_stl_allocator<int> destroy_alloc(destroy_heap);
  int* destroy_ptr = destroy_alloc.allocate(3);
  int destroy_heap_owned = (destroy_heap != NULL &&
                            pointer_owned(destroy_ptr) &&
                            mi_heap_contains(destroy_heap, destroy_ptr));
  destroy_alloc.deallocate(destroy_ptr, 3);
  int destroy_alloc_eq = (destroy_alloc == mi_heap_destroy_stl_allocator<double>(destroy_alloc));

  if (heap != NULL) mi_heap_delete(heap);
  if (other_heap != NULL) mi_heap_delete(other_heap);
  if (destroy_heap != NULL) mi_heap_destroy(destroy_heap);

  printf("@@CXX_STL_ALLOCATOR trait=%d plain=%d plain_vector=%d construct=%d destroy=%d exception=%d heap=%d heap_vector=%d heap_eq=%d heap_ne=%d destroy_heap=%d destroy_eq=%d\n",
         trait_ok,
         plain_owned,
         plain_vector_ok,
         construct_ok,
         destroy_ok,
         exception_ok,
         heap_owned,
         heap_vector_ok,
         heap_eq_ok,
         heap_ne_ok,
         destroy_heap_owned,
         destroy_alloc_eq);
  return (trait_ok == 1 &&
          plain_owned == 1 &&
          plain_vector_ok == 1 &&
          construct_ok == 1 &&
          destroy_ok == 1 &&
          exception_ok == 1 &&
          heap_owned == 1 &&
          heap_vector_ok == 1 &&
          heap_eq_ok == 1 &&
          heap_ne_ok == 1 &&
          destroy_heap_owned == 1 &&
          destroy_alloc_eq == 1 ? 0 : 1);
}
"""

PROCESS_CAPTURE_C = r"""
#define _GNU_SOURCE
#include <mimalloc.h>

#include <stdio.h>
#include <string.h>

static void emit_process_info(const char* phase) {
  size_t elapsed = 0;
  size_t user = 0;
  size_t system = 0;
  size_t current_rss = 0;
  size_t peak_rss = 0;
  size_t current_commit = 0;
  size_t peak_commit = 0;
  size_t page_faults = 0;
  mi_process_info(&elapsed, &user, &system, &current_rss, &peak_rss, &current_commit, &peak_commit, &page_faults);
  printf("@@PROCESS %s %zu %zu %zu %zu %zu %zu %zu %zu\n",
         phase, elapsed, user, system, current_rss, peak_rss, current_commit, peak_commit, page_faults);
}

int main(void) {
  mi_process_info(NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL);
  emit_process_info("initial");

  const size_t size = 4 * 1024 * 1024;
  unsigned char* p = (unsigned char*)mi_malloc(size);
  if (p == NULL) return 1;
  for (size_t i = 0; i < size; i += 4096) {
    p[i] = (unsigned char)i;
  }
  emit_process_info("allocated");

  mi_free(p);
  mi_collect(true);
  emit_process_info("collected");
  return 0;
}
"""

ENV_CAPTURE_C = r"""
#define _GNU_SOURCE
#include <mimalloc.h>

#include <stdio.h>
#include <string.h>
#include <unistd.h>

static char output_buf[8192];
static size_t output_len = 0;
static size_t output_callbacks = 0;

static void capture_output(const char* msg, void* arg) {
  (void)arg;
  if (msg == NULL) return;
  output_callbacks++;
  size_t len = strlen(msg);
  size_t room = sizeof(output_buf) - output_len - 1;
  if (len > room) len = room;
  memcpy(output_buf + output_len, msg, len);
  output_len += len;
  output_buf[output_len] = 0;
}

int main(void) {
  char line[256];
  mi_register_output(capture_output, NULL);
  for (int i = 0; i < _mi_option_last; i++) {
    mi_option_t option = (mi_option_t)i;
    long value = mi_option_get(option);
    size_t size = mi_option_get_size(option);
    int enabled = mi_option_is_enabled(option) ? 1 : 0;
    int len = snprintf(line,
                       sizeof(line),
                       "@@OPTION %d %ld %zu %d\n",
                       i,
                       value,
                       size,
                       enabled);
    if (len > 0) (void)write(1, line, (size_t)len);
  }
  int len = snprintf(line,
                     sizeof(line),
                     "@@OUTPUT callbacks=%zu deprecated=%d invalid=%d verbose=%d len=%zu\n",
                     output_callbacks,
                     strstr(output_buf, "deprecated") != NULL ? 1 : 0,
                     strstr(output_buf, "invalid value") != NULL ? 1 : 0,
                     strstr(output_buf, "option '") != NULL ? 1 : 0,
                     output_len);
  if (len > 0) (void)write(1, line, (size_t)len);
  return 0;
}
"""

THROTTLE_CAPTURE_C = r"""
#define _GNU_SOURCE
#include <mimalloc.h>

#include <errno.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>

void _mi_warning_message(const char* fmt, ...);
void _mi_error_message(int err, const char* fmt, ...);

static size_t output_callbacks = 0;
static size_t warning_messages = 0;
static size_t error_messages = 0;

static void capture_output(const char* msg, void* arg) {
  (void)arg;
  if (msg == NULL || msg[0] == 0) return;
  output_callbacks++;
  if (strstr(msg, "mimalloc: warning:") != NULL) warning_messages++;
  if (strstr(msg, "mimalloc: error:") != NULL) error_messages++;
}

int main(void) {
  char line[256];
  mi_register_output(capture_output, NULL);
  mi_process_init();
  for (int i = 0; i < 5; i++) {
    _mi_warning_message("throttle warning\n");
  }
  for (int i = 0; i < 5; i++) {
    _mi_error_message(ENOMEM, "throttle error\n");
  }
  int len = snprintf(line,
                     sizeof(line),
                     "@@THROTTLE callbacks=%zu warnings=%zu errors=%zu\n",
                     output_callbacks,
                     warning_messages,
                     error_messages);
  if (len > 0) (void)write(1, line, (size_t)len);
  return 0;
}
"""

PROCESS_LIFECYCLE_C = r"""
#define _GNU_SOURCE
#include <mimalloc.h>

#include <stdio.h>
#include <string.h>
#include <unistd.h>

void _mi_auto_process_done(void);

static char output_buf[16384];
static size_t output_len = 0;
static size_t output_callbacks = 0;

static void capture_output(const char* msg, void* arg) {
  (void)arg;
  if (msg == NULL || msg[0] == 0) return;
  output_callbacks++;
  size_t len = strlen(msg);
  size_t room = sizeof(output_buf) - output_len - 1;
  if (len > room) len = room;
  memcpy(output_buf + output_len, msg, len);
  output_len += len;
  output_buf[output_len] = 0;
}

int main(void) {
  char line[256];
  mi_register_output(capture_output, NULL);
  mi_process_init();
  mi_process_init();
  void* p = mi_malloc(1024);
  if (p != NULL) {
    memset(p, 0xA5, 1024);
    mi_free(p);
  }
  _mi_auto_process_done();
  mi_process_done();
  mi_process_done();

  int len = snprintf(line,
                     sizeof(line),
                     "@@PROCESS_LIFECYCLE callbacks=%zu stats=%d process=%d option=%d len=%zu\n",
                     output_callbacks,
                     strstr(output_buf, "heap ") != NULL ? 1 : 0,
                     strstr(output_buf, "process") != NULL ? 1 : 0,
                     strstr(output_buf, "option '") != NULL ? 1 : 0,
                     output_len);
  if (len > 0) (void)write(1, line, (size_t)len);
  return p == NULL ? 1 : 0;
}
"""

NUMA_AFFINITY_CAPTURE_C = r"""
#define _GNU_SOURCE
#include <mimalloc.h>

#include <stdint.h>
#include <stdio.h>
#include <sys/mman.h>

static void* map_aligned_arena_memory(size_t size) {
  size_t alignment = mi_arena_min_alignment();
  void* mapping = mmap(NULL, size + alignment, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
  if (mapping == MAP_FAILED) return MAP_FAILED;
  return (void*)(((uintptr_t)mapping + alignment - 1) & ~(uintptr_t)(alignment - 1));
}

int main(void) {
  size_t size = mi_arena_min_size();
  void* mem0 = map_aligned_arena_memory(size);
  void* mem1 = map_aligned_arena_memory(size);
  mi_arena_id_t arena0 = NULL;
  mi_arena_id_t arena1 = NULL;
  int managed0 = mem0 != MAP_FAILED && mi_manage_os_memory_ex(mem0, size, true, false, true, 0, false, &arena0);
  int managed1 = mem1 != MAP_FAILED && mi_manage_os_memory_ex(mem1, size, true, false, true, 1, false, &arena1);

  mi_heap_t* heap = mi_heap_new();
  mi_option_set(mi_option_disallow_os_alloc, 1);
  mi_option_set(mi_option_disallow_arena_alloc, 0);

  mi_heap_set_numa_affinity(heap, 1);
  void* node1 = heap == NULL ? NULL : mi_heap_malloc(heap, 4 * 1024 * 1024);
  int node1_in_0 = node1 != NULL && mi_arena_contains(arena0, node1);
  int node1_in_1 = node1 != NULL && mi_arena_contains(arena1, node1);
  mi_free(node1);

  mi_heap_set_numa_affinity(heap, 4);
  void* node0 = heap == NULL ? NULL : mi_heap_malloc(heap, 4 * 1024 * 1024);
  int node0_in_0 = node0 != NULL && mi_arena_contains(arena0, node0);
  int node0_in_1 = node0 != NULL && mi_arena_contains(arena1, node0);
  mi_free(node0);

  mi_heap_set_numa_affinity(heap, -1);
  void* any = heap == NULL ? NULL : mi_heap_malloc(heap, 4 * 1024 * 1024);
  int reset_any = any != NULL && (mi_arena_contains(arena0, any) || mi_arena_contains(arena1, any));
  mi_free(any);

  if (heap != NULL) mi_heap_delete(heap);
  printf("@@NUMA managed=%d%d node1=%d%d node0=%d%d reset=%d\n",
         managed0, managed1, node1_in_0, node1_in_1, node0_in_0, node0_in_1, reset_any);
  return managed0 && managed1 && node1 != NULL && node0 != NULL && reset_any ? 0 : 1;
}
"""

ARENA_FALLBACK_CAPTURE_C = r"""
#define _GNU_SOURCE
#include <mimalloc.h>

#include <errno.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/mman.h>

static int error_count;
static int last_error;
static size_t output_len;
static int output_has_percent;

static void output_cb(const char* msg, void* arg) {
  (void)arg;
  if (msg != NULL) {
    output_len += strlen(msg);
    if (strchr(msg, '%') != NULL) output_has_percent = 1;
  }
}

static void error_cb(int err, void* arg) {
  (void)arg;
  error_count++;
  last_error = err;
}

static void* map_aligned_arena_memory(size_t size) {
  size_t alignment = mi_arena_min_alignment();
  void* mapping = mmap(NULL, size + alignment, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
  if (mapping == MAP_FAILED) return MAP_FAILED;
  return (void*)(((uintptr_t)mapping + alignment - 1) & ~(uintptr_t)(alignment - 1));
}

static int run_case(const char* name) {
  int exclusive = strcmp(name, "exclusive_arena_disabled") == 0;
  size_t size = mi_arena_min_size();
  void* mem = map_aligned_arena_memory(size);
  mi_arena_id_t arena = NULL;
  int managed = mem != MAP_FAILED && mi_manage_os_memory_ex(mem, size, true, false, true, 0, exclusive, &arena);
  mi_heap_t* heap = exclusive ? mi_heap_new_in_arena(arena) : mi_heap_new();
  if (heap != NULL) mi_heap_set_numa_affinity(heap, 0);

  mi_register_output(output_cb, NULL);
  mi_register_error(error_cb, NULL);
  mi_option_set(mi_option_show_errors, 1);
  mi_option_set(mi_option_verbose, 0);
  mi_option_set(mi_option_arena_reserve, 0);

  if (strcmp(name, "arena_only") == 0) {
    mi_option_set(mi_option_disallow_os_alloc, 1);
    mi_option_set(mi_option_disallow_arena_alloc, 0);
  }
  else if (strcmp(name, "os_fallback") == 0) {
    mi_option_set(mi_option_disallow_os_alloc, 0);
    mi_option_set(mi_option_disallow_arena_alloc, 1);
  }
  else if (strcmp(name, "all_disabled") == 0) {
    mi_option_set(mi_option_disallow_os_alloc, 1);
    mi_option_set(mi_option_disallow_arena_alloc, 1);
  }
  else if (strcmp(name, "exclusive_arena_disabled") == 0) {
    mi_option_set(mi_option_disallow_os_alloc, 0);
    mi_option_set(mi_option_disallow_arena_alloc, 1);
  }
  else {
    return 2;
  }

  errno = 0;
  void* p = heap == NULL ? NULL : mi_heap_malloc(heap, 4 * 1024 * 1024);
  int ptr = p != NULL;
  int in_arena = p != NULL && mi_arena_contains(arena, p);
  int saved_errno = errno;
  mi_free(p);
  if (heap != NULL) mi_heap_delete(heap);

  printf("@@ARENA_FALLBACK case=%s managed=%d heap=%d ptr=%d in_arena=%d errno=%d errors=%d last_error=%d output=%zu output_has_percent=%d\n",
         name,
         managed,
         heap != NULL,
         ptr,
         in_arena,
         saved_errno,
         error_count,
         last_error,
         output_len,
         output_has_percent);
  return 0;
}

int main(int argc, char** argv) {
  if (argc != 2) return 2;
  return run_case(argv[1]);
}
"""

FREE_HINT_RELEASE_CAPTURE_C = r"""
#define _GNU_SOURCE
#include <mimalloc.h>

#include <stdio.h>
#include <stdlib.h>
#include <sys/wait.h>
#include <unistd.h>

static int free_hint_case_status(int op) {
  fflush(stdout);
  pid_t pid = fork();
  if (pid < 0) return -1;
  if (pid == 0) {
    FILE* devnull = fopen("/dev/null", "w");
    if (devnull != NULL) {
      (void)dup2(fileno(devnull), fileno(stderr));
    }
    switch (op) {
      case 0: {
        void* p = mi_malloc(32);
        if (p == NULL) _exit(10);
        mi_free_size(p, 1024 * 1024);
        _exit(0);
      }
      case 1: {
        void* p = mi_malloc_aligned_at(32, 64, 32);
        if (p == NULL) _exit(11);
        mi_free_size_aligned(p, 32, 64);
        _exit(0);
      }
      case 2: {
        void* p = mi_malloc_aligned_at(32, 64, 32);
        if (p == NULL) _exit(12);
        mi_free_aligned(p, 64);
        _exit(0);
      }
      default:
        _exit(41);
    }
  }
  int status = 0;
  if (waitpid(pid, &status, 0) < 0) return -2;
  if (WIFSIGNALED(status)) return 128 + WTERMSIG(status);
  if (WIFEXITED(status)) return WEXITSTATUS(status);
  return -3;
}

int main(void) {
  printf("@@FREE_HINT_RELEASE oversize=%d size_aligned=%d aligned=%d\n",
         free_hint_case_status(0),
         free_hint_case_status(1),
         free_hint_case_status(2));
  return 0;
}
"""

STRESS_RELEASE_CAPTURE_C = r"""
#define _GNU_SOURCE
#include <mimalloc.h>
#include <mimalloc-stats.h>

#include <pthread.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

#define RELEASE_STRESS_THREADS 8
#define RELEASE_STRESS_SLOTS 48
#define RELEASE_STRESS_INCOMING 8
#define RELEASE_STRESS_ROUNDS 384
#define RELEASE_STRESS_WAVES 3

static int64_t stats_alloc_current(const mi_stats_t* stats) {
  return stats->malloc_normal.current + stats->malloc_huge.current;
}

static int64_t stats_alloc_peak(const mi_stats_t* stats) {
  return stats->malloc_normal.peak + stats->malloc_huge.peak;
}

static int64_t stats_alloc_total(const mi_stats_t* stats) {
  return stats->malloc_normal.total + stats->malloc_huge.total;
}

struct release_stress_arg {
  void* incoming[RELEASE_STRESS_INCOMING];
  size_t id;
  int use_threadpool;
  int ok;
};

static void* release_stress_worker(void* arg) {
  struct release_stress_arg* result = (struct release_stress_arg*)arg;
  void* slots[RELEASE_STRESS_SLOTS];
  memset(slots, 0, sizeof(slots));
  result->ok = 1;

  mi_thread_init();
  if (result->use_threadpool) mi_thread_set_in_threadpool();

  for (size_t i = 0; i < RELEASE_STRESS_INCOMING; i++) {
    if (result->incoming[i] == NULL || !mi_is_in_heap_region(result->incoming[i])) result->ok = 0;
    mi_free(result->incoming[i]);
    result->incoming[i] = NULL;
  }

  for (size_t round = 0; round < RELEASE_STRESS_ROUNDS; round++) {
    size_t slot = (round * 11 + result->id * 7) % RELEASE_STRESS_SLOTS;
    if (slots[slot] != NULL) {
      if ((round % 4) == 0) {
        size_t new_size = 1 + ((round * 23 + result->id * 17) % 4096);
        void* resized = mi_realloc(slots[slot], new_size);
        if (resized == NULL || !mi_check_owned(resized) || mi_usable_size(resized) < new_size) {
          result->ok = 0;
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
      size_t size = 1 + ((round * 31 + result->id * 13) % 8192);
      void* p = NULL;
      if ((round % 4) == 0) {
        p = mi_malloc(size);
      }
      else if ((round % 4) == 1) {
        p = mi_zalloc(size);
      }
      else if ((round % 4) == 2) {
        size_t alignment = (size_t)16 << ((round + result->id) % 3);
        p = mi_malloc_aligned(size, alignment);
        if (p != NULL && (((uintptr_t)p % alignment) != 0)) result->ok = 0;
      }
      else {
        size_t count = 1 + (round % 5);
        size_t elem_size = 1 + (size / count);
        size = count * elem_size;
        p = mi_mallocn(count, elem_size);
      }
      if (p == NULL || !mi_check_owned(p) || mi_usable_size(p) < size) {
        result->ok = 0;
        mi_free(p);
      }
      else {
        slots[slot] = p;
      }
    }
    if ((round % 89) == 0) mi_collect(false);
  }

  for (size_t i = 0; i < RELEASE_STRESS_SLOTS; i++) mi_free(slots[i]);
  mi_collect(true);
  mi_thread_done();
  return NULL;
}

int main(void) {
  mi_stats_t before;
  mi_stats_t after;
  mi_stats_init(&before);
  mi_stats_init(&after);
  int before_ok = mi_stats_get(&before) ? 1 : 0;
  int all_started = 1;
  int all_workers = 1;

  for (size_t wave = 0; wave < RELEASE_STRESS_WAVES; wave++) {
    pthread_t threads[RELEASE_STRESS_THREADS];
    struct release_stress_arg args[RELEASE_STRESS_THREADS];
    size_t started = 0;
    memset(threads, 0, sizeof(threads));
    memset(args, 0, sizeof(args));
    for (size_t t = 0; t < RELEASE_STRESS_THREADS; t++) {
      args[t].id = t + (wave * RELEASE_STRESS_THREADS);
      args[t].use_threadpool = ((t + wave) % 2) == 0;
      for (size_t i = 0; i < RELEASE_STRESS_INCOMING; i++) {
        args[t].incoming[i] = mi_malloc(1 + (((wave + 1) * (t + 2) * (i + 3)) % 1024));
        if (args[t].incoming[i] == NULL || !mi_check_owned(args[t].incoming[i])) all_workers = 0;
      }
      if (pthread_create(&threads[t], NULL, release_stress_worker, &args[t]) == 0) {
        started++;
      }
      else {
        all_started = 0;
        all_workers = 0;
        for (size_t i = 0; i < RELEASE_STRESS_INCOMING; i++) mi_free(args[t].incoming[i]);
      }
    }
    if (started != RELEASE_STRESS_THREADS) all_started = 0;
    for (size_t t = 0; t < started; t++) {
      if (pthread_join(threads[t], NULL) != 0 || !args[t].ok) all_workers = 0;
    }
    mi_collect(true);
  }

  int after_ok = mi_stats_get(&after) ? 1 : 0;
  int stats_ok = before_ok && after_ok &&
                 stats_alloc_current(&after) >= 0 &&
                 stats_alloc_peak(&after) >= stats_alloc_current(&after) &&
                 stats_alloc_total(&after) >= stats_alloc_total(&before);
  printf("@@STRESS_RELEASE started=%d workers=%d stats=%d\n", all_started, all_workers, stats_ok);
  return (all_started && all_workers && stats_ok ? 0 : 1);
}
"""

HUGE_OS_PAGES_CAPTURE_C = r"""
#define _GNU_SOURCE
#include <mimalloc.h>

#include <stdio.h>

int main(void) {
  size_t reserved = 123;
  mi_arena_id_t arena = (mi_arena_id_t)0x1;
  int zero_at = mi_reserve_huge_os_pages_at(0, -1, 0);
  int zero_interleave = mi_reserve_huge_os_pages_interleave(0, 0, 0);
  int zero_legacy = mi_reserve_huge_os_pages(0, 0.0, &reserved);
  int zero_ex = mi_reserve_huge_os_pages_at_ex(0, -1, 0, false, &arena);
  printf("@@HUGE_OS zero_at=%d zero_interleave=%d zero_legacy=%d zero_reserved=%zu zero_ex=%d zero_arena_null=%d\n",
         zero_at,
         zero_interleave,
         zero_legacy,
         reserved,
         zero_ex,
         arena == NULL ? 1 : 0);

  reserved = 123;
  arena = (mi_arena_id_t)0x1;
  int one_at = mi_reserve_huge_os_pages_at(1, -1, 0);
  int one_interleave = mi_reserve_huge_os_pages_interleave(1, 0, 0);
  int one_legacy = mi_reserve_huge_os_pages(1, 0.0, &reserved);
  int one_ex = mi_reserve_huge_os_pages_at_ex(1, -1, 0, false, &arena);
  printf("@@HUGE_OS one_at=%d one_interleave=%d one_legacy=%d one_reserved=%zu one_ex=%d one_arena_null=%d\n",
         one_at,
         one_interleave,
         one_legacy,
         reserved,
         one_ex,
         arena == NULL ? 1 : 0);
  return 0;
}
"""

REDIRECT_STATIC_CAPTURE_C = r"""
#define _GNU_SOURCE
#include <mimalloc.h>

#include <malloc.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>

extern void cfree(void*);
extern size_t malloc_size(const void*);

static int pointer_aligned(void* p, size_t alignment) {
  return p != NULL && (((uintptr_t)p % alignment) == 0);
}

static int check_malloc_family(void) {
  int ok = 1;
  size_t page_size = (size_t)sysconf(_SC_PAGESIZE);

  unsigned char* p = (unsigned char*)malloc(24);
  ok = ok && p != NULL && mi_check_owned(p);
  if (p != NULL) {
    p[0] = 0xA5;
    p = (unsigned char*)realloc(p, 48);
    ok = ok && p != NULL && mi_check_owned(p) && p[0] == 0xA5;
    ok = ok && malloc_usable_size(p) >= 48 && malloc_size(p) == malloc_usable_size(p);
    free(p);
  }

  unsigned char* c = (unsigned char*)calloc(4, 8);
  ok = ok && c != NULL && mi_check_owned(c);
  for (size_t i = 0; c != NULL && i < 32; i++) {
    ok = ok && c[i] == 0;
  }
  cfree(c);

  void* pm = NULL;
  ok = ok && posix_memalign(&pm, 32, 64) == 0 && pointer_aligned(pm, 32) && mi_check_owned(pm);
  free(pm);

  void* mm = memalign(64, 65);
  ok = ok && pointer_aligned(mm, 64) && mi_check_owned(mm);
  free(mm);

  void* va = valloc(66);
  ok = ok && pointer_aligned(va, page_size) && mi_check_owned(va);
  free(va);

  void* pv = pvalloc(page_size + 1);
  ok = ok && pointer_aligned(pv, page_size) && mi_check_owned(pv) && malloc_usable_size(pv) >= page_size * 2;
  free(pv);

  void* aa = aligned_alloc(128, 128);
  ok = ok && pointer_aligned(aa, 128) && mi_check_owned(aa);
  free(aa);

  void* ra = reallocarray(NULL, 4, 16);
  ok = ok && ra != NULL && mi_check_owned(ra) && malloc_usable_size(ra) >= 64;
  free(ra);

  return ok;
}

int main(void) {
  void* p = malloc(64);
  int malloc_owned = (p != NULL && mi_check_owned(p));
  free(p);

  void* q = mi_malloc(64);
  int mi_malloc_owned = (q != NULL && mi_check_owned(q));
  mi_free(q);

  printf("@@REDIRECT static is_redirected=%d malloc_owned=%d mi_malloc_owned=%d family_ok=%d\n",
         mi_is_redirected() ? 1 : 0,
         malloc_owned,
         mi_malloc_owned,
         check_malloc_family());
  return mi_malloc_owned == 1 ? 0 : 1;
}
"""

REDIRECT_DYNAMIC_CAPTURE_C = r"""
#define _GNU_SOURCE
#include <dlfcn.h>

#include <malloc.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>

typedef bool (*mi_is_redirected_fun)(void);
typedef bool (*mi_check_owned_fun)(const void*);
typedef void* (*mi_malloc_fun)(size_t);
typedef void (*mi_free_fun)(void*);
typedef void* (*malloc_fun)(size_t);
typedef void* (*calloc_fun)(size_t, size_t);
typedef void* (*realloc_fun)(void*, size_t);
typedef void (*free_fun)(void*);
typedef void (*cfree_fun)(void*);
typedef size_t (*malloc_size_fun)(const void*);
typedef int (*posix_memalign_fun)(void**, size_t, size_t);
typedef void* (*memalign_fun)(size_t, size_t);
typedef void* (*aligned_alloc_fun)(size_t, size_t);
typedef void* (*reallocarray_fun)(void*, size_t, size_t);

static int pointer_aligned(void* p, size_t alignment) {
  return p != NULL && (((uintptr_t)p % alignment) == 0);
}

static int check_malloc_family_dynamic(mi_check_owned_fun check_owned) {
  int ok = 1;
  size_t page_size = (size_t)sysconf(_SC_PAGESIZE);
  malloc_fun mallocp = (malloc_fun)dlsym(RTLD_DEFAULT, "malloc");
  calloc_fun callocp = (calloc_fun)dlsym(RTLD_DEFAULT, "calloc");
  realloc_fun reallocp = (realloc_fun)dlsym(RTLD_DEFAULT, "realloc");
  free_fun freep = (free_fun)dlsym(RTLD_DEFAULT, "free");
  cfree_fun cfreep = (cfree_fun)dlsym(RTLD_DEFAULT, "cfree");
  malloc_size_fun malloc_usable_sizep = (malloc_size_fun)dlsym(RTLD_DEFAULT, "malloc_usable_size");
  malloc_size_fun malloc_sizep = (malloc_size_fun)dlsym(RTLD_DEFAULT, "malloc_size");
  posix_memalign_fun posix_memalignp = (posix_memalign_fun)dlsym(RTLD_DEFAULT, "posix_memalign");
  memalign_fun memalignp = (memalign_fun)dlsym(RTLD_DEFAULT, "memalign");
  malloc_fun vallocp = (malloc_fun)dlsym(RTLD_DEFAULT, "valloc");
  malloc_fun pvallocp = (malloc_fun)dlsym(RTLD_DEFAULT, "pvalloc");
  aligned_alloc_fun aligned_allocp = (aligned_alloc_fun)dlsym(RTLD_DEFAULT, "aligned_alloc");
  reallocarray_fun reallocarrayp = (reallocarray_fun)dlsym(RTLD_DEFAULT, "reallocarray");
  if (mallocp == NULL || callocp == NULL || reallocp == NULL || freep == NULL ||
      cfreep == NULL || malloc_usable_sizep == NULL || malloc_sizep == NULL ||
      posix_memalignp == NULL || memalignp == NULL || vallocp == NULL ||
      pvallocp == NULL || aligned_allocp == NULL || reallocarrayp == NULL) {
    return 0;
  }

  unsigned char* p = (unsigned char*)mallocp(24);
  ok = ok && p != NULL && check_owned(p);
  if (p != NULL) {
    p[0] = 0xA5;
    p = (unsigned char*)reallocp(p, 48);
    ok = ok && p != NULL && check_owned(p) && p[0] == 0xA5;
    ok = ok && malloc_usable_sizep(p) >= 48 && malloc_sizep(p) == malloc_usable_sizep(p);
    freep(p);
  }

  unsigned char* c = (unsigned char*)callocp(4, 8);
  ok = ok && c != NULL && check_owned(c);
  for (size_t i = 0; c != NULL && i < 32; i++) {
    ok = ok && c[i] == 0;
  }
  cfreep(c);

  void* pm = NULL;
  ok = ok && posix_memalignp(&pm, 32, 64) == 0 && pointer_aligned(pm, 32) && check_owned(pm);
  freep(pm);

  void* mm = memalignp(64, 65);
  ok = ok && pointer_aligned(mm, 64) && check_owned(mm);
  freep(mm);

  void* va = vallocp(66);
  ok = ok && pointer_aligned(va, page_size) && check_owned(va);
  freep(va);

  void* pv = pvallocp(page_size + 1);
  ok = ok && pointer_aligned(pv, page_size) && check_owned(pv) && malloc_usable_sizep(pv) >= page_size * 2;
  freep(pv);

  void* aa = aligned_allocp(128, 128);
  ok = ok && pointer_aligned(aa, 128) && check_owned(aa);
  freep(aa);

  void* ra = reallocarrayp(NULL, 4, 16);
  ok = ok && ra != NULL && check_owned(ra) && malloc_usable_sizep(ra) >= 64;
  freep(ra);

  return ok;
}

int main(int argc, char** argv) {
  const char* mode = "preload";
  void* handle = RTLD_DEFAULT;
  if (argc == 2) {
    mode = "dlopen";
    handle = dlopen(argv[1], RTLD_NOW | RTLD_LOCAL);
    if (handle == NULL) {
      fprintf(stderr, "dlopen failed: %s\n", dlerror());
      return 2;
    }
  }

  mi_is_redirected_fun is_redirected = (mi_is_redirected_fun)dlsym(handle, "mi_is_redirected");
  mi_check_owned_fun check_owned = (mi_check_owned_fun)dlsym(handle, "mi_check_owned");
  mi_malloc_fun alloc = (mi_malloc_fun)dlsym(handle, "mi_malloc");
  mi_free_fun dealloc = (mi_free_fun)dlsym(handle, "mi_free");
  if (is_redirected == NULL || check_owned == NULL || alloc == NULL || dealloc == NULL) {
    fprintf(stderr, "missing mimalloc dynamic symbols\n");
    return 3;
  }

  void* p = malloc(64);
  int malloc_owned = (p != NULL && check_owned(p));
  free(p);

  void* q = alloc(64);
  int mi_malloc_owned = (q != NULL && check_owned(q));
  dealloc(q);
  int family_ok = (argc == 2 ? -1 : check_malloc_family_dynamic(check_owned));

  printf("@@REDIRECT %s is_redirected=%d malloc_owned=%d mi_malloc_owned=%d family_ok=%d\n",
         mode,
         is_redirected() ? 1 : 0,
         malloc_owned,
         mi_malloc_owned,
         family_ok);
  return mi_malloc_owned == 1 ? 0 : 1;
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


def rust_staticlib_release() -> pathlib.Path:
    subprocess.run(
        ["cargo", "rustc", "--release", "--lib", "--", "--crate-type", "staticlib"],
        cwd=ROOT,
        check=True,
    )
    libs = sorted(
        (ROOT / "target/release/deps").glob("libmechanical_mimalloc-*.a"),
        key=lambda path: path.stat().st_mtime,
        reverse=True,
    )
    if not libs:
        raise SystemExit("no Rust release staticlib artifact found")
    return libs[0]


def rust_cdylib() -> pathlib.Path:
    subprocess.run(
        ["cargo", "rustc", "--lib", "--", "--crate-type", "cdylib"],
        cwd=ROOT,
        check=True,
    )
    libs = sorted(
        (ROOT / "target/debug/deps").glob("libmechanical_mimalloc-*.so"),
        key=lambda path: path.stat().st_mtime,
        reverse=True,
    )
    if not libs:
        raise SystemExit("no Rust cdylib artifact found")
    return libs[0]


def compile_binary(
    cc: str,
    output: pathlib.Path,
    objects: list[pathlib.Path],
    cflags: list[str] | None = None,
) -> None:
    subprocess.run(
        [
            cc,
            "-std=c11",
            "-I",
            str(ROOT / "mimalloc-3.3.2/include"),
            "-I",
            str(ROOT / "mimalloc-3.3.2/src"),
            *(cflags or []),
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


def compile_shared_library(
    cc: str,
    output: pathlib.Path,
    objects: list[pathlib.Path],
    cflags: list[str] | None = None,
) -> None:
    subprocess.run(
        [
            cc,
            "-std=c11",
            "-shared",
            "-fPIC",
            "-I",
            str(ROOT / "mimalloc-3.3.2/include"),
            "-I",
            str(ROOT / "mimalloc-3.3.2/src"),
            *(cflags or []),
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


def compile_cxx_binary(
    cxx: str,
    output: pathlib.Path,
    objects: list[pathlib.Path],
    cflags: list[str] | None = None,
) -> None:
    subprocess.run(
        [
            cxx,
            "-std=c++20",
            "-I",
            str(ROOT / "mimalloc-3.3.2/include"),
            "-I",
            str(ROOT / "mimalloc-3.3.2/src"),
            *(cflags or []),
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


def run_args(binary: pathlib.Path, args: list[str]) -> str:
    completed = subprocess.run(
        [str(binary), *args],
        cwd=ROOT,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
    )
    return completed.stdout


def run_args_status(binary: pathlib.Path, args: list[str]) -> tuple[int, str, str]:
    completed = subprocess.run(
        [str(binary), *args],
        cwd=ROOT,
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    return completed.returncode, completed.stdout, completed.stderr


def run_with_env(binary: pathlib.Path, env_updates: dict[str, str], label: str) -> str:
    env = {
        "PATH": os.environ.get("PATH", ""),
        "HOME": os.environ.get("HOME", ""),
        "TMPDIR": os.environ.get("TMPDIR", "/tmp"),
    }
    env.update(env_updates)
    try:
        completed = subprocess.run(
            [str(binary)],
            cwd=ROOT,
            env=env,
            check=True,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=10,
        )
    except subprocess.TimeoutExpired as err:
        stdout = (err.stdout or b"").decode(errors="replace") if isinstance(err.stdout, bytes) else (err.stdout or "")
        stderr = (err.stderr or b"").decode(errors="replace") if isinstance(err.stderr, bytes) else (err.stderr or "")
        raise AssertionError(
            f"{label} timed out for {binary.name} with env {env_updates}; stdout={stdout!r}; stderr={stderr!r}"
        ) from err
    return completed.stdout


STAT_COUNT_KEYS = {
    "pages",
    "reserved",
    "committed",
    "page_committed",
    "pages_abandoned",
    "threads",
    "malloc_normal",
    "malloc_huge",
    "malloc_requested",
    "segments",
    "segments_abandoned",
    "segments_cache",
    "_segments_reserved",
    "heaps",
    "theaps",
}

STAT_COUNTER_KEYS = {
    "reset",
    "purged",
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
    "pages_reclaim_on_alloc",
    "pages_reclaim_on_free",
    "pages_reabandon_full",
    "pages_unabandon_busy_wait",
    "heaps_delete_wait",
}

PROCESS_KEYS = {
    "elapsed_msecs",
    "user_msecs",
    "system_msecs",
    "page_faults",
    "rss_current",
    "rss_peak",
    "commit_current",
    "commit_peak",
}

OPTION_NAMES = (
    "show_errors",
    "show_stats",
    "verbose",
    "deprecated_eager_commit",
    "arena_eager_commit",
    "purge_decommits",
    "allow_large_os_pages",
    "reserve_huge_os_pages",
    "reserve_huge_os_pages_at",
    "reserve_os_memory",
    "deprecated_segment_cache",
    "deprecated_page_reset",
    "deprecated_abandoned_page_purge",
    "deprecated_segment_reset",
    "deprecated_eager_commit_delay",
    "purge_delay",
    "use_numa_nodes",
    "disallow_os_alloc",
    "os_tag",
    "max_errors",
    "max_warnings",
    "deprecated_max_segment_reclaim",
    "destroy_on_exit",
    "arena_reserve",
    "arena_purge_mult",
    "deprecated_purge_extend_delay",
    "disallow_arena_alloc",
    "retry_on_oom",
    "visit_abandoned",
    "guarded_min",
    "guarded_max",
    "guarded_precise",
    "guarded_sample_rate",
    "guarded_sample_seed",
    "generic_collect",
    "page_reclaim_on_free",
    "page_full_retain",
    "page_max_candidates",
    "max_vabits",
    "pagemap_commit",
    "page_commit_on_demand",
    "page_max_reclaim",
    "page_cross_thread_max_reclaim",
    "allow_thp",
    "minimal_purge_size",
    "arena_max_object_size",
    "arena_is_numa_local",
)

LEGACY_OPTION_NAMES = {
    "eager_region_commit": "arena_eager_commit",
    "reset_decommits": "purge_decommits",
    "large_os_pages": "allow_large_os_pages",
    "reset_delay": "purge_delay",
    "limit_os_alloc": "disallow_os_alloc",
    "decommit_extend_delay": "deprecated_purge_extend_delay",
    "abandoned_reclaim_on_free": "page_reclaim_on_free",
}

ENV_CANONICAL_VALUES = {
    "arena_eager_commit": "2",
    "allow_large_os_pages": "0",
    "reserve_huge_os_pages": "0",
    "reserve_huge_os_pages_at": "-1",
    "reserve_os_memory": "0",
    "disallow_os_alloc": "0",
    "destroy_on_exit": "0",
    "arena_reserve": "0",
    "disallow_arena_alloc": "0",
    "guarded_sample_rate": "0",
    "allow_thp": "0",
    "arena_is_numa_local": "0",
}

TOP_KEYS = (
    {"stat_version", "mimalloc_version", "process", "malloc_bins", "page_bins", "chunk_bins"}
    | STAT_COUNT_KEYS
    | STAT_COUNTER_KEYS
)


def parse_json_documents(output: str) -> dict[str, object]:
    docs: dict[str, object] = {}
    pos = 0
    while pos < len(output):
        if output.startswith("\n", pos):
            pos += 1
            continue
        if not output.startswith("@@JSON ", pos):
            raise AssertionError(f"unexpected JSON capture output at byte {pos}: {output[pos:pos + 80]!r}")
        header_end = output.index("\n", pos)
        _, name, length_text = output[pos:header_end].split()
        length = int(length_text)
        doc_start = header_end + 1
        doc_text = output[doc_start : doc_start + length]
        doc_end = doc_start + length
        if output[doc_end : doc_end + 7] != "\n@@END\n":
            raise AssertionError(f"missing JSON terminator for {name}")
        docs[name] = json.loads(doc_text)
        pos = doc_end + 7
    return docs


def schema(value: object) -> object:
    if isinstance(value, dict):
        return {key: schema(value[key]) for key in sorted(value)}
    if isinstance(value, list):
        return ["list", len(value), schema(value[0]) if value else None]
    return type(value).__name__


def require_int(value: object, name: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise AssertionError(f"{name} is not an integer")
    return value


def validate_count_object(value: object, name: str) -> None:
    if not isinstance(value, dict) or set(value) != {"total", "peak", "current"}:
        raise AssertionError(f"{name} has unexpected count schema")
    total = require_int(value["total"], f"{name}.total")
    peak = require_int(value["peak"], f"{name}.peak")
    current = require_int(value["current"], f"{name}.current")
    if total < 0 or peak < 0 or current < 0 or current > peak:
        raise AssertionError(f"{name} has invalid count values")


def validate_bin_counts(items: object, name: str) -> None:
    if not isinstance(items, list) or not items:
        raise AssertionError(f"{name} is not a non-empty array")
    for index, item in enumerate(items):
        item_name = f"{name}[{index}]"
        if not isinstance(item, dict) or set(item) != {"total", "peak", "current", "block_size", "page_size"}:
            raise AssertionError(f"{item_name} has unexpected bin schema")
        total = require_int(item["total"], f"{item_name}.total")
        peak = require_int(item["peak"], f"{item_name}.peak")
        current = require_int(item["current"], f"{item_name}.current")
        block_size = require_int(item["block_size"], f"{item_name}.block_size")
        page_size = require_int(item["page_size"], f"{item_name}.page_size")
        if total < 0 or peak < 0 or current < 0 or current > peak:
            raise AssertionError(f"{item_name} has invalid count values")
        if block_size < 0 or page_size < 0:
            raise AssertionError(f"{item_name} has invalid size values")


def validate_chunk_bins(items: object, name: str) -> None:
    expected_bins = ["S", "X", "M", "L", "H", " "]
    if not isinstance(items, list) or len(items) != len(expected_bins):
        raise AssertionError(f"{name} has unexpected length")
    if [item.get("bin") for item in items if isinstance(item, dict)] != expected_bins:
        raise AssertionError(f"{name} has unexpected labels")
    for index, item in enumerate(items):
        item_name = f"{name}[{index}]"
        if not isinstance(item, dict) or set(item) != {"total", "peak", "current", "bin"}:
            raise AssertionError(f"{item_name} has unexpected chunk-bin schema")
        total = require_int(item["total"], f"{item_name}.total")
        peak = require_int(item["peak"], f"{item_name}.peak")
        current = require_int(item["current"], f"{item_name}.current")
        if total < 0 or peak < 0 or current < 0 or current > peak:
            raise AssertionError(f"{item_name} has invalid count values")


def validate_stats_json(doc: object, name: str) -> None:
    if not isinstance(doc, dict) or set(doc) != TOP_KEYS:
        raise AssertionError(f"{name} has unexpected top-level schema")
    if doc["stat_version"] != 5 or doc["mimalloc_version"] != 30302:
        raise AssertionError(f"{name} has unexpected version fields")

    process = doc["process"]
    if not isinstance(process, dict) or set(process) != PROCESS_KEYS:
        raise AssertionError(f"{name}.process has unexpected schema")
    for key in PROCESS_KEYS:
        if require_int(process[key], f"{name}.process.{key}") < 0:
            raise AssertionError(f"{name}.process.{key} is negative")

    for key in STAT_COUNT_KEYS:
        validate_count_object(doc[key], f"{name}.{key}")
    for key in STAT_COUNTER_KEYS:
        if require_int(doc[key], f"{name}.{key}") < 0:
            raise AssertionError(f"{name}.{key} is negative")

    validate_bin_counts(doc["malloc_bins"], f"{name}.malloc_bins")
    validate_bin_counts(doc["page_bins"], f"{name}.page_bins")
    if len(doc["malloc_bins"]) != len(doc["page_bins"]):
        raise AssertionError(f"{name} malloc/page bin lengths differ")
    validate_chunk_bins(doc["chunk_bins"], f"{name}.chunk_bins")


def validate_json_capture(c_docs: dict[str, object], rust_docs: dict[str, object]) -> None:
    common_names = {
        "stats_get_json_buffer",
        "stats_as_json_buffer",
        "heap_stats_get_json_buffer",
        "subproc_stats_get_json_buffer",
        "stats_get_json_allocated",
    }
    if set(c_docs) != common_names:
        raise AssertionError(f"unexpected C JSON documents: {sorted(c_docs)}")
    if set(rust_docs) != common_names | {"stats_get_json_from_buffer"}:
        raise AssertionError(f"unexpected Rust JSON documents: {sorted(rust_docs)}")

    for name in sorted(common_names):
        validate_stats_json(c_docs[name], f"c.{name}")
        validate_stats_json(rust_docs[name], f"rust.{name}")
        if schema(c_docs[name]) != schema(rust_docs[name]):
            raise AssertionError(f"JSON schema mismatch for {name}")

    validate_stats_json(rust_docs["stats_get_json_from_buffer"], "rust.stats_get_json_from_buffer")
    if schema(rust_docs["stats_get_json_from_buffer"]) != schema(rust_docs["stats_as_json_buffer"]):
        raise AssertionError("Rust mi_stats_get_json_from schema differs from mi_stats_as_json")


def parse_cxx_new_handler_capture(output: str) -> dict[str, int]:
    fields = output.strip().split()
    if len(fields) != 4 or fields[0] != "@@CXX_NEW":
        raise AssertionError(f"unexpected C++ new-handler output: {output!r}")
    values: dict[str, int] = {}
    for field in fields[1:]:
        key, value = field.split("=", 1)
        values[key] = int(value)
    expected = {"overflow_null": 1, "handler_calls": 1, "new_success": 1}
    if values != expected:
        raise AssertionError(f"unexpected C++ new-handler values: {values}")
    return values


def parse_cxx_override_capture(output: str) -> dict[str, int]:
    fields = output.strip().split()
    if len(fields) != 7 or fields[0] != "@@CXX_OVERRIDE":
        raise AssertionError(f"unexpected C++ override output: {output!r}")
    values: dict[str, int] = {}
    for field in fields[1:]:
        key, value = field.split("=", 1)
        values[key] = int(value)
    expected = {
        "scalar": 1,
        "array": 1,
        "nothrow": 1,
        "aligned": 1,
        "raw_aligned": 1,
        "raw_aligned_nothrow": 1,
    }
    if values != expected:
        raise AssertionError(f"unexpected C++ override values: {values}")
    return values


def parse_cxx_stl_allocator_capture(output: str) -> dict[str, int]:
    fields = output.strip().split()
    if len(fields) != 13 or fields[0] != "@@CXX_STL_ALLOCATOR":
        raise AssertionError(f"unexpected C++ STL allocator output: {output!r}")
    values: dict[str, int] = {}
    for field in fields[1:]:
        key, value = field.split("=", 1)
        values[key] = int(value)
    expected = {
        "trait": 1,
        "plain": 1,
        "plain_vector": 1,
        "construct": 1,
        "destroy": 1,
        "exception": 1,
        "heap": 1,
        "heap_vector": 1,
        "heap_eq": 1,
        "heap_ne": 1,
        "destroy_heap": 1,
        "destroy_eq": 1,
    }
    if values != expected:
        raise AssertionError(f"unexpected C++ STL allocator values: {values}")
    return values


def parse_redirect_capture(output: str) -> dict[str, int | str]:
    fields = output.strip().split()
    if len(fields) < 5 or fields[0] != "@@REDIRECT":
        raise AssertionError(f"unexpected redirect capture output: {output!r}")
    values: dict[str, int | str] = {"mode": fields[1]}
    for field in fields[2:]:
        key, value = field.split("=", 1)
        values[key] = int(value)
    return values


def validate_redirect_capture(
    c_static_bin: pathlib.Path,
    rust_static_bin: pathlib.Path,
    c_dynamic_bin: pathlib.Path,
    rust_dynamic_bin: pathlib.Path,
    c_shared: pathlib.Path,
    rust_shared: pathlib.Path,
) -> None:
    cases = {
        "static": (
            parse_redirect_capture(run(c_static_bin)),
            parse_redirect_capture(run(rust_static_bin)),
        ),
        "dlopen": (
            parse_redirect_capture(run_args(c_dynamic_bin, [str(c_shared)])),
            parse_redirect_capture(run_args(rust_dynamic_bin, [str(rust_shared)])),
        ),
        "preload": (
            parse_redirect_capture(
                run_with_env(c_dynamic_bin, {"LD_PRELOAD": str(c_shared)}, "C redirect preload")
            ),
            parse_redirect_capture(
                run_with_env(rust_dynamic_bin, {"LD_PRELOAD": str(rust_shared)}, "Rust redirect preload")
            ),
        ),
    }
    for mode, (c_values, rust_values) in cases.items():
        if c_values != rust_values:
            raise AssertionError(f"{mode} mismatch: C={c_values} Rust={rust_values}")
        if c_values["mode"] != mode:
            raise AssertionError(f"{mode} reported wrong mode: {c_values}")
        if c_values["mi_malloc_owned"] != 1:
            raise AssertionError(f"{mode} mi_malloc allocation was not owned: {c_values}")
    if cases["dlopen"][0]["malloc_owned"] != 0:
        raise AssertionError(f"dlopen unexpectedly redirects plain malloc: {cases['dlopen'][0]}")
    if cases["preload"][0]["malloc_owned"] != 1:
        raise AssertionError(f"LD_PRELOAD did not redirect plain malloc: {cases['preload'][0]}")
    if cases["static"][0]["family_ok"] != 1:
        raise AssertionError(f"static malloc family aliases failed: {cases['static'][0]}")
    if cases["dlopen"][0]["family_ok"] != -1:
        raise AssertionError(f"dlopen unexpectedly exercised malloc family aliases: {cases['dlopen'][0]}")
    if cases["preload"][0]["family_ok"] != 1:
        raise AssertionError(f"LD_PRELOAD malloc family aliases failed: {cases['preload'][0]}")


PROCESS_CAPTURE_KEYS = (
    "elapsed",
    "user",
    "system",
    "current_rss",
    "peak_rss",
    "current_commit",
    "peak_commit",
    "page_faults",
)


def parse_process_capture(output: str) -> dict[str, dict[str, int]]:
    phases: dict[str, dict[str, int]] = {}
    for line in output.splitlines():
        fields = line.split()
        if len(fields) != 10 or fields[0] != "@@PROCESS":
            raise AssertionError(f"unexpected process capture line: {line!r}")
        phase = fields[1]
        phases[phase] = {
            key: int(value)
            for key, value in zip(PROCESS_CAPTURE_KEYS, fields[2:], strict=True)
        }
    expected = {"initial", "allocated", "collected"}
    if set(phases) != expected:
        raise AssertionError(f"unexpected process phases: {sorted(phases)}")
    return phases


def close_enough(left: int, right: int, absolute: int, ratio: float) -> bool:
    if abs(left - right) <= absolute:
        return True
    high = max(left, right)
    low = min(left, right)
    return high == 0 or (low > 0 and high / low <= ratio)


def validate_process_capture(c_info: dict[str, dict[str, int]], rust_info: dict[str, dict[str, int]]) -> None:
    for label, info in (("c", c_info), ("rust", rust_info)):
        for phase, fields in info.items():
            for key, value in fields.items():
                if value < 0:
                    raise AssertionError(f"{label}.{phase}.{key} is negative")
            if fields["peak_commit"] < fields["current_commit"]:
                raise AssertionError(f"{label}.{phase} commit peak is below current")
            if fields["peak_rss"] < fields["current_rss"]:
                raise AssertionError(f"{label}.{phase} RSS peak is below current")
        if info["allocated"]["elapsed"] < info["initial"]["elapsed"]:
            raise AssertionError(f"{label} elapsed time moved backward after allocation")
        if info["collected"]["elapsed"] < info["allocated"]["elapsed"]:
            raise AssertionError(f"{label} elapsed time moved backward after collection")

    for phase in ("initial", "allocated", "collected"):
        c_phase = c_info[phase]
        rust_phase = rust_info[phase]
        for key in ("elapsed", "user", "system"):
            if abs(c_phase[key] - rust_phase[key]) > 5000:
                raise AssertionError(f"{phase}.{key} differs by more than 5s")
        if abs(c_phase["page_faults"] - rust_phase["page_faults"]) > 10000:
            raise AssertionError(f"{phase}.page_faults differs by more than 10000")
        for key in ("current_rss", "peak_rss", "current_commit", "peak_commit"):
            if not close_enough(c_phase[key], rust_phase[key], 64 * 1024 * 1024, 4.0):
                raise AssertionError(f"{phase}.{key} differs beyond RSS/commit tolerance: C={c_phase[key]} Rust={rust_phase[key]}")


def parse_env_capture(output: str) -> tuple[dict[int, tuple[int, int, int]], dict[str, int]]:
    options: dict[int, tuple[int, int, int]] = {}
    output_flags: dict[str, int] | None = None
    for line in output.splitlines():
        fields = line.split()
        if len(fields) == 5 and fields[0] == "@@OPTION":
            options[int(fields[1])] = (int(fields[2]), int(fields[3]), int(fields[4]))
        elif len(fields) == 6 and fields[0] == "@@OUTPUT":
            output_flags = {}
            for field in fields[1:]:
                key, value = field.split("=", 1)
                output_flags[key] = int(value)
        else:
            raise AssertionError(f"unexpected env capture line: {line!r}")
    if set(options) != set(range(len(OPTION_NAMES))):
        raise AssertionError(f"unexpected env option indexes: {sorted(options)}")
    if output_flags is None:
        raise AssertionError("missing env output flags")
    return options, output_flags


def env_case_matrix() -> list[tuple[str, dict[str, str]]]:
    cases: list[tuple[str, dict[str, str]]] = []
    for name in OPTION_NAMES:
        cases.append((f"canonical_{name}", {f"mimalloc_{name}": ENV_CANONICAL_VALUES.get(name, "7")}))
    for legacy, canonical in LEGACY_OPTION_NAMES.items():
        cases.append((f"legacy_{legacy}", {f"mimalloc_{legacy}": ENV_CANONICAL_VALUES.get(canonical, "11")}))
    for raw in ("", "1", "TRUE", "true", "YES", "on"):
        cases.append((f"bool_true_{raw or 'empty'}", {"mimalloc_verbose": raw}))
    for raw in ("0", "FALSE", "false", "NO", "off"):
        cases.append((f"bool_false_{raw}", {"mimalloc_verbose": raw}))
    for raw in ("4097", "3K", "3KB", "3KiB", "2M", "2MiB", "1G", "1GiB", "1T", "-9"):
        cases.append((f"size_suffix_{raw}", {"mimalloc_reserve_os_memory": raw}))
    cases.extend(
        [
            ("invalid_verbose", {"mimalloc_verbose": "bogus"}),
            ("invalid_size", {"mimalloc_reserve_os_memory": "12XB"}),
            ("uppercase_name", {"MIMALLOC_VERBOSE": "5"}),
            ("mixedcase_name", {"MiMaLlOc_VeRbOsE": "6"}),
            ("duplicate_case_first_lower", {"mimalloc_verbose": "3", "MIMALLOC_VERBOSE": "4"}),
            ("duplicate_case_first_upper", {"MIMALLOC_VERBOSE": "4", "mimalloc_verbose": "3"}),
        ]
    )
    return cases


def validate_env_capture(c_bin: pathlib.Path, rust_bin: pathlib.Path) -> None:
    for label, env in env_case_matrix():
        c_output = run_with_env(c_bin, env, label)
        rust_output = run_with_env(rust_bin, env, label)
        c_options, c_flags = parse_env_capture(c_output)
        rust_options, rust_flags = parse_env_capture(rust_output)
        if c_options != rust_options:
            raise AssertionError(f"{label} option mismatch:\nC={c_output}\nRust={rust_output}")
        if label.startswith("legacy_") and c_flags["deprecated"] != rust_flags["deprecated"]:
            raise AssertionError(f"{label} deprecated warning mismatch: C={c_flags} Rust={rust_flags}")
        if label.startswith("invalid_") and c_flags["invalid"] != rust_flags["invalid"]:
            raise AssertionError(f"{label} invalid warning mismatch: C={c_flags} Rust={rust_flags}")


def parse_throttle_capture(output: str) -> dict[str, int]:
    fields = output.strip().split()
    if len(fields) != 4 or fields[0] != "@@THROTTLE":
        raise AssertionError(f"unexpected throttle capture output: {output!r}")
    values: dict[str, int] = {}
    for field in fields[1:]:
        key, value = field.split("=", 1)
        values[key] = int(value)
    return values


def throttle_case_matrix() -> list[tuple[str, dict[str, str]]]:
    return [
        (
            "zero_limits",
            {
                "mimalloc_show_errors": "1",
                "mimalloc_verbose": "0",
                "mimalloc_max_warnings": "0",
                "mimalloc_max_errors": "0",
            },
        ),
        (
            "finite_limits",
            {
                "mimalloc_show_errors": "1",
                "mimalloc_verbose": "0",
                "mimalloc_max_warnings": "2",
                "mimalloc_max_errors": "3",
            },
        ),
        (
            "unlimited_limits",
            {
                "mimalloc_show_errors": "1",
                "mimalloc_verbose": "0",
                "mimalloc_max_warnings": "-1",
                "mimalloc_max_errors": "-1",
            },
        ),
    ]


def validate_throttle_capture(c_bin: pathlib.Path, rust_bin: pathlib.Path) -> None:
    for label, env in throttle_case_matrix():
        c_output = run_with_env(c_bin, env, label)
        rust_output = run_with_env(rust_bin, env, label)
        c_counts = parse_throttle_capture(c_output)
        rust_counts = parse_throttle_capture(rust_output)
        if (c_counts["warnings"], c_counts["errors"]) != (
            rust_counts["warnings"],
            rust_counts["errors"],
        ):
            raise AssertionError(f"{label} throttle mismatch: C={c_counts} Rust={rust_counts}")


def parse_process_lifecycle_capture(output: str) -> dict[str, int]:
    fields = output.strip().split()
    if len(fields) != 6 or fields[0] != "@@PROCESS_LIFECYCLE":
        raise AssertionError(f"unexpected process lifecycle output: {output!r}")
    values: dict[str, int] = {}
    for field in fields[1:]:
        key, value = field.split("=", 1)
        values[key] = int(value)
    return values


def process_lifecycle_case_matrix() -> list[tuple[str, dict[str, str]]]:
    return [
        ("default", {}),
        ("show_stats", {"mimalloc_show_stats": "1"}),
        ("verbose", {"mimalloc_verbose": "1"}),
        ("destroy_on_exit", {"mimalloc_destroy_on_exit": "1"}),
        ("auto_done_skip", {"mimalloc_destroy_on_exit": "2"}),
    ]


def validate_process_lifecycle_capture(c_bin: pathlib.Path, rust_bin: pathlib.Path) -> None:
    for label, env in process_lifecycle_case_matrix():
        c_output = run_with_env(c_bin, env, label)
        rust_output = run_with_env(rust_bin, env, label)
        c_counts = parse_process_lifecycle_capture(c_output)
        rust_counts = parse_process_lifecycle_capture(rust_output)
        compared = ("stats", "process", "option")
        if tuple(c_counts[key] for key in compared) != tuple(rust_counts[key] for key in compared):
            raise AssertionError(f"{label} process lifecycle mismatch: C={c_counts} Rust={rust_counts}")


def parse_numa_affinity_capture(output: str) -> dict[str, int]:
    fields = output.strip().split()
    if len(fields) != 5 or fields[0] != "@@NUMA":
        raise AssertionError(f"unexpected NUMA affinity output: {output!r}")
    values: dict[str, int] = {}
    for field in fields[1:]:
        key, value = field.split("=", 1)
        values[key] = int(value)
    return values


def validate_numa_affinity_capture(c_bin: pathlib.Path, rust_bin: pathlib.Path) -> None:
    cases = [
        (
            "numa_affinity_forced",
            {"mimalloc_use_numa_nodes": "4", "mimalloc_arena_reserve": "0"},
            {"managed": 11, "node1": 1, "node0": 10, "reset": 1},
        ),
        (
            "numa_affinity_single_node",
            {"mimalloc_use_numa_nodes": "1", "mimalloc_arena_reserve": "0"},
            {"managed": 11, "node1": 10, "node0": 10, "reset": 1},
        ),
    ]
    for label, env, expected in cases:
        c_counts = parse_numa_affinity_capture(run_with_env(c_bin, env, f"{label}_c"))
        rust_counts = parse_numa_affinity_capture(run_with_env(rust_bin, env, f"{label}_rust"))
        if c_counts != rust_counts:
            raise AssertionError(f"{label} mismatch: C={c_counts} Rust={rust_counts}")
        if c_counts != expected:
            raise AssertionError(f"{label} unexpected values: {c_counts}")


def parse_arena_fallback_capture(output: str) -> dict[str, str]:
    fields = output.strip().split()
    if len(fields) != 11 or fields[0] != "@@ARENA_FALLBACK":
        raise AssertionError(f"unexpected arena fallback output: {output!r}")
    values: dict[str, str] = {}
    for field in fields[1:]:
        key, value = field.split("=", 1)
        values[key] = value
    return values


def validate_arena_fallback_capture(c_bin: pathlib.Path, rust_bin: pathlib.Path) -> None:
    cases = {
        "arena_only": {"ptr": "1", "in_arena": "1", "errno": "0", "errors": "0"},
        "os_fallback": {"ptr": "1", "in_arena": "0", "errno": "0", "errors": "0"},
        "all_disabled": {"ptr": "0", "in_arena": "0", "errno": "12"},
        "exclusive_arena_disabled": {"ptr": "0", "in_arena": "0", "errno": "12"},
    }
    for name, expected in cases.items():
        c_status, c_stdout, _ = run_args_status(c_bin, [name])
        rust_status, rust_stdout, _ = run_args_status(rust_bin, [name])
        if c_status != rust_status:
            raise AssertionError(
                f"{name} status mismatch: C(status={c_status}, out={c_stdout!r}) "
                f"Rust(status={rust_status}, out={rust_stdout!r})"
            )
        if c_status != 0:
            raise AssertionError(f"{name} exited with status {c_status}: {c_stdout!r}")
        c_values = parse_arena_fallback_capture(c_stdout)
        rust_values = parse_arena_fallback_capture(rust_stdout)
        for key, value in {"case": name, "managed": "1", "heap": "1", **expected}.items():
            if c_values.get(key) != value or rust_values.get(key) != value:
                raise AssertionError(f"{name} unexpected {key}: C={c_values} Rust={rust_values}")
        if expected["ptr"] == "0":
            if int(c_values["output"]) <= 0 or int(rust_values["output"]) <= 0:
                raise AssertionError(f"{name} missing error output: C={c_values} Rust={rust_values}")
            if c_values["output_has_percent"] != "0" or rust_values["output_has_percent"] != "0":
                raise AssertionError(f"{name} unformatted error output: C={c_values} Rust={rust_values}")


def main() -> int:
    cc = os.environ.get("CC") or shutil.which("cc") or shutil.which("gcc")
    if cc is None:
        raise SystemExit("no C compiler found; set CC")
    cxx = os.environ.get("CXX") or shutil.which("c++") or shutil.which("g++") or shutil.which("clang++")
    if cxx is None:
        raise SystemExit("no C++ compiler found; set CXX")

    BUILD_DIR.mkdir(parents=True, exist_ok=True)
    scenario = BUILD_DIR / "behavioral-parity.c"
    json_capture = BUILD_DIR / "json-capture.c"
    cxx_new_handler_capture = BUILD_DIR / "cxx-new-handler.cpp"
    cxx_override_capture = BUILD_DIR / "cxx-override.cpp"
    cxx_stl_allocator_capture = BUILD_DIR / "cxx-stl-allocator.cpp"
    process_capture = BUILD_DIR / "process-capture.c"
    env_capture = BUILD_DIR / "env-capture.c"
    throttle_capture = BUILD_DIR / "throttle-capture.c"
    process_lifecycle = BUILD_DIR / "process-lifecycle.c"
    numa_affinity_capture = BUILD_DIR / "numa-affinity-capture.c"
    arena_fallback_capture = BUILD_DIR / "arena-fallback-capture.c"
    free_hint_release_capture = BUILD_DIR / "free-hint-release-capture.c"
    stress_release_capture = BUILD_DIR / "stress-release-capture.c"
    huge_os_pages_capture = BUILD_DIR / "huge-os-pages-capture.c"
    redirect_static_capture = BUILD_DIR / "redirect-static-capture.c"
    redirect_dynamic_capture = BUILD_DIR / "redirect-dynamic-capture.c"
    scenario.write_text(SCENARIO_C)
    json_capture.write_text(JSON_CAPTURE_C)
    cxx_new_handler_capture.write_text(CXX_NEW_HANDLER_CAPTURE)
    cxx_override_capture.write_text(CXX_OVERRIDE_CAPTURE)
    cxx_stl_allocator_capture.write_text(CXX_STL_ALLOCATOR_CAPTURE)
    process_capture.write_text(PROCESS_CAPTURE_C)
    env_capture.write_text(ENV_CAPTURE_C)
    throttle_capture.write_text(THROTTLE_CAPTURE_C)
    process_lifecycle.write_text(PROCESS_LIFECYCLE_C)
    numa_affinity_capture.write_text(NUMA_AFFINITY_CAPTURE_C)
    arena_fallback_capture.write_text(ARENA_FALLBACK_CAPTURE_C)
    free_hint_release_capture.write_text(FREE_HINT_RELEASE_CAPTURE_C)
    stress_release_capture.write_text(STRESS_RELEASE_CAPTURE_C)
    huge_os_pages_capture.write_text(HUGE_OS_PAGES_CAPTURE_C)
    redirect_static_capture.write_text(REDIRECT_STATIC_CAPTURE_C)
    redirect_dynamic_capture.write_text(REDIRECT_DYNAMIC_CAPTURE_C)

    rust_lib = rust_staticlib()
    rust_release_lib = rust_staticlib_release()
    rust_shared = rust_cdylib()
    c_bin = BUILD_DIR / "behavioral-parity-c"
    rust_bin = BUILD_DIR / "behavioral-parity-rust"
    c_json_bin = BUILD_DIR / "json-capture-c"
    rust_json_bin = BUILD_DIR / "json-capture-rust"
    c_cxx_new_handler_bin = BUILD_DIR / "cxx-new-handler-c"
    rust_cxx_new_handler_bin = BUILD_DIR / "cxx-new-handler-rust"
    c_cxx_override_bin = BUILD_DIR / "cxx-override-c"
    rust_cxx_override_bin = BUILD_DIR / "cxx-override-rust"
    c_cxx_stl_allocator_bin = BUILD_DIR / "cxx-stl-allocator-c"
    rust_cxx_stl_allocator_bin = BUILD_DIR / "cxx-stl-allocator-rust"
    c_process_bin = BUILD_DIR / "process-capture-c"
    rust_process_bin = BUILD_DIR / "process-capture-rust"
    c_env_bin = BUILD_DIR / "env-capture-c"
    rust_env_bin = BUILD_DIR / "env-capture-rust"
    c_throttle_bin = BUILD_DIR / "throttle-capture-c"
    rust_throttle_bin = BUILD_DIR / "throttle-capture-rust"
    c_lifecycle_bin = BUILD_DIR / "process-lifecycle-c"
    rust_lifecycle_bin = BUILD_DIR / "process-lifecycle-rust"
    c_numa_affinity_bin = BUILD_DIR / "numa-affinity-c"
    rust_numa_affinity_bin = BUILD_DIR / "numa-affinity-rust"
    c_arena_fallback_bin = BUILD_DIR / "arena-fallback-c"
    rust_arena_fallback_bin = BUILD_DIR / "arena-fallback-rust"
    c_free_hint_release_bin = BUILD_DIR / "free-hint-release-c"
    rust_free_hint_release_bin = BUILD_DIR / "free-hint-release-rust"
    c_stress_release_bin = BUILD_DIR / "stress-release-c"
    rust_stress_release_bin = BUILD_DIR / "stress-release-rust"
    c_huge_os_pages_bin = BUILD_DIR / "huge-os-pages-c"
    rust_huge_os_pages_bin = BUILD_DIR / "huge-os-pages-rust"
    c_redirect_static_bin = BUILD_DIR / "redirect-static-c"
    rust_redirect_static_bin = BUILD_DIR / "redirect-static-rust"
    c_redirect_dynamic_bin = BUILD_DIR / "redirect-dynamic-c"
    rust_redirect_dynamic_bin = BUILD_DIR / "redirect-dynamic-rust"
    c_shared = BUILD_DIR / "libmimalloc-c.so"

    compile_binary(cc, c_bin, [ROOT / "mimalloc-3.3.2/src/static.c", scenario])
    compile_binary(cc, rust_bin, [scenario, rust_lib])
    compile_binary(cc, c_json_bin, [ROOT / "mimalloc-3.3.2/src/static.c", json_capture])
    compile_binary(cc, rust_json_bin, [json_capture, rust_lib], ["-DRUST_HELPER_CAPTURE"])
    compile_cxx_binary(cxx, c_cxx_new_handler_bin, [cxx_new_handler_capture, ROOT / "mimalloc-3.3.2/src/static.c"])
    compile_cxx_binary(cxx, rust_cxx_new_handler_bin, [cxx_new_handler_capture, rust_lib])
    compile_cxx_binary(
        cxx,
        c_cxx_override_bin,
        [cxx_override_capture, ROOT / "mimalloc-3.3.2/src/static.c"],
        ["-DMI_MALLOC_OVERRIDE"],
    )
    compile_cxx_binary(cxx, rust_cxx_override_bin, [cxx_override_capture, rust_lib])
    compile_cxx_binary(cxx, c_cxx_stl_allocator_bin, [cxx_stl_allocator_capture, ROOT / "mimalloc-3.3.2/src/static.c"])
    compile_cxx_binary(cxx, rust_cxx_stl_allocator_bin, [cxx_stl_allocator_capture, rust_lib])
    compile_binary(cc, c_process_bin, [ROOT / "mimalloc-3.3.2/src/static.c", process_capture])
    compile_binary(cc, rust_process_bin, [process_capture, rust_lib])
    compile_binary(cc, c_env_bin, [ROOT / "mimalloc-3.3.2/src/static.c", env_capture])
    compile_binary(cc, rust_env_bin, [env_capture, rust_lib])
    compile_binary(cc, c_throttle_bin, [ROOT / "mimalloc-3.3.2/src/static.c", throttle_capture])
    compile_binary(cc, rust_throttle_bin, [throttle_capture, rust_lib])
    compile_binary(cc, c_lifecycle_bin, [ROOT / "mimalloc-3.3.2/src/static.c", process_lifecycle])
    compile_binary(cc, rust_lifecycle_bin, [process_lifecycle, rust_lib])
    compile_binary(cc, c_numa_affinity_bin, [ROOT / "mimalloc-3.3.2/src/static.c", numa_affinity_capture])
    compile_binary(cc, rust_numa_affinity_bin, [numa_affinity_capture, rust_lib])
    compile_binary(cc, c_arena_fallback_bin, [ROOT / "mimalloc-3.3.2/src/static.c", arena_fallback_capture])
    compile_binary(cc, rust_arena_fallback_bin, [arena_fallback_capture, rust_lib])
    compile_binary(
        cc,
        c_free_hint_release_bin,
        [ROOT / "mimalloc-3.3.2/src/static.c", free_hint_release_capture],
        ["-DNDEBUG"],
    )
    compile_binary(cc, rust_free_hint_release_bin, [free_hint_release_capture, rust_release_lib], ["-DNDEBUG"])
    compile_binary(
        cc,
        c_stress_release_bin,
        [ROOT / "mimalloc-3.3.2/src/static.c", stress_release_capture],
        ["-DNDEBUG"],
    )
    compile_binary(cc, rust_stress_release_bin, [stress_release_capture, rust_release_lib], ["-DNDEBUG"])
    compile_binary(cc, c_huge_os_pages_bin, [ROOT / "mimalloc-3.3.2/src/static.c", huge_os_pages_capture])
    compile_binary(cc, rust_huge_os_pages_bin, [huge_os_pages_capture, rust_lib])
    compile_binary(
        cc,
        c_redirect_static_bin,
        [ROOT / "mimalloc-3.3.2/src/static.c", redirect_static_capture],
        ["-DMI_MALLOC_OVERRIDE"],
    )
    compile_binary(cc, rust_redirect_static_bin, [redirect_static_capture, rust_lib])
    compile_binary(cc, c_redirect_dynamic_bin, [redirect_dynamic_capture])
    compile_binary(cc, rust_redirect_dynamic_bin, [redirect_dynamic_capture])
    compile_shared_library(
        cc,
        c_shared,
        [ROOT / "mimalloc-3.3.2/src/static.c"],
        ["-DMI_MALLOC_OVERRIDE"],
    )

    c_output = run(c_bin)
    rust_output = run(rust_bin)
    if c_output != rust_output:
        print("behavioral parity mismatch", file=sys.stderr)
        print("--- C mimalloc ---", file=sys.stderr)
        print(c_output, file=sys.stderr)
        print("--- Rust mimalloc ---", file=sys.stderr)
        print(rust_output, file=sys.stderr)
        return 1

    c_json_docs = parse_json_documents(run(c_json_bin))
    rust_json_docs = parse_json_documents(run(rust_json_bin))
    try:
        validate_json_capture(c_json_docs, rust_json_docs)
    except AssertionError as err:
        print(f"JSON parity mismatch: {err}", file=sys.stderr)
        return 1

    try:
        c_cxx_new_handler = parse_cxx_new_handler_capture(run(c_cxx_new_handler_bin))
        rust_cxx_new_handler = parse_cxx_new_handler_capture(run(rust_cxx_new_handler_bin))
        if c_cxx_new_handler != rust_cxx_new_handler:
            raise AssertionError(f"C={c_cxx_new_handler} Rust={rust_cxx_new_handler}")
    except AssertionError as err:
        print(f"C++ new-handler parity mismatch: {err}", file=sys.stderr)
        return 1

    try:
        c_cxx_override = parse_cxx_override_capture(run(c_cxx_override_bin))
        rust_cxx_override = parse_cxx_override_capture(run(rust_cxx_override_bin))
        if c_cxx_override != rust_cxx_override:
            raise AssertionError(f"C={c_cxx_override} Rust={rust_cxx_override}")
    except AssertionError as err:
        print(f"C++ override parity mismatch: {err}", file=sys.stderr)
        return 1

    try:
        c_cxx_stl_allocator = parse_cxx_stl_allocator_capture(run(c_cxx_stl_allocator_bin))
        rust_cxx_stl_allocator = parse_cxx_stl_allocator_capture(run(rust_cxx_stl_allocator_bin))
        if c_cxx_stl_allocator != rust_cxx_stl_allocator:
            raise AssertionError(f"C={c_cxx_stl_allocator} Rust={rust_cxx_stl_allocator}")
    except AssertionError as err:
        print(f"C++ STL allocator parity mismatch: {err}", file=sys.stderr)
        return 1

    try:
        validate_process_capture(parse_process_capture(run(c_process_bin)), parse_process_capture(run(rust_process_bin)))
    except AssertionError as err:
        print(f"process info parity mismatch: {err}", file=sys.stderr)
        return 1

    try:
        validate_env_capture(c_env_bin, rust_env_bin)
    except AssertionError as err:
        print(f"environment option parity mismatch: {err}", file=sys.stderr)
        return 1

    try:
        validate_throttle_capture(c_throttle_bin, rust_throttle_bin)
    except AssertionError as err:
        print(f"warning/error throttle parity mismatch: {err}", file=sys.stderr)
        return 1

    try:
        validate_process_lifecycle_capture(c_lifecycle_bin, rust_lifecycle_bin)
    except AssertionError as err:
        print(f"process lifecycle parity mismatch: {err}", file=sys.stderr)
        return 1

    try:
        validate_numa_affinity_capture(c_numa_affinity_bin, rust_numa_affinity_bin)
    except AssertionError as err:
        print(f"NUMA affinity parity mismatch: {err}", file=sys.stderr)
        return 1

    try:
        validate_arena_fallback_capture(c_arena_fallback_bin, rust_arena_fallback_bin)
    except AssertionError as err:
        print(f"arena fallback parity mismatch: {err}", file=sys.stderr)
        return 1

    c_free_hint_release = run(c_free_hint_release_bin)
    rust_free_hint_release = run(rust_free_hint_release_bin)
    if c_free_hint_release != rust_free_hint_release:
        print("free hint release parity mismatch", file=sys.stderr)
        print("--- C mimalloc ---", file=sys.stderr)
        print(c_free_hint_release, file=sys.stderr)
        print("--- Rust mimalloc ---", file=sys.stderr)
        print(rust_free_hint_release, file=sys.stderr)
        return 1

    c_stress_release = run(c_stress_release_bin)
    rust_stress_release = run(rust_stress_release_bin)
    if c_stress_release != rust_stress_release:
        print("stress release parity mismatch", file=sys.stderr)
        print("--- C mimalloc ---", file=sys.stderr)
        print(c_stress_release, file=sys.stderr)
        print("--- Rust mimalloc ---", file=sys.stderr)
        print(rust_stress_release, file=sys.stderr)
        return 1

    c_huge_os_pages = run(c_huge_os_pages_bin)
    rust_huge_os_pages = run(rust_huge_os_pages_bin)
    if c_huge_os_pages != rust_huge_os_pages:
        print("huge OS page parity mismatch", file=sys.stderr)
        print("--- C mimalloc ---", file=sys.stderr)
        print(c_huge_os_pages, file=sys.stderr)
        print("--- Rust mimalloc ---", file=sys.stderr)
        print(rust_huge_os_pages, file=sys.stderr)
        return 1

    try:
        validate_redirect_capture(
            c_redirect_static_bin,
            rust_redirect_static_bin,
            c_redirect_dynamic_bin,
            rust_redirect_dynamic_bin,
            c_shared,
            rust_shared,
        )
    except AssertionError as err:
        print(f"redirection parity mismatch: {err}", file=sys.stderr)
        return 1

    print("ok: C and Rust mimalloc behavior smoke outputs match")
    print("ok: C and Rust mimalloc JSON schemas parse and match")
    print("ok: C and Rust mimalloc C++ new-handler behavior matches")
    print("ok: C and Rust mimalloc C++ override behavior matches")
    print("ok: C and Rust mimalloc C++ STL allocator behavior matches")
    print("ok: C and Rust mimalloc process info is within tolerance")
    print("ok: C and Rust mimalloc environment option parsing matches")
    print("ok: C and Rust mimalloc warning/error throttling matches")
    print("ok: C and Rust mimalloc process lifecycle output matches")
    print("ok: C and Rust mimalloc NUMA affinity behavior matches")
    print("ok: C and Rust mimalloc arena fallback behavior matches")
    print("ok: C and Rust mimalloc release free-hint behavior matches")
    print("ok: C and Rust mimalloc release stress behavior matches")
    print("ok: C and Rust mimalloc huge OS page fallback behavior matches")
    print("ok: C and Rust mimalloc static/dynamic redirection behavior matches")
    print(c_output, end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
