#define _GNU_SOURCE

#include <dlfcn.h>
#include <errno.h>
#include <fcntl.h>
#include <stdatomic.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <unistd.h>

/* Exact requested lengths through 64 KiB. Larger requests use power-of-two bins. */
#define EXACT_MAX (64U * 1024U)
#define LARGE_BINS 64U
#define ALIGNMENTS 64U

typedef void *(*copy_fn)(void *, const void *, size_t);
typedef void *(*checked_copy_fn)(void *, const void *, size_t, size_t);

static copy_fn real_memcpy;
static copy_fn real_memmove;
static checked_copy_fn real_memcpy_chk;
static checked_copy_fn real_memmove_chk;
static _Atomic uint64_t memcpy_exact[EXACT_MAX + 1U];
static _Atomic uint64_t memmove_exact[EXACT_MAX + 1U];
static _Atomic uint64_t memcpy_large[LARGE_BINS];
static _Atomic uint64_t memmove_large[LARGE_BINS];
static _Atomic uint64_t memcpy_align[ALIGNMENTS][ALIGNMENTS];
static _Atomic uint64_t memmove_align[ALIGNMENTS][ALIGNMENTS];
/* non-overlap, destination below source, destination above source, exact alias */
static _Atomic uint64_t memmove_overlap[4];
static _Atomic uint64_t memcpy_total;
static _Atomic uint64_t memmove_total;
static _Thread_local int tracing_disabled;

static unsigned large_bin(size_t size) {
  unsigned bin = 0;
  while (size > 1U && bin + 1U < LARGE_BINS) {
    size >>= 1U;
    ++bin;
  }
  return bin;
}

static void record_size(_Atomic uint64_t *exact, _Atomic uint64_t *large,
                        size_t size) {
  if (size <= EXACT_MAX) {
    atomic_fetch_add_explicit(&exact[size], 1U, memory_order_relaxed);
  } else {
    atomic_fetch_add_explicit(&large[large_bin(size)], 1U,
                              memory_order_relaxed);
  }
}

static void *fallback_memcpy(void *destination, const void *source, size_t size) {
  volatile unsigned char *dst = (volatile unsigned char *)destination;
  const volatile unsigned char *src =
      (const volatile unsigned char *)source;
  for (size_t index = 0; index < size; ++index) {
    dst[index] = src[index];
  }
  return destination;
}

static void *fallback_memmove(void *destination, const void *source,
                              size_t size) {
  volatile unsigned char *dst = (volatile unsigned char *)destination;
  const volatile unsigned char *src =
      (const volatile unsigned char *)source;
  if ((uintptr_t)dst <= (uintptr_t)src) {
    for (size_t index = 0; index < size; ++index) {
      dst[index] = src[index];
    }
  } else {
    for (size_t index = size; index != 0; --index) {
      dst[index - 1U] = src[index - 1U];
    }
  }
  return destination;
}

static void resolve_symbols(void) {
  if (real_memcpy != NULL && real_memmove != NULL) {
    return;
  }
  tracing_disabled = 1;
  real_memcpy = (copy_fn)dlsym(RTLD_NEXT, "memcpy");
  real_memmove = (copy_fn)dlsym(RTLD_NEXT, "memmove");
  real_memcpy_chk = (checked_copy_fn)dlsym(RTLD_NEXT, "__memcpy_chk");
  real_memmove_chk = (checked_copy_fn)dlsym(RTLD_NEXT, "__memmove_chk");
  tracing_disabled = 0;
}

static void initialize(void) __attribute__((constructor));
static void initialize(void) { resolve_symbols(); }

static void record_memcpy(void *destination, const void *source, size_t size) {
  if (tracing_disabled) {
    return;
  }
  record_size(memcpy_exact, memcpy_large, size);
  atomic_fetch_add_explicit(
      &memcpy_align[(uintptr_t)destination & 63U][(uintptr_t)source & 63U], 1U,
      memory_order_relaxed);
  atomic_fetch_add_explicit(&memcpy_total, 1U, memory_order_relaxed);
}

static void record_memmove(void *destination, const void *source, size_t size) {
  if (tracing_disabled) {
    return;
  }
  const uintptr_t dst = (uintptr_t)destination;
  const uintptr_t src = (uintptr_t)source;
  unsigned overlap = 0;
  if (dst == src) {
    overlap = 3;
  } else if (dst < src && size > src - dst) {
    overlap = 1;
  } else if (src < dst && size > dst - src) {
    overlap = 2;
  }
  record_size(memmove_exact, memmove_large, size);
  atomic_fetch_add_explicit(&memmove_align[dst & 63U][src & 63U], 1U,
                            memory_order_relaxed);
  atomic_fetch_add_explicit(&memmove_overlap[overlap], 1U,
                            memory_order_relaxed);
  atomic_fetch_add_explicit(&memmove_total, 1U, memory_order_relaxed);
}

void *memcpy(void *destination, const void *source, size_t size) {
  if (real_memcpy == NULL) {
    if (tracing_disabled) {
      return fallback_memcpy(destination, source, size);
    }
    resolve_symbols();
  }
  record_memcpy(destination, source, size);
  return real_memcpy != NULL ? real_memcpy(destination, source, size)
                             : fallback_memcpy(destination, source, size);
}

void *memmove(void *destination, const void *source, size_t size) {
  if (real_memmove == NULL) {
    if (tracing_disabled) {
      return fallback_memmove(destination, source, size);
    }
    resolve_symbols();
  }
  record_memmove(destination, source, size);
  return real_memmove != NULL ? real_memmove(destination, source, size)
                              : fallback_memmove(destination, source, size);
}

void *__memcpy_chk(void *destination, const void *source, size_t size,
                   size_t destination_size) {
  if (real_memcpy_chk == NULL && !tracing_disabled) {
    resolve_symbols();
  }
  record_memcpy(destination, source, size);
  if (real_memcpy_chk != NULL) {
    return real_memcpy_chk(destination, source, size, destination_size);
  }
  if (size > destination_size) {
    abort();
  }
  return fallback_memcpy(destination, source, size);
}

void *__memmove_chk(void *destination, const void *source, size_t size,
                    size_t destination_size) {
  if (real_memmove_chk == NULL && !tracing_disabled) {
    resolve_symbols();
  }
  record_memmove(destination, source, size);
  if (real_memmove_chk != NULL) {
    return real_memmove_chk(destination, source, size, destination_size);
  }
  if (size > destination_size) {
    abort();
  }
  return fallback_memmove(destination, source, size);
}

static void write_counts(FILE *output, const char *operation,
                         _Atomic uint64_t *exact, _Atomic uint64_t *large) {
  for (size_t size = 0; size <= EXACT_MAX; ++size) {
    const uint64_t calls =
        atomic_load_explicit(&exact[size], memory_order_relaxed);
    if (calls != 0) {
      fprintf(output, "size\t%s\texact\t%zu\t%zu\t%llu\n", operation, size,
              size, (unsigned long long)calls);
    }
  }
  for (unsigned bin = 0; bin < LARGE_BINS; ++bin) {
    const uint64_t calls =
        atomic_load_explicit(&large[bin], memory_order_relaxed);
    if (calls != 0) {
      const uint64_t lower = UINT64_C(1) << bin;
      const uint64_t upper = bin == 63U ? UINT64_MAX : (lower << 1U) - 1U;
      fprintf(output, "size\t%s\trange\t%llu\t%llu\t%llu\n", operation,
              (unsigned long long)lower, (unsigned long long)upper,
              (unsigned long long)calls);
    }
  }
}

static void write_alignments(FILE *output, const char *operation,
                             _Atomic uint64_t counts[ALIGNMENTS][ALIGNMENTS]) {
  for (unsigned dst = 0; dst < ALIGNMENTS; ++dst) {
    for (unsigned src = 0; src < ALIGNMENTS; ++src) {
      const uint64_t calls =
          atomic_load_explicit(&counts[dst][src], memory_order_relaxed);
      if (calls != 0) {
        fprintf(output, "alignment\t%s\t%u\t%u\t%llu\n", operation, dst,
                src, (unsigned long long)calls);
      }
    }
  }
}

static void write_results(void) __attribute__((destructor));
static void write_results(void) {
  const uint64_t memcpy_calls =
      atomic_load_explicit(&memcpy_total, memory_order_relaxed);
  const uint64_t memmove_calls =
      atomic_load_explicit(&memmove_total, memory_order_relaxed);
  if (memcpy_calls == 0 && memmove_calls == 0) {
    return;
  }

  tracing_disabled = 1;
  const char *directory = getenv("COPYTRACE_OUTPUT_DIR");
  if (directory == NULL || directory[0] == '\0') {
    directory = ".";
  }
  (void)mkdir(directory, 0755);

  char path[4096];
  const int length = snprintf(path, sizeof(path), "%s/copytrace-%ld.tsv", directory,
                              (long)getpid());
  if (length <= 0 || (size_t)length >= sizeof(path)) {
    return;
  }
  FILE *output = fopen(path, "w");
  if (output == NULL) {
    return;
  }

  fprintf(output, "meta\tpid\t%ld\n", (long)getpid());
  fprintf(output, "meta\tmemcpy_calls\t%llu\n",
          (unsigned long long)memcpy_calls);
  fprintf(output, "meta\tmemmove_calls\t%llu\n",
          (unsigned long long)memmove_calls);
  write_counts(output, "memcpy", memcpy_exact, memcpy_large);
  write_counts(output, "memmove", memmove_exact, memmove_large);
  write_alignments(output, "memcpy", memcpy_align);
  write_alignments(output, "memmove", memmove_align);
  static const char *const overlap_names[] = {
      "non-overlap", "destination-below", "destination-above", "exact-alias"};
  for (unsigned index = 0; index < 4; ++index) {
    const uint64_t calls =
        atomic_load_explicit(&memmove_overlap[index], memory_order_relaxed);
    if (calls != 0) {
      fprintf(output, "overlap\tmemmove\t%s\t%llu\n", overlap_names[index],
              (unsigned long long)calls);
    }
  }
  fclose(output);
}
