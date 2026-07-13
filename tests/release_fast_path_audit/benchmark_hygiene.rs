use super::source_item;

fn assert_timed_helper_has_no_harness_allocation(source_name: &str, source: &str, marker: &str) {
    let item = source_item(source_name, source, marker);
    for forbidden in [
        "vec!",
        "Vec::",
        "Box::",
        "String::",
        "format!",
        "thread::spawn",
        "channel(",
        "sync_channel(",
    ] {
        assert!(
            !item.contains(forbidden),
            "{marker} in {source_name} must keep {forbidden} outside its timed body"
        );
    }
    for forbidden in [
        "assert_no_harness_allocations",
        "harness_allocation_check_enabled",
        "std::env",
        "env::",
    ] {
        assert!(
            !item.contains(forbidden),
            "{marker} in {source_name} must keep {forbidden} outside its timed body"
        );
    }
}

#[test]
fn benchmark_timed_helpers_do_not_allocate_harness_storage() {
    let hotpath = std::fs::read_to_string("benches/hotpath.rs").expect("read hotpath benchmark");
    for marker in [
        "fn paired_alloc_free<",
        "fn paired_alloc_free_layout<",
        "fn free_routine<",
        "fn lifo_stack<",
        "fn single_reuse<",
    ] {
        assert_timed_helper_has_no_harness_allocation("benches/hotpath.rs", &hotpath, marker);
    }

    let remote = std::fs::read_to_string("benches/support/remote_fixture.rs")
        .expect("read remote-free fixture");
    assert_timed_helper_has_no_harness_allocation(
        "benches/support/remote_fixture.rs",
        &remote,
        "fn remote_free_timed_routine<",
    );

    let page_batch =
        std::fs::read_to_string("benches/page_batch.rs").expect("read page-batch benchmark");
    assert_timed_helper_has_no_harness_allocation(
        "benches/page_batch.rs",
        &page_batch,
        "fn measure_prepared_transition(",
    );
    assert!(
        page_batch.contains("const LENGTHS: &[usize] = &[1, 8, 32, 128];")
            && page_batch.contains("[(\"empty\", 0), (\"nonempty\", 3)]")
            && page_batch.contains("fixture.prepare();\n        let start = Instant::now();")
            && page_batch.contains("forced_page_free_collect")
            && page_batch.contains("local_to_available")
            && page_batch.contains("detached_remote")
            && page_batch.contains("secure_encoded"),
        "Stage 3 slow-path cases and out-of-timer preparation must remain explicit"
    );
    assert!(
        hotpath.contains("hot-path benchmark setup allocation failed")
            && remote.contains("remote-free benchmark setup allocation failed"),
        "untimed benchmark setup must reject null allocator results before deallocation"
    );
}

#[test]
fn benchmark_configuration_hoists_the_runtime_allocation_guard() {
    let hotpath = std::fs::read_to_string("benches/hotpath.rs").expect("read hotpath benchmark");
    let timed_configuration = source_item("benches/hotpath.rs", &hotpath, "fn iter_timed<");
    assert!(
        timed_configuration.contains("if verify_harness")
            && timed_configuration.contains("assert_no_harness_allocations_enabled")
            && hotpath.contains("let verify_harness = harness_allocation_check_enabled();"),
        "hot-path configuration must choose diagnostic mode before Criterion iterations"
    );

    let remote = std::fs::read_to_string("benches/support/remote_fixture.rs")
        .expect("read remote-free fixture");
    assert!(
        source_item(
            "benches/support/remote_fixture.rs",
            &remote,
            "pub fn measure_remote_free(",
        )
        .contains("if self.verify_harness"),
        "remote-free fixture must choose diagnostic mode before its sample loop"
    );

    let guard = std::fs::read_to_string("benches/support/no_alloc.rs")
        .expect("read benchmark allocation guard");
    assert!(
        source_item(
            "benches/support/no_alloc.rs",
            &guard,
            "pub fn assert_no_harness_allocations_enabled<",
        )
        .contains("let before = snapshot();"),
        "the enabled allocation guard must snapshot without consulting the environment"
    );
    assert!(
        !source_item(
            "benches/support/no_alloc.rs",
            &guard,
            "pub fn assert_no_harness_allocations_enabled<",
        )
        .contains("std::env"),
        "the timed diagnostic guard must not inspect process environment state"
    );
}
