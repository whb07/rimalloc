#define _GNU_SOURCE
#include <mimalloc.h>
#include <mimalloc/prim.h>

#include <pthread.h>
#include <signal.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/resource.h>
#include <sys/wait.h>
#include <unistd.h>

static bool permissions(void* address, size_t size, char result[5]) {
  FILE* maps = fopen("/proc/self/maps", "r");
  if (maps == NULL) return false;
  char line[512], mode[5];
  unsigned long begin, end;
  bool found = false;
  while (fgets(line, sizeof(line), maps) != NULL) {
    if (sscanf(line, "%lx-%lx %4s", &begin, &end, mode) == 3 &&
        begin <= (uintptr_t)address && (uintptr_t)address + size <= end) {
      memcpy(result, mode, 5);
      found = true;
      break;
    }
  }
  fclose(maps);
  return found;
}

static void* allocation_reuse(void* unused) {
  (void)unused;
  void* blocks[128];
  const size_t size = 32768;
  mi_thread_init();
  for (unsigned cycle = 0; cycle < 3; ++cycle) {
    for (size_t i = 0; i < 128; ++i) {
      blocks[i] = mi_malloc(size);
      if (blocks[i] == NULL) return (void*)(uintptr_t)1;
      memset(blocks[i], (unsigned char)(i + cycle), size);
    }
    for (size_t i = 0; i < 128; ++i) {
      const unsigned char* bytes = blocks[i];
      for (size_t j = 0; j < size; ++j) {
        if (bytes[j] != (unsigned char)(i + cycle)) return (void*)(uintptr_t)1;
      }
      mi_free(blocks[i]);
    }
    mi_collect(true);
  }
  mi_thread_done();
  return NULL;
}

int main(int argc, char** argv) {
  if (argc != 2 || (strcmp(argv[1], "0") != 0 && strcmp(argv[1], "1") != 0)) return 2;
  const bool expected = argv[1][0] == '1';
  struct rlimit core = {0, 0};
  if (setrlimit(RLIMIT_CORE, &core) != 0) return 3;
  const long page = sysconf(_SC_PAGESIZE);
  if (page <= 0) return 4;
  const size_t size = 4 * (size_t)page;
  mi_process_init();
  bool large = false, zero = false, needs_recommit = false;
  void* mapping = NULL;
  if (_mi_prim_alloc(NULL, size, (size_t)page, true, false, &large, &zero, &mapping) != 0 ||
      mapping == NULL || large) return 5;
  volatile unsigned char* bytes = mapping;
  for (size_t i = 0; i < size; ++i) bytes[i] = 0x5a;
  if (_mi_prim_decommit(mapping, size, &needs_recommit) != 0) return 6;
  char decommitted[5], recommitted[5];
  if (!permissions(mapping, size, decommitted)) return 7;

  // This child probes protection on an owned primitive mapping. It does not
  // dereference a freed application allocation, and cannot corrupt the parent.
  pid_t child = fork();
  if (child < 0) return 8;
  if (child == 0) {
    bytes[0] = 0xa5;
    bytes[size - 1] = 0xa5;
    _exit(0);
  }
  int status = 0;
  if (waitpid(child, &status, 0) != child) return 9;
  const bool protected_access = WIFSIGNALED(status) && WTERMSIG(status) == SIGSEGV;
  const bool writable_access = WIFEXITED(status) && WEXITSTATUS(status) == 0;

  if (_mi_prim_commit(mapping, size, &zero) != 0 || !permissions(mapping, size, recommitted)) return 10;
  bool discarded = true;
  for (size_t i = 0; i < size; ++i) {
    if (bytes[i] != 0) discarded = false;
    bytes[i] = 0x3c;
  }
  const bool restored = recommitted[0] == 'r' && recommitted[1] == 'w';
  if (_mi_prim_free(mapping, size) != 0) return 11;
  pthread_t worker;
  void* outcome = NULL;
  if (pthread_create(&worker, NULL, allocation_reuse, NULL) != 0 ||
      pthread_join(worker, &outcome) != 0) return 12;
  const bool reused = outcome == NULL;
  printf("expected=%d needs_recommit=%d permissions=%s child_segv=%d recommit_rw=%d zero=%d reuse=%d\n",
         expected, needs_recommit, decommitted, protected_access, restored, discarded, reused);
  const bool protection_matches = expected ?
      strcmp(decommitted, "---p") == 0 && protected_access :
      decommitted[0] == 'r' && decommitted[1] == 'w' && writable_access;
  return needs_recommit == expected && protection_matches && restored && discarded && reused ? 0 : 1;
}
