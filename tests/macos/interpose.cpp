#include <cassert>
#include <cstdint>
#include <cstdio>
#include <dlfcn.h>
#include <limits>
#include <new>
#include <string>
#include <thread>
#include <vector>

using owned_fn = bool (*)(const void*);
static std::vector<int> early(100, 42);
struct alignas(4096) Aligned { char bytes[4096]; };
struct HandlerCalled {};
static void failing_handler() { throw HandlerCalled{}; }

int main() {
  auto owned = reinterpret_cast<owned_fn>(dlsym(RTLD_DEFAULT, "mi_check_owned"));
  assert(owned && owned(early.data()) && early[99] == 42);
  std::string text(4096, 'x');
  assert(owned(text.data()));
  auto* p = new char;
  assert(owned(p) && reinterpret_cast<uintptr_t>(p) % 16 == 0);
  delete p;
  auto* array = new char[257];
  assert(owned(array));
  delete[] array;
  auto* aligned = new Aligned;
  assert(owned(aligned) && reinterpret_cast<uintptr_t>(aligned) % 4096 == 0);
  delete aligned;
  aligned = new (std::nothrow) Aligned;
  assert(owned(aligned));
  delete aligned;
  volatile size_t enormous = std::numeric_limits<size_t>::max();
  std::set_new_handler(nullptr);
  try { (void)::operator new(enormous); assert(false); }
  catch (const std::bad_alloc&) {}
  assert(::operator new(enormous, std::nothrow) == nullptr);
  std::set_new_handler(failing_handler);
  try { (void)::operator new(enormous); assert(false); }
  catch (const HandlerCalled&) {}
  assert(::operator new(enormous, std::nothrow) == nullptr);
  std::set_new_handler(nullptr);
  std::vector<std::thread> threads;
  for (int i = 0; i < 8; ++i) {
    auto* values = new std::vector<int>(1024, i);
    threads.emplace_back([values, owned, i] {
      assert(owned(values) && owned(values->data()) && values->back() == i);
      delete values;
    });
  }
  for (auto& thread : threads) thread.join();
  std::puts("macOS C++ interposition: startup, alignment, exceptions, new_handler, and remote frees passed");
}
