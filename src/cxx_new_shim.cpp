#include <new>
#include <stddef.h>

extern "C" {
void* mi_malloc(size_t size) noexcept;
void* mi_malloc_aligned(size_t size, size_t alignment) noexcept;
void* mi_new_nothrow(size_t size) noexcept;
void* mi_new_aligned_nothrow(size_t size, size_t alignment) noexcept;
}

static void* mi_cxx_new_impl(size_t size) {
  for (;;) {
    void* p = mi_malloc(size);
    if (p != nullptr) return p;
    std::new_handler handler = std::get_new_handler();
    if (handler == nullptr) throw std::bad_alloc();
    handler();
  }
}

static void* mi_cxx_new_aligned_impl(size_t size, std::align_val_t alignment) {
  for (;;) {
    void* p = mi_malloc_aligned(size, static_cast<size_t>(alignment));
    if (p != nullptr) return p;
    std::new_handler handler = std::get_new_handler();
    if (handler == nullptr) throw std::bad_alloc();
    handler();
  }
}

void* operator new(size_t size) {
  return mi_cxx_new_impl(size);
}

void* operator new[](size_t size) {
  return mi_cxx_new_impl(size);
}

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
