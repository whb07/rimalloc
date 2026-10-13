// Loaded into each application to reject measurements using the wrong malloc.
#define _GNU_SOURCE
#include <dlfcn.h>
#include <fcntl.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

__attribute__((constructor)) static void check_allocator(void) {
  const char* expected = getenv("BENCH_ALLOCATOR_EXPECTED");
  const char* marker = getenv("BENCH_ALLOCATOR_CHECK");
  Dl_info info;
  char wanted[4096], actual[4096];
  void* malloc_symbol = dlsym(RTLD_DEFAULT, "malloc");
  bool (*owned)(const void*) = (bool (*)(const void*))dlsym(RTLD_DEFAULT, "mi_check_owned");
  void* (*allocate)(size_t) = (void* (*)(size_t))malloc_symbol;
  void (*release)(void*) = (void (*)(void*))dlsym(RTLD_DEFAULT, "free");
  // "glibc" expects malloc from libc itself; other names are the preloaded library's path.
  bool system = expected && strcmp(expected, "glibc") == 0;
  if (!expected || !marker || !allocate || !release || !dladdr(malloc_symbol, &info) ||
      !realpath(info.dli_fname, actual) ||
      (system ? strstr(actual, "/libc.so") == NULL
              : !realpath(expected, wanted) || strcmp(wanted, actual) != 0)) {
    fputs("benchmark allocator check: wrong malloc library\n", stderr);
    _exit(121);
  }
  // Ownership is only provable for mimalloc builds; other allocators pass on the owner check.
  void* p = allocate(64);
  if (!p || (owned && !owned(p))) {
    fputs("benchmark allocator check: malloc block not owned\n", stderr);
    _exit(121);
  }
  release(p);
  int fd = open(marker, O_WRONLY | O_CREAT | O_APPEND, 0600);
  char line[64];
  int len = snprintf(line, sizeof(line), "%ld\n", (long)getpid());
  if (fd < 0 || write(fd, line, (size_t)len) != len) _exit(122);
  close(fd);
}
