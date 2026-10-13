#include <dlfcn.h>
#include <new>
#include <stddef.h>
#include <stdlib.h>

extern "C" {
void* mi_malloc(size_t size) noexcept;
void* mi_malloc_aligned(size_t size, size_t alignment) noexcept;
void* mi_new_nothrow(size_t size) noexcept;
void* mi_new_aligned_nothrow(size_t size, size_t alignment) noexcept;
}

// The C++ runtime is looked up when an allocation fails instead of being linked, so the library
// does not load libstdc++ into processes that are not C++. A C++ program already has it loaded;
// elsewhere there is no handler to call and nothing to catch the exception, so failure aborts,
// as in C mimalloc. This is the same lookup `mi_new` uses for the handler.
static std::new_handler mi_cxx_get_new_handler() {
  using get_new_handler_fn = std::new_handler (*)();
  auto get = reinterpret_cast<get_new_handler_fn>(dlsym(RTLD_DEFAULT, "_ZSt15get_new_handlerv"));
  return get != nullptr ? get() : nullptr;
}

[[noreturn]] static void mi_cxx_throw_bad_alloc() {
  using throw_fn = void (*)();
  auto raise = reinterpret_cast<throw_fn>(dlsym(RTLD_DEFAULT, "_ZSt17__throw_bad_allocv"));
  if (raise != nullptr) raise();  // std::__throw_bad_alloc(): throws std::bad_alloc
  abort();
}

static void* mi_cxx_new_impl(size_t size) {
  for (;;) {
    void* p = mi_malloc(size);
    if (p != nullptr) return p;
    std::new_handler handler = mi_cxx_get_new_handler();
    if (handler == nullptr) mi_cxx_throw_bad_alloc();
    handler();
  }
}

extern "C" void* mi_cxx_new_slow(size_t size) {
  return mi_cxx_new_impl(size);
}

static void* mi_cxx_new_aligned_impl(size_t size, std::align_val_t alignment) {
  for (;;) {
    void* p = mi_malloc_aligned(size, static_cast<size_t>(alignment));
    if (p != nullptr) return p;
    std::new_handler handler = mi_cxx_get_new_handler();
    if (handler == nullptr) mi_cxx_throw_bad_alloc();
    handler();
  }
}

#if !defined(MI_CXX_USE_FAST_NEW)
void* operator new(size_t size) {
  return mi_cxx_new_impl(size);
}

void* operator new[](size_t size) {
  return mi_cxx_new_impl(size);
}
#endif

void* operator new(size_t size, const std::nothrow_t&) noexcept {
  return mi_new_nothrow(size);
}

void* operator new[](size_t size, const std::nothrow_t&) noexcept {
  return mi_new_nothrow(size);
}

void* operator new(size_t size, std::align_val_t alignment) {
  return mi_cxx_new_aligned_impl(size, alignment);
}

void* operator new[](size_t size, std::align_val_t alignment) {
  return mi_cxx_new_aligned_impl(size, alignment);
}

void* operator new(size_t size, std::align_val_t alignment, const std::nothrow_t&) noexcept {
  return mi_new_aligned_nothrow(size, static_cast<size_t>(alignment));
}

void* operator new[](size_t size, std::align_val_t alignment, const std::nothrow_t&) noexcept {
  return mi_new_aligned_nothrow(size, static_cast<size_t>(alignment));
}
