#include <stdbool.h>
#include <stddef.h>

#if MI_TRACK_VALGRIND
#include <valgrind/memcheck.h>

void _mi_track_malloc_size_shim(void* p, size_t reqsize, size_t size, bool zero) {
  (void)reqsize;
  VALGRIND_MALLOCLIKE_BLOCK(p, size, 0, zero);
}

void _mi_track_free_size_shim(void* p, size_t size) {
  (void)size;
  VALGRIND_FREELIKE_BLOCK(p, 0);
}

void _mi_track_resize_shim(void* p, size_t oldsize, size_t newsize) {
  VALGRIND_RESIZEINPLACE_BLOCK(p, oldsize, newsize, 0);
}

void _mi_track_mem_defined_shim(void* p, size_t size) {
  VALGRIND_MAKE_MEM_DEFINED(p, size);
}

void _mi_track_mem_undefined_shim(void* p, size_t size) {
  VALGRIND_MAKE_MEM_UNDEFINED(p, size);
}

void _mi_track_mem_noaccess_shim(void* p, size_t size) {
  VALGRIND_MAKE_MEM_NOACCESS(p, size);
}

#elif MI_TRACK_ASAN
#include <sanitizer/asan_interface.h>

void _mi_track_malloc_size_shim(void* p, size_t reqsize, size_t size, bool zero) {
  (void)reqsize;
  (void)zero;
  ASAN_UNPOISON_MEMORY_REGION(p, size);
}

void _mi_track_free_size_shim(void* p, size_t size) {
  ASAN_POISON_MEMORY_REGION(p, size);
}

void _mi_track_resize_shim(void* p, size_t oldsize, size_t newsize) {
  ASAN_POISON_MEMORY_REGION(p, oldsize);
  ASAN_UNPOISON_MEMORY_REGION(p, newsize);
}

void _mi_track_mem_defined_shim(void* p, size_t size) {
  ASAN_UNPOISON_MEMORY_REGION(p, size);
}

void _mi_track_mem_undefined_shim(void* p, size_t size) {
  ASAN_UNPOISON_MEMORY_REGION(p, size);
}

void _mi_track_mem_noaccess_shim(void* p, size_t size) {
  ASAN_POISON_MEMORY_REGION(p, size);
}
#endif
