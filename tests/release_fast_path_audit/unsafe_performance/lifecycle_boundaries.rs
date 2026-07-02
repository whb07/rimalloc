use super::*;

#[path = "lifecycle_boundaries/options_guarded.rs"]
mod options_guarded;
#[path = "lifecycle_boundaries/platform_hooks.rs"]
mod platform_hooks;
#[path = "lifecycle_boundaries/process_lifecycle.rs"]
mod process_lifecycle;
#[path = "lifecycle_boundaries/random_cpu.rs"]
mod random_cpu;
#[path = "lifecycle_boundaries/thread_tls.rs"]
mod thread_tls;

#[test]
fn lifecycle_and_local_boundary_helpers_stay_classified() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let sources = UnsafePerformanceSources::load(root);

    process_lifecycle::assert_process_lifecycle_boundaries(&sources);
    thread_tls::assert_thread_tls_boundaries(&sources);
    options_guarded::assert_options_and_guarded_boundaries(&sources);
    random_cpu::assert_random_and_cpu_boundaries(&sources);
    platform_hooks::assert_platform_hook_boundaries(&sources);
}
