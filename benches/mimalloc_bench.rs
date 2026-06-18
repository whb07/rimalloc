//! Native Criterion ports of selected `mimalloc-bench/bench` workloads.
//!
//! These benchmarks do not spawn the upstream C/C++ executables. The workload
//! logic is ported to Rust and all measured allocator traffic goes through an
//! explicit allocator handle, either this crate's Rust mimalloc implementation
//! or C mimalloc 3.3.2 loaded from `mimalloc-3.3.2`.
//!
//! Examples:
//!   cargo bench --features bench --bench mimalloc_bench -- glibc-simple
//!   MIMALLOC_BENCH_SCENARIOS=mleak10 cargo bench --features bench --bench mimalloc_bench
//!
//! Useful environment variables:
//!   MIMALLOC_BENCH_SCENARIOS=glibc-simple,mleak10,xmalloc-testN
//!   MIMALLOC_BENCH_ALLOCATORS=rust,c
//!   MIMALLOC_RUST_LIB=/path/to/libmechanical_mimalloc.so
//!   MIMALLOC_C_LIB=/path/to/libmimalloc.so
//!   MIMALLOC_BENCH_SKIP_BUILD=1
//!   MIMALLOC_BENCH_SAMPLE_SIZE=10
//!   MIMALLOC_BENCH_MEASUREMENT_SECS=10
//!   MIMALLOC_BENCH_WARMUP_SECS=1

use criterion::{BenchmarkId, Criterion, SamplingMode, criterion_group, criterion_main};
use std::env;
use std::ffi::{CStr, CString, c_char, c_int, c_uint, c_void};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicPtr, AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

#[allow(dead_code)]
#[path = "support/cfrac_core.rs"]
mod cfrac_core;
mod rptest_tables;

#[derive(Clone, Copy)]
struct DylibAllocator {
    malloc: unsafe extern "C" fn(usize) -> *mut c_void,
    calloc: unsafe extern "C" fn(usize, usize) -> *mut c_void,
    realloc: unsafe extern "C" fn(*mut c_void, usize) -> *mut c_void,
    free: unsafe extern "C" fn(*mut c_void),
    memalign: unsafe extern "C" fn(usize, usize) -> *mut c_void,
    new_array: unsafe extern "C" fn(usize) -> *mut c_void,
    delete_array: unsafe extern "C" fn(*mut c_void),
    delete_array_sized: unsafe extern "C" fn(*mut c_void, usize),
}

#[derive(Clone, Copy)]
enum BenchAllocator {
    Rust(&'static DylibAllocator),
    C(&'static DylibAllocator),
}

impl BenchAllocator {
    fn name(self) -> &'static str {
        match self {
            BenchAllocator::Rust(_) => "rust",
            BenchAllocator::C(_) => "c-mimalloc-3.3.2",
        }
    }

    unsafe fn malloc(self, size: usize) -> *mut u8 {
        unsafe {
            match self {
                BenchAllocator::Rust(alloc) | BenchAllocator::C(alloc) => {
                    (alloc.malloc)(size).cast()
                }
            }
        }
    }

    unsafe fn calloc(self, count: usize, size: usize) -> *mut u8 {
        unsafe {
            match self {
                BenchAllocator::Rust(alloc) | BenchAllocator::C(alloc) => {
                    (alloc.calloc)(count, size).cast()
                }
            }
        }
    }

    unsafe fn realloc(self, ptr: *mut u8, size: usize) -> *mut u8 {
        unsafe {
            match self {
                BenchAllocator::Rust(alloc) | BenchAllocator::C(alloc) => {
                    (alloc.realloc)(ptr.cast(), size).cast()
                }
            }
        }
    }

    unsafe fn free(self, ptr: *mut u8) {
        unsafe {
            if ptr.is_null() {
                return;
            }
            match self {
                BenchAllocator::Rust(alloc) | BenchAllocator::C(alloc) => {
                    (alloc.free)(ptr.cast());
                }
            }
        }
    }

    unsafe fn memalign(self, alignment: usize, size: usize) -> *mut u8 {
        unsafe {
            match self {
                BenchAllocator::Rust(alloc) | BenchAllocator::C(alloc) => {
                    (alloc.memalign)(alignment, size).cast()
                }
            }
        }
    }

    unsafe fn new_array(self, size: usize) -> *mut u8 {
        unsafe {
            match self {
                BenchAllocator::Rust(alloc) | BenchAllocator::C(alloc) => {
                    (alloc.new_array)(size).cast()
                }
            }
        }
    }

    unsafe fn delete_array(self, ptr: *mut u8) {
        unsafe {
            if ptr.is_null() {
                return;
            }
            match self {
                BenchAllocator::Rust(alloc) | BenchAllocator::C(alloc) => {
                    (alloc.delete_array)(ptr.cast());
                }
            }
        }
    }

    unsafe fn delete_array_sized(self, ptr: *mut u8, size: usize) {
        unsafe {
            if ptr.is_null() {
                return;
            }
            match self {
                BenchAllocator::Rust(alloc) | BenchAllocator::C(alloc) => {
                    (alloc.delete_array_sized)(ptr.cast(), size);
                }
            }
        }
    }
}

struct Scenario {
    name: &'static str,
    run: unsafe fn(BenchAllocator) -> Duration,
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn csv_env(name: &str) -> Vec<String> {
    env::var(name)
        .ok()
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn skip_build() -> bool {
    env::var_os("MIMALLOC_BENCH_SKIP_BUILD").is_some()
}

fn logical_cpus() -> usize {
    std::thread::available_parallelism().map_or(1, |n| n.get())
}

fn run_status(mut command: Command, action: &str) {
    let status = command
        .status()
        .unwrap_or_else(|err| panic!("failed to run {action}: {err}"));
    assert!(status.success(), "{action} failed with status {status}");
}

fn find_mimalloc_shared_lib(build_dir: &Path) -> Option<PathBuf> {
    let mut stack = vec![build_dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir).ok()?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if name == "libmimalloc.so" || name.starts_with("libmimalloc.so.") {
                return Some(path);
            }
        }
    }
    None
}

fn ensure_rust_mimalloc(root: &Path) -> PathBuf {
    if let Some(path) = env::var_os("MIMALLOC_RUST_LIB") {
        let path = PathBuf::from(path);
        assert!(
            path.exists(),
            "MIMALLOC_RUST_LIB does not exist: {}",
            path.display()
        );
        return path;
    }

    let lib = root.join("target/release/libmechanical_mimalloc.so");
    if !skip_build() {
        let mut command = Command::new("cargo");
        command.current_dir(root).arg("build").arg("--release");
        run_status(command, "cargo build --release for Rust mimalloc cdylib");
    }
    assert!(
        lib.exists(),
        "missing Rust mimalloc cdylib at {}; run `cargo build --release` or set MIMALLOC_RUST_LIB",
        lib.display()
    );
    lib
}

fn ensure_c_mimalloc(root: &Path) -> PathBuf {
    if let Some(path) = env::var_os("MIMALLOC_C_LIB") {
        let path = PathBuf::from(path);
        assert!(
            path.exists(),
            "MIMALLOC_C_LIB does not exist: {}",
            path.display()
        );
        return path;
    }

    let source_dir = root.join("mimalloc-3.3.2");
    assert!(
        source_dir.join("CMakeLists.txt").exists(),
        "missing mimalloc-3.3.2 source directory at {}",
        source_dir.display()
    );
    let build_dir = root.join("target/c-mimalloc-3.3.2");
    if find_mimalloc_shared_lib(&build_dir).is_none() && !skip_build() {
        let mut configure = Command::new("cmake");
        configure
            .current_dir(root)
            .arg("-S")
            .arg(&source_dir)
            .arg("-B")
            .arg(&build_dir)
            .arg("-DCMAKE_BUILD_TYPE=Release")
            .arg("-DMI_BUILD_SHARED=ON")
            .arg("-DMI_BUILD_STATIC=OFF")
            .arg("-DMI_BUILD_OBJECT=OFF")
            .arg("-DMI_BUILD_TESTS=OFF")
            .arg("-DMI_OVERRIDE=ON");
        run_status(configure, "configure C mimalloc 3.3.2");

        let mut build = Command::new("cmake");
        build
            .current_dir(root)
            .arg("--build")
            .arg(&build_dir)
            .arg("--config")
            .arg("Release")
            .arg("--parallel")
            .arg(logical_cpus().to_string());
        run_status(build, "build C mimalloc 3.3.2");
    }

    find_mimalloc_shared_lib(&build_dir).unwrap_or_else(|| {
        panic!(
            "could not find libmimalloc.so under {}; build it or set MIMALLOC_C_LIB",
            build_dir.display()
        )
    })
}

#[cfg(target_os = "linux")]
mod dylib {
    use super::*;

    const RTLD_NOW: c_int = 2;
    const RTLD_LOCAL: c_int = 0;

    unsafe extern "C" {
        fn dlopen(filename: *const c_char, flags: c_int) -> *mut c_void;
        fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
        fn dlerror() -> *const c_char;
    }

    fn last_error() -> String {
        unsafe {
            let err = dlerror();
            if err.is_null() {
                "unknown dynamic loader error".to_owned()
            } else {
                CStr::from_ptr(err).to_string_lossy().into_owned()
            }
        }
    }

    unsafe fn symbol(handle: *mut c_void, name: &CStr) -> *mut c_void {
        unsafe {
            let ptr = dlsym(handle, name.as_ptr());
            assert!(
                !ptr.is_null(),
                "failed to load symbol {}: {}",
                name.to_string_lossy(),
                last_error()
            );
            ptr
        }
    }

    pub fn load_allocator(path: &Path) -> DylibAllocator {
        let path = CString::new(path.as_os_str().as_encoded_bytes()).unwrap_or_else(|_| {
            panic!(
                "allocator library path contains an interior NUL byte: {}",
                path.display()
            )
        });
        unsafe {
            let handle = dlopen(path.as_ptr(), RTLD_NOW | RTLD_LOCAL);
            assert!(
                !handle.is_null(),
                "failed to dlopen C mimalloc: {}",
                last_error()
            );

            let malloc = symbol(handle, c"malloc");
            let calloc = symbol(handle, c"calloc");
            let realloc = symbol(handle, c"realloc");
            let free = symbol(handle, c"free");
            let memalign = symbol(handle, c"memalign");
            let new_array = symbol(handle, c"_Znam");
            let delete_array = symbol(handle, c"_ZdaPv");
            let delete_array_sized = symbol(handle, c"_ZdaPvm");

            DylibAllocator {
                malloc: std::mem::transmute::<
                    *mut c_void,
                    unsafe extern "C" fn(usize) -> *mut c_void,
                >(malloc),
                calloc: std::mem::transmute::<
                    *mut c_void,
                    unsafe extern "C" fn(usize, usize) -> *mut c_void,
                >(calloc),
                realloc: std::mem::transmute::<
                    *mut c_void,
                    unsafe extern "C" fn(*mut c_void, usize) -> *mut c_void,
                >(realloc),
                free: std::mem::transmute::<*mut c_void, unsafe extern "C" fn(*mut c_void)>(free),
                memalign: std::mem::transmute::<
                    *mut c_void,
                    unsafe extern "C" fn(usize, usize) -> *mut c_void,
                >(memalign),
                new_array: std::mem::transmute::<
                    *mut c_void,
                    unsafe extern "C" fn(usize) -> *mut c_void,
                >(new_array),
                delete_array: std::mem::transmute::<*mut c_void, unsafe extern "C" fn(*mut c_void)>(
                    delete_array,
                ),
                delete_array_sized: std::mem::transmute::<
                    *mut c_void,
                    unsafe extern "C" fn(*mut c_void, usize),
                >(delete_array_sized),
            }
        }
    }
}

#[cfg(not(target_os = "linux"))]
mod dylib {
    use super::*;

    pub fn load_allocator(_path: &Path) -> DylibAllocator {
        panic!(
            "native mimalloc-bench Criterion ports currently support shared-library allocator comparison on Linux only"
        )
    }
}

fn rust_allocator(root: &Path) -> &'static DylibAllocator {
    static RUST_ALLOCATOR: OnceLock<DylibAllocator> = OnceLock::new();
    RUST_ALLOCATOR.get_or_init(|| {
        let path = ensure_rust_mimalloc(root);
        dylib::load_allocator(&path)
    })
}

fn c_allocator(root: &Path) -> &'static DylibAllocator {
    static C_ALLOCATOR: OnceLock<DylibAllocator> = OnceLock::new();
    C_ALLOCATOR.get_or_init(|| {
        let path = ensure_c_mimalloc(root);
        dylib::load_allocator(&path)
    })
}

fn allocators(root: &Path) -> Vec<BenchAllocator> {
    let selected = csv_env("MIMALLOC_BENCH_ALLOCATORS");
    let selected = if selected.is_empty() {
        vec!["rust".to_owned(), "c".to_owned()]
    } else {
        selected
    };

    selected
        .into_iter()
        .map(|name| match name.as_str() {
            "rust" => BenchAllocator::Rust(rust_allocator(root)),
            "c" | "c-mimalloc" | "c-mimalloc-3.3.2" => BenchAllocator::C(c_allocator(root)),
            other => panic!("unknown MIMALLOC_BENCH_ALLOCATORS entry `{other}`"),
        })
        .collect()
}

fn scenarios() -> Vec<Scenario> {
    let procs = logical_cpus();
    let mut scenarios = vec![
        Scenario {
            name: "glibc-simple",
            run: glibc_simple::run,
        },
        Scenario {
            name: "glibc-thread",
            run: glibc_thread::run,
        },
        Scenario {
            name: "malloc-large",
            run: malloc_large::run,
        },
        Scenario {
            name: "malloc-large-old",
            run: malloc_large_old::run,
        },
        Scenario {
            name: "cache-scratch1",
            run: cache_workloads::run_scratch1,
        },
        Scenario {
            name: "cache-thrash1",
            run: cache_workloads::run_thrash1,
        },
        Scenario {
            name: "xmalloc-testN",
            run: xmalloc_test::run,
        },
        Scenario {
            name: "mstressN",
            run: mstress::run,
        },
        Scenario {
            name: "sh6benchN",
            run: sh6bench::run,
        },
        Scenario {
            name: "sh8benchN",
            run: sh8bench::run,
        },
        Scenario {
            name: "alloc-test1",
            run: alloc_test::run_one,
        },
        Scenario {
            name: "alloc-testN",
            run: alloc_test::run_n,
        },
        Scenario {
            name: "larsonN",
            run: larson::run_unsized,
        },
        Scenario {
            name: "larsonN-sized",
            run: larson::run_sized,
        },
        Scenario {
            name: "rptestN",
            run: rptest::run,
        },
        Scenario {
            name: "barnes",
            run: barnes::run,
        },
        Scenario {
            name: "mleak10",
            run: |alloc| unsafe { mleak::run(alloc, 5) },
        },
        Scenario {
            name: "mleak100",
            run: |alloc| unsafe { mleak::run(alloc, 50) },
        },
    ];

    if procs != 1 {
        scenarios.push(Scenario {
            name: "cache-scratchN",
            run: cache_workloads::run_scratch_n,
        });
        scenarios.push(Scenario {
            name: "cache-thrashN",
            run: cache_workloads::run_thrash_n,
        });
    }

    let selected = csv_env("MIMALLOC_BENCH_SCENARIOS");
    if selected.is_empty() {
        scenarios
    } else {
        scenarios
            .into_iter()
            .filter(|scenario| selected.iter().any(|name| name == scenario.name))
            .collect()
    }
}

mod pthread {
    use super::*;

    pub type PthreadT = usize;

    unsafe extern "C" {
        pub fn pthread_create(
            thread: *mut PthreadT,
            attr: *const c_void,
            start: extern "C" fn(*mut c_void) -> *mut c_void,
            arg: *mut c_void,
        ) -> c_int;
        pub fn pthread_join(thread: PthreadT, retval: *mut *mut c_void) -> c_int;
        pub fn pthread_setconcurrency(new_level: c_int) -> c_int;
    }
}

struct Mt19937 {
    state: [u32; 624],
    index: usize,
}

impl Mt19937 {
    fn new(seed: u32) -> Self {
        let mut state = [0; 624];
        state[0] = seed;
        for i in 1..624 {
            state[i] = 1812433253u32
                .wrapping_mul(state[i - 1] ^ (state[i - 1] >> 30))
                .wrapping_add(i as u32);
        }
        Self { state, index: 624 }
    }

    fn twist(&mut self) {
        const UPPER_MASK: u32 = 0x8000_0000;
        const LOWER_MASK: u32 = 0x7fff_ffff;
        const MATRIX_A: u32 = 0x9908_b0df;

        for i in 0..624 {
            let x = (self.state[i] & UPPER_MASK) | (self.state[(i + 1) % 624] & LOWER_MASK);
            let mut xa = x >> 1;
            if (x & 1) != 0 {
                xa ^= MATRIX_A;
            }
            self.state[i] = self.state[(i + 397) % 624] ^ xa;
        }
        self.index = 0;
    }

    fn next_u32(&mut self) -> u32 {
        if self.index >= 624 {
            self.twist();
        }
        let mut y = self.state[self.index];
        self.index += 1;
        y ^= y >> 11;
        y ^= (y << 7) & 0x9d2c_5680;
        y ^= (y << 15) & 0xefc6_0000;
        y ^= y >> 18;
        y
    }

    fn range_inclusive(&mut self, min: usize, max: usize) -> usize {
        let urange = (max - min) as u64;
        let urngrange = u32::MAX as u64;
        assert!(
            urange <= urngrange,
            "malloc-large distribution range exceeds mt19937 output range"
        );
        if urngrange == urange {
            return min + self.next_u32() as usize;
        }

        let uerange = urange + 1;
        let scaling = urngrange / uerange;
        let past = uerange * scaling;
        loop {
            let ret = self.next_u32() as u64;
            if ret < past {
                return min + (ret / scaling) as usize;
            }
        }
    }
}

struct Lran2 {
    x: i64,
    y: i64,
    v: [i64; 97],
}

impl Lran2 {
    fn new(seed: i64) -> Self {
        const LRAN2_MAX: i64 = 714_025;
        const IA: i64 = 1366;
        const IC: i64 = 150_889;

        let mut v = [0; 97];
        let mut x = (IC - seed) % LRAN2_MAX;
        if x < 0 {
            x = -x;
        }
        for item in &mut v {
            x = (IA * x + IC) % LRAN2_MAX;
            *item = x;
        }
        x = (IA * x + IC) % LRAN2_MAX;
        Self { x, y: x, v }
    }

    fn next(&mut self) -> i64 {
        const LRAN2_MAX: i64 = 714_025;
        const IA: i64 = 1366;
        const IC: i64 = 150_889;

        let j = (self.y % 97) as usize;
        self.y = self.v[j];
        self.x = (IA * self.x + IC) % LRAN2_MAX;
        self.v[j] = self.x;
        self.y
    }
}

mod glibc_simple {
    use super::*;

    const NUM_ITERS: usize = 2_000_000;
    const MAX_ALLOCS: usize = 1600;
    const ALLOCS: [usize; 4] = [25, 100, 400, MAX_ALLOCS];

    struct ThreadState {
        alloc: BenchAllocator,
        arr: *mut *mut u8,
        size: usize,
    }

    unsafe fn do_benchmark(
        alloc: BenchAllocator,
        iters: usize,
        size: usize,
        n: usize,
        arr: *mut *mut u8,
    ) {
        unsafe {
            for _ in 0..iters {
                for i in 0..n {
                    let p = alloc.malloc(size);
                    assert!(!p.is_null(), "allocation failed in glibc-simple");
                    for g in 0..size {
                        p.add(g).write_volatile(g as u8);
                    }
                    *arr.add(i) = p;
                }

                for i in 0..(n / 2) {
                    alloc.free(*arr.add(i));
                }

                for i in ((n / 2)..n).rev() {
                    alloc.free(*arr.add(i));
                }
            }
        }
    }

    unsafe fn bench_size(alloc: BenchAllocator, size: usize) {
        unsafe {
            let arr_bytes = MAX_ALLOCS * std::mem::size_of::<*mut u8>();
            let arr = alloc.malloc(arr_bytes).cast::<*mut u8>();
            assert!(
                !arr.is_null(),
                "pointer array allocation failed in glibc-simple"
            );

            for &n in &ALLOCS {
                do_benchmark(alloc, NUM_ITERS / n, size, n, arr);
            }

            for &n in &ALLOCS {
                do_benchmark(alloc, NUM_ITERS / n, size, n, arr);
            }

            let mut state = ThreadState { alloc, arr, size };
            let mut thread = 0;
            let rc = pthread::pthread_create(
                &mut thread,
                ptr::null(),
                thread_entry,
                (&mut state as *mut ThreadState).cast(),
            );
            assert_eq!(rc, 0, "pthread_create failed in glibc-simple: {rc}");
            let rc = pthread::pthread_join(thread, ptr::null_mut());
            assert_eq!(rc, 0, "pthread_join failed in glibc-simple: {rc}");

            for &n in &ALLOCS {
                do_benchmark(alloc, NUM_ITERS / n, size, n, arr);
            }

            alloc.free(arr.cast());
        }
    }

    extern "C" fn thread_entry(arg: *mut c_void) -> *mut c_void {
        unsafe {
            let state = &mut *arg.cast::<ThreadState>();
            for &n in &ALLOCS {
                do_benchmark(state.alloc, NUM_ITERS / n, state.size, n, state.arr);
            }
            ptr::null_mut()
        }
    }

    pub unsafe fn run(alloc: BenchAllocator) -> Duration {
        unsafe {
            let start = Instant::now();
            bench_size(alloc, 16);
            bench_size(alloc, 32);
            bench_size(alloc, 64);
            start.elapsed()
        }
    }
}

mod glibc_thread {
    use super::*;

    const BENCHMARK_DURATION_SECS: c_uint = 2;
    const RAND_MAX_VALUE: f32 = 2_147_483_647.0;
    const WORKING_SET_SIZE: usize = 1024;
    const MIN_ALLOCATION_SIZE: f32 = 4.0;
    const MAX_ALLOCATION_SIZE: f32 = 32_768.0;
    const NUM_BLOCK_SIZES: usize = 8000;
    const NUM_OFFSETS: usize = WORKING_SET_SIZE * 4;

    static TIMEOUT: AtomicBool = AtomicBool::new(false);

    struct RandomValues {
        block_sizes: [usize; NUM_BLOCK_SIZES],
        offsets: [usize; NUM_OFFSETS],
    }

    struct ThreadArgs {
        alloc: BenchAllocator,
        random: *const RandomValues,
        working_set: *mut *mut u8,
        iters: usize,
    }

    unsafe extern "C" {
        fn signal(signum: c_int, handler: extern "C" fn(c_int)) -> usize;
        fn alarm(seconds: c_uint) -> c_uint;
        fn srand(seed: c_uint);
        fn rand() -> c_int;
    }

    extern "C" fn alarm_handler(_: c_int) {
        TIMEOUT.store(true, Ordering::SeqCst);
    }

    fn get_block_size(rand_data: c_int) -> usize {
        let exponent = -2.0f32;
        let min_pow = MIN_ALLOCATION_SIZE.powf(exponent + 1.0);
        let max_pow = MAX_ALLOCATION_SIZE.powf(exponent + 1.0);
        let r = rand_data as f32 / RAND_MAX_VALUE;
        ((max_pow - min_pow) * r + min_pow).powf(1.0 / (exponent + 1.0)) as usize
    }

    unsafe fn init_random_values() -> RandomValues {
        unsafe {
            srand(1);
            let mut block_sizes = [0; NUM_BLOCK_SIZES];
            let mut offsets = [0; NUM_OFFSETS];
            for size in &mut block_sizes {
                *size = get_block_size(rand());
            }
            for offset in &mut offsets {
                *offset = rand() as usize % WORKING_SET_SIZE;
            }
            RandomValues {
                block_sizes,
                offsets,
            }
        }
    }

    fn next_index(state: &mut usize, limit: usize) -> usize {
        if *state >= limit - 1 {
            *state = 0;
        } else {
            *state += 1;
        }
        *state
    }

    unsafe fn malloc_benchmark_loop(
        alloc: BenchAllocator,
        random: &RandomValues,
        ptr_arr: *mut *mut u8,
    ) -> usize {
        unsafe {
            let mut offset_state = 0;
            let mut block_state = 0;
            let mut iters = 0;

            while !TIMEOUT.load(Ordering::SeqCst) {
                let next_idx = random.offsets[next_index(&mut offset_state, NUM_OFFSETS)];
                let next_block = random.block_sizes[next_index(&mut block_state, NUM_BLOCK_SIZES)];

                alloc.free(*ptr_arr.add(next_idx));

                let p = alloc.malloc(next_block);
                assert!(!p.is_null(), "allocation failed in glibc-thread");
                for i in 0..next_block {
                    p.add(i).write_volatile(i as u8);
                }
                *ptr_arr.add(next_idx) = p;
                iters += 1;
            }

            for i in 0..WORKING_SET_SIZE {
                alloc.free(*ptr_arr.add(i));
                *ptr_arr.add(i) = ptr::null_mut();
            }
            iters
        }
    }

    extern "C" fn thread_entry(arg: *mut c_void) -> *mut c_void {
        unsafe {
            let args = &mut *arg.cast::<ThreadArgs>();
            args.iters = malloc_benchmark_loop(args.alloc, &*args.random, args.working_set);
        }
        ptr::null_mut()
    }

    unsafe fn do_benchmark(
        alloc: BenchAllocator,
        num_threads: usize,
        random: &RandomValues,
    ) -> usize {
        unsafe {
            if num_threads == 1 {
                let mut working_set = Box::new([ptr::null_mut(); WORKING_SET_SIZE]);
                return malloc_benchmark_loop(alloc, random, working_set.as_mut_ptr());
            }

            let mut working_sets = Vec::with_capacity(num_threads);
            let mut args = Vec::with_capacity(num_threads);
            let mut threads = Vec::with_capacity(num_threads);

            for _ in 0..num_threads {
                working_sets.push(Box::new([ptr::null_mut(); WORKING_SET_SIZE]));
                args.push(ThreadArgs {
                    alloc,
                    random,
                    working_set: ptr::null_mut(),
                    iters: 0,
                });
                threads.push(0);
            }

            for i in 0..num_threads {
                args[i].working_set = working_sets[i].as_mut_ptr();
                let rc = pthread::pthread_create(
                    &mut threads[i],
                    ptr::null(),
                    thread_entry,
                    (&mut args[i] as *mut ThreadArgs).cast(),
                );
                assert_eq!(rc, 0, "pthread_create failed in glibc-thread: {rc}");
            }

            let mut iters = 0;
            for i in 0..num_threads {
                let rc = pthread::pthread_join(threads[i], ptr::null_mut());
                assert_eq!(rc, 0, "pthread_join failed in glibc-thread: {rc}");
                iters += args[i].iters;
            }
            iters
        }
    }

    pub unsafe fn run(alloc: BenchAllocator) -> Duration {
        unsafe {
            let random = init_random_values();
            TIMEOUT.store(false, Ordering::SeqCst);
            signal(14, alarm_handler);
            alarm(BENCHMARK_DURATION_SECS);
            let iters = do_benchmark(alloc, logical_cpus(), &random);
            alarm(0);
            assert!(iters > 0, "glibc-thread produced zero iterations");
            Duration::from_secs_f64(1_000_000_000.0 / iters as f64)
        }
    }
}

mod malloc_large {
    use super::*;

    const NUM_BUFFERS: usize = 20;
    const MIN_BUFFER_SIZE: usize = 5 * 1024 * 1024;
    const MAX_BUFFER_SIZE: usize = 25 * 1024 * 1024;
    const NUM_ITERATIONS: usize = 2000;

    pub unsafe fn run(alloc: BenchAllocator) -> Duration {
        unsafe {
            let mut rng = Mt19937::new(42);
            let mut buffers = [ptr::null_mut(); NUM_BUFFERS];
            let start = Instant::now();
            for _ in 0..NUM_ITERATIONS {
                let buffer_idx = rng.range_inclusive(0, NUM_BUFFERS - 1);
                let new_size = rng.range_inclusive(MIN_BUFFER_SIZE, MAX_BUFFER_SIZE);
                let new_buffer = alloc.malloc(new_size);
                assert!(!new_buffer.is_null(), "allocation failed in malloc-large");
                ptr::write_bytes(new_buffer, 0, new_size);
                let old = std::mem::replace(&mut buffers[buffer_idx], new_buffer);
                alloc.free(old);
            }
            let elapsed = start.elapsed();
            for buffer in buffers {
                alloc.free(buffer);
            }
            elapsed
        }
    }
}

mod malloc_large_old {
    use super::*;

    const N: usize = 5000;
    const SIZE: usize = 1 << 21;

    struct ThreadArg {
        alloc: BenchAllocator,
    }

    extern "C" fn thread_entry(arg: *mut c_void) -> *mut c_void {
        unsafe {
            let alloc = (*arg.cast::<ThreadArg>()).alloc;
            for _ in 0..N {
                let p = alloc.malloc(SIZE);
                assert!(!p.is_null(), "allocation failed in malloc-large-old");
                for k in 0..SIZE {
                    p.add(k).write(k as u8);
                }
                alloc.free(p);
            }
        }
        ptr::null_mut()
    }

    pub unsafe fn run(alloc: BenchAllocator) -> Duration {
        unsafe {
            let start = Instant::now();
            let mut thread = 0;
            let mut arg = ThreadArg { alloc };
            let rc = pthread::pthread_create(
                &mut thread,
                ptr::null(),
                thread_entry,
                (&mut arg as *mut ThreadArg).cast(),
            );
            assert_eq!(rc, 0, "pthread_create failed in malloc-large-old: {rc}");
            let rc = pthread::pthread_join(thread, ptr::null_mut());
            assert_eq!(rc, 0, "pthread_join failed in malloc-large-old: {rc}");
            start.elapsed()
        }
    }
}

mod cache_workloads {
    use super::*;

    const ITERATIONS: usize = 1000;
    const OBJ_SIZE: usize = 1;
    const REPETITIONS: usize = 2_000_000;

    struct WorkerArg {
        alloc: BenchAllocator,
        object: *mut u8,
        obj_size: usize,
        iterations: usize,
        repetitions: usize,
    }

    unsafe fn touch_object(obj: *mut u8, obj_size: usize, repetitions: usize) {
        unsafe {
            // The optimized upstream C++ binary keeps this touch loop on a volatile stack byte.
            let _ = obj;
            for _ in 0..repetitions {
                for k in 0..obj_size {
                    let mut ch = k as u8;
                    ptr::write_volatile(&mut ch, ch);
                    ch = ptr::read_volatile(&ch).wrapping_add(1);
                    ptr::write_volatile(&mut ch, ch);
                }
            }
        }
    }

    extern "C" fn scratch_worker(arg: *mut c_void) -> *mut c_void {
        unsafe {
            let w = &*(arg.cast::<WorkerArg>());
            w.alloc.free(w.object);
            for _ in 0..w.iterations {
                let obj = w.alloc.malloc(w.obj_size);
                assert!(!obj.is_null(), "allocation failed in cache-scratch");
                touch_object(obj, w.obj_size, w.repetitions);
                w.alloc.free(obj);
            }
        }
        ptr::null_mut()
    }

    extern "C" fn thrash_worker(arg: *mut c_void) -> *mut c_void {
        unsafe {
            let w = &*(arg.cast::<WorkerArg>());
            for _ in 0..w.iterations {
                let obj = w.alloc.malloc(w.obj_size);
                assert!(!obj.is_null(), "allocation failed in cache-thrash");
                touch_object(obj, w.obj_size, w.repetitions);
                w.alloc.free(obj);
            }
        }
        ptr::null_mut()
    }

    unsafe fn run_case(alloc: BenchAllocator, nthreads: usize, scratch: bool) -> Duration {
        unsafe {
            let _ = pthread::pthread_setconcurrency(logical_cpus() as c_int);
            let threads = alloc.calloc(nthreads, std::mem::size_of::<pthread::PthreadT>());
            assert!(
                !threads.is_null(),
                "thread handle allocation failed in cache workload"
            );
            let threads = threads.cast::<pthread::PthreadT>();

            let mut args = Vec::with_capacity(nthreads);
            let reps_per_thread = REPETITIONS / nthreads;

            for _ in 0..nthreads {
                let object = if scratch {
                    let p = alloc.malloc(OBJ_SIZE);
                    assert!(
                        !p.is_null(),
                        "initial object allocation failed in cache-scratch"
                    );
                    p
                } else {
                    ptr::null_mut()
                };
                args.push(WorkerArg {
                    alloc,
                    object,
                    obj_size: OBJ_SIZE,
                    iterations: ITERATIONS,
                    repetitions: reps_per_thread,
                });
            }

            let start = Instant::now();
            for i in 0..nthreads {
                let entry = if scratch {
                    scratch_worker
                } else {
                    thrash_worker
                };
                let rc = pthread::pthread_create(
                    threads.add(i),
                    ptr::null(),
                    entry,
                    (&mut args[i] as *mut WorkerArg).cast(),
                );
                assert_eq!(rc, 0, "pthread_create failed in cache workload: {rc}");
            }

            for i in 0..nthreads {
                let rc = pthread::pthread_join(*threads.add(i), ptr::null_mut());
                assert_eq!(rc, 0, "pthread_join failed in cache workload: {rc}");
            }
            let elapsed = start.elapsed();

            alloc.free(threads.cast());
            elapsed
        }
    }

    pub unsafe fn run_scratch1(alloc: BenchAllocator) -> Duration {
        unsafe { run_case(alloc, 1, true) }
    }

    pub unsafe fn run_scratch_n(alloc: BenchAllocator) -> Duration {
        unsafe { run_case(alloc, logical_cpus(), true) }
    }

    pub unsafe fn run_thrash1(alloc: BenchAllocator) -> Duration {
        unsafe { run_case(alloc, 1, false) }
    }

    pub unsafe fn run_thrash_n(alloc: BenchAllocator) -> Duration {
        unsafe { run_case(alloc, logical_cpus(), false) }
    }
}

mod xmalloc_test {
    use super::*;

    const OBJECT_SIZE: usize = 64;
    const RUN_TIME: Duration = Duration::from_secs(5);
    const OBJECTS_PER_BATCH: usize = 4096;
    const BATCH_COUNT_LIMIT: usize = 100;
    const POSSIBLE_SIZES: [usize; 17] = [
        8,
        12,
        16,
        24,
        32,
        48,
        64,
        96,
        128,
        192,
        256,
        (256 * 3) / 2,
        512,
        (512 * 3) / 2,
        1024,
        (1024 * 3) / 2,
        2048,
    ];

    #[repr(C)]
    struct Batch {
        next_batch: *mut Batch,
        objects: [*mut u8; OBJECTS_PER_BATCH],
    }

    struct Queue {
        batches: *mut Batch,
        batch_count: usize,
    }

    struct State {
        alloc: BenchAllocator,
        done: AtomicBool,
        queue: Mutex<Queue>,
        empty_cv: Condvar,
        full_cv: Condvar,
        counters: Vec<AtomicUsize>,
    }

    struct ThreadArg {
        id: usize,
        state: *const State,
    }

    unsafe fn free_batch(alloc: BenchAllocator, batch: *mut Batch) {
        unsafe {
            for &object in &(*batch).objects {
                alloc.free(object);
            }
            alloc.free(batch.cast());
        }
    }

    fn enqueue_batch(state: &State, batch: *mut Batch) {
        let mut queue = state.queue.lock().unwrap();
        while queue.batch_count >= BATCH_COUNT_LIMIT && !state.done.load(Ordering::Acquire) {
            queue = state.full_cv.wait(queue).unwrap();
        }
        unsafe {
            (*batch).next_batch = queue.batches;
        }
        queue.batches = batch;
        queue.batch_count += 1;
        state.empty_cv.notify_one();
    }

    fn dequeue_batch(state: &State) -> *mut Batch {
        let mut queue = state.queue.lock().unwrap();
        while queue.batches.is_null() && !state.done.load(Ordering::Acquire) {
            queue = state.empty_cv.wait(queue).unwrap();
        }
        let result = queue.batches;
        if !result.is_null() {
            unsafe {
                queue.batches = (*result).next_batch;
            }
            queue.batch_count -= 1;
            state.full_cv.notify_one();
        }
        result
    }

    extern "C" fn allocator_thread(arg: *mut c_void) -> *mut c_void {
        unsafe {
            let thread_arg = &*arg.cast::<ThreadArg>();
            let state = &*thread_arg.state;
            let mut rng = Lran2::new(thread_arg.id as i64);

            while !state.done.load(Ordering::Acquire) {
                let batch = state
                    .alloc
                    .malloc(std::mem::size_of::<Batch>())
                    .cast::<Batch>();
                assert!(!batch.is_null(), "batch allocation failed in xmalloc-testN");
                (*batch).next_batch = ptr::null_mut();
                for i in 0..OBJECTS_PER_BATCH {
                    let size = if OBJECT_SIZE > 0 {
                        OBJECT_SIZE
                    } else {
                        POSSIBLE_SIZES[rng.next() as usize % POSSIBLE_SIZES.len()]
                    };
                    let object = state.alloc.malloc(size);
                    assert!(
                        !object.is_null(),
                        "object allocation failed in xmalloc-testN"
                    );
                    ptr::write_bytes(object, i as u8, size.min(128));
                    (*batch).objects[i] = object;
                }
                enqueue_batch(state, batch);
            }
        }
        ptr::null_mut()
    }

    extern "C" fn releaser_thread(arg: *mut c_void) -> *mut c_void {
        unsafe {
            let thread_arg = &*arg.cast::<ThreadArg>();
            let state = &*thread_arg.state;

            while !state.done.load(Ordering::Acquire) {
                let batch = dequeue_batch(state);
                if !batch.is_null() {
                    free_batch(state.alloc, batch);
                }
                state.counters[thread_arg.id].fetch_add(OBJECTS_PER_BATCH, Ordering::Relaxed);
            }
        }
        ptr::null_mut()
    }

    unsafe fn cleanup_remaining(state: &State) {
        unsafe {
            loop {
                let batch = {
                    let mut queue = state.queue.lock().unwrap();
                    let batch = queue.batches;
                    if batch.is_null() {
                        ptr::null_mut()
                    } else {
                        queue.batches = (*batch).next_batch;
                        queue.batch_count -= 1;
                        batch
                    }
                };
                if batch.is_null() {
                    break;
                }
                free_batch(state.alloc, batch);
            }
        }
    }

    pub unsafe fn run(alloc: BenchAllocator) -> Duration {
        unsafe {
            let num_workers = logical_cpus();
            let mut threads = vec![0; num_workers * 2];
            let mut args = Vec::with_capacity(num_workers);
            let state = State {
                alloc,
                done: AtomicBool::new(false),
                queue: Mutex::new(Queue {
                    batches: ptr::null_mut(),
                    batch_count: 0,
                }),
                empty_cv: Condvar::new(),
                full_cv: Condvar::new(),
                counters: (0..num_workers).map(|_| AtomicUsize::new(0)).collect(),
            };

            for i in 0..num_workers {
                args.push(ThreadArg {
                    id: i,
                    state: &state,
                });
            }

            let begin = Instant::now();
            for i in 0..num_workers {
                let rc = pthread::pthread_create(
                    &mut threads[i * 2],
                    ptr::null(),
                    releaser_thread,
                    (&mut args[i] as *mut ThreadArg).cast(),
                );
                assert_eq!(
                    rc, 0,
                    "pthread_create releaser failed in xmalloc-testN: {rc}"
                );

                let rc = pthread::pthread_create(
                    &mut threads[i * 2 + 1],
                    ptr::null(),
                    allocator_thread,
                    (&mut args[i] as *mut ThreadArg).cast(),
                );
                assert_eq!(
                    rc, 0,
                    "pthread_create allocator failed in xmalloc-testN: {rc}"
                );
            }

            while begin.elapsed() <= RUN_TIME {
                std::thread::sleep(Duration::from_millis(1));
            }

            state.done.store(true, Ordering::Release);
            state.empty_cv.notify_all();
            state.full_cv.notify_all();

            for thread in threads {
                let rc = pthread::pthread_join(thread, ptr::null_mut());
                assert_eq!(rc, 0, "pthread_join failed in xmalloc-testN: {rc}");
            }

            let elapsed = begin.elapsed().as_secs_f64();
            let total: usize = state
                .counters
                .iter()
                .map(|counter| counter.load(Ordering::Relaxed))
                .sum();
            cleanup_remaining(&state);

            let mfree_per_sec = (total as f64 / elapsed) * 1e-6;
            assert!(mfree_per_sec > 0.0, "xmalloc-testN produced zero free rate");
            Duration::from_secs_f64(100.0 / mfree_per_sec)
        }
    }
}

mod mstress {
    use super::*;

    const SCALE: usize = 50;
    const ITER: usize = 25;
    const TRANSFERS: usize = 1000;
    const COOKIE: usize = if usize::BITS == 64 {
        0xbf58_476d_1ce4_e5b9
    } else {
        0x1ce4_e5b9
    };

    struct State {
        alloc: BenchAllocator,
        transfers: Vec<AtomicPtr<u8>>,
    }

    struct ThreadArg {
        state: *const State,
        tid: usize,
    }

    fn pick(r: &mut usize) -> usize {
        let mut x = *r;
        if usize::BITS > 32 {
            x ^= x >> 30;
            x = x.wrapping_mul(0xbf58_476d_1ce4_e5b9);
            x ^= x >> 27;
            x = x.wrapping_mul(0x94d0_49bb_1331_11eb);
            x ^= x >> 31;
        } else {
            x ^= x >> 16;
            x = x.wrapping_mul(0x7feb_352d);
            x ^= x >> 15;
            x = x.wrapping_mul(0x846c_a68b);
            x ^= x >> 16;
        }
        *r = x;
        x
    }

    fn chance(percent: usize, r: &mut usize) -> bool {
        pick(r) % 100 <= percent
    }

    unsafe fn alloc_items(alloc: BenchAllocator, mut items: usize, r: &mut usize) -> *mut u8 {
        unsafe {
            if chance(1, r) {
                if chance(1, r) {
                    items *= 10000;
                } else if chance(10, r) {
                    items *= 1000;
                } else {
                    items *= 100;
                }
            }
            if items == 40 {
                items += 1;
            }

            let bytes = items * std::mem::size_of::<usize>();
            let p = alloc.malloc(bytes).cast::<usize>();
            if !p.is_null() {
                for i in 0..items {
                    p.add(i).write((items - i) ^ COOKIE);
                }
            }
            p.cast()
        }
    }

    unsafe fn free_items(alloc: BenchAllocator, p: *mut u8) {
        unsafe {
            if !p.is_null() {
                let q = p.cast::<usize>();
                let items = q.read() ^ COOKIE;
                for i in 0..items {
                    assert_eq!(
                        q.add(i).read() ^ COOKIE,
                        items - i,
                        "memory corruption in mstressN at block {p:p}, item {i}"
                    );
                }
            }
            alloc.free(p);
        }
    }

    unsafe fn stress(state: &State, tid: usize) {
        unsafe {
            let alloc = state.alloc;
            let mut r = (tid + 1) * 43;
            let max_item_shift = 5;
            let max_item_retained_shift = max_item_shift + 2;
            let mut allocs = 100 * SCALE * (tid % 8 + 1);
            let mut retain = allocs / 2;
            let retain_cap = retain;
            let mut data = ptr::null_mut::<*mut u8>();
            let mut data_size = 0;
            let mut data_top = 0;
            let retained = alloc
                .malloc(retain_cap * std::mem::size_of::<*mut u8>())
                .cast::<*mut u8>();
            assert!(
                !retained.is_null(),
                "retained pointer allocation failed in mstressN"
            );
            let mut retain_top = 0;

            while allocs > 0 || retain > 0 {
                if retain == 0 || (chance(50, &mut r) && allocs > 0) {
                    allocs -= 1;
                    if data_top >= data_size {
                        data_size += 100000;
                        data = alloc
                            .realloc(data.cast(), data_size * std::mem::size_of::<*mut u8>())
                            .cast::<*mut u8>();
                        assert!(
                            !data.is_null(),
                            "data pointer allocation failed in mstressN"
                        );
                    }
                    data.add(data_top).write(alloc_items(
                        alloc,
                        1usize << (pick(&mut r) % max_item_shift),
                        &mut r,
                    ));
                    data_top += 1;
                } else {
                    retained.add(retain_top).write(alloc_items(
                        alloc,
                        1usize << (pick(&mut r) % max_item_retained_shift),
                        &mut r,
                    ));
                    retain_top += 1;
                    retain -= 1;
                }

                if chance(66, &mut r) && data_top > 0 {
                    let idx = pick(&mut r) % data_top;
                    free_items(alloc, data.add(idx).read());
                    data.add(idx).write(ptr::null_mut());
                }

                if chance(25, &mut r) && data_top > 0 {
                    let data_idx = pick(&mut r) % data_top;
                    let transfer_idx = pick(&mut r) % TRANSFERS;
                    let p = data.add(data_idx).read();
                    let q = state.transfers[transfer_idx].swap(p, Ordering::Acquire);
                    data.add(data_idx).write(q);
                }
            }

            for i in 0..retain_top {
                free_items(alloc, retained.add(i).read());
            }
            for i in 0..data_top {
                free_items(alloc, data.add(i).read());
            }
            alloc.free(retained.cast());
            alloc.free(data.cast());
        }
    }

    extern "C" fn thread_entry(arg: *mut c_void) -> *mut c_void {
        unsafe {
            let arg = &*arg.cast::<ThreadArg>();
            stress(&*arg.state, arg.tid);
        }
        ptr::null_mut()
    }

    unsafe fn run_os_threads(state: &State, nthreads: usize) {
        unsafe {
            let threads = state
                .alloc
                .malloc(nthreads * std::mem::size_of::<pthread::PthreadT>())
                .cast::<pthread::PthreadT>();
            assert!(!threads.is_null(), "thread allocation failed in mstressN");
            ptr::write_bytes(threads, 0, nthreads);

            let mut args = Vec::with_capacity(nthreads);
            for tid in 0..nthreads {
                args.push(ThreadArg { state, tid });
            }

            for tid in 1..nthreads {
                let rc = pthread::pthread_create(
                    threads.add(tid),
                    ptr::null(),
                    thread_entry,
                    (&mut args[tid] as *mut ThreadArg).cast(),
                );
                assert_eq!(rc, 0, "pthread_create failed in mstressN: {rc}");
            }

            stress(state, 0);

            for tid in 1..nthreads {
                let rc = pthread::pthread_join(*threads.add(tid), ptr::null_mut());
                assert_eq!(rc, 0, "pthread_join failed in mstressN: {rc}");
            }

            state.alloc.free(threads.cast());
        }
    }

    pub unsafe fn run(alloc: BenchAllocator) -> Duration {
        unsafe {
            let nthreads = logical_cpus();
            let state = State {
                alloc,
                transfers: (0..TRANSFERS)
                    .map(|_| AtomicPtr::new(ptr::null_mut()))
                    .collect(),
            };
            let mut r = 43 * 43;

            let start = Instant::now();
            for n in 0..ITER {
                run_os_threads(&state, nthreads);
                for transfer in &state.transfers {
                    if chance(50, &mut r) || n + 1 == ITER {
                        let p = transfer.swap(ptr::null_mut(), Ordering::Acquire);
                        free_items(alloc, p);
                    }
                }
            }
            start.elapsed()
        }
    }
}

mod sh6bench {
    use super::*;

    const CALL_COUNT: usize = 10_000;
    const MIN_BLOCK_SIZE: usize = 1;
    const MAX_BLOCK_SIZE: usize = 1000;

    struct ThreadArg {
        alloc: BenchAllocator,
        call_count: usize,
    }

    unsafe fn free_slot(alloc: BenchAllocator, slot: *mut *mut u8) {
        unsafe {
            let p = slot.read();
            if !p.is_null() {
                alloc.free(p);
                slot.write(ptr::null_mut());
            }
        }
    }

    unsafe fn do_bench(alloc: BenchAllocator, call_count: usize) {
        unsafe {
            let memory = alloc
                .calloc(call_count, std::mem::size_of::<*mut u8>())
                .cast::<*mut u8>();
            assert!(
                !memory.is_null(),
                "pointer array allocation failed in sh6benchN"
            );

            let mut mp = 0usize;
            let mut save_start = call_count;
            let mut save_end = call_count;

            for _ in 0..call_count {
                let mut size_base = MIN_BLOCK_SIZE;
                while size_base < MAX_BLOCK_SIZE {
                    let mut size = size_base;
                    while size != 0 {
                        let mut iterations = 1;
                        if size < 10000 {
                            iterations = 10;
                        }
                        if size < 1000 {
                            iterations *= 5;
                        }
                        if size < 100 {
                            iterations *= 5;
                        }

                        for _ in 0..iterations {
                            let p = alloc.malloc(size);
                            assert!(!p.is_null(), "allocation failed in sh6benchN");
                            p.write(0);
                            p.add(size - 1).write(0);
                            memory.add(mp).write(p);
                            mp += 1;

                            if mp == save_start {
                                mp = save_end;
                            }

                            if mp >= call_count {
                                mp = 0;
                                save_start = save_end;
                                if save_start >= call_count {
                                    save_start = 0;
                                }
                                save_end = save_start + (call_count / 5);
                                if save_end > call_count {
                                    save_end = call_count;
                                }

                                while mp < save_start {
                                    free_slot(alloc, memory.add(mp));
                                    mp += 1;
                                }

                                mp = call_count;
                                while mp > save_end {
                                    mp -= 1;
                                    free_slot(alloc, memory.add(mp));
                                }

                                mp = 0;
                            }
                        }

                        size /= 2;
                    }
                    size_base = size_base * 3 / 2 + 1;
                }
            }

            for i in 0..call_count {
                free_slot(alloc, memory.add(i));
            }
            alloc.free(memory.cast());
        }
    }

    extern "C" fn thread_entry(arg: *mut c_void) -> *mut c_void {
        unsafe {
            let arg = &*arg.cast::<ThreadArg>();
            do_bench(arg.alloc, arg.call_count);
        }
        ptr::null_mut()
    }

    pub unsafe fn run(alloc: BenchAllocator) -> Duration {
        unsafe {
            let nthreads = logical_cpus() * 2;
            let call_count = (CALL_COUNT / nthreads).max(1);
            let mut threads = vec![0; nthreads];
            let mut args = Vec::with_capacity(nthreads);
            for _ in 0..nthreads {
                args.push(ThreadArg { alloc, call_count });
            }

            let start = Instant::now();
            for i in 0..nthreads {
                let rc = pthread::pthread_create(
                    &mut threads[i],
                    ptr::null(),
                    thread_entry,
                    (&mut args[i] as *mut ThreadArg).cast(),
                );
                assert_eq!(rc, 0, "pthread_create failed in sh6benchN: {rc}");
            }
            for thread in threads {
                let rc = pthread::pthread_join(thread, ptr::null_mut());
                assert_eq!(rc, 0, "pthread_join failed in sh6benchN: {rc}");
            }
            start.elapsed()
        }
    }
}

mod sh8bench {
    use super::*;

    const ITERATIONS: usize = 700_000;
    const HEAP_SIZE: usize = 1_000_000;
    const FOREIGN_ALLOC_COUNT: usize = 10_000_000;
    const HISTOGRAM: &[(usize, usize)] = &[
        (8, 1000),
        (16, 5000),
        (48, 1000),
        (72, 100),
        (148, 100),
        (200, 100),
        (520, 10),
        (1056, 5),
        (4096, 3),
        (9162, 1),
        (34562, 1),
        (168524, 1),
    ];

    struct State {
        alloc: BenchAllocator,
        thread_count: usize,
        foreign_slots: *mut *mut u8,
        foreign_slot_count: usize,
    }

    struct ThreadArg {
        state: *const State,
        index: usize,
    }

    unsafe fn free_slot(alloc: BenchAllocator, slot: *mut *mut u8) {
        unsafe {
            let p = slot.read();
            if !p.is_null() {
                alloc.free(p);
                slot.write(ptr::null_mut());
            }
        }
    }

    unsafe fn do_bench(arg: &ThreadArg) {
        unsafe {
            let state = &*arg.state;
            let alloc = state.alloc;
            let foreign_slot_size = state.foreign_slot_count / state.thread_count;
            let foreign_frees_reset = foreign_slot_size / state.thread_count;
            let mut foreign_frees_left = foreign_slot_size;
            let mut foreign_frees_per_thread = foreign_frees_reset;
            let mut foreign_frees = state.foreign_slots.add(foreign_frees_reset * arg.index);

            let iterations = ITERATIONS / state.thread_count;
            let heap_size = HEAP_SIZE / state.thread_count;
            let memory = alloc
                .calloc(heap_size, std::mem::size_of::<*mut u8>())
                .cast::<*mut u8>();
            assert!(
                !memory.is_null(),
                "pointer array allocation failed in sh8benchN"
            );

            let mut mp = 0usize;
            let mut save_start = heap_size;
            let mut save_end = heap_size;
            let mut size_index = 0usize;

            for _ in 0..iterations {
                while foreign_frees_left != 0 {
                    let p = foreign_frees.read_volatile();
                    if p.is_null() {
                        break;
                    }
                    let owner = p.cast::<*mut *mut u8>().read();
                    assert_eq!(
                        owner, foreign_frees,
                        "foreign allocation ownership mismatch in sh8benchN"
                    );
                    alloc.free(p);
                    foreign_frees.write_volatile(ptr::null_mut());
                    foreign_frees = foreign_frees.add(1);
                    foreign_frees_left -= 1;
                    foreign_frees_per_thread -= 1;

                    if foreign_frees_per_thread == 0 {
                        foreign_frees_per_thread = foreign_frees_reset;
                        foreign_frees = foreign_frees.add(foreign_slot_size - foreign_frees_reset);
                    }
                }

                size_index += 1;
                if size_index == HISTOGRAM.len() {
                    size_index = 0;
                }
                let (size, mut count) = HISTOGRAM[size_index];

                while count != 0 {
                    count -= 1;
                    if mp == save_start {
                        mp = save_end;
                    }

                    if mp >= heap_size {
                        mp = 0;
                        save_start = save_end;
                        if save_start >= heap_size {
                            save_start = 0;
                        }
                        save_end = save_start + (heap_size / 5);
                        if save_end > heap_size {
                            save_end = heap_size;
                        }

                        while mp < save_start {
                            free_slot(alloc, memory.add(mp));
                            mp += 1;
                        }

                        mp = heap_size;
                        while mp > save_end {
                            mp -= 1;
                            free_slot(alloc, memory.add(mp));
                        }

                        mp = 0;
                    } else {
                        let p = alloc.malloc(size);
                        assert!(!p.is_null(), "allocation failed in sh8benchN");
                        p.write(0);
                        p.add(size - 1).write(0);
                        memory.add(mp).write(p);
                        mp += 1;
                    }
                }
            }

            for i in 0..heap_size {
                free_slot(alloc, memory.add(i));
            }
            alloc.free(memory.cast());
        }
    }

    extern "C" fn thread_entry(arg: *mut c_void) -> *mut c_void {
        unsafe {
            do_bench(&*arg.cast::<ThreadArg>());
        }
        ptr::null_mut()
    }

    unsafe fn allocate_foreign_slots(state: &State) {
        unsafe {
            for i in 0..state.foreign_slot_count {
                let slot = state.foreign_slots.add(i);
                let p = state.alloc.malloc(HISTOGRAM[0].0);
                if !p.is_null() {
                    p.cast::<*mut *mut u8>().write(slot);
                }
                slot.write_volatile(p);
            }
        }
    }

    unsafe fn cleanup_foreign_slots(state: &State) {
        unsafe {
            for i in 0..state.foreign_slot_count {
                let slot = state.foreign_slots.add(i);
                let p = slot.read_volatile();
                slot.write_volatile(ptr::null_mut());
                state.alloc.free(p);
            }
        }
    }

    pub unsafe fn run(alloc: BenchAllocator) -> Duration {
        unsafe {
            let thread_count = logical_cpus() * 2;
            let foreign_slot_count =
                (FOREIGN_ALLOC_COUNT / (thread_count * thread_count)) * thread_count * thread_count;
            let foreign_slots = alloc
                .calloc(foreign_slot_count, std::mem::size_of::<*mut u8>())
                .cast::<*mut u8>();
            assert!(
                !foreign_slots.is_null(),
                "foreign slot allocation failed in sh8benchN"
            );

            let state = State {
                alloc,
                thread_count,
                foreign_slots,
                foreign_slot_count,
            };
            let mut threads = vec![0; thread_count];
            let mut args = Vec::with_capacity(thread_count);
            for index in 0..thread_count {
                args.push(ThreadArg {
                    state: &state,
                    index,
                });
            }

            let start = Instant::now();
            for i in 0..thread_count {
                let rc = pthread::pthread_create(
                    &mut threads[i],
                    ptr::null(),
                    thread_entry,
                    (&mut args[i] as *mut ThreadArg).cast(),
                );
                assert_eq!(rc, 0, "pthread_create failed in sh8benchN: {rc}");
            }
            allocate_foreign_slots(&state);
            for thread in threads {
                let rc = pthread::pthread_join(thread, ptr::null_mut());
                assert_eq!(rc, 0, "pthread_join failed in sh8benchN: {rc}");
            }
            let elapsed = start.elapsed();
            cleanup_foreign_slots(&state);
            alloc.free(foreign_slots.cast());
            elapsed
        }
    }
}

mod alloc_test {
    use super::*;

    const ITER_COUNT: usize = 100_000_000;
    const MAX_ITEMS_TOTAL: usize = 1 << 18;
    const MAX_ITEM_SIZE_EXP: usize = 10;
    const RND_SEED: u64 = 41;
    const PARETO: [f64; 7] = [
        0.262144, 0.393216, 0.245760, 0.081920, 0.015360, 0.001536, 0.000064,
    ];

    #[repr(C)]
    struct TestBin {
        ptr: *mut u8,
        size: u32,
        reincarnation: u32,
    }

    struct Prng {
        seed: u64,
    }

    impl Prng {
        fn new(seed: u64) -> Self {
            Self { seed }
        }

        fn rng32(&mut self) -> u32 {
            let ret = self.seed.wrapping_mul(0xd989_bcac_c137_dcd5);
            self.seed ^= self.seed >> 11;
            self.seed ^= self.seed << 31;
            self.seed ^= self.seed >> 18;
            (ret >> 32) as u32
        }

        fn rng64(&mut self) -> u64 {
            let hi = self.rng32() as u64;
            (hi << 32) + self.rng32() as u64
        }
    }

    struct ParetoData {
        probability_ranges: [u32; 6],
        offsets: [u32; 8],
    }

    impl ParetoData {
        fn new(item_count: usize) -> Self {
            let mut probability_ranges = [0; 6];
            probability_ranges[0] = (u32::MAX as f64 * PARETO[0]) as u32;
            probability_ranges[5] = (u32::MAX as f64 * (1.0 - PARETO[6])) as u32;
            for i in 1..5 {
                probability_ranges[i] =
                    probability_ranges[i - 1].wrapping_add((u32::MAX as f64 * PARETO[i]) as u32);
            }

            let mut offsets = [0; 8];
            offsets[7] = item_count as u32;
            for i in 0..6 {
                offsets[i + 1] = offsets[i] + (item_count as f64 * PARETO[6 - i]) as u32;
            }

            Self {
                probability_ranges,
                offsets,
            }
        }

        fn pick(&self, rnum1: u32, rnum2: u32) -> usize {
            let mut idx = 6;
            if rnum1 < self.probability_ranges[0] {
                idx = 0;
            } else if rnum1 < self.probability_ranges[1] {
                idx = 1;
            } else if rnum1 < self.probability_ranges[2] {
                idx = 2;
            } else if rnum1 < self.probability_ranges[3] {
                idx = 3;
            } else if rnum1 < self.probability_ranges[4] {
                idx = 4;
            } else if rnum1 < self.probability_ranges[5] {
                idx = 5;
            }
            let range_size = self.offsets[idx + 1] - self.offsets[idx];
            self.offsets[idx] as usize + (rnum2 % range_size) as usize
        }
    }

    struct ThreadArg {
        alloc: BenchAllocator,
        thread_id: usize,
        iter_count: usize,
        max_items: usize,
        dummy: usize,
    }

    fn calc_size_with_stats_adjustment(mut rand_num: u64, max_size_exp: usize) -> usize {
        let max_size_exp = max_size_exp - 3;
        let stat_class_base = (rand_num & ((1u64 << max_size_exp) - 1)) + 1;
        rand_num >>= max_size_exp;
        let idx = stat_class_base.trailing_zeros() as usize + 2;
        let size_mask = (1u64 << idx) - 1;
        ((rand_num & size_mask) + 1 + (1u64 << idx)) as usize
    }

    unsafe fn alloc_item(alloc: BenchAllocator, size: usize) -> *mut u8 {
        unsafe {
            let p = alloc.malloc(size);
            assert!(!p.is_null(), "allocation failed in alloc-test");
            ptr::write_bytes(p, size as u8, size);
            p
        }
    }

    unsafe fn read_full(ptr: *mut u8, size: usize) -> usize {
        unsafe {
            let mut dummy = 0usize;
            let words = size / std::mem::size_of::<usize>();
            let word_ptr = ptr.cast::<usize>();
            for i in 0..words {
                dummy = dummy.wrapping_add(word_ptr.add(i).read_unaligned());
            }
            let tail = ptr.add(words * std::mem::size_of::<usize>());
            for i in 0..(size % std::mem::size_of::<usize>()) {
                dummy = dummy.wrapping_add(tail.add(i).read() as usize);
            }
            dummy
        }
    }

    unsafe fn random_pos_random_size(arg: &mut ThreadArg) {
        unsafe {
            let alloc = arg.alloc;
            let max_items = arg.max_items;
            let pareto = ParetoData::new(max_items);
            let base = alloc
                .malloc(max_items * std::mem::size_of::<TestBin>())
                .cast::<TestBin>();
            assert!(!base.is_null(), "slot allocation failed in alloc-test");
            ptr::write_bytes(
                base.cast::<u8>(),
                0,
                max_items * std::mem::size_of::<TestBin>(),
            );

            let mut rng = Prng::new(RND_SEED);
            let mut dummy = 0usize;

            for i in 0..(max_items / 32) {
                let rand_num = rng.rng32();
                for j in 0..32 {
                    if ((rand_num >> j) & 1) != 0 {
                        let size = calc_size_with_stats_adjustment(rng.rng64(), MAX_ITEM_SIZE_EXP);
                        let slot = base.add(i * 32 + j);
                        (*slot).size = size as u32;
                        (*slot).ptr = alloc_item(alloc, size);
                    }
                }
            }

            for _ in 0..32 {
                for _ in 0..(arg.iter_count >> 5) {
                    let idx = pareto.pick(rng.rng32(), rng.rng32());
                    let slot = base.add(idx);
                    let p = (*slot).ptr;
                    if !p.is_null() {
                        dummy = dummy.wrapping_add(read_full(p, (*slot).size as usize));
                        alloc.free(p);
                        (*slot).ptr = ptr::null_mut();
                    } else {
                        let size = calc_size_with_stats_adjustment(rng.rng64(), MAX_ITEM_SIZE_EXP);
                        (*slot).size = size as u32;
                        (*slot).ptr = alloc_item(alloc, size);
                    }
                }
            }

            for idx in 0..max_items {
                let slot = base.add(idx);
                let p = (*slot).ptr;
                if !p.is_null() {
                    dummy = dummy.wrapping_add(read_full(p, (*slot).size as usize));
                    alloc.free(p);
                }
            }

            alloc.free(base.cast());
            arg.dummy = dummy;
        }
    }

    extern "C" fn thread_entry(arg: *mut c_void) -> *mut c_void {
        unsafe {
            random_pos_random_size(&mut *arg.cast::<ThreadArg>());
        }
        ptr::null_mut()
    }

    unsafe fn run_threads(alloc: BenchAllocator, thread_count: usize) -> Duration {
        unsafe {
            let max_items = MAX_ITEMS_TOTAL / thread_count;
            let mut threads = vec![0; thread_count];
            let mut args = Vec::with_capacity(thread_count);
            for thread_id in 0..thread_count {
                args.push(ThreadArg {
                    alloc,
                    thread_id,
                    iter_count: ITER_COUNT,
                    max_items,
                    dummy: 0,
                });
            }

            let start = Instant::now();
            for i in 0..thread_count {
                let rc = pthread::pthread_create(
                    &mut threads[i],
                    ptr::null(),
                    thread_entry,
                    (&mut args[i] as *mut ThreadArg).cast(),
                );
                assert_eq!(rc, 0, "pthread_create failed in alloc-test: {rc}");
            }
            for thread in threads {
                let rc = pthread::pthread_join(thread, ptr::null_mut());
                assert_eq!(rc, 0, "pthread_join failed in alloc-test: {rc}");
            }
            let elapsed = start.elapsed();
            let dummy = args.iter().fold(0usize, |acc, arg| {
                acc.wrapping_add(arg.dummy).wrapping_add(arg.thread_id)
            });
            std::hint::black_box(dummy);
            elapsed
        }
    }

    pub unsafe fn run_one(alloc: BenchAllocator) -> Duration {
        unsafe { run_threads(alloc, 1) }
    }

    pub unsafe fn run_n(alloc: BenchAllocator) -> Duration {
        unsafe { run_threads(alloc, logical_cpus().min(16)) }
    }
}

mod larson {
    use super::*;

    const SLEEP_SECS: u64 = 5;
    const MIN_SIZE: usize = 8;
    const MAX_SIZE: usize = 1000;
    const CHUNKS_PER_THREAD: usize = 5000;
    const NUM_ROUNDS: usize = 100;
    const SEED: i64 = 4141;

    struct State {
        alloc: BenchAllocator,
        sized_delete: bool,
        stop: AtomicBool,
        blocks: Vec<AtomicPtr<u8>>,
        sizes: Vec<AtomicUsize>,
    }

    struct ThreadData {
        state: *const State,
        thread_no: usize,
        num_blocks: usize,
        array_start: usize,
        array_size: usize,
        min_size: usize,
        max_size: usize,
        rng: Lran2,
        allocs: usize,
        frees: usize,
    }

    unsafe fn larson_new(alloc: BenchAllocator, size: usize) -> *mut u8 {
        unsafe {
            let p = alloc.new_array(size);
            assert!(!p.is_null(), "allocation failed in larson");
            p
        }
    }

    unsafe fn larson_delete(alloc: BenchAllocator, p: *mut u8, size: usize, sized_delete: bool) {
        unsafe {
            if sized_delete {
                alloc.delete_array_sized(p, size);
            } else {
                alloc.delete_array(p);
            }
        }
    }

    unsafe fn warmup(state: &State, rng: &mut Lran2, start: usize, num_chunks: usize) {
        unsafe {
            for i in 0..num_chunks {
                let size = MIN_SIZE + rng.next() as usize % (MAX_SIZE - MIN_SIZE);
                let p = larson_new(state.alloc, size);
                state.blocks[start + i].store(p, Ordering::Relaxed);
                state.sizes[start + i].store(size, Ordering::Relaxed);
            }

            for cblks in (1..=num_chunks).rev() {
                let victim = rng.next() as usize % cblks;
                let last = cblks - 1;
                let victim_ptr = state.blocks[start + victim].load(Ordering::Relaxed);
                let victim_size = state.sizes[start + victim].load(Ordering::Relaxed);
                let last_ptr = state.blocks[start + last].load(Ordering::Relaxed);
                let last_size = state.sizes[start + last].load(Ordering::Relaxed);
                state.blocks[start + victim].store(last_ptr, Ordering::Relaxed);
                state.sizes[start + victim].store(last_size, Ordering::Relaxed);
                state.blocks[start + last].store(victim_ptr, Ordering::Relaxed);
                state.sizes[start + last].store(victim_size, Ordering::Relaxed);
            }

            for _ in 0..(4 * num_chunks) {
                let victim = rng.next() as usize % num_chunks;
                let idx = start + victim;
                let old = state.blocks[idx].load(Ordering::Relaxed);
                let old_size = state.sizes[idx].load(Ordering::Relaxed);
                larson_delete(state.alloc, old, old_size, state.sized_delete);

                let size = MIN_SIZE + rng.next() as usize % (MAX_SIZE - MIN_SIZE);
                let p = larson_new(state.alloc, size);
                state.blocks[idx].store(p, Ordering::Relaxed);
                state.sizes[idx].store(size, Ordering::Relaxed);
            }
        }
    }

    unsafe fn exercise_heap(data: &mut ThreadData) {
        unsafe {
            let state = &*data.state;
            let range = data.max_size - data.min_size;
            for _ in 0..data.num_blocks {
                let victim = data.rng.next() as usize % data.array_size;
                let idx = data.array_start + victim;
                let old = state.blocks[idx].load(Ordering::Relaxed);
                let old_size = state.sizes[idx].load(Ordering::Relaxed);
                larson_delete(state.alloc, old, old_size, state.sized_delete);
                data.frees += 1;

                let block_size = if range == 0 {
                    data.min_size
                } else {
                    data.min_size + data.rng.next() as usize % range
                };
                let p = larson_new(state.alloc, block_size);
                state.blocks[idx].store(p, Ordering::Relaxed);
                state.sizes[idx].store(block_size, Ordering::Relaxed);
                data.allocs += 1;

                p.write(b'a');
                p.add(1).write(b'b');

                if state.stop.load(Ordering::Relaxed) {
                    break;
                }
            }
            std::hint::black_box(data.thread_no);
        }
    }

    extern "C" fn thread_entry(arg: *mut c_void) -> *mut c_void {
        unsafe {
            let data = &mut *arg.cast::<ThreadData>();
            let state = &*data.state;
            while !state.stop.load(Ordering::Relaxed) {
                exercise_heap(data);
            }
        }
        ptr::null_mut()
    }

    unsafe fn cleanup(state: &State) {
        unsafe {
            for i in 0..state.blocks.len() {
                let p = state.blocks[i].swap(ptr::null_mut(), Ordering::Relaxed);
                let size = state.sizes[i].load(Ordering::Relaxed);
                larson_delete(state.alloc, p, size, state.sized_delete);
            }
        }
    }

    unsafe fn run_case(alloc: BenchAllocator, sized_delete: bool) -> Duration {
        unsafe {
            let num_threads = logical_cpus();
            let total_chunks = num_threads * CHUNKS_PER_THREAD;
            let state = State {
                alloc,
                sized_delete,
                stop: AtomicBool::new(false),
                blocks: (0..total_chunks)
                    .map(|_| AtomicPtr::new(ptr::null_mut()))
                    .collect(),
                sizes: (0..total_chunks).map(|_| AtomicUsize::new(0)).collect(),
            };
            let mut global_rng = Lran2::new(SEED);
            warmup(&state, &mut global_rng, 0, total_chunks);

            let mut args = Vec::with_capacity(num_threads);
            for i in 0..num_threads {
                let seed = global_rng.next();
                args.push(ThreadData {
                    state: &state,
                    thread_no: i + 1,
                    num_blocks: NUM_ROUNDS * CHUNKS_PER_THREAD,
                    array_start: i * CHUNKS_PER_THREAD,
                    array_size: CHUNKS_PER_THREAD,
                    min_size: MIN_SIZE,
                    max_size: MAX_SIZE,
                    rng: Lran2::new(seed),
                    allocs: 0,
                    frees: 0,
                });
            }

            let mut threads = vec![0; num_threads];
            for i in 0..num_threads {
                let rc = pthread::pthread_create(
                    &mut threads[i],
                    ptr::null(),
                    thread_entry,
                    (&mut args[i] as *mut ThreadData).cast(),
                );
                assert_eq!(rc, 0, "pthread_create failed in larson: {rc}");
            }

            let start = Instant::now();
            std::thread::sleep(Duration::from_secs(SLEEP_SECS));
            state.stop.store(true, Ordering::Relaxed);
            for thread in threads {
                let rc = pthread::pthread_join(thread, ptr::null_mut());
                assert_eq!(rc, 0, "pthread_join failed in larson: {rc}");
            }
            let elapsed = start.elapsed().as_secs_f64();
            let sum_allocs: usize = args.iter().map(|arg| arg.allocs).sum();
            cleanup(&state);

            assert!(sum_allocs > 0, "larson produced zero allocations");
            let throughput = sum_allocs as f64 / elapsed;
            Duration::from_secs_f64(1.0e9 / throughput)
        }
    }

    pub unsafe fn run_unsized(alloc: BenchAllocator) -> Duration {
        unsafe { run_case(alloc, false) }
    }

    pub unsafe fn run_sized(alloc: BenchAllocator) -> Duration {
        unsafe { run_case(alloc, true) }
    }
}

mod rptest {
    use super::*;

    const CROSS_RATE: usize = 2;
    const LOOP_COUNT: usize = 500;
    const ALLOC_COUNT: usize = 1000;
    const OP_COUNT: usize = 100;
    const MIN_SIZE: usize = 8;
    const MAX_SIZE: usize = 16000;
    const ALIGNMENT: [usize; 3] = [0, 8, 16];

    struct Tables {
        random_size_lin: Vec<usize>,
        alloc_ops: Vec<usize>,
        free_ops: Vec<usize>,
        alloc_scatter: usize,
        free_scatter: usize,
    }

    struct RunState {
        start: AtomicBool,
        sync: AtomicI32,
        cross_thread_counter: AtomicI32,
        tables: Tables,
    }

    struct ThreadPointers {
        pointers: *mut *mut u8,
        count: usize,
        next: *mut ThreadPointers,
        allocated: *const AtomicI32,
    }

    struct ThreadArg {
        state: *const RunState,
        alloc: BenchAllocator,
        args: *mut ThreadArg,
        num_threads: usize,
        index: usize,
        cross_rate: usize,
        loop_count: usize,
        alloc_count: usize,
        foreign: AtomicUsize,
        allocated: AtomicI32,
        peak_allocated: i32,
        ticks: f64,
        mops: u64,
        accumulator: u64,
    }

    fn build_tables() -> Tables {
        let size_range = MAX_SIZE - MIN_SIZE;
        let random_size: Vec<_> = rptest_tables::RANDOM_SIZE
            .iter()
            .map(|&size| {
                MIN_SIZE + ((size_range as f64) * ((size as f64) / 100_000.0)).round() as usize
            })
            .collect();

        let effective_range = size_range.max(1);
        let random_size_lin: Vec<_> = (0..random_size.len())
            .map(|i| {
                let w0 = 1.0 - (random_size[i] - MIN_SIZE) as f64 / effective_range as f64;
                let w1 = 1.0
                    - (random_size[(i + 1) % random_size.len()] - MIN_SIZE) as f64
                        / effective_range as f64;
                let even = (random_size[(i + 2) % random_size.len()] - MIN_SIZE) as f64;
                MIN_SIZE + (even * ((w0 + w1) - 1.0).abs()) as usize
            })
            .collect();

        let alloc_ops = rptest_tables::NUM_ALLOC_OPS
            .iter()
            .map(|&ops| (OP_COUNT as f64 * (ops as f64 / 100.0)) as usize)
            .collect();
        let free_ops = rptest_tables::NUM_FREE_OPS
            .iter()
            .map(|&ops| (0.8 * OP_COUNT as f64 * (ops as f64 / 100.0)) as usize)
            .collect();

        let mut iprime = 0;
        let mut alloc_scatter = rptest_tables::PRIMES[iprime];
        while ALLOC_COUNT.is_multiple_of(alloc_scatter) {
            iprime += 1;
            alloc_scatter = rptest_tables::PRIMES[iprime];
        }
        iprime += 1;
        let mut free_scatter = rptest_tables::PRIMES[iprime];
        while ALLOC_COUNT.is_multiple_of(free_scatter) {
            iprime += 1;
            free_scatter = rptest_tables::PRIMES[iprime];
        }

        Tables {
            random_size_lin,
            alloc_ops,
            free_ops,
            alloc_scatter,
            free_scatter,
        }
    }

    unsafe fn benchmark_malloc(alloc: BenchAllocator, alignment: usize, size: usize) -> *mut u8 {
        unsafe {
            let ptr = if alignment == 0 {
                alloc.calloc(1, size)
            } else {
                let ptr = alloc.memalign(alignment, size);
                if !ptr.is_null() {
                    ptr::write_bytes(ptr, 0xcd, size);
                }
                ptr
            };
            assert!(!ptr.is_null(), "allocation failed in rptestN");
            if alignment != 0 {
                assert_eq!(
                    (ptr as usize) % alignment,
                    0,
                    "rptestN allocation was not {alignment}-byte aligned"
                );
            }
            ptr
        }
    }

    unsafe fn benchmark_free(alloc: BenchAllocator, ptr: *mut u8) {
        unsafe { alloc.free(ptr) }
    }

    unsafe fn put_cross_thread_memory(
        state: &RunState,
        ptr: &AtomicUsize,
        pointers: *mut ThreadPointers,
    ) {
        unsafe {
            loop {
                let prev = ptr.load(Ordering::SeqCst);
                (*pointers).next = (prev & !0xf) as *mut ThreadPointers;
                let tag = state.cross_thread_counter.fetch_add(1, Ordering::SeqCst) + 1;
                let newval = (pointers as usize) | ((tag as usize) & 0xf);
                if ptr
                    .compare_exchange(prev, newval, Ordering::SeqCst, Ordering::SeqCst)
                    .is_ok()
                {
                    break;
                }
            }
        }
    }

    fn get_cross_thread_memory(state: &RunState, ptr: &AtomicUsize) -> *mut ThreadPointers {
        loop {
            let prev = ptr.load(Ordering::SeqCst);
            let current = (prev & !0xf) as *mut ThreadPointers;
            if current.is_null() {
                return ptr::null_mut();
            }
            let tag = state.cross_thread_counter.fetch_add(1, Ordering::SeqCst) + 1;
            let newval = (tag as usize) & 0xf;
            if ptr
                .compare_exchange(prev, newval, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                return current;
            }
        }
    }

    unsafe fn touch_allocation(ptr: *mut u8, size: usize) {
        unsafe {
            *(ptr.cast::<i32>()) = size as i32;
            let num_pages = (size - 1) / 4096;
            for page in 1..num_pages {
                *ptr.add(page * 4096) = 1;
            }
            *ptr.add(size - 1) = 1;
        }
    }

    unsafe fn free_foreign(
        alloc: BenchAllocator,
        mut foreign: *mut ThreadPointers,
        timed: bool,
        ticks: &mut f64,
        mops: &mut u64,
    ) {
        unsafe {
            let start = timed.then(Instant::now);
            while !foreign.is_null() {
                let mut allocated = 0i32;
                for i in 0..(*foreign).count {
                    let ptr = *(*foreign).pointers.add(i);
                    allocated -= *(ptr.cast::<i32>());
                    benchmark_free(alloc, ptr);
                    *mops += 1;
                }

                let next = (*foreign).next;
                allocated -= ((*foreign).count * std::mem::size_of::<*mut u8>()
                    + std::mem::size_of::<ThreadPointers>()) as i32;
                (&*(*foreign).allocated).fetch_add(allocated, Ordering::SeqCst);
                benchmark_free(alloc, (*foreign).pointers.cast());
                benchmark_free(alloc, foreign.cast());
                *mops += 2;
                foreign = next;
            }
            if let Some(start) = start {
                *ticks += start.elapsed().as_secs_f64();
            }
        }
    }

    unsafe fn allocate_block(
        alloc: BenchAllocator,
        size_index: usize,
        iop: usize,
        size: usize,
    ) -> *mut u8 {
        unsafe {
            let alignment = if size < 4096 {
                ALIGNMENT[(size_index + iop) % ALIGNMENT.len()]
            } else {
                0
            };
            let ptr = benchmark_malloc(alloc, alignment, size);
            touch_allocation(ptr, size);
            ptr
        }
    }

    unsafe fn benchmark_worker(argptr: *mut ThreadArg) {
        unsafe {
            let arg = &mut *argptr;
            let state = &*arg.state;
            let tables = &state.tables;
            let random_sizes = &tables.random_size_lin;

            let pointers_size = std::mem::size_of::<*mut u8>() * arg.alloc_count;
            let pointers = benchmark_malloc(arg.alloc, 16, pointers_size).cast::<*mut u8>();
            ptr::write_bytes(pointers, 0, arg.alloc_count);
            arg.allocated
                .fetch_add(pointers_size as i32, Ordering::SeqCst);

            while !state.start.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(10));
            }

            arg.ticks = 0.0;
            arg.mops = 0;
            let mut alloc_idx = 0usize;
            let mut free_idx = 0usize;
            let mut cross_index = 0usize;
            for iter in 0..2 {
                let mut size_index = ((arg.index + 1) * ((iter + 1) * 37)) % random_sizes.len();
                let mut iter_ticks_elapsed = 0.0;
                let mut do_foreign = true;

                for iloop in 0..arg.loop_count {
                    let foreign = get_cross_thread_memory(state, &arg.foreign);
                    let start = Instant::now();
                    let mut allocated = 0i32;
                    let free_op_count = tables.free_ops[(iter + iloop) % tables.free_ops.len()];
                    let alloc_op_count = tables.alloc_ops[(iter + iloop) % tables.alloc_ops.len()];

                    for _ in 0..free_op_count {
                        let ptr = *pointers.add(free_idx);
                        if !ptr.is_null() {
                            allocated -= *(ptr.cast::<i32>());
                            benchmark_free(arg.alloc, ptr);
                            arg.mops += 1;
                            *pointers.add(free_idx) = ptr::null_mut();
                        }
                        free_idx = (free_idx + tables.free_scatter) % arg.alloc_count;
                    }

                    free_foreign(arg.alloc, foreign, false, &mut arg.ticks, &mut arg.mops);

                    for iop in 0..alloc_op_count {
                        let old = *pointers.add(alloc_idx);
                        if !old.is_null() {
                            allocated -= *(old.cast::<i32>());
                            benchmark_free(arg.alloc, old);
                            arg.mops += 1;
                        }
                        let size = random_sizes[(size_index + 2) % random_sizes.len()];
                        let ptr = allocate_block(arg.alloc, size_index, iop, size);
                        *pointers.add(alloc_idx) = ptr;
                        allocated += size as i32;
                        arg.mops += 1;

                        alloc_idx = (alloc_idx + tables.alloc_scatter) % arg.alloc_count;
                        size_index = (size_index + 1) % random_sizes.len();
                    }

                    let mut foreign = ptr::null_mut();
                    if arg.cross_rate != 0 && (iloop % arg.cross_rate) == 0 && do_foreign {
                        foreign =
                            benchmark_malloc(arg.alloc, 16, std::mem::size_of::<ThreadPointers>())
                                .cast::<ThreadPointers>();
                        let foreign_pointers = benchmark_malloc(
                            arg.alloc,
                            16,
                            std::mem::size_of::<*mut u8>() * alloc_op_count,
                        )
                        .cast::<*mut u8>();
                        (*foreign).count = alloc_op_count;
                        (*foreign).pointers = foreign_pointers;
                        (*foreign).allocated = &arg.allocated;
                        (*foreign).next = ptr::null_mut();
                        allocated += (alloc_op_count * std::mem::size_of::<*mut u8>()
                            + std::mem::size_of::<ThreadPointers>())
                            as i32;
                        arg.mops += 2;

                        for iop in 0..alloc_op_count {
                            let size = random_sizes[(size_index + 2) % random_sizes.len()];
                            let ptr = allocate_block(arg.alloc, size_index, iop, size);
                            *foreign_pointers.add(iop) = ptr;
                            allocated += size as i32;
                            arg.mops += 1;
                            size_index = (size_index + 1) % random_sizes.len();
                        }
                    }

                    let ticks_elapsed = start.elapsed().as_secs_f64();
                    iter_ticks_elapsed += ticks_elapsed;
                    arg.ticks += ticks_elapsed;

                    let current_allocated =
                        arg.allocated.fetch_add(allocated, Ordering::SeqCst) + allocated;
                    if arg.peak_allocated < current_allocated {
                        arg.peak_allocated = current_allocated;
                    }

                    if !foreign.is_null() {
                        cross_index = (cross_index + 1) % arg.num_threads;
                        if arg.num_threads > 1 && cross_index == arg.index {
                            cross_index = (cross_index + 1) % arg.num_threads;
                        }
                        let cross_arg = &*arg.args.add(cross_index);
                        put_cross_thread_memory(state, &cross_arg.foreign, foreign);
                    }

                    if state.sync.load(Ordering::SeqCst) > 0 {
                        do_foreign = false;
                    }

                    if iter_ticks_elapsed > 120.0 {
                        break;
                    }
                }

                state.sync.fetch_add(1, Ordering::SeqCst);
                while state.sync.load(Ordering::SeqCst) != 0 {
                    let foreign = get_cross_thread_memory(state, &arg.foreign);
                    if !foreign.is_null() {
                        free_foreign(arg.alloc, foreign, true, &mut arg.ticks, &mut arg.mops);
                    }
                    std::thread::sleep(Duration::from_millis(1));
                }

                let start = Instant::now();
                let mut allocated = 0i32;
                for iptr in 0..arg.alloc_count {
                    let ptr = *pointers.add(iptr);
                    if !ptr.is_null() {
                        allocated -= *(ptr.cast::<i32>());
                        benchmark_free(arg.alloc, ptr);
                        arg.mops += 1;
                        *pointers.add(iptr) = ptr::null_mut();
                    }
                }
                arg.ticks += start.elapsed().as_secs_f64();
                arg.allocated.fetch_add(allocated, Ordering::SeqCst);

                std::thread::sleep(Duration::from_millis(10));
                let foreign = get_cross_thread_memory(state, &arg.foreign);
                free_foreign(arg.alloc, foreign, false, &mut arg.ticks, &mut arg.mops);
            }

            std::thread::sleep(Duration::from_secs(1));
            state.sync.fetch_add(1, Ordering::SeqCst);
            while state.sync.load(Ordering::SeqCst) != 0 {
                let foreign = get_cross_thread_memory(state, &arg.foreign);
                if !foreign.is_null() {
                    free_foreign(arg.alloc, foreign, true, &mut arg.ticks, &mut arg.mops);
                }
                std::thread::sleep(Duration::from_millis(1));
            }

            benchmark_free(arg.alloc, pointers.cast());
            arg.allocated
                .fetch_sub(pointers_size as i32, Ordering::SeqCst);
            arg.accumulator += arg.mops;
        }
    }

    extern "C" fn thread_entry(arg: *mut c_void) -> *mut c_void {
        unsafe { benchmark_worker(arg.cast()) };
        ptr::null_mut()
    }

    unsafe fn run_iteration(alloc: BenchAllocator, num_threads: usize) -> (u64, f64) {
        unsafe {
            let mut state = RunState {
                start: AtomicBool::new(false),
                sync: AtomicI32::new(0),
                cross_thread_counter: AtomicI32::new(0),
                tables: build_tables(),
            };
            let state_ptr = &mut state as *mut RunState;
            let cross_rate = if num_threads == 1 { 0 } else { CROSS_RATE };

            let mut args = Vec::with_capacity(num_threads);
            for index in 0..num_threads {
                args.push(ThreadArg {
                    state: state_ptr,
                    alloc,
                    args: ptr::null_mut(),
                    num_threads,
                    index,
                    cross_rate,
                    loop_count: LOOP_COUNT,
                    alloc_count: ALLOC_COUNT,
                    foreign: AtomicUsize::new(0),
                    allocated: AtomicI32::new(0),
                    peak_allocated: 0,
                    ticks: 0.0,
                    mops: 0,
                    accumulator: 0,
                });
            }
            let args_ptr = args.as_mut_ptr();
            for arg in &mut args {
                arg.args = args_ptr;
            }

            let mut threads = vec![0; num_threads];
            for (i, thread) in threads.iter_mut().enumerate() {
                let rc = pthread::pthread_create(
                    thread,
                    ptr::null(),
                    thread_entry,
                    args_ptr.add(i).cast(),
                );
                assert_eq!(rc, 0, "pthread_create failed in rptestN: {rc}");
            }

            std::thread::sleep(Duration::from_secs(1));
            state.start.store(true, Ordering::SeqCst);

            while state.sync.load(Ordering::SeqCst) < num_threads as i32 {
                std::thread::sleep(Duration::from_secs(1));
            }
            std::thread::sleep(Duration::from_secs(1));
            state.sync.store(0, Ordering::SeqCst);

            std::thread::sleep(Duration::from_secs(1));
            while state.sync.load(Ordering::SeqCst) < num_threads as i32 {
                std::thread::sleep(Duration::from_secs(1));
            }
            std::thread::sleep(Duration::from_secs(1));
            state.sync.store(0, Ordering::SeqCst);

            std::thread::sleep(Duration::from_secs(1));
            while state.sync.load(Ordering::SeqCst) < num_threads as i32 {
                std::thread::sleep(Duration::from_secs(1));
            }
            std::thread::sleep(Duration::from_secs(1));
            state.sync.store(0, Ordering::SeqCst);

            for thread in threads {
                let rc = pthread::pthread_join(thread, ptr::null_mut());
                assert_eq!(rc, 0, "pthread_join failed in rptestN: {rc}");
            }

            let mops = args.iter().map(|arg| arg.mops).sum();
            let ticks = args.iter().map(|arg| arg.ticks).sum();
            assert!(args.iter().all(|arg| arg.accumulator != 0));
            (mops, ticks)
        }
    }

    pub unsafe fn run(alloc: BenchAllocator) -> Duration {
        unsafe {
            let num_threads = logical_cpus();
            let mut mops = 0u64;
            let mut ticks = 0.0;
            for _ in 0..2 {
                let (iter_mops, iter_ticks) = run_iteration(alloc, num_threads);
                mops += iter_mops;
                ticks += iter_ticks;
            }
            if ticks == 0.0 {
                ticks = 1.0;
            }
            let average_mops = mops as f64 / ticks;
            Duration::from_secs_f64(2_000_000.0 / average_mops)
        }
    }
}

mod barnes {
    use super::*;

    const NDIM: usize = 3;
    const NSUB: usize = 1 << NDIM;
    const MAX_BODIES_PER_LEAF: usize = 10;
    const BODY: i16 = 1;
    const CELL: i16 = 2;
    const LEAF: i16 = 3;
    const MAXLEVEL: i32 = 8 * std::mem::size_of::<i32>() as i32 - 2;
    const IMAX: i32 = 1 << MAXLEVEL;
    const N_BODY: usize = 163_840;
    const SEED: i32 = 123;
    const DTIME: f64 = 0.025;
    const EPS: f64 = 0.05;
    const TOL: f64 = 1.0;
    const FCELLS: f64 = 2.0;
    const FLEAVES: f64 = 5.0;
    const TSTOP: f64 = 0.075;
    const MFRAC: f64 = 0.999;
    const PI: f64 = std::f64::consts::PI;

    #[derive(Clone, Copy, Default, PartialEq, Eq)]
    enum NodeRef {
        #[default]
        None,
        Body(usize),
        Cell(usize),
        Leaf(usize),
    }

    #[derive(Clone, Copy)]
    struct Body {
        typ: i16,
        mass: f64,
        pos: [f64; NDIM],
        cost: i32,
        level: i32,
        parent: NodeRef,
        child_num: i32,
        vel: [f64; NDIM],
        acc: [f64; NDIM],
        phi: f64,
    }

    impl Default for Body {
        fn default() -> Self {
            Self {
                typ: BODY,
                mass: 0.0,
                pos: [0.0; NDIM],
                cost: 0,
                level: 0,
                parent: NodeRef::None,
                child_num: 0,
                vel: [0.0; NDIM],
                acc: [0.0; NDIM],
                phi: 0.0,
            }
        }
    }

    #[derive(Clone, Copy)]
    struct Cell {
        typ: i16,
        mass: f64,
        pos: [f64; NDIM],
        cost: i32,
        level: i32,
        child_num: i32,
        done: bool,
        subp: [NodeRef; NSUB],
    }

    impl Default for Cell {
        fn default() -> Self {
            Self {
                typ: CELL,
                mass: 0.0,
                pos: [0.0; NDIM],
                cost: 0,
                level: 0,
                child_num: 0,
                done: false,
                subp: [NodeRef::None; NSUB],
            }
        }
    }

    #[derive(Clone, Copy)]
    struct Leaf {
        typ: i16,
        mass: f64,
        pos: [f64; NDIM],
        cost: i32,
        level: i32,
        parent: NodeRef,
        child_num: i32,
        done: bool,
        num_bodies: usize,
        bodyp: [usize; MAX_BODIES_PER_LEAF],
    }

    impl Default for Leaf {
        fn default() -> Self {
            Self {
                typ: LEAF,
                mass: 0.0,
                pos: [0.0; NDIM],
                cost: 0,
                level: 0,
                parent: NodeRef::None,
                child_num: 0,
                done: false,
                num_bodies: 0,
                bodyp: [0; MAX_BODIES_PER_LEAF],
            }
        }
    }

    struct ForeignArray<T> {
        alloc: BenchAllocator,
        ptr: *mut T,
        len: usize,
    }

    impl<T> ForeignArray<T> {
        unsafe fn new(alloc: BenchAllocator, len: usize) -> Self {
            unsafe {
                let bytes = len
                    .checked_mul(std::mem::size_of::<T>())
                    .expect("barnes allocation size overflow");
                let ptr = alloc.malloc(bytes).cast::<T>();
                assert!(!ptr.is_null(), "allocation failed in barnes");
                Self { alloc, ptr, len }
            }
        }

        unsafe fn get(&self, index: usize) -> &T {
            assert!(index < self.len);
            unsafe { &*self.ptr.add(index) }
        }

        unsafe fn get_mut(&mut self, index: usize) -> &mut T {
            assert!(index < self.len);
            unsafe { &mut *self.ptr.add(index) }
        }

        unsafe fn write(&mut self, index: usize, value: T) {
            assert!(index < self.len);
            unsafe { self.ptr.add(index).write(value) };
        }
    }

    impl<T> Drop for ForeignArray<T> {
        fn drop(&mut self) {
            unsafe { self.alloc.free(self.ptr.cast()) }
        }
    }

    struct Rand {
        a: i64,
        b: i64,
        randx: i64,
    }

    impl Rand {
        fn new(seed: i32) -> Self {
            const MULT: i64 = 1_103_515_245;
            const ADD: i64 = 12_345;
            const MASK: i64 = 0x7fff_ffff;
            let mut a = 1i64;
            let mut b = 0i64;
            let randx = (a * seed as i64 + b) & MASK;
            a = (MULT * a) & MASK;
            b = (MULT * b + ADD) & MASK;
            Self { a, b, randx }
        }

        fn prand(&mut self) -> f64 {
            const MASK: i64 = 0x7fff_ffff;
            const TWOTO31: f64 = 2_147_483_648.0;
            let last = self.randx;
            self.randx = (self.a * self.randx + self.b) & MASK;
            last as f64 / TWOTO31
        }

        fn xrand(&mut self, low: f64, high: f64) -> f64 {
            low + (high - low) * self.prand()
        }
    }

    struct Sim {
        alloc: BenchAllocator,
        nbody: usize,
        maxleaf: usize,
        maxcell: usize,
        maxmybody: usize,
        maxmycell: usize,
        maxmyleaf: usize,
        bodies: ForeignArray<Body>,
        cells: ForeignArray<Cell>,
        leaves: ForeignArray<Leaf>,
        mybodytab: ForeignArray<usize>,
        mycelltab: ForeignArray<usize>,
        myleaftab: ForeignArray<usize>,
        mynbody: usize,
        mynumcell: usize,
        mynumleaf: usize,
        myncell: usize,
        mynleaf: usize,
        nstep: usize,
        tnow: f64,
        dthf: f64,
        epssq: f64,
        tolsq: f64,
        rmin: [f64; NDIM],
        min: [f64; NDIM],
        max: [f64; NDIM],
        rsize: f64,
        root: usize,
        pos0: [f64; NDIM],
        acc0: [f64; NDIM],
        phi0: f64,
        dr: [f64; NDIM],
        drsq: f64,
        pmem: NodeRef,
        pskip: usize,
        myn2bterm: i32,
        mynbcterm: i32,
        skipself: bool,
        checksum: f64,
    }

    impl Sim {
        unsafe fn new(alloc: BenchAllocator) -> Self {
            unsafe {
                let maxleaf = (FLEAVES * N_BODY as f64) as usize;
                let maxcell = (FCELLS * maxleaf as f64) as usize;
                let maxmybody = N_BODY + maxleaf * MAX_BODIES_PER_LEAF;
                let maxmycell = maxcell;
                let maxmyleaf = maxleaf;
                let mut sim = Self {
                    alloc,
                    nbody: N_BODY,
                    maxleaf,
                    maxcell,
                    maxmybody,
                    maxmycell,
                    maxmyleaf,
                    bodies: ForeignArray::new(alloc, N_BODY),
                    cells: ForeignArray::new(alloc, maxcell),
                    leaves: ForeignArray::new(alloc, maxleaf),
                    mybodytab: ForeignArray::new(alloc, maxmybody),
                    mycelltab: ForeignArray::new(alloc, maxmycell),
                    myleaftab: ForeignArray::new(alloc, maxmyleaf),
                    mynbody: 0,
                    mynumcell: 0,
                    mynumleaf: 0,
                    myncell: 0,
                    mynleaf: 0,
                    nstep: 0,
                    tnow: 0.0,
                    dthf: 0.5 * DTIME,
                    epssq: EPS * EPS,
                    tolsq: TOL * TOL,
                    rmin: [0.0; NDIM],
                    min: [0.0; NDIM],
                    max: [0.0; NDIM],
                    rsize: 0.0,
                    root: 0,
                    pos0: [0.0; NDIM],
                    acc0: [0.0; NDIM],
                    phi0: 0.0,
                    dr: [0.0; NDIM],
                    drsq: 0.0,
                    pmem: NodeRef::None,
                    pskip: 0,
                    myn2bterm: 0,
                    mynbcterm: 0,
                    skipself: false,
                    checksum: 0.0,
                };
                sim.testdata();
                sim.setbound();
                sim.find_my_initial_bodies();
                sim
            }
        }

        unsafe fn body(&self, index: usize) -> &Body {
            unsafe { self.bodies.get(index) }
        }

        unsafe fn body_mut(&mut self, index: usize) -> &mut Body {
            unsafe { self.bodies.get_mut(index) }
        }

        unsafe fn cell(&self, index: usize) -> &Cell {
            unsafe { self.cells.get(index) }
        }

        unsafe fn cell_mut(&mut self, index: usize) -> &mut Cell {
            unsafe { self.cells.get_mut(index) }
        }

        unsafe fn leaf(&self, index: usize) -> &Leaf {
            unsafe { self.leaves.get(index) }
        }

        unsafe fn leaf_mut(&mut self, index: usize) -> &mut Leaf {
            unsafe { self.leaves.get_mut(index) }
        }

        unsafe fn testdata(&mut self) {
            unsafe {
                let mut rng = Rand::new(SEED);
                let rsc = 9.0 * PI / 16.0;
                let vsc = (1.0 / rsc).sqrt();
                let mut cmr = [0.0; NDIM];
                let mut cmv = [0.0; NDIM];
                let halfnbody = self.nbody.div_ceil(2);
                for i in 0..halfnbody {
                    let mut body = Body {
                        typ: BODY,
                        mass: 1.0 / self.nbody as f64,
                        cost: 1,
                        ..Body::default()
                    };
                    let mut r = 1.0 / (rng.xrand(0.0, MFRAC).powf(-2.0 / 3.0) - 1.0).sqrt();
                    while r > 9.0 {
                        r = 1.0 / (rng.xrand(0.0, MFRAC).powf(-2.0 / 3.0) - 1.0).sqrt();
                    }
                    body.pos = Self::pickshell(&mut rng, rsc * r);
                    add_assign(&mut cmr, body.pos);
                    let mut x;
                    let mut y;
                    loop {
                        x = rng.xrand(0.0, 1.0);
                        y = rng.xrand(0.0, 0.1);
                        if y <= x * x * (1.0 - x * x).powf(3.5) {
                            break;
                        }
                    }
                    let v = 2.0_f64.sqrt() * x / (1.0 + r * r).powf(0.25);
                    body.vel = Self::pickshell(&mut rng, vsc * v);
                    add_assign(&mut cmv, body.vel);
                    self.bodies.write(i, body);
                }

                for i in halfnbody..self.nbody {
                    let cp = *self.body(i - halfnbody);
                    let mut body = Body {
                        typ: BODY,
                        mass: 1.0 / self.nbody as f64,
                        cost: 1,
                        ..Body::default()
                    };
                    for k in 0..NDIM {
                        body.pos[k] = cp.pos[k] + 4.0;
                        cmr[k] += body.pos[k];
                        body.vel[k] = cp.vel[k];
                        cmv[k] += body.vel[k];
                    }
                    self.bodies.write(i, body);
                }

                div_scalar(&mut cmr, self.nbody as f64);
                div_scalar(&mut cmv, self.nbody as f64);
                for i in 0..self.nbody {
                    let body = self.body_mut(i);
                    sub_assign(&mut body.pos, cmr);
                    sub_assign(&mut body.vel, cmv);
                }
            }
        }

        fn pickshell(rng: &mut Rand, rad: f64) -> [f64; NDIM] {
            loop {
                let vec = [
                    rng.xrand(-1.0, 1.0),
                    rng.xrand(-1.0, 1.0),
                    rng.xrand(-1.0, 1.0),
                ];
                let rsq = dot(vec, vec);
                if rsq <= 1.0 {
                    return mul_scalar(vec, rad / rsq.sqrt());
                }
            }
        }

        unsafe fn setbound(&mut self) {
            unsafe {
                self.min = [1e99; NDIM];
                self.max = [-1e99; NDIM];
                for i in 0..self.nbody {
                    let pos = self.body(i).pos;
                    for (k, value) in pos.into_iter().enumerate() {
                        self.min[k] = self.min[k].min(value);
                        self.max[k] = self.max[k].max(value);
                    }
                }
                let mut side: f64 = 0.0;
                for k in 0..NDIM {
                    self.max[k] -= self.min[k];
                    side = side.max(self.max[k]);
                    self.rmin[k] = self.min[k] - side / 100000.0;
                }
                self.rsize = 1.00002 * side;
                self.max = [-1e99; NDIM];
                self.min = [1e99; NDIM];
            }
        }

        unsafe fn find_my_initial_bodies(&mut self) {
            unsafe {
                self.mynbody = self.nbody;
                for i in 0..self.nbody {
                    self.mybodytab.write(i, i);
                }
            }
        }

        unsafe fn init_root(&mut self) {
            unsafe {
                self.root = 0;
                self.mynumcell = 1;
                self.myncell = 1;
                self.mynumleaf = 0;
                self.mynleaf = 0;
                let root = Cell {
                    typ: CELL,
                    level: IMAX >> 1,
                    done: false,
                    subp: [NodeRef::None; NSUB],
                    ..Cell::default()
                };
                self.cells.write(self.root, root);
                self.mycelltab.write(0, self.root);
            }
        }

        unsafe fn makecell(&mut self, parent: NodeRef) -> usize {
            unsafe {
                assert!(
                    self.mynumcell < self.maxmycell,
                    "barnes exhausted cell arena"
                );
                let index = self.mynumcell;
                self.mynumcell += 1;
                let level = match parent {
                    NodeRef::Cell(parent) => self.cell(parent).level >> 1,
                    _ => IMAX >> 1,
                };
                let cell = Cell {
                    typ: CELL,
                    level,
                    done: false,
                    subp: [NodeRef::None; NSUB],
                    ..Cell::default()
                };
                self.cells.write(index, cell);
                self.mycelltab.write(self.myncell, index);
                self.myncell += 1;
                index
            }
        }

        unsafe fn makeleaf(&mut self, parent: NodeRef) -> usize {
            unsafe {
                assert!(
                    self.mynumleaf < self.maxmyleaf,
                    "barnes exhausted leaf arena"
                );
                let index = self.mynumleaf;
                self.mynumleaf += 1;
                let level = match parent {
                    NodeRef::Cell(parent) => self.cell(parent).level >> 1,
                    _ => IMAX >> 1,
                };
                let leaf = Leaf {
                    typ: LEAF,
                    level,
                    parent,
                    done: false,
                    bodyp: [0; MAX_BODIES_PER_LEAF],
                    ..Leaf::default()
                };
                self.leaves.write(index, leaf);
                self.myleaftab.write(self.mynleaf, index);
                self.mynleaf += 1;
                index
            }
        }

        unsafe fn maketree(&mut self) {
            unsafe {
                self.myncell = 1;
                self.mynleaf = 0;
                self.mynumcell = 1;
                self.mynumleaf = 0;
                for slot in 0..self.mynbody {
                    let body_index = *self.mybodytab.get(slot);
                    if self.body(body_index).mass != 0.0 {
                        self.loadtree(body_index);
                    }
                }
                self.hackcofm();
            }
        }

        unsafe fn loadtree(&mut self, body_index: usize) {
            unsafe {
                let xp = self.intcoord(self.body(body_index).pos);
                let mut mynode = self.root;
                let mut kid_index = subindex(xp, self.cell(mynode).level);
                let mut level = self.cell(mynode).level >> 1;

                loop {
                    assert_ne!(level, 0, "barnes tree ran out of levels");
                    match self.cell(mynode).subp[kid_index] {
                        NodeRef::None => {
                            let leaf = self.makeleaf(NodeRef::Cell(mynode));
                            {
                                let body = self.body_mut(body_index);
                                body.parent = NodeRef::Leaf(leaf);
                                body.level = level;
                                body.child_num = 0;
                            }
                            {
                                let leaf_ref = self.leaf_mut(leaf);
                                leaf_ref.child_num = kid_index as i32;
                                leaf_ref.bodyp[0] = body_index;
                                leaf_ref.num_bodies = 1;
                            }
                            self.cell_mut(mynode).subp[kid_index] = NodeRef::Leaf(leaf);
                            return;
                        }
                        NodeRef::Leaf(leaf) => {
                            if self.leaf(leaf).num_bodies == MAX_BODIES_PER_LEAF {
                                let cell = self.subdivide_leaf(leaf, NodeRef::Cell(mynode), level);
                                self.cell_mut(mynode).subp[kid_index] = NodeRef::Cell(cell);
                            } else {
                                let child_num = self.leaf(leaf).num_bodies;
                                {
                                    let body = self.body_mut(body_index);
                                    body.parent = NodeRef::Leaf(leaf);
                                    body.level = level;
                                    body.child_num = child_num as i32;
                                }
                                let leaf_ref = self.leaf_mut(leaf);
                                leaf_ref.bodyp[child_num] = body_index;
                                leaf_ref.num_bodies += 1;
                                return;
                            }
                        }
                        NodeRef::Cell(cell) => {
                            mynode = cell;
                            kid_index = subindex(xp, level);
                            level >>= 1;
                        }
                        NodeRef::Body(_) => unreachable!("barnes tree stores bodies in leaves"),
                    }
                    if let NodeRef::Cell(cell) = self.cell(mynode).subp[kid_index] {
                        mynode = cell;
                        kid_index = subindex(xp, level);
                        level >>= 1;
                    }
                }
            }
        }

        unsafe fn subdivide_leaf(&mut self, leaf: usize, parent: NodeRef, level: i32) -> usize {
            unsafe {
                let num_bodies = self.leaf(leaf).num_bodies;
                let mut bodies = [0usize; MAX_BODIES_PER_LEAF];
                for (i, body) in bodies.iter_mut().enumerate().take(num_bodies) {
                    *body = self.leaf(leaf).bodyp[i];
                }
                self.leaf_mut(leaf).num_bodies = 0;

                let cell = self.makecell(parent);
                let child_num = self.leaf(leaf).child_num;
                self.cell_mut(cell).child_num = child_num;

                let first = bodies[0];
                let first_index = subindex(self.intcoord(self.body(first).pos), level);
                {
                    let cell_ref = self.cell_mut(cell);
                    cell_ref.subp[first_index] = NodeRef::Leaf(leaf);
                }
                {
                    let leaf_ref = self.leaf_mut(leaf);
                    leaf_ref.child_num = first_index as i32;
                    leaf_ref.parent = NodeRef::Cell(cell);
                    leaf_ref.level = level >> 1;
                    leaf_ref.bodyp[0] = first;
                    leaf_ref.num_bodies = 1;
                }
                {
                    let body = self.body_mut(first);
                    body.parent = NodeRef::Leaf(leaf);
                    body.child_num = 0;
                    body.level = level >> 1;
                }

                for &body_index in bodies.iter().take(num_bodies).skip(1) {
                    let index = subindex(self.intcoord(self.body(body_index).pos), level);
                    let target_leaf = match self.cell(cell).subp[index] {
                        NodeRef::None => {
                            let new_leaf = self.makeleaf(NodeRef::Cell(cell));
                            self.leaf_mut(new_leaf).child_num = index as i32;
                            self.cell_mut(cell).subp[index] = NodeRef::Leaf(new_leaf);
                            new_leaf
                        }
                        NodeRef::Leaf(existing) => existing,
                        _ => unreachable!("new barnes subdivision only contains leaves"),
                    };
                    let child = self.leaf(target_leaf).num_bodies;
                    self.leaf_mut(target_leaf).bodyp[child] = body_index;
                    self.leaf_mut(target_leaf).num_bodies += 1;
                    let body = self.body_mut(body_index);
                    body.parent = NodeRef::Leaf(target_leaf);
                    body.child_num = child as i32;
                    body.level = level >> 1;
                }
                cell
            }
        }

        unsafe fn hackcofm(&mut self) {
            unsafe {
                for i in (0..self.mynleaf).rev() {
                    let leaf_index = *self.myleaftab.get(i);
                    let num_bodies = self.leaf(leaf_index).num_bodies;
                    let mut mass = 0.0;
                    let mut cost = 0;
                    let mut pos = [0.0; NDIM];
                    for j in 0..num_bodies {
                        let body_index = self.leaf(leaf_index).bodyp[j];
                        let body = self.body(body_index);
                        mass += body.mass;
                        cost += body.cost;
                        add_assign(&mut pos, mul_scalar(body.pos, body.mass));
                    }
                    div_scalar(&mut pos, mass);
                    let leaf = self.leaf_mut(leaf_index);
                    leaf.mass = mass;
                    leaf.cost = cost;
                    leaf.pos = pos;
                    leaf.done = true;
                }

                for i in (0..self.myncell).rev() {
                    let cell_index = *self.mycelltab.get(i);
                    let mut mass = 0.0;
                    let mut cost = 0;
                    let mut pos = [0.0; NDIM];
                    for child in self.cell(cell_index).subp {
                        if child != NodeRef::None {
                            let child_mass = self.mass(child);
                            mass += child_mass;
                            cost += self.cost(child);
                            add_assign(&mut pos, mul_scalar(self.pos(child), child_mass));
                            self.set_done(child, false);
                        }
                    }
                    div_scalar(&mut pos, mass);
                    let cell = self.cell_mut(cell_index);
                    cell.mass = mass;
                    cell.cost = cost;
                    cell.pos = pos;
                    cell.done = true;
                }
            }
        }

        unsafe fn housekeep(&mut self) {
            self.min = [1e99; NDIM];
            self.max = [-1e99; NDIM];
        }

        unsafe fn collect_my_bodies(&mut self) {
            unsafe {
                self.mynbody = 0;
                self.collect_bodies(NodeRef::Cell(self.root));
            }
        }

        unsafe fn collect_bodies(&mut self, node: NodeRef) {
            unsafe {
                match node {
                    NodeRef::Leaf(leaf) => {
                        for i in 0..self.leaf(leaf).num_bodies {
                            let body = self.leaf(leaf).bodyp[i];
                            self.mybodytab.write(self.mynbody, body);
                            self.mynbody += 1;
                        }
                    }
                    NodeRef::Cell(cell) => {
                        for child in self.cell(cell).subp {
                            if child != NodeRef::None {
                                self.collect_bodies(child);
                            }
                        }
                    }
                    NodeRef::Body(body) => {
                        self.mybodytab.write(self.mynbody, body);
                        self.mynbody += 1;
                    }
                    NodeRef::None => {}
                }
            }
        }

        unsafe fn compute_forces(&mut self) {
            unsafe {
                for i in 0..self.mynbody {
                    let body_index = *self.mybodytab.get(i);
                    let acc1 = self.body(body_index).acc;
                    self.body_mut(body_index).cost = 0;
                    self.hackgrav(body_index);
                    if self.nstep > 0 {
                        let dacc = sub(self.body(body_index).acc, acc1);
                        let dvel = mul_scalar(dacc, self.dthf);
                        add_assign(&mut self.body_mut(body_index).vel, dvel);
                    }
                }
            }
        }

        unsafe fn hackgrav(&mut self, body_index: usize) {
            unsafe {
                self.pskip = body_index;
                self.pos0 = self.body(body_index).pos;
                self.phi0 = 0.0;
                self.acc0 = [0.0; NDIM];
                self.myn2bterm = 0;
                self.mynbcterm = 0;
                self.skipself = false;
                self.walksub(NodeRef::Cell(self.root), self.rsize * self.rsize);
                let myn2bterm = self.myn2bterm;
                let mynbcterm = self.mynbcterm;
                let phi0 = self.phi0;
                let acc0 = self.acc0;
                let body = self.body_mut(body_index);
                body.phi = phi0;
                body.acc = acc0;
                body.cost = myn2bterm + mynbcterm;
            }
        }

        unsafe fn walksub(&mut self, node: NodeRef, dsq: f64) {
            unsafe {
                if self.subdivp(node, dsq) {
                    match node {
                        NodeRef::Cell(cell) => {
                            for child in self.cell(cell).subp {
                                if child != NodeRef::None {
                                    self.walksub(child, dsq / 4.0);
                                }
                            }
                        }
                        NodeRef::Leaf(leaf) => {
                            for i in 0..self.leaf(leaf).num_bodies {
                                let body = self.leaf(leaf).bodyp[i];
                                if body != self.pskip {
                                    self.gravsub(NodeRef::Body(body));
                                } else {
                                    self.skipself = true;
                                }
                            }
                        }
                        NodeRef::Body(body) => {
                            if body != self.pskip {
                                self.gravsub(node);
                            } else {
                                self.skipself = true;
                            }
                        }
                        NodeRef::None => {}
                    }
                } else {
                    self.gravsub(node);
                }
            }
        }

        unsafe fn subdivp(&mut self, node: NodeRef, dsq: f64) -> bool {
            unsafe {
                self.dr = sub(self.pos(node), self.pos0);
                self.drsq = dot(self.dr, self.dr);
                self.pmem = node;
                self.tolsq * self.drsq < dsq
            }
        }

        unsafe fn gravsub(&mut self, node: NodeRef) {
            unsafe {
                if node != self.pmem {
                    self.dr = sub(self.pos(node), self.pos0);
                    self.drsq = dot(self.dr, self.dr);
                }
                self.drsq += self.epssq;
                let drabs = self.drsq.sqrt();
                let phii = self.mass(node) / drabs;
                self.phi0 -= phii;
                let mor3 = phii / self.drsq;
                add_assign(&mut self.acc0, mul_scalar(self.dr, mor3));
                if self.typ(node) != BODY {
                    self.mynbcterm += 1;
                } else {
                    self.myn2bterm += 1;
                }
            }
        }

        unsafe fn stepsystem(&mut self) {
            unsafe {
                self.init_root();
                self.maketree();
                self.housekeep();
                self.collect_my_bodies();
                self.compute_forces();

                for i in 0..self.mynbody {
                    let body_index = *self.mybodytab.get(i);
                    let acc = self.body(body_index).acc;
                    let vel = self.body(body_index).vel;
                    let dvel = mul_scalar(acc, self.dthf);
                    let vel1 = add(vel, dvel);
                    let dpos = mul_scalar(vel1, DTIME);
                    {
                        let body = self.body_mut(body_index);
                        add_assign(&mut body.pos, dpos);
                        body.vel = add(vel1, dvel);
                    }
                    let pos = self.body(body_index).pos;
                    for (k, value) in pos.into_iter().enumerate() {
                        self.min[k] = self.min[k].min(value);
                        self.max[k] = self.max[k].max(value);
                    }
                }

                self.rsize = 0.0;
                for k in 0..NDIM {
                    self.max[k] -= self.min[k];
                    self.rsize = self.rsize.max(self.max[k]);
                    self.rmin[k] = self.min[k] - self.rsize / 100000.0;
                }
                self.rsize *= 1.00002;
                self.min = [1e99; NDIM];
                self.max = [-1e99; NDIM];
                self.nstep += 1;
                self.tnow += DTIME;
            }
        }

        unsafe fn run(&mut self) {
            unsafe {
                while self.tnow < TSTOP + 0.1 * DTIME {
                    self.stepsystem();
                }
                let mut sum = 0.0;
                for i in 0..self.nbody {
                    let body = self.body(i);
                    sum += body.phi + body.pos[0] + body.vel[1] + body.acc[2];
                }
                self.checksum = sum;
                std::hint::black_box(self.checksum);
                std::hint::black_box((
                    self.maxleaf,
                    self.maxcell,
                    self.maxmybody,
                    self.alloc.name(),
                ));
            }
        }

        unsafe fn typ(&self, node: NodeRef) -> i16 {
            unsafe {
                match node {
                    NodeRef::Body(body) => self.body(body).typ,
                    NodeRef::Cell(cell) => self.cell(cell).typ,
                    NodeRef::Leaf(leaf) => self.leaf(leaf).typ,
                    NodeRef::None => 0,
                }
            }
        }

        unsafe fn mass(&self, node: NodeRef) -> f64 {
            unsafe {
                match node {
                    NodeRef::Body(body) => self.body(body).mass,
                    NodeRef::Cell(cell) => self.cell(cell).mass,
                    NodeRef::Leaf(leaf) => self.leaf(leaf).mass,
                    NodeRef::None => 0.0,
                }
            }
        }

        unsafe fn cost(&self, node: NodeRef) -> i32 {
            unsafe {
                match node {
                    NodeRef::Body(body) => self.body(body).cost,
                    NodeRef::Cell(cell) => self.cell(cell).cost,
                    NodeRef::Leaf(leaf) => self.leaf(leaf).cost,
                    NodeRef::None => 0,
                }
            }
        }

        unsafe fn pos(&self, node: NodeRef) -> [f64; NDIM] {
            unsafe {
                match node {
                    NodeRef::Body(body) => self.body(body).pos,
                    NodeRef::Cell(cell) => self.cell(cell).pos,
                    NodeRef::Leaf(leaf) => self.leaf(leaf).pos,
                    NodeRef::None => [0.0; NDIM],
                }
            }
        }

        unsafe fn set_done(&mut self, node: NodeRef, done: bool) {
            unsafe {
                match node {
                    NodeRef::Cell(cell) => self.cell_mut(cell).done = done,
                    NodeRef::Leaf(leaf) => self.leaf_mut(leaf).done = done,
                    _ => {}
                }
            }
        }

        fn intcoord(&self, rp: [f64; NDIM]) -> [i32; NDIM] {
            let mut xp = [0; NDIM];
            for k in 0..NDIM {
                let xsc = (rp[k] - self.rmin[k]) / self.rsize;
                assert!((0.0..1.0).contains(&xsc), "barnes body out of root bounds");
                xp[k] = (IMAX as f64 * xsc).floor() as i32;
            }
            xp
        }
    }

    fn subindex(x: [i32; NDIM], level: i32) -> usize {
        let mut i = 0usize;
        let mut yes = false;
        if (x[0] & level) != 0 {
            i += NSUB >> 1;
            yes = true;
        }
        for (k, value) in x.iter().enumerate().skip(1) {
            if (((value & level) != 0) && !yes) || (((value & level) == 0) && yes) {
                i += NSUB >> (k + 1);
                yes = true;
            } else {
                yes = false;
            }
        }
        i
    }

    fn add(a: [f64; NDIM], b: [f64; NDIM]) -> [f64; NDIM] {
        [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
    }

    fn sub(a: [f64; NDIM], b: [f64; NDIM]) -> [f64; NDIM] {
        [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
    }

    fn mul_scalar(a: [f64; NDIM], scalar: f64) -> [f64; NDIM] {
        [a[0] * scalar, a[1] * scalar, a[2] * scalar]
    }

    fn dot(a: [f64; NDIM], b: [f64; NDIM]) -> f64 {
        a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
    }

    fn add_assign(a: &mut [f64; NDIM], b: [f64; NDIM]) {
        for k in 0..NDIM {
            a[k] += b[k];
        }
    }

    fn sub_assign(a: &mut [f64; NDIM], b: [f64; NDIM]) {
        for k in 0..NDIM {
            a[k] -= b[k];
        }
    }

    fn div_scalar(a: &mut [f64; NDIM], scalar: f64) {
        for value in a {
            *value /= scalar;
        }
    }

    pub unsafe fn run(alloc: BenchAllocator) -> Duration {
        unsafe {
            let start = Instant::now();
            let mut sim = Sim::new(alloc);
            sim.run();
            start.elapsed()
        }
    }
}

mod mleak {
    use super::*;

    const THREADS: usize = 10;
    const BASEITER: usize = 100;
    const ALLOC_WORDS: usize = 16;
    const TRANSFERS: usize = THREADS * BASEITER;

    struct LeakState {
        alloc: BenchAllocator,
        transfer: *mut AtomicPtr<u8>,
    }

    static CURRENT_STATE: AtomicPtr<LeakState> = AtomicPtr::new(ptr::null_mut());

    unsafe extern "C" {
        fn srand(seed: c_uint);
        fn rand() -> c_int;
    }

    unsafe fn leak_one(state: &LeakState) {
        unsafe {
            let p = state
                .alloc
                .calloc(ALLOC_WORDS, std::mem::size_of::<*mut c_void>());
            assert!(!p.is_null(), "calloc failed in mleak");
            let i = (rand() as usize) % TRANSFERS;
            let q = (*state.transfer.add(i)).swap(p, Ordering::SeqCst);
            state.alloc.free(q);
        }
    }

    extern "C" fn thread_entry(_arg: *mut c_void) -> *mut c_void {
        unsafe {
            let state = CURRENT_STATE.load(Ordering::SeqCst);
            assert!(!state.is_null(), "mleak thread started without state");
            leak_one(&*state);
        }
        ptr::null_mut()
    }

    unsafe fn run_os_threads(alloc: BenchAllocator) {
        unsafe {
            let threads = alloc.calloc(THREADS, std::mem::size_of::<pthread::PthreadT>());
            assert!(
                !threads.is_null(),
                "thread handle allocation failed in mleak"
            );
            let threads = threads.cast::<pthread::PthreadT>();

            for i in 0..THREADS {
                let rc = pthread::pthread_create(
                    threads.add(i),
                    ptr::null(),
                    thread_entry,
                    i as *mut c_void,
                );
                assert_eq!(rc, 0, "pthread_create failed in mleak: {rc}");
            }

            for i in 0..THREADS {
                let rc = pthread::pthread_join(*threads.add(i), ptr::null_mut());
                assert_eq!(rc, 0, "pthread_join failed in mleak: {rc}");
            }

            alloc.free(threads.cast());
        }
    }

    pub unsafe fn run(alloc: BenchAllocator, iter: usize) -> Duration {
        unsafe {
            let start = Instant::now();
            srand(0x1ce4e5b9);
            let mut transfer: [AtomicPtr<u8>; TRANSFERS] =
                std::array::from_fn(|_| AtomicPtr::new(ptr::null_mut()));
            let mut state = LeakState {
                alloc,
                transfer: transfer.as_mut_ptr(),
            };

            CURRENT_STATE.store(&mut state, Ordering::SeqCst);
            for _ in 0..(iter * BASEITER) {
                run_os_threads(alloc);
            }
            CURRENT_STATE.store(ptr::null_mut(), Ordering::SeqCst);

            for slot in &transfer {
                alloc.free(slot.load(Ordering::SeqCst));
            }
            start.elapsed()
        }
    }
}

fn bench_native_ports(c: &mut Criterion) {
    let root = workspace_root();
    let scenarios = scenarios();
    let allocators = allocators(&root);

    assert!(
        !scenarios.is_empty(),
        "no native mimalloc-bench scenarios selected; check MIMALLOC_BENCH_SCENARIOS"
    );
    assert!(
        !allocators.is_empty(),
        "no allocators selected; check MIMALLOC_BENCH_ALLOCATORS"
    );

    let mut group = c.benchmark_group("mimalloc-bench-native");
    group.sample_size(
        env::var("MIMALLOC_BENCH_SAMPLE_SIZE")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(10)
            .max(10),
    );
    group.measurement_time(Duration::from_secs(
        env::var("MIMALLOC_BENCH_MEASUREMENT_SECS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(10)
            .max(1),
    ));
    group.warm_up_time(Duration::from_secs(
        env::var("MIMALLOC_BENCH_WARMUP_SECS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(1)
            .max(1),
    ));
    group.sampling_mode(SamplingMode::Flat);

    for scenario in &scenarios {
        for &allocator in &allocators {
            group.bench_function(BenchmarkId::new(scenario.name, allocator.name()), |b| {
                b.iter_custom(|iters| {
                    let mut total = Duration::ZERO;
                    for _ in 0..iters {
                        total += unsafe { (scenario.run)(allocator) };
                    }
                    total
                });
            });
        }
    }

    group.finish();
}

criterion_group!(mimalloc_bench_native, bench_native_ports);
criterion_main!(mimalloc_bench_native);
