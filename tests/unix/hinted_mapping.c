#define _GNU_SOURCE
#include <mimalloc.h>
#include <mimalloc/internal.h>

#include <assert.h>
#include <errno.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <sys/mman.h>
#include <unistd.h>

extern void* __real_mmap(void*, size_t, int, int, int, off_t);
extern int __real_munmap(void*, size_t);

static bool active;
static bool fail_second;
static size_t page_size, alignment, request_size;
static size_t mmap_calls, second_size, mapped_bytes, unmapped_bytes;
static uintptr_t live_pages[256];
static size_t live_count;

static void record_mapping(void* p, size_t size) {
  assert(p != MAP_FAILED && size % page_size == 0);
  for (size_t offset = 0; offset < size; offset += page_size) {
    assert(live_count < sizeof(live_pages) / sizeof(live_pages[0]));
    live_pages[live_count++] = (uintptr_t)p + offset;
  }
  mapped_bytes += size;
}

int __wrap_munmap(void* p, size_t size) {
  int err = __real_munmap(p, size);
  if (!active) return err;
  assert(err == 0 && size % page_size == 0);
  for (size_t offset = 0; offset < size; offset += page_size) {
    uintptr_t page = (uintptr_t)p + offset;
    size_t i = 0;
    while (i < live_count && live_pages[i] != page) ++i;
    assert(i < live_count);  // Reject unowned or duplicate releases.
    live_pages[i] = live_pages[--live_count];
  }
  unmapped_bytes += size;
  return err;
}

void* __wrap_mmap(void* addr, size_t size, int prot, int flags, int fd, off_t offset) {
  if (!active) return __real_mmap(addr, size, prot, flags, fd, offset);
  ++mmap_calls;
  if (mmap_calls == 1) {
    assert(addr != NULL && (uintptr_t)addr % alignment == 0);
    assert(size == request_size && fd == -1 && offset == 0);
    assert((flags & MAP_ANONYMOUS) != 0 && (flags & MAP_FIXED) == 0);

    // Supply a genuine, OS-page-aligned mapping that misses the requested
    // alignment. The extra page lets us choose either the base or base+page.
    void* mapping = __real_mmap(NULL, size + page_size, prot, flags, fd, offset);
    assert(mapping != MAP_FAILED);
    record_mapping(mapping, size + page_size);
    void* result = mapping;
    if ((uintptr_t)mapping % alignment == 0) {
      result = (char*)mapping + page_size;
      assert(__wrap_munmap(mapping, page_size) == 0);
    } else {
      assert(__wrap_munmap((char*)mapping + size, page_size) == 0);
    }
    assert((uintptr_t)result % page_size == 0);
    assert((uintptr_t)result % alignment != 0);
    return result;
  }
  if (mmap_calls == 2) {
    second_size = size;
    if (fail_second) {
      errno = ENOMEM;
      return MAP_FAILED;
    }
  }
  void* result = __real_mmap(addr, size, prot, flags, fd, offset);
  if (result != MAP_FAILED) record_mapping(result, size);
  return result;
}

int main(int argc, char** argv) {
  if (sizeof(void*) < 8) {
    puts("skipping: aligned address hints require a 64-bit target");
    return 0;
  }
  assert(argc == 2);
  fail_second = argv[1][0] == 'f';
  long os_page_size = sysconf(_SC_PAGESIZE);
  assert(os_page_size > 0);
  page_size = (size_t)os_page_size;
  alignment = 16 * page_size;
  request_size = 4 * alignment;  // Eligible for the direct aligned attempt.

  mi_process_init();
  mi_option_set(mi_option_show_errors, 0);
  mi_option_set(mi_option_verbose, 0);
  mi_option_set(mi_option_allow_thp, 0);
  void* warm = mi_malloc(16);
  assert(warm != NULL);
  mi_free(warm);

  mi_memid_t memid = _mi_memid_none();
  active = true;
  void* p = _mi_os_alloc_aligned(request_size, alignment, true, false, &memid);
  if (p != NULL) {
    assert((uintptr_t)p % alignment == 0);
    assert(memid.mem.os.base == p && memid.mem.os.size == request_size);
    // Touch only the valid returned allocation, then release its backing.
    for (size_t offset = 0; offset < request_size; offset += page_size) {
      ((volatile unsigned char*)p)[offset] = 0x5a;
    }
    _mi_os_free(memid.mem.os.base, memid.mem.os.size, memid);
  }
  active = false;

  printf("retry=%s calls=%zu second_size=%zu mapped=%zu unmapped=%zu live=%zu result=%s\n",
         fail_second ? "failure" : "success", mmap_calls, second_size,
         mapped_bytes, unmapped_bytes, live_count * page_size,
         p == NULL ? "null" : "allocated");
  // Check the external page ledger even if allocator accounting balances.
  bool balanced = live_count == 0 && mapped_bytes == unmapped_bytes;
  bool expected_result = fail_second ? p == NULL : p != NULL;
  bool outer_retry = mmap_calls == 2 && second_size == request_size + alignment;
  return balanced && expected_result && outer_retry ? 0 : 1;
}
