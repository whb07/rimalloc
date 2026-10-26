//! A fork must not hand the child an allocator lock held by a thread that does
//! not exist in the child.
#![cfg(all(unix, not(miri)))]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use rimalloc::raw::mimalloc::*;

/// Creates and deletes a heap with live blocks, taking the heap-list and page
/// locks that the child below needs as well.
fn churn_heap() {
    // SAFETY: every block belongs to the fresh heap and is freed exactly once.
    unsafe {
        let heap = mi_heap_new();
        assert!(!heap.is_null());
        let block = mi_heap_malloc(heap, 64);
        assert!(!block.is_null());
        mi_free(block);
        mi_heap_delete(heap);
    }
}

fn wait_with_timeout(pid: libc::pid_t, timeout: Duration) -> Option<libc::c_int> {
    let start = Instant::now();
    loop {
        let mut status = 0;
        // SAFETY: `pid` is a child of this process and `status` is writable.
        let waited = unsafe { libc::waitpid(pid, &mut status, libc::WNOHANG) };
        if waited == pid {
            return Some(status);
        }
        assert_eq!(waited, 0, "waitpid failed");
        if start.elapsed() > timeout {
            return None;
        }
        thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn forked_child_can_allocate_while_parent_threads_hold_locks() {
    let stop = Arc::new(AtomicBool::new(false));
    let workers: Vec<_> = (0..4)
        .map(|_| {
            let stop = Arc::clone(&stop);
            thread::spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    churn_heap();
                }
            })
        })
        .collect();

    let mut hung = 0;
    for _ in 0..200 {
        // SAFETY: the child calls only the allocator and `_exit`.
        let pid = unsafe { libc::fork() };
        assert!(pid >= 0, "fork failed");
        if pid == 0 {
            churn_heap();
            // SAFETY: terminates the child without running the parent's exit handlers.
            unsafe { libc::_exit(0) };
        }
        match wait_with_timeout(pid, Duration::from_secs(5)) {
            Some(status) => assert!(
                libc::WIFEXITED(status) && libc::WEXITSTATUS(status) == 0,
                "child failed with status {status:#x}"
            ),
            None => {
                hung += 1;
                // SAFETY: `pid` is a child of this process.
                unsafe {
                    libc::kill(pid, libc::SIGKILL);
                    libc::waitpid(pid, std::ptr::null_mut(), 0);
                }
            }
        }
    }

    stop.store(true, Ordering::Relaxed);
    for worker in workers {
        worker.join().unwrap();
    }
    assert_eq!(
        hung, 0,
        "{hung} forked children blocked on an inherited allocator lock"
    );
}
