//! Runs one allocator resource scenario and emits one JSON-lines record.

use rimalloc::{MiMalloc, ProcessInfo, StatsSnapshot};
use std::alloc::{GlobalAlloc, Layout, System};
use std::env;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::mpsc;
use std::thread;
use std::time::Instant;

#[derive(Clone, Copy)]
enum ProbeAllocator {
    Public,
    Raw,
    System,
}

impl ProbeAllocator {
    fn parse(value: &str) -> Self {
        match value {
            "rimalloc_public" => Self::Public,
            "rimalloc_raw" => Self::Raw,
            "system" => Self::System,
            _ => panic!("unsupported probe allocator: {value}"),
        }
    }

    unsafe fn allocate(self, layout: Layout) -> *mut u8 {
        match self {
            Self::Public => unsafe { MiMalloc.alloc(layout) },
            Self::Raw if layout.align() <= 16 => unsafe {
                rimalloc::raw::mi_malloc(layout.size()).cast()
            },
            Self::Raw => unsafe {
                rimalloc::raw::mi_malloc_aligned(layout.size(), layout.align()).cast()
            },
            Self::System => unsafe { System.alloc(layout) },
        }
    }

    unsafe fn deallocate(self, pointer: *mut u8, layout: Layout) {
        match self {
            Self::Public => unsafe { MiMalloc.dealloc(pointer, layout) },
            Self::Raw => unsafe { rimalloc::raw::mi_free(pointer.cast()) },
            Self::System => unsafe { System.dealloc(pointer, layout) },
        }
    }
}

#[derive(Clone)]
struct Config {
    scenario: String,
    allocator_name: String,
    allocator: ProbeAllocator,
    artifact_mode: String,
    features: String,
    size: usize,
    alignment: usize,
    operations: usize,
    threads: usize,
    live_set: usize,
    seed: u64,
}

struct ProbeBatch(Vec<(*mut u8, Layout)>);

// SAFETY: a batch exclusively owns its allocation pointers. Channel transfer
// moves the batch, and only the receiving thread frees the pointers.
unsafe impl Send for ProbeBatch {}

fn value(name: &str, default: &str) -> String {
    env::var(name).unwrap_or_else(|_| default.to_owned())
}

fn number<T: std::str::FromStr>(name: &str, default: &str) -> T {
    value(name, default)
        .parse()
        .unwrap_or_else(|_| panic!("invalid {name}"))
}

fn config() -> Config {
    let allocator_name = value("MIMALLOC_PROBE_ALLOCATOR", "rimalloc_public");
    Config {
        scenario: value("MIMALLOC_PROBE_SCENARIO", "alloc_free"),
        allocator: ProbeAllocator::parse(&allocator_name),
        allocator_name,
        artifact_mode: value("MIMALLOC_PROBE_ARTIFACT_MODE", "default-rust"),
        features: value("MIMALLOC_PROBE_FEATURES", "bench"),
        size: number("MIMALLOC_PROBE_SIZE", "64"),
        alignment: number("MIMALLOC_PROBE_ALIGNMENT", "8"),
        operations: number("MIMALLOC_PROBE_OPERATIONS", "100000"),
        threads: number("MIMALLOC_PROBE_THREADS", "1"),
        live_set: number("MIMALLOC_PROBE_LIVE_SET", "4096"),
        seed: number("MIMALLOC_PROBE_SEED", "16045690984503098046"),
    }
}

fn allocate_batch(allocator: ProbeAllocator, layout: Layout, count: usize) -> ProbeBatch {
    ProbeBatch(
        (0..count)
            .map(|_| {
                let pointer = unsafe { allocator.allocate(layout) };
                assert!(!pointer.is_null(), "probe allocation failed");
                (pointer, layout)
            })
            .collect(),
    )
}

fn free_batch(allocator: ProbeAllocator, allocations: ProbeBatch) {
    for (pointer, layout) in allocations.0 {
        unsafe { allocator.deallocate(pointer, layout) };
    }
}

fn local_alloc_free(config: &Config, layout: Layout) -> usize {
    let mut completed = 0usize;
    let mut peak = 0usize;
    while completed < config.operations {
        let count = config.live_set.min(config.operations - completed).max(1);
        let allocations = allocate_batch(config.allocator, layout, count);
        peak = peak.max(count.saturating_mul(config.size));
        free_batch(config.allocator, allocations);
        completed += count;
    }
    peak
}

fn remote_free_1x1(config: &Config, layout: Layout) -> usize {
    let (command_tx, command_rx) = mpsc::sync_channel::<usize>(1);
    let (allocation_tx, allocation_rx) = mpsc::sync_channel(1);
    let allocator = config.allocator;
    let owner = thread::spawn(move || {
        while let Ok(count) = command_rx.recv() {
            if allocation_tx
                .send(allocate_batch(allocator, layout, count))
                .is_err()
            {
                break;
            }
        }
    });
    let mut completed = 0usize;
    let mut peak = 0usize;
    while completed < config.operations {
        let count = config.live_set.min(config.operations - completed).max(1);
        command_tx.send(count).expect("request remote batch");
        let allocations = allocation_rx.recv().expect("receive remote batch");
        peak = peak.max(count.saturating_mul(config.size));
        free_batch(config.allocator, allocations);
        completed += count;
    }
    drop(command_tx);
    owner.join().expect("probe owner thread");
    peak
}

fn option_json(value: Option<usize>) -> String {
    value.map_or_else(|| "null".to_owned(), |number| number.to_string())
}

fn stat_json(stats: Option<&StatsSnapshot>, value: impl FnOnce(&StatsSnapshot) -> usize) -> String {
    stats.map_or_else(|| "null".to_owned(), |snapshot| value(snapshot).to_string())
}

fn json_string(value: &str) -> String {
    format!("{:?}", value)
}

fn main() {
    let config = config();
    assert!(config.operations > 0, "operations must be nonzero");
    assert!(config.live_set > 0, "live set must be nonzero");
    let layout =
        Layout::from_size_align(config.size, config.alignment).expect("valid probe layout");
    let process_start = ProcessInfo::current();
    let allocator_start = (!matches!(config.allocator, ProbeAllocator::System))
        .then(|| StatsSnapshot::capture().ok())
        .flatten();
    let started = Instant::now();
    let logical_peak = match config.scenario.as_str() {
        "alloc_free" => local_alloc_free(&config, layout),
        "remote_free_1x1" => remote_free_1x1(&config, layout),
        scenario => panic!("unsupported probe scenario: {scenario}"),
    };
    let wall_ns = started.elapsed().as_nanos();
    if !matches!(config.allocator, ProbeAllocator::System) {
        unsafe { rimalloc::raw::mi_collect(true) };
    }
    let process_end = ProcessInfo::current();
    let allocator_end = (!matches!(config.allocator, ProbeAllocator::System))
        .then(|| StatsSnapshot::capture().ok())
        .flatten();
    let page_faults_delta = process_end
        .page_faults()
        .get()
        .saturating_sub(process_start.page_faults().get());
    let operations_per_second = config.operations as f64 * 1_000_000_000.0 / wall_ns as f64;
    let features = config
        .features
        .split(',')
        .filter(|item| !item.is_empty())
        .map(json_string)
        .collect::<Vec<_>>()
        .join(",");
    let record = format!(
        concat!(
            "{{\"schema\":1,\"scenario\":{},\"allocator\":{},\"artifact_mode\":{},",
            "\"features\":[{}],\"size\":{},\"alignment\":{},\"threads\":{},",
            "\"operations\":{},\"seed\":{},\"wall_ns\":{},\"operations_per_second\":{},",
            "\"logical_live_bytes_start\":0,\"logical_live_bytes_peak\":{},",
            "\"logical_live_bytes_end\":0,\"rss_current_start\":{},\"rss_current_end\":{},",
            "\"rss_peak_end\":{},\"commit_current_start\":{},\"commit_current_end\":{},",
            "\"commit_peak_end\":{},\"page_faults_delta\":{},",
            "\"allocator_reserved_current_start\":{},\"allocator_reserved_current_end\":{},",
            "\"allocator_reserved_peak_end\":{},\"allocator_committed_current_start\":{},",
            "\"allocator_committed_current_end\":{},\"allocator_committed_peak_end\":{},",
            "\"allocator_pages_current_end\":{},\"allocator_pages_abandoned_end\":{},",
            "\"exit_status\":0}}"
        ),
        json_string(&config.scenario),
        json_string(&config.allocator_name),
        json_string(&config.artifact_mode),
        features,
        config.size,
        config.alignment,
        config.threads,
        config.operations,
        config.seed,
        wall_ns,
        operations_per_second,
        logical_peak,
        option_json(process_start.current_rss().map(|value| value.get())),
        option_json(process_end.current_rss().map(|value| value.get())),
        option_json(process_end.peak_rss().map(|value| value.get())),
        option_json(process_start.current_commit().map(|value| value.get())),
        option_json(process_end.current_commit().map(|value| value.get())),
        option_json(process_end.peak_commit().map(|value| value.get())),
        page_faults_delta,
        stat_json(allocator_start.as_ref(), |stats| stats
            .reserved_current()
            .get()),
        stat_json(allocator_end.as_ref(), |stats| stats
            .reserved_current()
            .get()),
        stat_json(allocator_end.as_ref(), |stats| stats.reserved_peak().get()),
        stat_json(allocator_start.as_ref(), |stats| stats
            .committed_current()
            .get()),
        stat_json(allocator_end.as_ref(), |stats| stats
            .committed_current()
            .get()),
        stat_json(allocator_end.as_ref(), |stats| stats.committed_peak().get()),
        stat_json(
            allocator_end.as_ref(),
            |stats| stats.pages_current().get().max(0) as usize
        ),
        stat_json(
            allocator_end.as_ref(),
            |stats| stats.pages_abandoned_current().get().max(0) as usize
        ),
    );
    if let Some(path) = env::var_os("MIMALLOC_PROBE_OUTPUT") {
        let mut output = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .expect("open probe output");
        writeln!(output, "{record}").expect("write probe output");
    } else {
        println!("{record}");
    }
}
