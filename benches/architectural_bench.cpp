// Linux architectural probes. Harness storage uses the system allocator; only
// explicit calls through Api use the allocator under test. See benches/README.md.
#include "mimalloc.h"
#include <algorithm>
#include <barrier>
#include <chrono>
#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <dlfcn.h>
#include <fstream>
#include <iostream>
#include <limits>
#include <stdexcept>
#include <string>
#include <thread>
#include <vector>
#include <sys/resource.h>
#include <unistd.h>

using Clock = std::chrono::steady_clock;
struct Config {
  std::string library, scenario;
  std::string action = "idle";
  size_t bytes = 64 * 1024 * 1024, block = 4096, threads = 4, cycles = 8;
  size_t idle_ms = 10000;
};
static size_t os_page;

struct Api {
  void* handle;
  decltype(&mi_malloc) alloc;
  decltype(&mi_free) free;
  decltype(&mi_collect) collect;
  decltype(&mi_thread_done) thread_done;
  decltype(&mi_heap_new) heap_new;
  decltype(&mi_heap_malloc) heap_alloc;
  decltype(&mi_heap_delete) heap_delete;
  decltype(&mi_heap_collect) heap_collect;
  decltype(&mi_heap_visit_blocks) visit;
  decltype(&mi_option_get) option_get;
  decltype(&mi_option_set) option_set;
  template<class T> T symbol(const char* name) {
    auto p = dlsym(handle, name);
    if (!p) throw std::runtime_error(std::string("missing allocator symbol: ") + name);
    return reinterpret_cast<T>(p);
  }
  explicit Api(const std::string& path) {
    handle = dlopen(path.c_str(), RTLD_NOW | RTLD_LOCAL);
    if (!handle) throw std::runtime_error(dlerror());
#define LOAD(field, name) field = symbol<decltype(field)>(#name)
    LOAD(alloc, mi_malloc); LOAD(free, mi_free); LOAD(collect, mi_collect);
    LOAD(thread_done, mi_thread_done); LOAD(heap_new, mi_heap_new);
    LOAD(heap_alloc, mi_heap_malloc); LOAD(heap_delete, mi_heap_delete);
    LOAD(heap_collect, mi_heap_collect);
    LOAD(visit, mi_heap_visit_blocks); LOAD(option_get, mi_option_get);
    LOAD(option_set, mi_option_set);
#undef LOAD
    // Reject a dependency's symbols (e.g. an accidental system mimalloc).
    Dl_info owner{};
    for (auto address : {reinterpret_cast<void*>(alloc), reinterpret_cast<void*>(free)}) {
      if (!dladdr(address, &owner)) throw std::runtime_error("cannot identify allocator symbol");
      char requested[4096], actual[4096];
      if (!realpath(path.c_str(), requested) || !realpath(owner.dli_fname, actual) ||
          std::strcmp(requested, actual)) throw std::runtime_error("allocator symbol belongs to another library");
    }
  }
  // Keep the handle loaded through process teardown: allocator TLS can outlive Api.
};

struct Snapshot {
  size_t rss;
  long minor, major;
};
static Snapshot snapshot() {
  std::ifstream statm("/proc/self/statm");
  size_t virtual_pages, resident_pages;
  if (!(statm >> virtual_pages >> resident_pages)) throw std::runtime_error("cannot read RSS");
  rusage usage{};
  if (getrusage(RUSAGE_SELF, &usage)) throw std::runtime_error("getrusage failed");
  return {resident_pages * os_page, usage.ru_minflt, usage.ru_majflt};
}
static double elapsed(Clock::time_point start) {
  return std::chrono::duration<double, std::nano>(Clock::now() - start).count();
}
static void emit(const std::string& phase, size_t cycle, double ns, size_t operations,
                 size_t live, Snapshot before, long pages = -1) {
  const auto after = snapshot();
  std::cout << "{\"phase\":\"" << phase << "\",\"cycle\":" << cycle
            << ",\"elapsed_ns\":" << ns << ",\"operations\":" << operations
            << ",\"live_bytes\":" << live << ",\"rss_bytes\":" << after.rss
            << ",\"rss_delta_bytes\":" << static_cast<long long>(after.rss) - static_cast<long long>(before.rss)
            << ",\"minor_faults\":" << after.minor - before.minor
            << ",\"major_faults\":" << after.major - before.major
            << ",\"occupied_pages\":" << pages << "}\n";
}
static void touch(void* p, size_t bytes) {
  if (!p) throw std::runtime_error("allocation failed");
  auto data = static_cast<volatile unsigned char*>(p);
  // Touch every OS page intersecting the payload, including an unaligned last page.
  for (size_t i = 0; i < bytes; i += os_page) data[i] = 0xa5;
  data[bytes - 1] = 0xa5;
}
static void check(void* p, size_t bytes) {
  auto data = static_cast<const volatile unsigned char*>(p);
  for (size_t i = 0; i < bytes; i += os_page)
    if (data[i] != 0xa5) throw std::runtime_error("payload corrupted");
  if (data[bytes - 1] != 0xa5) throw std::runtime_error("payload corrupted");
}
static void allocate(Api& api, std::vector<void*>& pointers, size_t block, mi_heap_t* heap = nullptr) {
  for (auto& p : pointers) { p = heap ? api.heap_alloc(heap, block) : api.alloc(block); touch(p, block); }
}
static void verify(const std::vector<void*>& pointers, size_t block) {
  for (auto p : pointers) if (p) check(p, block);
}
static void release(Api& api, std::vector<void*>& pointers) {
  for (auto& p : pointers) if (p) { api.free(p); p = nullptr; }
}

using Pages = std::vector<std::vector<void*>>;
static bool page_visitor(const mi_heap_t*, const mi_heap_area_t*, void* block, size_t, void* arg) {
  auto& pages = *static_cast<Pages*>(arg);
  if (!block) pages.emplace_back();
  else {
    if (pages.empty()) std::abort();
    pages.back().push_back(block);
  }
  return true;
}
static void occupancy(Api& api, const Config& c) {
  auto heap = api.heap_new();
  if (!heap) throw std::runtime_error("heap creation failed");
  std::vector<void*> all(c.bytes / c.block);
  auto before = snapshot(); auto start = Clock::now();
  allocate(api, all, c.block, heap);
  const auto fill_ns = elapsed(start);
  Pages pages;
  if (!api.visit(heap, true, page_visitor, &pages)) throw std::runtime_error("heap visitor failed");
  size_t visited = 0;
  pages.erase(std::remove_if(pages.begin(), pages.end(), [](const auto& p) { return p.empty(); }), pages.end());
  for (auto& page : pages) visited += page.size();
  if (visited != all.size() || pages.empty()) throw std::runtime_error("heap visitor allocation count mismatch");
  emit("filled", 0, fill_ns, all.size(), all.size() * c.block, before, pages.size());
  // The visitor determines actual allocation pages, without address masking or
  // assuming that Rust and C choose identical page geometries.
  for (size_t step = 0; step < 4; ++step) {
    size_t live = 0, count = 0, occupied = 0;
    for (const auto& page : pages) verify(page, c.block);
    before = snapshot(); start = Clock::now();
    for (auto& page : pages) {
      const size_t keep = step == 0 ? (page.size() + 1) / 2 :
                          step == 1 ? (page.size() + 9) / 10 : step == 2 ? 1 : 0;
      for (size_t i = keep; i < page.size(); ++i) if (page[i]) { api.free(page[i]); page[i] = nullptr; ++count; }
      live += keep; occupied += keep != 0;
    }
    const double free_ns = elapsed(start);
    const std::string label = step == 0 ? "half-per-page" : step == 1 ? "tenth-per-page" :
                              step == 2 ? "one-per-page" : "empty";
    emit(label, 0, free_ns, count, live * c.block, before, occupied);
    before = snapshot(); start = Clock::now(); api.heap_collect(heap, true);
    emit(label + "-collect", 0, elapsed(start), 1, live * c.block, before, occupied);
  }
  // Same number of live blocks as the sparse one-per-page case, packed by fresh
  // allocation after collection. This distinguishes occupancy from live bytes.
  std::vector<void*> packed(pages.size());
  before = snapshot(); start = Clock::now(); allocate(api, packed, c.block, heap);
  const double packed_ns = elapsed(start);
  Pages packed_pages;
  if (!api.visit(heap, true, page_visitor, &packed_pages)) throw std::runtime_error("heap visitor failed");
  size_t occupied = 0;
  for (const auto& page : packed_pages) occupied += !page.empty();
  emit("packed-same-live", 0, packed_ns, packed.size(), packed.size() * c.block, before, occupied);
  verify(packed, c.block); release(api, packed); api.heap_delete(heap); api.collect(true);
}

static void cross_thread(Api& api, const Config& c) {
  const size_t per_thread = (c.bytes / c.block) / c.threads;
  std::vector<std::vector<void*>> pools(c.threads, std::vector<void*>(per_thread));
  std::barrier barrier(static_cast<std::ptrdiff_t>(c.threads + 1));
  std::vector<std::thread> workers;
  for (size_t i = 0; i < c.threads; ++i) workers.emplace_back([&, i] {
    for (size_t cycle = 0; cycle < c.cycles; ++cycle) {
      barrier.arrive_and_wait(); allocate(api, pools[i], c.block); barrier.arrive_and_wait();
      barrier.arrive_and_wait(); // Main thread frees the fan-in pool.
      barrier.arrive_and_wait(); release(api, pools[i]); barrier.arrive_and_wait();
      barrier.arrive_and_wait();
    }
    api.thread_done();
  });
  const size_t count = per_thread * c.threads;
  for (size_t cycle = 0; cycle < c.cycles; ++cycle) {
    auto before = snapshot(); auto start = Clock::now();
    barrier.arrive_and_wait(); barrier.arrive_and_wait();
    emit("fan-in-allocate", cycle, elapsed(start), count, count * c.block, before);
    for (auto& pool : pools) verify(pool, c.block);
    before = snapshot(); start = Clock::now(); for (auto& pool : pools) release(api, pool);
    emit("fan-in-free", cycle, elapsed(start), count, 0, before);
    barrier.arrive_and_wait();
    before = snapshot(); start = Clock::now(); for (auto& pool : pools) allocate(api, pool, c.block);
    emit("fan-out-allocate", cycle, elapsed(start), count, count * c.block, before);
    for (auto& pool : pools) verify(pool, c.block);
    before = snapshot(); start = Clock::now(); barrier.arrive_and_wait(); barrier.arrive_and_wait();
    emit("fan-out-free", cycle, elapsed(start), count, 0, before);
    barrier.arrive_and_wait();
  }
  for (auto& worker : workers) worker.join();
  auto before = snapshot(); auto start = Clock::now(); api.collect(true);
  emit("final-collect", c.cycles, elapsed(start), 1, 0, before);
}

static void oscillation(Api& api, const Config& c) {
  // Explicit immediate purge makes the pressure/refault experiment repeatable;
  // other scenarios preserve the library's configured purge policy.
  std::vector<void*> pool(c.bytes / c.block);
  for (size_t cycle = 0; cycle < c.cycles; ++cycle) {
    auto before = snapshot(); auto start = Clock::now(); allocate(api, pool, c.block);
    emit("refill-touch", cycle, elapsed(start), pool.size(), pool.size() * c.block, before);
    verify(pool, c.block);
    before = snapshot(); start = Clock::now(); release(api, pool);
    emit("free", cycle, elapsed(start), pool.size(), 0, before);
    before = snapshot(); start = Clock::now(); api.collect(true);
    emit("forced-purge", cycle, elapsed(start), 1, 0, before);
  }
}

static void churn(Api& api, const Config& c) {
  const size_t per_thread = (c.bytes / c.block) / c.threads;
  std::vector<std::vector<void*>> pools(c.threads, std::vector<void*>(per_thread));
  std::vector<std::thread> workers; workers.reserve(c.threads);
  for (size_t cycle = 0; cycle < c.cycles; ++cycle) {
    auto before = snapshot(); auto start = Clock::now();
    for (size_t i = 0; i < c.threads; ++i) workers.emplace_back([&, i] {
      allocate(api, pools[i], c.block);
      // Deterministic allocator teardown before OS-thread exit. Allocations
      // survive the owner and are freed by the receiving thread after join.
      api.thread_done();
    });
    for (auto& worker : workers) worker.join();
    workers.clear();
    const size_t count = per_thread * c.threads;
    emit("create-allocate-exit", cycle, elapsed(start), count, count * c.block, before);
    for (auto& pool : pools) verify(pool, c.block);
    before = snapshot(); start = Clock::now(); for (auto& pool : pools) release(api, pool);
    emit("free-after-owner-exit", cycle, elapsed(start), count, 0, before);
    before = snapshot(); start = Clock::now(); api.collect(true);
    emit("collect-abandoned", cycle, elapsed(start), 1, 0, before);
  }
}

// X33: all four arms have the same initialized owner/helper history. The
// controller samples /proc externally and acknowledges each phase on stdin.
static void burst_idle(Api& api, const Config& c) {
  const auto thread_init = api.symbol<decltype(&mi_thread_init)>("mi_thread_init");
  std::vector<void*> pool(c.bytes / c.block, nullptr);
  const bool empty = c.action == "empty";
  const size_t count = empty ? 0 : pool.size();
  enum class Command { fill, free, collect, stop };
  struct Worker {
    std::barrier<> sync{2};
    Command command = Command::fill;
    double work_ns = 0;
  } owner, helper;
  auto work = [&](Worker& worker) {
    thread_init();
    worker.sync.arrive_and_wait();
    for (;;) {
      worker.sync.arrive_and_wait();
      if (worker.command == Command::free) {
        for (size_t i = 0; i < count; ++i) {
          const auto* data = static_cast<const unsigned char*>(pool[i]);
          for (size_t j = 0; j < c.block; ++j)
            if (data[j] != 0xa5) throw std::runtime_error("burst payload corrupted");
        }
      }
      auto start = Clock::now();
      switch (worker.command) {
        case Command::fill:
          for (size_t i = 0; i < count; ++i) {
            if (pool[i]) throw std::runtime_error("burst block still live");
            pool[i] = api.alloc(c.block);
            if (!pool[i]) throw std::runtime_error("burst allocation failed");
            std::memset(pool[i], 0xa5, c.block);
          }
          break;
        case Command::free: release(api, pool); break;
        case Command::collect: api.collect(true); break;
        case Command::stop: api.thread_done(); break;
      }
      worker.work_ns = elapsed(start);
      const bool stop = worker.command == Command::stop;
      worker.sync.arrive_and_wait();
      if (stop) return;
    }
  };
  std::thread helper_thread(work, std::ref(helper));
  helper.sync.arrive_and_wait();
  std::thread owner_thread(work, std::ref(owner));
  owner.sync.arrive_and_wait();
  auto dispatch = [](Worker& worker, Command command) {
    worker.command = command;
    worker.sync.arrive_and_wait();
    worker.sync.arrive_and_wait();
    return worker.work_ns;
  };
  auto event = [&](const char* phase, size_t live, double ns = 0, double work_ns = 0) {
    timespec ts{};
    if (clock_gettime(CLOCK_MONOTONIC, &ts)) std::abort();
    rusage usage{};
    if (getrusage(RUSAGE_SELF, &usage)) std::abort();
    std::cout << "{\"phase\":\"" << phase << "\",\"monotonic_ns\":"
              << static_cast<unsigned long long>(ts.tv_sec) * 1000000000ULL + ts.tv_nsec
              << ",\"live_bytes\":" << live << ",\"blocks\":" << count
              << ",\"elapsed_ns\":" << ns << ",\"work_ns\":" << work_ns
              << ",\"minor_faults\":" << usage.ru_minflt
              << ",\"major_faults\":" << usage.ru_majflt
              << ",\"purge_delay_ms\":" << api.option_get(mi_option_purge_delay)
              << ",\"arena_purge_mult\":" << api.option_get(mi_option_arena_purge_mult)
              << ",\"page_full_retain\":" << api.option_get(mi_option_page_full_retain)
              << ",\"allow_thp\":" << api.option_get(mi_option_allow_thp) << "}" << std::endl;
    if (std::cin.get() != '\n') std::abort();
  };
  const auto idle = std::chrono::milliseconds(c.idle_ms);
  event("ready", 0);  // Warm the output path before measuring the baseline.
  event("baseline", 0);
  auto start = Clock::now();
  auto work_ns = dispatch(owner, Command::fill);
  event("filled", count * c.block, elapsed(start), work_ns);
  start = Clock::now(); work_ns = dispatch(owner, Command::free);
  auto freed = Clock::now();
  event("freed", 0, elapsed(start), work_ns);
  std::this_thread::sleep_until(freed + idle);
  event("before-intervention", 0);
  start = Clock::now(); work_ns = 0;
  if (c.action == "owner-collect") work_ns = dispatch(owner, Command::collect);
  else if (c.action == "other-collect") work_ns = dispatch(helper, Command::collect);
  else if (c.action == "owner-exit") {
    work_ns = dispatch(owner, Command::stop);
    owner_thread.join();
  }
  event("intervened", 0, elapsed(start), work_ns);
  std::this_thread::sleep_until(freed + idle * 2);
  event("settled", 0);
  start = Clock::now();
  if (c.action == "owner-exit") {
    owner_thread = std::thread(work, std::ref(owner));
    owner.sync.arrive_and_wait();
  }
  work_ns = dispatch(owner, Command::fill);
  event("refilled", count * c.block, elapsed(start), work_ns);
  start = Clock::now(); work_ns = dispatch(owner, Command::free);
  freed = Clock::now();
  event("refreed", 0, elapsed(start), work_ns);
  std::this_thread::sleep_until(freed + idle);
  event("final", 0);
  dispatch(owner, Command::stop); owner_thread.join();
  dispatch(helper, Command::stop); helper_thread.join();
}

static size_t positive(const char* value) {
  if (!*value || *value == '-') throw std::runtime_error("expected a positive integer");
  size_t used = 0;
  auto n = std::stoull(value, &used);
  if (used != std::strlen(value) || n == 0 || n > std::numeric_limits<size_t>::max())
    throw std::runtime_error("expected a positive integer");
  return static_cast<size_t>(n);
}
int main(int argc, char** argv) {
  try {
    Config c;
    for (int i = 1; i < argc; i += 2) {
      if (i + 1 == argc) throw std::runtime_error("each option needs a value");
      std::string option = argv[i];
      if (option == "--library") c.library = argv[i + 1];
      else if (option == "--scenario") c.scenario = argv[i + 1];
      else if (option == "--bytes") c.bytes = positive(argv[i + 1]);
      else if (option == "--block-size") c.block = positive(argv[i + 1]);
      else if (option == "--threads") c.threads = positive(argv[i + 1]);
      else if (option == "--cycles") c.cycles = positive(argv[i + 1]);
      else if (option == "--action") c.action = argv[i + 1];
      else if (option == "--idle-ms") c.idle_ms = positive(argv[i + 1]);
      else throw std::runtime_error("unknown option: " + option);
    }
    if (c.library.empty() || c.bytes / c.block < c.threads ||
        c.threads >= static_cast<size_t>(std::numeric_limits<std::ptrdiff_t>::max()))
      throw std::runtime_error("provide --library and at least one block per thread");
    if (c.scenario != "segment-occupancy" && c.scenario != "cross-thread" &&
        c.scenario != "decommit-oscillation" && c.scenario != "thread-churn" &&
        c.scenario != "burst-idle")
      throw std::runtime_error("unknown scenario");
    if (c.action != "idle" && c.action != "owner-collect" && c.action != "other-collect" &&
        c.action != "owner-exit" && c.action != "empty") throw std::runtime_error("unknown action");
    const long page = sysconf(_SC_PAGESIZE);
    if (page <= 0) throw std::runtime_error("cannot determine OS page size");
    os_page = static_cast<size_t>(page);
    Api api(c.library);
    std::cout.precision(17);
    if (c.scenario == "burst-idle") { burst_idle(api, c); return 0; }
    if (c.scenario == "decommit-oscillation") {
      api.option_set(mi_option_purge_delay, 0);
      api.option_set(mi_option_arena_purge_mult, 1);
    }
    std::cout << "{\"phase\":\"baseline\",\"cycle\":0,\"elapsed_ns\":0,\"operations\":0,\"live_bytes\":0,\"rss_bytes\":"
              << snapshot().rss << ",\"rss_delta_bytes\":0,\"minor_faults\":0,\"major_faults\":0,\"occupied_pages\":-1,\"purge_delay_ms\":"
              << api.option_get(mi_option_purge_delay) << "}\n";
    if (c.scenario == "segment-occupancy") occupancy(api, c);
    else if (c.scenario == "cross-thread") cross_thread(api, c);
    else if (c.scenario == "decommit-oscillation") oscillation(api, c);
    else churn(api, c);
    return 0;
  } catch (const std::exception& error) {
    std::cerr << "architectural benchmark: " << error.what() << '\n'; return 1;
  }
}
