//! Criterion wrapper around the upstream `mimalloc-bench/bench` executables.
//!
//! The timed work stays in the original C/C++ benchmark programs. This harness
//! only prepares the benchmark executables, builds C mimalloc 3.3.2, and runs
//! each scenario under `LD_PRELOAD` for the Rust and C allocators.
//!
//! Examples:
//!   cargo bench --features bench --bench mimalloc_bench -- cfrac
//!   cargo bench --features bench --bench mimalloc_bench -- larsonN-sized/rust
//!
//! Useful environment variables:
//!   MIMALLOC_BENCH_SCENARIOS=cfrac,alloc-testN
//!   MIMALLOC_BENCH_ALLOCATORS=rust,c,system
//!   MIMALLOC_BENCH_PROCS=32
//!   MIMALLOC_RUST_LIB=/path/to/libmechanical_mimalloc.so
//!   MIMALLOC_C_LIB=/path/to/libmimalloc.so
//!   MIMALLOC_BENCH_SOURCE_DIR=/path/to/mimalloc-bench
//!   MIMALLOC_BENCH_BIN_DIR=/path/to/mimalloc-bench/out/bench
//!   MIMALLOC_BENCH_SKIP_BUILD=1

use criterion::{BenchmarkId, Criterion, SamplingMode, criterion_group, criterion_main};
use std::env;
use std::ffi::OsString;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Clone, Copy)]
enum Metric {
    Wall,
    LarsonRelative,
    RptestRelative,
    XmallocRelative,
    GlibcThreadRelative,
}

#[derive(Clone)]
struct Scenario {
    name: &'static str,
    executable: &'static str,
    args: Vec<String>,
    stdin: Option<PathBuf>,
    metric: Metric,
}

#[derive(Clone)]
struct Allocator {
    name: &'static str,
    preload: Option<PathBuf>,
    sysmalloc: bool,
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn logical_cpus() -> usize {
    env::var("MIMALLOC_BENCH_PROCS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|&value| value > 0)
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, |n| n.get()))
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

fn run_status(mut command: Command, action: &str) {
    let status = command
        .status()
        .unwrap_or_else(|err| panic!("failed to run {action}: {err}"));
    assert!(status.success(), "{action} failed with status {status}");
}

fn ensure_rust_cdylib(root: &Path) -> PathBuf {
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
    if !lib.exists() && !skip_build() {
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

fn bench_source_root(root: &Path) -> PathBuf {
    env::var_os("MIMALLOC_BENCH_SOURCE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("mimalloc-bench"))
}

fn ensure_bench_executables(root: &Path) -> PathBuf {
    if let Some(path) = env::var_os("MIMALLOC_BENCH_BIN_DIR") {
        let path = PathBuf::from(path);
        assert!(
            path.exists(),
            "MIMALLOC_BENCH_BIN_DIR does not exist: {}",
            path.display()
        );
        return path;
    }

    let source_dir = bench_source_root(root).join("bench");
    let build_dir = root.join("target/mimalloc-bench");
    assert!(
        source_dir.join("CMakeLists.txt").exists(),
        "missing mimalloc-bench sources at {}",
        source_dir.display()
    );

    let required = [
        "cfrac",
        "espresso",
        "barnes",
        "alloc-test",
        "larson",
        "larson-sized",
        "sh6bench",
        "sh8bench",
        "xmalloc-test",
        "cache-thrash",
        "cache-scratch",
        "malloc-large",
        "mstress",
        "mleak",
        "rptest",
        "glibc-simple",
        "glibc-thread",
    ];
    let missing = required.iter().any(|name| !build_dir.join(name).exists());

    if missing && !skip_build() {
        let mut configure = Command::new("cmake");
        configure
            .current_dir(root)
            .arg("-S")
            .arg(&source_dir)
            .arg("-B")
            .arg(&build_dir)
            .arg("-DCMAKE_BUILD_TYPE=Release");
        run_status(configure, "configure mimalloc-bench executables");

        let mut build = Command::new("cmake");
        build
            .current_dir(root)
            .arg("--build")
            .arg(&build_dir)
            .arg("--config")
            .arg("Release")
            .arg("--parallel")
            .arg(logical_cpus().to_string());
        run_status(build, "build mimalloc-bench executables");
    }

    for name in required {
        assert!(
            build_dir.join(name).exists(),
            "missing benchmark executable {}; build mimalloc-bench or unset MIMALLOC_BENCH_SKIP_BUILD",
            build_dir.join(name).display()
        );
    }
    build_dir
}

fn scenarios(root: &Path, procs: usize) -> Vec<Scenario> {
    let bench_dir = bench_source_root(root).join("bench");
    let procs_x2 = procs.saturating_mul(2);
    let procs_max16 = procs.min(16);

    let mut scenarios = vec![
        Scenario {
            name: "cfrac",
            executable: "cfrac",
            args: vec!["17545186520507317056371138836327483792789528".into()],
            stdin: None,
            metric: Metric::Wall,
        },
        Scenario {
            name: "espresso",
            executable: "espresso",
            args: vec![
                bench_dir
                    .join("espresso/largest.espresso")
                    .display()
                    .to_string(),
            ],
            stdin: None,
            metric: Metric::Wall,
        },
        Scenario {
            name: "barnes",
            executable: "barnes",
            args: Vec::new(),
            stdin: Some(bench_dir.join("barnes/input")),
            metric: Metric::Wall,
        },
        Scenario {
            name: "alloc-test1",
            executable: "alloc-test",
            args: vec!["1".into()],
            stdin: None,
            metric: Metric::Wall,
        },
        Scenario {
            name: "larsonN",
            executable: "larson",
            args: vec![
                "5".into(),
                "8".into(),
                "1000".into(),
                "5000".into(),
                "100".into(),
                "4141".into(),
                procs.to_string(),
            ],
            stdin: None,
            metric: Metric::LarsonRelative,
        },
        Scenario {
            name: "larsonN-sized",
            executable: "larson-sized",
            args: vec![
                "5".into(),
                "8".into(),
                "1000".into(),
                "5000".into(),
                "100".into(),
                "4141".into(),
                procs.to_string(),
            ],
            stdin: None,
            metric: Metric::LarsonRelative,
        },
        Scenario {
            name: "sh6benchN",
            executable: "sh6bench",
            args: vec![procs_x2.to_string()],
            stdin: None,
            metric: Metric::Wall,
        },
        Scenario {
            name: "sh8benchN",
            executable: "sh8bench",
            args: vec![procs_x2.to_string()],
            stdin: None,
            metric: Metric::Wall,
        },
        Scenario {
            name: "xmalloc-testN",
            executable: "xmalloc-test",
            args: vec![
                "-w".into(),
                procs.to_string(),
                "-t".into(),
                "5".into(),
                "-s".into(),
                "64".into(),
            ],
            stdin: None,
            metric: Metric::XmallocRelative,
        },
        Scenario {
            name: "cache-thrash1",
            executable: "cache-thrash",
            args: vec![
                "1".into(),
                "1000".into(),
                "1".into(),
                "2000000".into(),
                procs.to_string(),
            ],
            stdin: None,
            metric: Metric::Wall,
        },
        Scenario {
            name: "cache-scratch1",
            executable: "cache-scratch",
            args: vec![
                "1".into(),
                "1000".into(),
                "1".into(),
                "2000000".into(),
                procs.to_string(),
            ],
            stdin: None,
            metric: Metric::Wall,
        },
        Scenario {
            name: "malloc-large",
            executable: "malloc-large",
            args: Vec::new(),
            stdin: None,
            metric: Metric::Wall,
        },
        Scenario {
            name: "mstressN",
            executable: "mstress",
            args: vec![procs.to_string(), "50".into(), "25".into()],
            stdin: None,
            metric: Metric::Wall,
        },
        Scenario {
            name: "mleak10",
            executable: "mleak",
            args: vec!["5".into()],
            stdin: None,
            metric: Metric::Wall,
        },
        Scenario {
            name: "mleak100",
            executable: "mleak",
            args: vec!["50".into()],
            stdin: None,
            metric: Metric::Wall,
        },
        Scenario {
            name: "rptestN",
            executable: "rptest",
            args: vec![
                procs.to_string(),
                "0".into(),
                "1".into(),
                "2".into(),
                "500".into(),
                "1000".into(),
                "100".into(),
                "8".into(),
                "16000".into(),
            ],
            stdin: None,
            metric: Metric::RptestRelative,
        },
        Scenario {
            name: "glibc-simple",
            executable: "glibc-simple",
            args: Vec::new(),
            stdin: None,
            metric: Metric::Wall,
        },
        Scenario {
            name: "glibc-thread",
            executable: "glibc-thread",
            args: vec![procs.to_string()],
            stdin: None,
            metric: Metric::GlibcThreadRelative,
        },
    ];

    if procs != 1 {
        scenarios.insert(
            4,
            Scenario {
                name: "alloc-testN",
                executable: "alloc-test",
                args: vec![procs_max16.to_string()],
                stdin: None,
                metric: Metric::Wall,
            },
        );
    }

    if procs != 1 {
        scenarios.push(Scenario {
            name: "cache-thrashN",
            executable: "cache-thrash",
            args: vec![
                procs.to_string(),
                "1000".into(),
                "1".into(),
                "2000000".into(),
                procs.to_string(),
            ],
            stdin: None,
            metric: Metric::Wall,
        });
        scenarios.push(Scenario {
            name: "cache-scratchN",
            executable: "cache-scratch",
            args: vec![
                procs.to_string(),
                "1000".into(),
                "1".into(),
                "2000000".into(),
                procs.to_string(),
            ],
            stdin: None,
            metric: Metric::Wall,
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

fn allocators(root: &Path) -> Vec<Allocator> {
    let selected = csv_env("MIMALLOC_BENCH_ALLOCATORS");
    let selected = if selected.is_empty() {
        vec!["rust".to_owned(), "c".to_owned()]
    } else {
        selected
    };

    let rust_lib = selected
        .iter()
        .any(|name| name == "rust")
        .then(|| ensure_rust_cdylib(root));
    let c_lib = selected
        .iter()
        .any(|name| name == "c" || name == "c-mimalloc" || name == "c-mimalloc-3.3.2")
        .then(|| ensure_c_mimalloc(root));

    selected
        .into_iter()
        .map(|name| match name.as_str() {
            "rust" => Allocator {
                name: "rust",
                preload: rust_lib.clone(),
                sysmalloc: false,
            },
            "c" | "c-mimalloc" | "c-mimalloc-3.3.2" => Allocator {
                name: "c-mimalloc-3.3.2",
                preload: c_lib.clone(),
                sysmalloc: false,
            },
            "system" | "sys" => Allocator {
                name: "system",
                preload: None,
                sysmalloc: true,
            },
            other => panic!("unknown MIMALLOC_BENCH_ALLOCATORS entry `{other}`"),
        })
        .collect()
}

fn parse_number_after(output: &str, marker: &str) -> Option<f64> {
    let start = output.find(marker)? + marker.len();
    let tail = &output[start..];
    let number: String = tail
        .chars()
        .skip_while(|ch| ch.is_whitespace())
        .take_while(|ch| ch.is_ascii_digit() || *ch == '.')
        .collect();
    number.parse().ok()
}

fn parse_number_before(output: &str, marker: &str) -> Option<f64> {
    let end = output.find(marker)?;
    let head = &output[..end];
    let number_rev: String = head
        .chars()
        .rev()
        .skip_while(|ch| ch.is_whitespace())
        .take_while(|ch| ch.is_ascii_digit() || *ch == '.')
        .collect();
    let number: String = number_rev.chars().rev().collect();
    number.parse().ok()
}

fn parse_first_number(output: &str) -> Option<f64> {
    output
        .split_whitespace()
        .find_map(|word| word.parse::<f64>().ok())
}

fn seconds_from_metric(metric: Metric, elapsed: Duration, stdout: &str) -> Duration {
    let seconds = match metric {
        Metric::Wall => return elapsed,
        Metric::LarsonRelative => parse_number_after(stdout, "relative time: "),
        Metric::RptestRelative => parse_number_before(stdout, " memory ops")
            .map(|ops_per_cpu_second| 2_000_000.0 / ops_per_cpu_second),
        Metric::XmallocRelative => parse_number_after(stdout, "rtime: "),
        Metric::GlibcThreadRelative => {
            parse_first_number(stdout).map(|iterations| 1_000_000_000.0 / iterations)
        }
    }
    .unwrap_or_else(|| {
        panic!(
            "could not parse benchmark metric from stdout:\n{}",
            &stdout[..stdout.len().min(4000)]
        )
    });
    Duration::from_secs_f64(seconds)
}

fn run_once(bin_dir: &Path, scenario: &Scenario, allocator: &Allocator) -> Duration {
    let executable = bin_dir.join(scenario.executable);
    let stdin = match &scenario.stdin {
        Some(path) => Stdio::from(
            File::open(path)
                .unwrap_or_else(|err| panic!("failed to open stdin {}: {err}", path.display())),
        ),
        None => Stdio::null(),
    };

    let mut command = Command::new(&executable);
    command
        .current_dir(bin_dir)
        .args(&scenario.args)
        .stdin(stdin)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(preload) = &allocator.preload {
        command.env("LD_PRELOAD", preload);
    }
    if allocator.sysmalloc {
        command.env("SYSMALLOC", "1");
    }
    if let Some(ld_library_path) = allocator
        .preload
        .as_ref()
        .and_then(|path| path.parent())
        .map(Path::as_os_str)
    {
        let mut value = OsString::from(ld_library_path);
        if let Some(existing) = env::var_os("LD_LIBRARY_PATH") {
            value.push(":");
            value.push(existing);
        }
        command.env("LD_LIBRARY_PATH", value);
    }

    let start = Instant::now();
    let output = command.output().unwrap_or_else(|err| {
        panic!(
            "failed to run {} for {}: {err}",
            executable.display(),
            scenario.name
        )
    });
    let elapsed = start.elapsed();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "benchmark {} with allocator {} failed with status {}\nstdout:\n{}\nstderr:\n{}",
        scenario.name,
        allocator.name,
        output.status,
        &stdout[..stdout.len().min(4000)],
        &stderr[..stderr.len().min(4000)]
    );

    seconds_from_metric(scenario.metric, elapsed, &stdout)
}

fn bench_mimalloc_bench(c: &mut Criterion) {
    let root = workspace_root();
    let procs = logical_cpus();
    let bin_dir = ensure_bench_executables(&root);
    let scenarios = scenarios(&root, procs);
    let allocators = allocators(&root);

    assert!(
        !scenarios.is_empty(),
        "no mimalloc-bench scenarios selected; check MIMALLOC_BENCH_SCENARIOS"
    );
    assert!(
        !allocators.is_empty(),
        "no allocators selected; check MIMALLOC_BENCH_ALLOCATORS"
    );

    let mut group = c.benchmark_group("mimalloc-bench");
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
        for allocator in &allocators {
            group.bench_function(BenchmarkId::new(scenario.name, allocator.name), |b| {
                b.iter_custom(|iters| {
                    let mut total = Duration::ZERO;
                    for _ in 0..iters {
                        total += run_once(&bin_dir, scenario, allocator);
                    }
                    total
                });
            });
        }
    }

    group.finish();
}

criterion_group!(mimalloc_bench, bench_mimalloc_bench);
criterion_main!(mimalloc_bench);
