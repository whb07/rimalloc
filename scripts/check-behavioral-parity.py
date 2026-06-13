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
static volatile size_t overflow_count = (size_t)-1;

typedef void* (*system_malloc_fun)(size_t);
typedef void (*system_free_fun)(void*);

static void output_callback(const char* msg, void* arg) {
  (void)arg;
  if (msg != NULL && msg[0] != '\0') {
    output_callbacks++;
  }
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

static void registered_error_callback_a(int err, void* arg) {
  if (err != 0 && arg == (void*)0xE11A) {
    registered_error_a++;
  }
}

static void registered_error_callback_b(int err, void* arg) {
  if (err != 0 && arg == (void*)0xE22B) {
    registered_error_b++;
  }
}

static void result_bool(const char* name, int ok) {
  printf("%s=%d\n", name, ok ? 1 : 0);
  if (!ok) failures++;
}

static void result_size(const char* name, size_t value) {
  printf("%s=%zu\n", name, value);
}

static void result_long(const char* name, long value) {
  printf("%s=%ld\n", name, value);
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

  registered_error_a = 0;
  registered_error_b = 0;
  mi_register_error(registered_error_callback_a, (void*)0xE11A);
  volatile void* err_alloc = mi_calloc(overflow_count, 2);
  (void)err_alloc;
  result_bool("register_error_a_called", registered_error_a > 0);
  mi_register_error(registered_error_callback_b, (void*)0xE22B);
  err_alloc = mi_calloc(overflow_count, 2);
  (void)err_alloc;
  result_bool("register_error_b_called", registered_error_b > 0);
  mi_register_error(NULL, NULL);
  size_t error_after_clear = registered_error_b;
  err_alloc = mi_calloc(overflow_count, 2);
  (void)err_alloc;
  result_bool("register_error_clear", registered_error_b == error_after_clear);

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
