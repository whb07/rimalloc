#include <assert.h>
#include <dlfcn.h>
#include <errno.h>
#include <mach/mach.h>
#include <malloc/malloc.h>
#include <pthread.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/wait.h>
#include <unistd.h>

typedef bool (*owned_fn)(const void*);
static owned_fn owned;
static void* early;

__attribute__((constructor)) static void before_main(void) {
  owned = (owned_fn)dlsym(RTLD_DEFAULT, "mi_check_owned");
  assert(owned != NULL);
  early = malloc(7);
  assert(early != NULL && owned(early));
  memset(early, 0x5a, 7);
}

static void check(void* p, size_t size, size_t alignment) {
  assert(p != NULL && owned(p));
  assert((uintptr_t)p % alignment == 0);
  assert(malloc_size(p) >= size);
  assert(malloc_zone_from_ptr(p) == malloc_default_zone());
}

static void* worker(void* p) {
  assert(((unsigned char*)p)[0] == 0x5a);
  free(p);
  for (size_t i = 1; i <= 4096; ++i) {
    void* q = malloc(i);
    check(q, i, 16);
    memset(q, 0xa5, i);
    q = realloc(q, i * 2);
    check(q, i * 2, 16);
    assert(((unsigned char*)q)[i - 1] == 0xa5);
    free(q);
  }
  return malloc(257);
}

int main(int argc, char** argv) {
  assert(argc == 2);
  assert(((unsigned char*)early)[6] == 0x5a);
  free(early);
  const size_t sizes[] = {0, 1, 7, 8, 15, 16, 17, 4095, 16384, 1048576};
  for (size_t i = 0; i < sizeof(sizes) / sizeof(sizes[0]); ++i) {
    size_t n = sizes[i];
    void* ps[32];
    for (size_t j = 0; j < 32; ++j) {
      ps[j] = malloc(n);
      check(ps[j], n, 16);
      memset(ps[j], 0xa5, n);
    }
    for (size_t j = 0; j < 32; ++j) free(ps[j]);
    unsigned char* p = calloc(n, 3);
    check(p, n * 3, 16);
    for (size_t j = 0; j < n * 3; ++j) assert(p[j] == 0);
    free(p);
  }
  volatile size_t enormous = SIZE_MAX;
  errno = 0;
  assert(calloc(enormous, 2) == NULL && errno == ENOMEM);
  void* p = malloc(64);
  memset(p, 0x5a, 64);
  errno = 0;
  assert(realloc(p, enormous) == NULL && errno == ENOMEM);
  assert(((unsigned char*)p)[63] == 0x5a);
  p = realloc(p, 1);
  check(p, 1, 16);
  free(p);
  p = malloc(64);
  assert(reallocf(p, enormous) == NULL);
  check(p = realloc(NULL, 31), 31, 16);
  free(p);
  for (size_t alignment = 16; alignment <= 1048576; alignment *= 4) {
    p = NULL;
    assert(posix_memalign(&p, alignment, 67) == 0);
    check(p, 67, alignment);
    free(p);
    p = aligned_alloc(alignment, alignment * 2);
    check(p, alignment * 2, alignment);
    free(p);
  }
  p = (void*)(uintptr_t)16;
  errno = EDOM;
  assert(posix_memalign(&p, 3, 67) == EINVAL);
  assert(p == (void*)(uintptr_t)16 && errno == EDOM);
  volatile size_t invalid_alignment = 3;
  assert(aligned_alloc(invalid_alignment, 96) == NULL && errno == EINVAL);
  p = valloc(123);
  check(p, 123, (size_t)getpagesize());
  errno = EDOM;
  free(p);
  assert(errno == EDOM);
  p = strdup("allocator");
  assert(owned(p) && strcmp(p, "allocator") == 0);
  free(p);
  p = strndup("allocator", 5);
  assert(owned(p) && strcmp(p, "alloc") == 0);
  free(p);

  malloc_zone_t* zone = malloc_default_zone();
  assert(strcmp(malloc_get_zone_name(zone), "mechanical_mimalloc") == 0);
  malloc_set_zone_name(zone, "renamed-mimalloc");
  assert(strcmp(malloc_get_zone_name(zone), "renamed-mimalloc") == 0);
  malloc_set_zone_name(zone, "mechanical_mimalloc");
  assert(zone->version == 10 && zone->reserved1 == NULL && zone->reserved2 == NULL);
  assert(zone->introspect != NULL && zone->introspect->check(zone));
  assert(zone->introspect->good_size(zone, 17) >= 17);
  p = zone->malloc(zone, 37);
  check(p, 37, 16);
  assert(zone->size(zone, p) >= 37 && zone->claimed_address(zone, p));
  zone->free_definite_size(zone, p, zone->size(zone, p));
  p = malloc_zone_calloc(zone, 17, 3);
  check(p, 51, 16);
  p = malloc_zone_realloc(zone, p, 257);
  check(p, 257, 16);
  malloc_zone_free(zone, p);
  p = malloc_zone_memalign(zone, 4096, 91);
  check(p, 91, 4096);
  malloc_zone_free(zone, p);
  p = malloc_zone_valloc(zone, 91);
  check(p, 91, (size_t)getpagesize());
  malloc_zone_free(zone, p);
  void* batch[16];
  assert(malloc_zone_batch_malloc(zone, 31, batch, 16) == 16);
  for (unsigned i = 0; i < 16; ++i) check(batch[i], 31, 16);
  malloc_zone_batch_free(zone, batch, 16);
  malloc_statistics_t stats;
  malloc_zone_statistics(zone, &stats);
  (void)malloc_zone_pressure_relief(zone, 0);
  vm_address_t* zones = NULL;
  unsigned count = 0;
  assert(malloc_get_all_zones(mach_task_self(), NULL, &zones, &count) == KERN_SUCCESS);
  bool found = false;
  for (unsigned i = 0; i < count; ++i) if (zones[i] == (vm_address_t)zone) found = true;
  assert(found);

  // A real system zone must remain usable, including crossing the free/realloc
  // interposition boundary with pointers owned by libmalloc.
  malloc_zone_t* foreign = malloc_create_zone(0, 0);
  assert(foreign != NULL && foreign != zone);
  malloc_set_zone_name(foreign, "foreign-test-zone");
  p = foreign->malloc(foreign, 73);
  assert(p && !owned(p));
  assert(malloc_zone_from_ptr(p) == foreign);
  assert(malloc_size(p) >= 73);
  memset(p, 0x5a, 73);
  p = realloc(p, 151);
  assert(p && !owned(p) && ((unsigned char*)p)[72] == 0x5a);
  free(p);
  p = foreign->malloc(foreign, 23);
  malloc_zone_free(zone, p);
  malloc_destroy_zone(foreign);

  // New SDKs route typed allocations through libmalloc's interposition compatibility path.
  void* (*typed_malloc)(size_t, uint64_t) = dlsym(RTLD_DEFAULT, "malloc_type_malloc");
  if (typed_malloc) {
    p = typed_malloc(73, 0);
    check(p, 73, 16);
    free(p);
  }

  void* plugin = dlopen(argv[1], RTLD_NOW | RTLD_LOCAL);
  assert(plugin != NULL);
  void* (*plugin_alloc)(size_t) = dlsym(plugin, "plugin_alloc");
  void (*plugin_free)(void*) = dlsym(plugin, "plugin_free");
  assert(plugin_alloc && plugin_free);
  p = plugin_alloc(111);
  check(p, 111, 16);
  free(p);
  plugin_free(malloc(91));
  p = plugin_alloc(99);
  assert(dlclose(plugin) == 0);
  check(p, 99, 16);
  free(p);

  pthread_t threads[8];
  for (unsigned i = 0; i < 8; ++i) {
    p = malloc(16);
    memset(p, 0x5a, 16);
    assert(pthread_create(&threads[i], NULL, worker, p) == 0);
  }
  for (unsigned i = 0; i < 8; ++i) {
    assert(pthread_join(threads[i], &p) == 0);
    check(p, 257, 16);
    free(p);
  }
  pid_t child = fork();
  assert(child >= 0);
  if (child == 0) {
    p = malloc(73);
    check(p, 73, 16);
    free(p);
    _exit(0);
  }
  int status;
  assert(waitpid(child, &status, 0) == child);
  assert(WIFEXITED(status) && WEXITSTATUS(status) == 0);
  puts("macOS C interposition: allocations, zones, foreign pointers, threads, and fork passed");
}
