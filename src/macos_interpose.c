// Darwin's malloc zone ABI is owned by the SDK. Keep its layout and callback
// signatures in C; all allocation and ownership decisions remain in Rust.
#include <errno.h>
#include <malloc/malloc.h>
#include <mach/mach.h>
#include <stdlib.h>
#include <string.h>
#include "mimalloc.h"

// libmalloc protects the zone's VM pages when changing its name. Isolate the
// table so those protection changes cannot cover other allocator globals.
union mac_zone_storage {
  malloc_zone_t zone;
  unsigned char page[PAGE_MAX_SIZE];
};
static union mac_zone_storage mi_zone_storage __attribute__((aligned(PAGE_MAX_SIZE)));
#define mi_zone (mi_zone_storage.zone)

static void* allocation_result(void* p) {
  if (p == NULL) errno = ENOMEM;
  return p;
}

static void* mac_malloc(size_t size) {
  return allocation_result(mi_malloc_aligned(size, 16));
}

static void* mac_calloc(size_t count, size_t size) {
  return allocation_result(mi_calloc_aligned(count, size, 16));
}

// dyld does not interpose references originating in this image. These calls
// reach libmalloc, so allocations made before takeover or by another zone keep
// their original owner. Do not export malloc/free aliases from this image.
static void mac_free(void* p) {
  if (p == NULL) return;
  int saved_errno = errno;
  if (mi_check_owned(p)) mi_free(p);
  else free(p);
  errno = saved_errno;
}

static void* mac_realloc(void* p, size_t size) {
  if (p == NULL) return mac_malloc(size);
  if (!mi_check_owned(p)) return realloc(p, size);
  return allocation_result(mi_realloc_aligned(p, size == 0 ? 1 : size, 16));
}

static void* mac_reallocf(void* p, size_t size) {
  if (p != NULL && !mi_check_owned(p)) return reallocf(p, size);
  void* q = mac_realloc(p, size);
  if (q == NULL) mac_free(p);
  return q;
}

static void* mac_valloc(size_t size) {
  return allocation_result(mi_valloc(size));
}

static int mac_posix_memalign(void** p, size_t alignment, size_t size) {
  int saved_errno = errno;
  int result = mi_posix_memalign(p, alignment, size);
  errno = saved_errno;
  return result;
}

static void* mac_aligned_alloc(size_t alignment, size_t size) {
  if (alignment == 0 || (alignment & (alignment - 1)) != 0) {
    errno = EINVAL;
    return NULL;
  }
  return allocation_result(mi_malloc_aligned(size, alignment));
}

static char* mac_strdup(const char* s) { return allocation_result(mi_strdup(s)); }
static char* mac_strndup(const char* s, size_t size) {
  return allocation_result(mi_strndup(s, size));
}

static size_t mac_size(const void* p) {
  if (p == NULL) return 0;
  return mi_check_owned(p) ? mi_usable_size(p) : malloc_size(p);
}

static size_t mac_good_size(size_t size) {
  return mi_good_size(size < 16 ? 16 : size);
}

static malloc_zone_t* mac_default_zone(void) {
  return &mi_zone;
}

static malloc_zone_t* mac_zone_from_ptr(const void* p) {
  return mi_check_owned(p) ? &mi_zone : malloc_zone_from_ptr(p);
}

static size_t zone_size(malloc_zone_t* zone, const void* p) {
  (void)zone;
  return mi_check_owned(p) ? mi_usable_size(p) : 0;
}

static void* zone_malloc(malloc_zone_t* zone, size_t size) {
  (void)zone;
  return mac_malloc(size);
}

static void* zone_calloc(malloc_zone_t* zone, size_t count, size_t size) {
  (void)zone;
  return mac_calloc(count, size);
}

static void* zone_valloc(malloc_zone_t* zone, size_t size) {
  (void)zone;
  return mac_valloc(size);
}

static void zone_free(malloc_zone_t* zone, void* p) {
  (void)zone;
  mac_free(p);
}

static void* zone_realloc(malloc_zone_t* zone, void* p, size_t size) {
  (void)zone;
  return mac_realloc(p, size);
}

static void zone_destroy(malloc_zone_t* zone) {
  (void)zone; // The process-wide zone has static lifetime.
}

static void* zone_memalign(malloc_zone_t* zone, size_t alignment, size_t size) {
  (void)zone;
  return mac_aligned_alloc(alignment, size);
}

static unsigned zone_batch_malloc(malloc_zone_t* zone, size_t size, void** results, unsigned count) {
  unsigned i = 0;
  for (; i < count; ++i) {
    results[i] = zone_malloc(zone, size);
    if (results[i] == NULL) break;
  }
  return i;
}

static void zone_batch_free(malloc_zone_t* zone, void** pointers, unsigned count) {
  for (unsigned i = 0; i < count; ++i) {
    zone_free(zone, pointers[i]);
    pointers[i] = NULL;
  }
}

static void zone_free_definite_size(malloc_zone_t* zone, void* p, size_t size) {
  (void)size;
  zone_free(zone, p);
}

static size_t zone_pressure_relief(malloc_zone_t* zone, size_t goal) {
  (void)zone; (void)goal;
  mi_collect(true);
  return 0;
}

static boolean_t zone_claimed_address(malloc_zone_t* zone, void* p) {
  (void)zone;
  return mi_is_in_heap_region(p);
}

static kern_return_t intro_enumerator(task_t task, void* context, unsigned mask,
    vm_address_t address, memory_reader_t reader, vm_range_recorder_t recorder) {
  (void)task; (void)context; (void)mask; (void)address; (void)reader; (void)recorder;
  // Remote heap enumeration is not implemented; do not report an empty heap as success.
  return KERN_NOT_SUPPORTED;
}

static size_t intro_good_size(malloc_zone_t* zone, size_t size) {
  (void)zone;
  return mac_good_size(size);
}

static boolean_t intro_check(malloc_zone_t* zone) { (void)zone; return true; }
static void intro_print(malloc_zone_t* zone, boolean_t verbose) {
  (void)zone; (void)verbose;
  mi_stats_print(NULL);
}
static void intro_log(malloc_zone_t* zone, void* p) { (void)zone; (void)p; }
static void intro_lock(malloc_zone_t* zone) { (void)zone; }
static boolean_t intro_locked(malloc_zone_t* zone) { (void)zone; return false; }
static void intro_statistics(malloc_zone_t* zone, malloc_statistics_t* stats) {
  (void)zone;
  memset(stats, 0, sizeof(*stats)); // Per-zone introspection statistics are unavailable.
}

static malloc_introspection_t mi_introspection = {
  .enumerator = intro_enumerator,
  .good_size = intro_good_size,
  .check = intro_check,
  .print = intro_print,
  .log = intro_log,
  .force_lock = intro_lock,
  .force_unlock = intro_lock,
  .statistics = intro_statistics,
  .zone_locked = intro_locked,
  .reinit_lock = intro_lock,
};

static union mac_zone_storage mi_zone_storage = { .zone = {
  .size = zone_size,
  .malloc = zone_malloc,
  .calloc = zone_calloc,
  .valloc = zone_valloc,
  .free = zone_free,
  .realloc = zone_realloc,
  .destroy = zone_destroy,
  .zone_name = "mechanical_mimalloc",
  .batch_malloc = zone_batch_malloc,
  .batch_free = zone_batch_free,
  .introspect = &mi_introspection,
  .version = 10,
  .memalign = zone_memalign,
  .free_definite_size = zone_free_definite_size,
  .pressure_relief = zone_pressure_relief,
  .claimed_address = zone_claimed_address,
} };

__attribute__((constructor)) static void mac_register_zone(void) {
  mi_process_init();
  malloc_zone_register(&mi_zone);
}

#define INTERPOSE(original, replacement_fn) \
  { (const void*)(replacement_fn), (const void*)(original) }

__attribute__((used, section("__DATA,__interpose")))
static const struct { const void* replacement; const void* original; } interposes[] = {
  INTERPOSE(malloc, mac_malloc),
  INTERPOSE(calloc, mac_calloc),
  INTERPOSE(realloc, mac_realloc),
  INTERPOSE(reallocf, mac_reallocf),
  INTERPOSE(free, mac_free),
  INTERPOSE(valloc, mac_valloc),
  INTERPOSE(posix_memalign, mac_posix_memalign),
  INTERPOSE(aligned_alloc, mac_aligned_alloc),
  INTERPOSE(strdup, mac_strdup),
  INTERPOSE(strndup, mac_strndup),
  INTERPOSE(malloc_size, mac_size),
  INTERPOSE(malloc_good_size, mac_good_size),
  INTERPOSE(malloc_default_zone, mac_default_zone),
  INTERPOSE(malloc_zone_from_ptr, mac_zone_from_ptr),
};
