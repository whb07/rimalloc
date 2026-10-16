// Darwin's malloc zone ABI is owned by the SDK. Keep its layout and callback
// signatures in C; allocation and pointer ownership are implemented in Rust.
#include <Availability.h>
#include <errno.h>
#include <malloc/malloc.h>
#include <mach/mach.h>
#include <stdbool.h>
#include <stdlib.h>
#include <string.h>
#include "mimalloc.h"

// Defined in src/platform/macos.rs; each classifies `p` with one page-map lookup.
bool _mi_macos_free(void* p);
size_t _mi_macos_size(const void* p);

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

// Darwin malloc returns 16-byte aligned blocks. On 64-bit targets every
// mimalloc size class of at least 16 bytes is a multiple of 16, so raising
// smaller requests to 16 keeps the plain allocation fast path aligned.
_Static_assert(sizeof(void*) == 8, "the 16-byte request floor assumes 64-bit size classes");
static size_t darwin_request(size_t size) {
  return size < 16 ? 16 : size;
}

static void* mac_malloc(size_t size) {
  return allocation_result(mi_malloc(darwin_request(size)));
}

static void* mac_calloc(size_t count, size_t size) {
  size_t total;
  if (__builtin_mul_overflow(count, size, &total)) {
    errno = ENOMEM;
    return NULL;
  }
  return allocation_result(mi_zalloc(darwin_request(total)));
}

// dyld does not interpose references originating in this image. These calls
// reach libmalloc, so allocations made before takeover or by another zone keep
// their original owner. Do not export malloc/free aliases from this image.
static void free_foreign(void* p) {
  int saved_errno = errno;
  free(p);
  errno = saved_errno;
}

// The allocator's own OS calls leave errno unchanged, so only the libmalloc
// path needs to preserve it.
static void mac_free(void* p) {
  if (p != NULL && !_mi_macos_free(p)) free_foreign(p);
}

static void* mac_realloc(void* p, size_t size) {
  if (p == NULL) return mac_malloc(size);
  if (!mi_is_in_heap_region(p)) return realloc(p, size);
  return allocation_result(mi_realloc(p, darwin_request(size)));
}

static void* mac_reallocf(void* p, size_t size) {
  if (p == NULL) return mac_malloc(size);
  if (!mi_is_in_heap_region(p)) return reallocf(p, size);
  void* q = mi_realloc(p, darwin_request(size));
  if (q == NULL) {
    mi_free(p);
    errno = ENOMEM;
  }
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

static char* copy_string(const char* s, size_t len) {
  char* p = mac_malloc(len + 1);
  if (p != NULL) {
    memcpy(p, s, len);
    p[len] = '\0';
  }
  return p;
}

static char* mac_strdup(const char* s) { return copy_string(s, strlen(s)); }
static char* mac_strndup(const char* s, size_t size) {
  return copy_string(s, strnlen(s, size));
}

static size_t mac_size(const void* p) {
  if (p == NULL) return 0;
  size_t size = _mi_macos_size(p);
  return size != 0 ? size : malloc_size(p);
}

static size_t mac_good_size(size_t size) {
  return mi_good_size(darwin_request(size));
}

static malloc_zone_t* mac_default_zone(void) {
  return &mi_zone;
}

static malloc_zone_t* mac_zone_from_ptr(const void* p) {
  return _mi_macos_size(p) != 0 ? &mi_zone : malloc_zone_from_ptr(p);
}

static size_t zone_size(malloc_zone_t* zone, const void* p) {
  (void)zone;
  return _mi_macos_size(p);
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

static malloc_zone_t* first_zone(void) {
  vm_address_t* zones = NULL;
  unsigned count = 0;
  if (malloc_get_all_zones(mach_task_self(), NULL, &zones, &count) != KERN_SUCCESS || count == 0) {
    return NULL;
  }
  return (malloc_zone_t*)zones[0];
}

// libmalloc sends malloc(), typed allocations, and the default zone to the
// first registered zone. Making this zone the first one also covers programs
// that link the allocator statically, where dyld ignores __interpose tuples.
__attribute__((constructor(101))) static void mac_register_zone(void) {
  mi_process_init();
  malloc_zone_t* purgeable = malloc_default_purgeable_zone();
  malloc_zone_register(&mi_zone);
  // Unregistering a zone moves the last registered zone, this one, into its
  // slot. Re-register the system zone, then the purgeable zone, so the
  // purgeable zone stays behind the system zone as libmalloc expects.
  malloc_zone_t* system = first_zone();
  if (system != NULL && system != &mi_zone) {
    malloc_zone_unregister(system);
    malloc_zone_register(system);
    if (purgeable != NULL) {
      malloc_zone_unregister(purgeable);
      malloc_zone_register(purgeable);
    }
  }
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
#if __MAC_OS_X_VERSION_MIN_REQUIRED >= 101500
  // Older deployment targets cannot reference aligned_alloc; libmalloc's
  // implementation still reaches this zone through its memalign callback.
  INTERPOSE(aligned_alloc, mac_aligned_alloc),
#endif
  INTERPOSE(strdup, mac_strdup),
  INTERPOSE(strndup, mac_strndup),
  INTERPOSE(malloc_size, mac_size),
  INTERPOSE(malloc_good_size, mac_good_size),
  INTERPOSE(malloc_default_zone, mac_default_zone),
  INTERPOSE(malloc_zone_from_ptr, mac_zone_from_ptr),
};
