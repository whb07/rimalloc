// Linked only into diagnostic Nginx via --wrap. Counts Nginx executable libc
// allocation sites, not hidden allocations inside libc or external libraries.
#define _GNU_SOURCE
#include <dlfcn.h>
#include <fcntl.h>
#include <malloc.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/types.h>
#include <unistd.h>

struct counters {
    uint64_t malloc_calls, calloc_calls, realloc_calls, free_calls, aligned_calls;
    uint64_t requested_bytes, usable_bytes, live_usable_bytes, peak_live_usable_bytes;
    uint64_t sizes[32];
};
static struct counters* stats;
static pid_t owner;
static int enabled = -1;
static struct counters* counter(void) {
    if (enabled < 0) enabled = getenv("NGINX_BENCH_TRACE") != NULL;
    if (!enabled) return NULL;
    pid_t pid = getpid();
    if (owner == pid) return stats;
    struct counters inherited = {0};
    if (stats) { inherited = *stats; munmap(stats, sizeof(*stats)); }
    const char* dir = getenv("NGINX_BENCH_TRACE");
    char path[4096];
    int n = snprintf(path, sizeof(path), "%s/%ld.bin", dir, (long)pid);
    if (n < 0 || (size_t)n >= sizeof(path)) _exit(122);
    int fd = open(path, O_RDWR | O_CREAT | O_TRUNC, 0600);
    if (fd < 0 || ftruncate(fd, sizeof(*stats))) _exit(122);
    stats = mmap(NULL, sizeof(*stats), PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
    close(fd);
    if (stats == MAP_FAILED) _exit(122);
    *stats = inherited;
    stats->peak_live_usable_bytes = stats->live_usable_bytes;
    owner = pid;
    return stats;
}
static void account(struct counters* c, void* p, size_t n) {
    if (!c || !p) return;
    size_t usable = malloc_usable_size(p);
    c->requested_bytes += n; c->usable_bytes += usable; c->live_usable_bytes += usable;
    if (c->live_usable_bytes > c->peak_live_usable_bytes) c->peak_live_usable_bytes = c->live_usable_bytes;
    size_t bin = 0, size = n;
    while (size > 1 && bin < 31) { size >>= 1; ++bin; }
    ++c->sizes[bin];
}
static void subtract(struct counters* c, void* p) {
    if (c && p) { size_t n = malloc_usable_size(p); c->live_usable_bytes = n <= c->live_usable_bytes ? c->live_usable_bytes - n : 0; }
}
void* __real_malloc(size_t);
void* __real_calloc(size_t, size_t);
void* __real_realloc(void*, size_t);
void __real_free(void*);
int __real_posix_memalign(void**, size_t, size_t);
void* __wrap_malloc(size_t n) { struct counters* c=counter(); if(c) ++c->malloc_calls; void* p=__real_malloc(n); account(c,p,n); return p; }
void* __wrap_calloc(size_t n, size_t size) { struct counters* c=counter(); if(c) ++c->calloc_calls; void* p=__real_calloc(n,size); account(c,p,n*size); return p; }
void* __wrap_realloc(void* old, size_t n) {
    struct counters* c=counter(); if(c) ++c->realloc_calls;
    size_t previous = old && c ? malloc_usable_size(old) : 0;
    void* p=__real_realloc(old,n);
    if (c && (p || n == 0)) { c->live_usable_bytes = previous <= c->live_usable_bytes ? c->live_usable_bytes-previous : 0; account(c,p,n); }
    return p;
}
void __wrap_free(void* p) { struct counters* c=counter(); if(c && p) ++c->free_calls; subtract(c,p); __real_free(p); }
int __wrap_posix_memalign(void** p, size_t align, size_t n) {
    struct counters* c=counter(); if(c) ++c->aligned_calls;
    int rc=__real_posix_memalign(p,align,n); if(!rc) account(c,*p,n); return rc;
}
