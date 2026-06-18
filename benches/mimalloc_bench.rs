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
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

#[derive(Clone, Copy)]
struct DylibAllocator {
    malloc: unsafe extern "C" fn(usize) -> *mut c_void,
    calloc: unsafe extern "C" fn(usize, usize) -> *mut c_void,
    realloc: unsafe extern "C" fn(*mut c_void, usize) -> *mut c_void,
    free: unsafe extern "C" fn(*mut c_void),
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
