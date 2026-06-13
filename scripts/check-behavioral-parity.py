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
#include <sys/mman.h>
#include <pthread.h>
#include <stdint.h>
#include <stdio.h>
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
  mi_free(c);

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
