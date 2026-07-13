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
}

#[test]
fn every_hotpath_timed_helper_uses_the_runtime_allocation_guard() {
    let hotpath = std::fs::read_to_string("benches/hotpath.rs").expect("read hotpath benchmark");
    for marker in [
        "fn paired_alloc_free_layout<",
        "fn free_routine<",
        "fn lifo_stack<",
        "fn single_reuse<",
    ] {
        let item = source_item("benches/hotpath.rs", &hotpath, marker);
        assert!(
            item.contains("assert_no_harness_allocations"),
            "{marker} must use the runtime harness-allocation guard"
        );
    }

    let remote = std::fs::read_to_string("benches/support/remote_fixture.rs")
        .expect("read remote-free fixture");
    assert!(
        source_item(
            "benches/support/remote_fixture.rs",
            &remote,
            "fn remote_free_timed_routine<",
        )
        .contains("assert_no_harness_allocations"),
        "remote-free timing must use the runtime harness-allocation guard"
    );
}
