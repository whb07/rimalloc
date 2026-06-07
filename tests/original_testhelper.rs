#![allow(dead_code)]

use std::sync::atomic::{AtomicUsize, Ordering};

static OK_COUNT: AtomicUsize = AtomicUsize::new(0);
static FAILED_COUNT: AtomicUsize = AtomicUsize::new(0);

pub(crate) fn reset_test_summary() {
    OK_COUNT.store(0, Ordering::Relaxed);
    FAILED_COUNT.store(0, Ordering::Relaxed);
}

pub(crate) fn check_result(result: bool, testname: &str, file: &str, line: u32) -> bool {
    if result {
        OK_COUNT.fetch_add(1, Ordering::Relaxed);
        eprintln!("test: {testname}... ok");
    } else {
        FAILED_COUNT.fetch_add(1, Ordering::Relaxed);
        eprintln!("test: {testname}... FAILED: {file}:{line}");
    }
    result
}

pub(crate) fn check(name: &str, result: bool) -> bool {
    check_result(result, name, file!(), line!())
}

pub(crate) fn print_test_summary() -> i32 {
    let ok = OK_COUNT.load(Ordering::Relaxed);
    let failed = FAILED_COUNT.load(Ordering::Relaxed);
    eprintln!("test summary: {ok} ok, {failed} failed");
    if failed == 0 { 0 } else { 1 }
}
