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
//!   MIMALLOC_BENCH_SCENARIOS=glibc-simple,mleak10
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
use std::sync::OnceLock;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::time::{Duration, Instant};

#[derive(Clone, Copy)]
struct DylibAllocator {
    malloc: unsafe extern "C" fn(usize) -> *mut c_void,
    calloc: unsafe extern "C" fn(usize, usize) -> *mut c_void,
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
    run: unsafe fn(BenchAllocator),
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
    let scenarios = vec![
        Scenario {
            name: "glibc-simple",
            run: glibc_simple::run,
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

    pub unsafe fn run(alloc: BenchAllocator) {
        unsafe {
            bench_size(alloc, 16);
            bench_size(alloc, 32);
            bench_size(alloc, 64);
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

    pub unsafe fn run(alloc: BenchAllocator, iter: usize) {
        unsafe {
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
                        let start = Instant::now();
                        unsafe { (scenario.run)(allocator) };
                        total += start.elapsed();
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
