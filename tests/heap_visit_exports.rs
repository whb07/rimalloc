use core::ffi::c_void;
use core::ptr;
use std::thread;

use mechanical_mimalloc::raw::{self, mi_heap_area_t, mi_heap_t};

const HUGE_ALLOCATION_SIZE: usize = 1024 * 1024;
const TARGET_CAPACITY: usize = 3;

struct VisitState {
    targets: [*mut c_void; TARGET_CAPACITY],
    target_count: usize,
    target_visits: [usize; TARGET_CAPACITY],
    areas: usize,
    blocks: usize,
}

impl Default for VisitState {
    fn default() -> Self {
        Self {
            targets: [ptr::null_mut(); TARGET_CAPACITY],
            target_count: 0,
            target_visits: [0; TARGET_CAPACITY],
            areas: 0,
            blocks: 0,
        }
    }
}

unsafe extern "C" fn count_blocks(
    _heap: *const mi_heap_t,
    area: *const mi_heap_area_t,
    block: *mut c_void,
    _block_size: usize,
    arg: *mut c_void,
) -> bool {
    let state = unsafe { &mut *arg.cast::<VisitState>() };
    assert!(!area.is_null());
    if block.is_null() {
        state.areas += 1;
    } else {
        state.blocks += 1;
        for index in 0..state.target_count {
            if block == state.targets[index] {
                state.target_visits[index] += 1;
            }
        }
    }
    true
}

#[test]
fn heap_visit_blocks_reports_live_huge_block() {
    unsafe {
        let heap = raw::mi_heap_new();
        assert!(!heap.is_null());

        let target = raw::mi_heap_malloc(heap, HUGE_ALLOCATION_SIZE);
        assert!(!target.is_null());

        let mut state = VisitState {
            targets: [target, ptr::null_mut(), ptr::null_mut()],
            target_count: 1,
            ..VisitState::default()
        };
        assert!(raw::mi_heap_visit_blocks(
            heap,
            true,
            Some(count_blocks),
            ptr::addr_of_mut!(state).cast(),
        ));
        assert!(state.areas > 0);
        assert_eq!(state.target_visits[0], 1);

        raw::mi_free(target);
        raw::mi_heap_delete(heap);
    }
}

#[test]
fn heap_visit_abandoned_blocks_reports_live_blocks() {
    unsafe {
        let heap = raw::mi_heap_new();
        assert!(!heap.is_null());
        let heap_address = heap as usize;

        let blocks = thread::spawn(move || {
            let heap = heap_address as *mut mi_heap_t;
            raw::mi_thread_init();
            let blocks = [
                raw::mi_heap_malloc(heap, 80),
                raw::mi_heap_malloc(heap, 160),
                raw::mi_heap_zalloc(heap, 240),
            ];
            assert!(blocks.iter().all(|block| !block.is_null()));
            raw::mi_thread_done();
            blocks.map(|block| block as usize)
        })
        .join()
        .expect("abandoned-page worker panicked");

        let mut state = VisitState {
            targets: blocks.map(|block| block as *mut c_void),
            target_count: blocks.len(),
            ..VisitState::default()
        };
        assert!(raw::mi_heap_visit_abandoned_blocks(
            heap,
            true,
            Some(count_blocks),
            ptr::addr_of_mut!(state).cast(),
        ));
        assert!(state.areas > 0);
        assert!(state.blocks >= blocks.len());
        assert_eq!(state.target_visits, [1; TARGET_CAPACITY]);

        for block in blocks {
            raw::mi_free(block as *mut c_void);
        }
        raw::mi_heap_delete(heap);
    }
}
