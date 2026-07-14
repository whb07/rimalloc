#define _GNU_SOURCE

#include <errno.h>
#include <dlfcn.h>
#include <pthread.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

typedef struct {
  size_t rss_current;
  size_t rss_peak;
  size_t commit_current;
  size_t commit_peak;
  size_t page_faults;
} process_snapshot_t;

typedef void (*mi_process_info_fn)(size_t *, size_t *, size_t *, size_t *,
                                   size_t *, size_t *, size_t *, size_t *);
typedef bool (*mi_check_owned_fn)(const void *);

typedef struct {
  pthread_mutex_t mutex;
  pthread_cond_t command_ready;
  pthread_cond_t batch_ready;
  void **pointers;
  size_t count;
  size_t size;
  size_t alignment;
  int stop;
  int command;
  int ready;
} remote_fixture_t;

static size_t env_size(const char *name, size_t fallback) {
  const char *text = getenv(name);
  if (text == NULL || *text == '\0') return fallback;
  errno = 0;
  char *end = NULL;
  unsigned long long value = strtoull(text, &end, 10);
  if (errno != 0 || end == text || *end != '\0' || value > SIZE_MAX) {
    fprintf(stderr, "invalid %s\n", name);
    exit(2);
  }
  return (size_t)value;
}

static process_snapshot_t snapshot(mi_process_info_fn process_info) {
  process_snapshot_t result = {0};
  process_info(NULL, NULL, NULL, &result.rss_current, &result.rss_peak,
               &result.commit_current, &result.commit_peak,
               &result.page_faults);
  return result;
}

static uint64_t monotonic_ns(void) {
  struct timespec time;
  if (clock_gettime(CLOCK_MONOTONIC, &time) != 0) {
    perror("clock_gettime");
    exit(2);
  }
  return (uint64_t)time.tv_sec * UINT64_C(1000000000) + (uint64_t)time.tv_nsec;
}

static void allocate_batch(void **pointers, size_t count, size_t size,
                           size_t alignment) {
  for (size_t index = 0; index < count; index++) {
    if (alignment <= _Alignof(max_align_t)) {
      pointers[index] = malloc(size);
    } else if (posix_memalign(&pointers[index], alignment, size) != 0) {
      pointers[index] = NULL;
    }
    if (pointers[index] == NULL) {
      fputs("probe allocation failed\n", stderr);
      exit(2);
    }
  }
}

static void free_batch(void **pointers, size_t count) {
  for (size_t index = 0; index < count; index++) free(pointers[index]);
}

static void *remote_owner(void *argument) {
  remote_fixture_t *fixture = argument;
  pthread_mutex_lock(&fixture->mutex);
  for (;;) {
    while (!fixture->command && !fixture->stop) {
      pthread_cond_wait(&fixture->command_ready, &fixture->mutex);
    }
    if (fixture->stop) break;
    size_t count = fixture->count;
    pthread_mutex_unlock(&fixture->mutex);
    allocate_batch(fixture->pointers, count, fixture->size,
                   fixture->alignment);
    pthread_mutex_lock(&fixture->mutex);
    fixture->command = 0;
    fixture->ready = 1;
    pthread_cond_signal(&fixture->batch_ready);
  }
  pthread_mutex_unlock(&fixture->mutex);
  return NULL;
}

static size_t run_local(void **pointers, size_t operations, size_t live_set,
                        size_t size, size_t alignment) {
  size_t completed = 0;
  size_t peak = 0;
  while (completed < operations) {
    size_t count = operations - completed;
    if (count > live_set) count = live_set;
    allocate_batch(pointers, count, size, alignment);
    if (count * size > peak) peak = count * size;
    free_batch(pointers, count);
    completed += count;
  }
  return peak;
}

static size_t run_remote(remote_fixture_t *fixture, size_t operations,
                         size_t live_set) {
  size_t completed = 0;
  size_t peak = 0;
  while (completed < operations) {
    size_t count = operations - completed;
    if (count > live_set) count = live_set;
    pthread_mutex_lock(&fixture->mutex);
    fixture->count = count;
    fixture->command = 1;
    pthread_cond_signal(&fixture->command_ready);
    while (!fixture->ready) {
      pthread_cond_wait(&fixture->batch_ready, &fixture->mutex);
    }
    fixture->ready = 0;
    pthread_mutex_unlock(&fixture->mutex);
    if (count * fixture->size > peak) peak = count * fixture->size;
    free_batch(fixture->pointers, count);
    completed += count;
  }
  return peak;
}

int main(void) {
  const char *scenario = getenv("MIMALLOC_PROBE_SCENARIO");
  const char *artifact_mode = getenv("MIMALLOC_PROBE_ARTIFACT_MODE");
  const char *features = getenv("MIMALLOC_PROBE_FEATURES");
  size_t size = env_size("MIMALLOC_PROBE_SIZE", 64);
  size_t alignment = env_size("MIMALLOC_PROBE_ALIGNMENT", 8);
  size_t operations = env_size("MIMALLOC_PROBE_OPERATIONS", 20000000);
  size_t live_set = env_size("MIMALLOC_PROBE_LIVE_SET", 4096);
  size_t threads = env_size("MIMALLOC_PROBE_THREADS", 1);
  if (scenario == NULL || artifact_mode == NULL || features == NULL ||
      operations == 0 || live_set == 0 || size == 0 ||
      alignment == 0 || (alignment & (alignment - 1)) != 0 ||
      size > SIZE_MAX / live_set || strcmp(features, "interpose") != 0) {
    fputs("missing or invalid probe configuration\n", stderr);
    return 2;
  }
  dlerror();
  mi_process_info_fn process_info =
      (mi_process_info_fn)dlsym(RTLD_DEFAULT, "mi_process_info");
  if (dlerror() != NULL || process_info == NULL) {
    fputs("preloaded allocator does not export mi_process_info\n", stderr);
    return 2;
  }
  dlerror();
  mi_check_owned_fn check_owned =
      (mi_check_owned_fn)dlsym(RTLD_DEFAULT, "mi_check_owned");
  if (dlerror() != NULL || check_owned == NULL) {
    fputs("preloaded allocator does not export mi_check_owned\n", stderr);
    return 2;
  }
  void *ownership_probe = NULL;
  allocate_batch(&ownership_probe, 1, size, alignment);
  if (!check_owned(ownership_probe)) {
    fputs("libc allocation did not route through the preloaded allocator\n",
          stderr);
    return 2;
  }
  free_batch(&ownership_probe, 1);
  void **pointers = calloc(live_set, sizeof(*pointers));
  if (pointers == NULL) return 2;

  remote_fixture_t fixture = {
      .mutex = PTHREAD_MUTEX_INITIALIZER,
      .command_ready = PTHREAD_COND_INITIALIZER,
      .batch_ready = PTHREAD_COND_INITIALIZER,
      .pointers = pointers,
      .size = size,
      .alignment = alignment,
  };
  pthread_t owner;
  int remote = strcmp(scenario, "remote_free_1x1") == 0;
  if (remote && pthread_create(&owner, NULL, remote_owner, &fixture) != 0) return 2;

  process_snapshot_t start = snapshot(process_info);
  uint64_t started = monotonic_ns();
  size_t logical_peak;
  if (strcmp(scenario, "alloc_free") == 0) {
    logical_peak = run_local(pointers, operations, live_set, size, alignment);
  } else if (remote) {
    logical_peak = run_remote(&fixture, operations, live_set);
  } else {
    fprintf(stderr, "unsupported probe scenario: %s\n", scenario);
    return 2;
  }
  uint64_t wall_ns = monotonic_ns() - started;
  process_snapshot_t end = snapshot(process_info);

  if (remote) {
    pthread_mutex_lock(&fixture.mutex);
    fixture.stop = 1;
    pthread_cond_signal(&fixture.command_ready);
    pthread_mutex_unlock(&fixture.mutex);
    pthread_join(owner, NULL);
  }
  pthread_cond_destroy(&fixture.batch_ready);
  pthread_cond_destroy(&fixture.command_ready);
  pthread_mutex_destroy(&fixture.mutex);
  free(pointers);

  printf(
      "{\"schema\":1,\"scenario\":\"%s\",\"allocator\":\"preloaded-libc\","
      "\"artifact_mode\":\"%s\",\"features\":[\"%s\"],\"size\":%zu,"
      "\"alignment\":%zu,\"threads\":%zu,\"operations\":%zu,"
      "\"seed\":16045690984503098046,\"wall_ns\":%llu,"
      "\"operations_per_second\":%.6f,\"logical_live_bytes_start\":0,"
      "\"logical_live_bytes_peak\":%zu,\"logical_live_bytes_end\":0,"
      "\"rss_current_start\":%zu,\"rss_current_end\":%zu,\"rss_peak_end\":%zu,"
      "\"commit_current_start\":%zu,\"commit_current_end\":%zu,"
      "\"commit_peak_end\":%zu,\"page_faults_delta\":%zu,"
      "\"allocator_reserved_current_start\":null,"
      "\"allocator_reserved_current_end\":null,\"allocator_reserved_peak_end\":null,"
      "\"allocator_committed_current_start\":null,"
      "\"allocator_committed_current_end\":null,"
      "\"allocator_committed_peak_end\":null,\"allocator_pages_current_end\":null,"
      "\"allocator_pages_abandoned_end\":null,\"exit_status\":0}\n",
      scenario, artifact_mode, features, size, alignment, threads, operations,
      (unsigned long long)wall_ns,
      (double)operations * 1000000000.0 / (double)wall_ns, logical_peak,
      start.rss_current, end.rss_current, end.rss_peak, start.commit_current,
      end.commit_current, end.commit_peak, end.page_faults - start.page_faults);
  return 0;
}
