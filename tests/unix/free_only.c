#include <mimalloc.h>
#include <mimalloc/types.h>

#include <dlfcn.h>
#include <pthread.h>
#include <stdio.h>
#include <string.h>

static void* (*allocate)(size_t);
static void (*release)(void*);
static void (*collect)(bool);
static mi_theap_t* (*get_default)(void);
static void* blocks[4096];
static size_t block_count = 4096;
static size_t block_size = 1024;

static void* free_only(void* unused) {
  (void)unused;
  for (size_t i = 0; i < block_count; ++i) {
    const unsigned char* bytes = blocks[i];
    for (size_t j = 0; j < block_size; ++j) {
      if (bytes[j] != (unsigned char)i) return (void*)1;
    }
    release(blocks[i]);
  }
  return NULL;
}

static bool consume(void) {
  pthread_t consumer;
  void* result = NULL;
  return pthread_create(&consumer, NULL, free_only, NULL) == 0 &&
         pthread_join(consumer, &result) == 0 && result == NULL;
}

int main(int argc, char** argv) {
  if (argc != 2 && (argc != 3 || strcmp(argv[2], "normal") != 0)) return 2;
  const bool normal = argc == 3;
  // Keep the harness on libc, including thread creation. The explicit APIs
  // exercise either the portable C ABI or the native interpose assembly.
  void* library = dlopen(argv[1], RTLD_NOW | RTLD_LOCAL);
  if (library == NULL) return 3;
  allocate = (void* (*)(size_t))dlsym(library, "mi_malloc");
  release = (void (*)(void*))dlsym(library, "mi_free");
  collect = (void (*)(bool))dlsym(library, "mi_collect");
  get_default = (mi_theap_t* (*)(void))dlsym(library, "mi_theap_get_default");
  if (!allocate || !release || !collect || !get_default) return 4;
  mi_theap_t* owner = get_default();
  if (normal) {
    // This non-full page takes the native remote CAS path. The owner must
    // recognize participation when it later collects the published list.
    block_count = 1;
    block_size = 64;
    blocks[0] = allocate(64);
    if (blocks[0] == NULL) return 5;
    memset(blocks[0], 0, 64);
    if (!consume()) return 6;
    for (size_t i = 0; i < 4096; ++i) {
      blocks[i] = allocate(64);
      if (blocks[i] == NULL) return 5;
    }
    for (size_t i = 0; i < 4096; ++i) release(blocks[i]);
    block_count = 4096;
    block_size = 1024;
  }
  for (size_t i = 0; i < 4096; ++i) {
    blocks[i] = allocate(1024);
    if (blocks[i] == NULL) return 5;
    memset(blocks[i], (unsigned char)i, 1024);
  }
  const bool normal_recognized = !normal || owner->pages[MI_BIN_FULL].first == NULL;
  if (!consume()) return 6;

  // Ordinary generic allocations must eventually drain the retained full
  // queue. The consumer never initializes a mimalloc TLD or calls collect.
  for (size_t i = 0; i < 2000; ++i) {
    void* p = allocate(2048);
    if (p == NULL) return 7;
    release(p);
  }
  const bool drained = owner->pages[MI_BIN_FULL].first == NULL;
  printf("free_only_owner_full_queue_drained=%d normal_remote_recognized=%d\n",
         drained, normal_recognized);
  collect(true);
  return drained && normal_recognized ? 0 : 1;
}
