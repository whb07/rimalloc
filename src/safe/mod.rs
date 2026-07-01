#![deny(clippy::must_use_candidate)]

mod allocation;
mod error;
mod heap;
mod options;
mod stats;

#[cfg(test)]
pub(super) mod failure_injection {
    use core::cell::Cell;

    std::thread_local! {
        static FAILING_ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
    }

    pub(super) fn fail_next_allocations(count: usize) {
        FAILING_ALLOCATIONS.set(count);
    }

    pub(super) fn should_fail_allocation() -> bool {
        FAILING_ALLOCATIONS.with(|remaining| {
            let count = remaining.get();
            if count == 0 {
                return false;
            }
            remaining.set(count - 1);
            true
        })
    }
}

pub use allocation::{Allocation, InitializedAllocation};
pub use error::{AllocError, LayoutError, ResizeError, ResizeErrorKind, TryAllocError};
pub use heap::{Heap, HeapAllocation, HeapError, HeapRef, InitializedHeapAllocation, MainHeap};
pub use options::{GlobalOptions, OptionError, OptionKey};
pub use stats::{ProcessInfo, StatCount, StatTotal, StatsError, StatsSnapshot};

#[cfg(test)]
mod tests {
    use super::*;
    use core::alloc::Layout;
    use core::ffi::{c_long, c_void};
    use core::fmt::{self, Write};
    use core::mem::MaybeUninit;
    use core::ptr;
    use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Mutex;

    use crate::unsafe_callbacks::{DeferredFreeCallbackGuard, ErrorCallbackGuard};

    static CALLBACK_TEST_LOCK: Mutex<()> = Mutex::new(());
    static FAILURE_INJECTION_TEST_LOCK: Mutex<()> = Mutex::new(());

    struct ShowErrorsGuard(c_long);

    impl ShowErrorsGuard {
        fn disabled() -> Self {
            let previous = GlobalOptions::get(OptionKey::ShowErrors);
            GlobalOptions::set(OptionKey::ShowErrors, 0).expect("disable expected error output");
            Self(previous)
        }
    }

    impl Drop for ShowErrorsGuard {
        fn drop(&mut self) {
            let _ = GlobalOptions::set(OptionKey::ShowErrors, self.0);
        }
    }

    #[test]
    fn owned_allocation_round_trips_bytes() {
        let layout = Layout::from_size_align(64, 16).expect("layout");
        let allocation = Allocation::new(layout).expect("allocation");
        assert_eq!(allocation.layout(), layout);
        assert_eq!(allocation.len(), 64);
        assert_eq!(allocation.as_ptr() as usize % 16, 0);
    }

    #[test]
    fn owned_allocation_covers_sizes_and_alignments() {
        let sizes = [1usize, 7, 64, 4096, 1 << 20];
        let alignments = [1usize, 2, 4, 8, 16, 64, 256, 4096];

        for size in sizes {
            for align in alignments {
                let layout = Layout::from_size_align(size, align).expect("layout");
                let allocation = Allocation::new(layout).expect("allocation");

                assert_eq!(allocation.layout(), layout);
                assert_eq!(allocation.len(), size);
                assert!(!allocation.is_empty());
                assert_ne!(allocation.as_ptr(), core::ptr::null());
                assert_eq!(allocation.as_ptr() as usize % align, 0);
            }
        }
    }

    #[test]
    fn zeroed_allocation_returns_zeroed_bytes() {
        let allocation = Allocation::zeroed_with_size_align(32, 8).expect("allocation");
        // SAFETY[AUDIT:owned-allocation]: the zeroed allocation is live for
        // its stored length and is only read during this test.
        let bytes = unsafe { core::slice::from_raw_parts(allocation.as_ptr(), allocation.len()) };
        assert!(bytes.iter().all(|byte| *byte == 0));
    }

    #[test]
    fn zeroed_allocation_covers_sizes_and_alignments() {
        let sizes = [1usize, 32, 257, 4096];
        let alignments = [1usize, 8, 64, 4096];

        for size in sizes {
            for align in alignments {
                let allocation =
                    Allocation::zeroed_with_size_align(size, align).expect("allocation");

                assert_eq!(allocation.len(), size);
                assert_eq!(allocation.layout().align(), align);
                assert_eq!(allocation.as_ptr() as usize % align, 0);

                // SAFETY[AUDIT:owned-allocation]: each zeroed allocation is
                // live for its stored length and is only read during this test.
                let bytes =
                    unsafe { core::slice::from_raw_parts(allocation.as_ptr(), allocation.len()) };
                assert!(bytes.iter().all(|byte| *byte == 0));
            }
        }
    }

    #[test]
    fn uninitialized_allocation_can_be_written_before_unsafe_read() {
        let mut allocation = Allocation::with_size_align(32, 16).expect("allocation");
        assert_eq!(
            allocation.as_non_null().as_ptr().cast_const(),
            allocation.as_ptr()
        );

        for i in 0..allocation.len() {
            // SAFETY[AUDIT:owned-allocation]: `i` stays within the allocation
            // length and the test owns unique mutable access.
            unsafe { allocation.as_mut_ptr().add(i).write((i % 251) as u8) };
        }

        // SAFETY[AUDIT:owned-allocation]: every byte was initialized by the
        // loop above and the allocation remains live for its stored length.
        let bytes = unsafe { core::slice::from_raw_parts(allocation.as_ptr(), allocation.len()) };
        assert_eq!(bytes[0], 0);
        assert_eq!(bytes[31], 31);
    }

    #[test]
    fn maybe_uninit_allocation_views_initialize_without_raw_pointer_loop() {
        let mut allocation = Allocation::with_size_align(8, 8).expect("allocation");
        assert_eq!(allocation.as_uninit_slice().len(), 8);
        for (idx, slot) in allocation.as_uninit_slice_mut().iter_mut().enumerate() {
            slot.write((idx + 1) as u8);
        }

        // SAFETY[AUDIT:owned-allocation]: the loop above initializes every
        // byte in the allocation before the initialized transition.
        let initialized = unsafe { allocation.assume_initialized() };
        assert_eq!(initialized.as_bytes(), &[1, 2, 3, 4, 5, 6, 7, 8]);

        let mut zero = Allocation::with_size_align(0, 64).expect("zero allocation");
        assert!(zero.as_uninit_slice().is_empty());
        assert!(zero.as_uninit_slice_mut().is_empty());
    }

    #[test]
    fn zero_sized_allocation_does_not_call_allocator() {
        for align in [1usize, 2, 4, 8, 16, 64, 4096] {
            let layout = Layout::from_size_align(0, align).expect("layout");
            let allocation = Allocation::new(layout).expect("allocation");
            assert!(allocation.is_empty());
            assert_eq!(allocation.layout(), layout);
            assert_eq!(allocation.as_ptr() as usize % align, 0);

            let zeroed = Allocation::zeroed(layout).expect("zeroed allocation");
            assert!(zeroed.is_empty());
            assert_eq!(zeroed.layout(), layout);
            assert_eq!(zeroed.as_ptr() as usize % align, 0);
        }
    }

    #[test]
    fn byte_allocation_constructors_use_alignment_one() {
        let zero = Allocation::new_bytes(0).expect("zero byte allocation");
        assert_eq!(zero.len(), 0);
        assert_eq!(zero.layout().align(), 1);
        assert!(zero.is_empty());

        let allocation = Allocation::new_bytes(17).expect("byte allocation");
        assert_eq!(allocation.len(), 17);
        assert_eq!(allocation.layout().align(), 1);

        let zeroed = Allocation::zeroed_bytes(19).expect("zeroed byte allocation");
        assert_eq!(zeroed.len(), 19);
        assert_eq!(zeroed.layout().align(), 1);
        assert!(zeroed.as_bytes().iter().all(|byte| *byte == 0));

        let initialized =
            InitializedAllocation::zeroed_bytes(23).expect("initialized byte allocation");
        assert_eq!(initialized.len(), 23);
        assert_eq!(initialized.layout().align(), 1);
        assert!(initialized.as_bytes().iter().all(|byte| *byte == 0));
    }

    #[test]
    fn allocation_is_move_only_and_drops_once() {
        let mut allocation = Allocation::with_size_align(64, 16).expect("allocation");
        let ptr = allocation.as_mut_ptr();
        // SAFETY[AUDIT:owned-allocation]: the allocation is live, uniquely
        // owned, and large enough for one byte.
        unsafe { ptr.write(42) };

        let moved = allocation;
        assert_eq!(moved.as_ptr(), ptr.cast_const());
        // SAFETY[AUDIT:owned-allocation]: `moved` still owns the live
        // allocation and the first byte was initialized above.
        assert_eq!(unsafe { moved.as_ptr().read() }, 42);
    }

    #[test]
    fn allocation_types_have_expected_thread_traits() {
        fn assert_traits<T: Send + Sync>() -> bool {
            let _ = core::marker::PhantomData::<T>;
            true
        }
        assert!(assert_traits::<Allocation>());
        assert!(assert_traits::<InitializedAllocation>());
    }

    #[test]
    fn invalid_layout_reports_rust_error() {
        let err = Allocation::with_size_align(8, 3).expect_err("invalid alignment");
        match err {
            TryAllocError::Layout(err) => {
                assert_eq!(err.size(), 8);
                assert_eq!(err.align(), 3);
            }
            TryAllocError::Alloc(_) => panic!("expected layout error"),
        }
    }

    #[test]
    fn overflow_layout_reports_rust_error_without_allocating() {
        let err = Allocation::zeroed_with_size_align(usize::MAX, 2).expect_err("invalid layout");
        match err {
            TryAllocError::Layout(err) => {
                assert_eq!(err.size(), usize::MAX);
                assert_eq!(err.align(), 2);
            }
            TryAllocError::Alloc(_) => panic!("expected layout error"),
        }
    }

    #[test]
    fn error_types_are_copy_clone_and_format_without_owned_strings() {
        fn assert_traits<T: Copy + Clone + core::fmt::Debug + Eq>() -> bool {
            let _ = core::marker::PhantomData::<T>;
            true
        }
        assert!(assert_traits::<AllocError>());
        assert!(assert_traits::<LayoutError>());
        assert!(assert_traits::<TryAllocError>());

        let layout = Layout::from_size_align(64, 16).expect("layout");
        let alloc = AllocError { layout };
        let layout_err = LayoutError { size: 1, align: 3 };
        let try_alloc = TryAllocError::Alloc(alloc);

        let mut buf = FixedBuf::new();
        write!(&mut buf, "{alloc}; {layout_err}; {try_alloc}").expect("format");
        assert!(buf.as_str().contains("mimalloc allocation failed"));
        assert!(buf.as_str().contains("invalid allocation layout"));
    }

    #[test]
    fn resize_error_preserves_allocation_access() {
        let allocation = Allocation::with_size_align(16, 8).expect("allocation");
        let layout_err = LayoutError { size: 1, align: 3 };
        let mut err = ResizeError {
            allocation,
            kind: ResizeErrorKind::Layout(layout_err),
        };
        assert!(matches!(err.kind(), ResizeErrorKind::Layout(_)));
        assert_eq!(err.allocation().len(), 16);
        assert_eq!(err.allocation_mut().len(), 16);
        let (allocation, kind) = err.into_parts();
        assert_eq!(allocation.len(), 16);
        assert_eq!(kind, ResizeErrorKind::Layout(layout_err));

        let alloc_kind = ResizeErrorKind::Alloc(AllocError {
            layout: Layout::from_size_align(32, 8).expect("layout"),
        });
        assert!(matches!(alloc_kind, ResizeErrorKind::Alloc(_)));
    }

    #[test]
    fn allocation_resize_grow_shrink_same_and_zero_edges() {
        let layout = Layout::from_size_align(16, 8).expect("layout");
        let mut allocation = Allocation::new(layout).expect("allocation");
        for idx in 0..allocation.len() {
            // SAFETY[AUDIT:owned-allocation]: `idx` stays within the live
            // allocation and the test owns unique mutable access.
            unsafe { allocation.as_mut_ptr().add(idx).write(idx as u8) };
        }

        let grown_layout = Layout::from_size_align(64, 8).expect("layout");
        let grown = allocation.resize(grown_layout).expect("grow");
        // SAFETY[AUDIT:owned-allocation]: the grown allocation is live and the
        // prefix length is the initialized prefix written above.
        let grown_bytes = unsafe { core::slice::from_raw_parts(grown.as_ptr(), 16) };
        assert_eq!(grown_bytes, (0u8..16).collect::<Vec<_>>().as_slice());

        let shrunk_layout = Layout::from_size_align(8, 8).expect("layout");
        let shrunk = grown.resize(shrunk_layout).expect("shrink");
        // SAFETY[AUDIT:owned-allocation]: the shrunk allocation is live and
        // the requested prefix is within the new layout.
        let shrunk_bytes = unsafe { core::slice::from_raw_parts(shrunk.as_ptr(), 8) };
        assert_eq!(shrunk_bytes, (0u8..8).collect::<Vec<_>>().as_slice());

        let same_ptr = shrunk.as_ptr();
        let same = shrunk.resize(shrunk_layout).expect("same size");
        assert_eq!(same.as_ptr(), same_ptr);

        let zero_layout = Layout::from_size_align(0, 16).expect("layout");
        let zero = same.resize(zero_layout).expect("nonzero to zero");
        assert!(zero.is_empty());
        assert_eq!(zero.as_ptr() as usize % 16, 0);

        let nonzero = zero
            .resize(Layout::from_size_align(4, 4).expect("layout"))
            .expect("zero to nonzero");
        assert_eq!(nonzero.len(), 4);
        assert_eq!(nonzero.as_ptr() as usize % 4, 0);
    }

    #[test]
    fn allocation_resize_invalid_layout_preserves_original() {
        let allocation = Allocation::with_size_align(24, 8).expect("allocation");
        let err = allocation
            .resize_with_size_align(usize::MAX, 2)
            .expect_err("layout error");
        assert!(matches!(err.kind(), ResizeErrorKind::Layout(_)));
        assert_eq!(err.allocation().len(), 24);
        let allocation = err.into_allocation();
        assert_eq!(allocation.len(), 24);
    }

    #[test]
    fn initialized_allocation_exposes_safe_slices_and_clone() {
        let allocation = Allocation::zeroed_with_size_align(32, 8).expect("allocation");
        assert!(allocation.as_bytes().iter().all(|byte| *byte == 0));

        let mut allocation = allocation;
        allocation.as_mut_bytes()[0] = 7;
        allocation.as_mut_bytes()[31] = 9;
        assert_eq!(allocation.as_bytes()[0], 7);
        assert_eq!(allocation.as_bytes()[31], 9);

        let cloned = allocation
            .try_clone_contents()
            .expect("clone initialized bytes");
        assert_eq!(cloned.as_bytes(), allocation.as_bytes());

        fn takes_bytes(bytes: impl AsRef<[u8]>) -> usize {
            bytes.as_ref().len()
        }
        fn fills_first(mut bytes: impl AsMut<[u8]>) {
            bytes.as_mut()[0] = 11;
        }

        assert_eq!(takes_bytes(&allocation), 32);
        fills_first(&mut allocation);
        assert_eq!(allocation.as_bytes()[0], 11);
    }

    #[test]
    fn uninitialized_allocation_requires_explicit_initialization() {
        let mut allocation = Allocation::with_size_align(8, 8).expect("allocation");
        for idx in 0..allocation.len() {
            // SAFETY[AUDIT:owned-allocation]: `idx` stays within the live
            // allocation and the test owns unique mutable access.
            unsafe { allocation.as_mut_ptr().add(idx).write((idx + 1) as u8) };
        }
        // SAFETY[AUDIT:owned-allocation]: the loop above initializes every
        // byte before the initialized transition.
        let initialized = unsafe { allocation.assume_initialized() };
        assert_eq!(initialized.as_bytes(), &[1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn initialized_resize_zeroes_grown_tail() {
        let mut allocation = Allocation::zeroed_with_size_align(4, 4).expect("allocation");
        allocation.as_mut_bytes().copy_from_slice(&[1, 2, 3, 4]);
        let resized = allocation
            .resize(Layout::from_size_align(8, 4).expect("layout"))
            .expect("resize initialized");
        assert_eq!(resized.as_bytes(), &[1, 2, 3, 4, 0, 0, 0, 0]);
    }

    #[test]
    fn deterministic_allocation_failure_returns_alloc_error_without_leaking_state() {
        let _lock = FAILURE_INJECTION_TEST_LOCK.lock().expect("failure lock");
        let layout = Layout::from_size_align(32, 8).expect("layout");

        failure_injection::fail_next_allocations(1);
        let err = Allocation::new(layout).expect_err("injected allocation failure");
        assert_eq!(err.layout(), layout);

        let allocation = Allocation::new(layout).expect("allocation after injected failure");
        assert_eq!(allocation.layout(), layout);
    }

    #[test]
    fn deterministic_resize_failure_preserves_original_allocation() {
        let _lock = FAILURE_INJECTION_TEST_LOCK.lock().expect("failure lock");
        let old_layout = Layout::from_size_align(8, 8).expect("old layout");
        let new_layout = Layout::from_size_align(32, 8).expect("new layout");
        let mut allocation = Allocation::new(old_layout).expect("allocation");
        for (idx, slot) in allocation.as_uninit_slice_mut().iter_mut().enumerate() {
            slot.write((idx + 1) as u8);
        }

        failure_injection::fail_next_allocations(1);
        let err = allocation
            .resize(new_layout)
            .expect_err("injected resize failure");
        assert!(matches!(err.kind(), ResizeErrorKind::Alloc(_)));
        assert_eq!(err.allocation().layout(), old_layout);
        let allocation = err.into_allocation();
        // SAFETY[AUDIT:owned-allocation]: failed resize preserves the original
        // live allocation and all bytes were initialized before the failure.
        let bytes = unsafe { core::slice::from_raw_parts(allocation.as_ptr(), allocation.len()) };
        assert_eq!(bytes, &[1, 2, 3, 4, 5, 6, 7, 8]);

        let grown = allocation.resize(new_layout).expect("resize after failure");
        assert_eq!(grown.layout(), new_layout);
    }

    #[test]
    fn heap_errors_and_owned_heap_allocation_paths_work() {
        assert!(matches!(HeapError::Create, HeapError::Create));
        assert!(matches!(
            HeapError::MainUnavailable,
            HeapError::MainUnavailable
        ));
        assert!(matches!(HeapError::Unsupported, HeapError::Unsupported));

        let heap = Heap::new().expect("heap");
        let allocation = heap
            .allocate(Layout::from_size_align(64, 16).expect("layout"))
            .expect("heap allocation");
        assert_eq!(allocation.len(), 64);
        assert_eq!(allocation.as_ptr() as usize % 16, 0);

        let zero = heap
            .allocate(Layout::from_size_align(0, 64).expect("layout"))
            .expect("zero heap allocation");
        assert!(zero.is_empty());
        assert_eq!(zero.as_ptr() as usize % 64, 0);

        let over_aligned = heap
            .allocate(Layout::from_size_align(128, 4096).expect("layout"))
            .expect("over-aligned heap allocation");
        assert_eq!(over_aligned.as_ptr() as usize % 4096, 0);
    }

    #[test]
    fn heap_zeroed_allocation_and_resize_preserve_ownership() {
        let heap = Heap::new().expect("heap");
        let zeroed = heap
            .allocate_zeroed(Layout::from_size_align(16, 8).expect("layout"))
            .expect("zeroed");
        assert!(zeroed.as_bytes().iter().all(|byte| *byte == 0));

        let zeroed = zeroed
            .resize_with_size_align(24, 8)
            .expect("grow initialized heap allocation");
        assert_eq!(zeroed.len(), 24);
        assert!(zeroed.as_bytes()[16..].iter().all(|byte| *byte == 0));

        let mut allocation = zeroed.into_allocation();
        for idx in 0..allocation.len() {
            // SAFETY[AUDIT:safe-heap]: `idx` stays within the live heap
            // allocation and the test owns unique mutable access.
            unsafe { allocation.as_mut_ptr().add(idx).write(idx as u8) };
        }
        let grown = allocation
            .resize(Layout::from_size_align(32, 8).expect("layout"))
            .expect("grow");
        // SAFETY[AUDIT:safe-heap]: the grown heap allocation is live and the
        // prefix length is the initialized prefix written above.
        let bytes = unsafe { core::slice::from_raw_parts(grown.as_ptr(), 16) };
        assert_eq!(bytes, (0u8..16).collect::<Vec<_>>().as_slice());

        let shrunk = grown
            .resize(Layout::from_size_align(8, 8).expect("layout"))
            .expect("shrink");
        assert_eq!(shrunk.len(), 8);

        let err = shrunk
            .resize_with_size_align(16, 3)
            .expect_err("invalid layout");
        assert!(matches!(err.kind(), ResizeErrorKind::Layout(_)));
        assert_eq!(err.allocation().len(), 8);
    }

    #[test]
    fn deterministic_heap_allocation_failure_preserves_heap_ownership() {
        let _lock = FAILURE_INJECTION_TEST_LOCK.lock().expect("failure lock");
        let heap = Heap::new().expect("heap");
        let layout = Layout::from_size_align(24, 8).expect("layout");

        failure_injection::fail_next_allocations(1);
        let err = heap
            .allocate(layout)
            .expect_err("injected heap allocation failure");
        assert_eq!(err.layout(), layout);

        let allocation = heap
            .allocate(layout)
            .expect("heap allocation after failure");
        assert_eq!(allocation.layout(), layout);

        let new_layout = Layout::from_size_align(48, 8).expect("new layout");
        failure_injection::fail_next_allocations(1);
        let err = allocation
            .resize(new_layout)
            .expect_err("injected heap resize failure");
        assert!(matches!(err.kind(), ResizeErrorKind::Alloc(_)));
        assert_eq!(err.allocation().layout(), layout);

        let allocation = err
            .into_allocation()
            .resize(new_layout)
            .expect("heap resize after failure");
        assert_eq!(allocation.layout(), new_layout);
        let another = heap
            .allocate(Layout::from_size_align(8, 8).expect("layout"))
            .expect("heap still usable");
        assert_eq!(another.len(), 8);
    }

    #[test]
    fn raw_product_allocation_failure_is_deterministic_and_null_returning() {
        let _quiet_expected_errors = ShowErrorsGuard::disabled();
        // SAFETY[AUDIT:owned-allocation]: this test calls raw allocation
        // entrypoints with overflow requests and asserts their null-return
        // failure contract without dereferencing returned pointers.
        unsafe {
            assert!(crate::mimalloc::mi_mallocn(usize::MAX, 2).is_null());
            assert!(crate::mimalloc::mi_calloc(usize::MAX, 2).is_null());
            assert!(
                crate::mimalloc::mi_heap_mallocn(crate::init::mi_heap_main(), usize::MAX, 2)
                    .is_null()
            );
        }
    }

    #[test]
    fn heap_byte_allocation_apis_use_alignment_one() {
        let heap = Heap::new().expect("heap");
        let zero = heap.allocate_bytes(0).expect("zero heap bytes");
        assert_eq!(zero.len(), 0);
        assert_eq!(zero.layout().align(), 1);
        assert!(zero.is_empty());

        let allocation = heap.allocate_bytes(17).expect("heap bytes");
        assert_eq!(allocation.len(), 17);
        assert_eq!(allocation.layout().align(), 1);

        let borrowed = heap.as_ref();
        let zeroed = borrowed
            .allocate_zeroed_bytes(19)
            .expect("borrowed zeroed heap bytes");
        assert_eq!(zeroed.len(), 19);
        assert_eq!(zeroed.layout().align(), 1);
        assert!(zeroed.as_bytes().iter().all(|byte| *byte == 0));

        let main = MainHeap::get().expect("main heap");
        let main_allocation = main.allocate_bytes(23).expect("main heap bytes");
        assert_eq!(main_allocation.len(), 23);
        assert_eq!(main_allocation.layout().align(), 1);

        let main_zeroed = main
            .allocate_zeroed_bytes(29)
            .expect("main heap zeroed bytes");
        assert_eq!(main_zeroed.len(), 29);
        assert_eq!(main_zeroed.layout().align(), 1);
        assert!(main_zeroed.as_bytes().iter().all(|byte| *byte == 0));
    }

    #[test]
    fn maybe_uninit_heap_views_initialize_without_raw_pointer_loop() {
        let heap = Heap::new().expect("heap");
        let mut allocation = heap
            .allocate(Layout::from_size_align(6, 8).expect("layout"))
            .expect("heap allocation");
        assert_eq!(allocation.as_uninit_slice().len(), 6);
        for (idx, slot) in allocation.as_uninit_slice_mut().iter_mut().enumerate() {
            slot.write((idx + 10) as u8);
        }

        // SAFETY[AUDIT:safe-heap]: the loop above initializes every byte in
        // the heap allocation before the initialized transition.
        let initialized = unsafe { allocation.assume_initialized() };
        assert_eq!(initialized.as_bytes(), &[10, 11, 12, 13, 14, 15]);

        let mut zero = heap
            .allocate(Layout::from_size_align(0, 64).expect("layout"))
            .expect("zero heap allocation");
        assert!(zero.as_uninit_slice().is_empty());
        assert!(zero.as_uninit_slice_mut().is_empty());
    }

    #[test]
    fn main_heap_allocates_without_owning_heap() {
        let main = MainHeap::get().expect("main heap");
        let allocation = main
            .allocate(Layout::from_size_align(32, 8).expect("layout"))
            .expect("main heap allocation");
        assert_eq!(allocation.len(), 32);

        let zeroed = main
            .allocate_zeroed(Layout::from_size_align(16, 8).expect("layout"))
            .expect("main heap zeroed allocation");
        assert!(zeroed.as_bytes().iter().all(|byte| *byte == 0));
    }

    #[test]
    fn main_heap_allocations_outlive_temporary_handle() {
        let mut allocation;
        let zeroed;
        {
            let main = MainHeap::get().expect("main heap");
            allocation = main
                .allocate(Layout::from_size_align(4, 4).expect("layout"))
                .expect("main heap allocation");
            zeroed = MainHeap::get()
                .expect("main heap")
                .as_ref()
                .allocate_zeroed(Layout::from_size_align(3, 1).expect("layout"))
                .expect("main heap zeroed allocation");
        }

        allocation.as_uninit_slice_mut()[0].write(42);
        // SAFETY[AUDIT:safe-heap]: the single live byte was initialized before
        // the initialized transition.
        let initialized = unsafe { allocation.assume_initialized() };
        assert_eq!(initialized.as_bytes()[0], 42);
        assert_eq!(zeroed.as_bytes(), &[0, 0, 0]);
    }

    #[test]
    fn safe_options_cover_supported_invalid_and_immutable_keys() {
        assert!(matches!(OptionError::InvalidKey, OptionError::InvalidKey));
        assert!(matches!(OptionError::Unsupported, OptionError::Unsupported));

        for key in OptionKey::ALL {
            assert_eq!(OptionKey::from_raw(key.as_raw()), Ok(key));
            assert_eq!(OptionKey::try_from(key.as_raw()), Ok(key));
            let _ = GlobalOptions::get(key);
        }
        assert_eq!(OptionKey::from_raw(-1), Err(OptionError::InvalidKey));
        assert_eq!(OptionKey::try_from(-1), Err(OptionError::InvalidKey));
        assert_eq!(OptionKey::from_raw(2), Err(OptionError::Unsupported));
        assert_eq!(
            GlobalOptions::set(OptionKey::ShowStats, 1),
            Err(OptionError::Unsupported)
        );

        let original = GlobalOptions::get(OptionKey::MaxWarnings);
        let changed = if original == 7 { 8 } else { 7 };
        GlobalOptions::set(OptionKey::MaxWarnings, changed).expect("set");
        assert_eq!(GlobalOptions::get(OptionKey::MaxWarnings), changed);
        GlobalOptions::reset(OptionKey::MaxWarnings).expect("reset");
        assert_eq!(
            GlobalOptions::get(OptionKey::MaxWarnings),
            OptionKey::MaxWarnings.default_value()
        );
        GlobalOptions::set(OptionKey::MaxWarnings, original).expect("restore");
    }

    #[test]
    fn global_options_mutations_are_process_global_and_restored() {
        let original = GlobalOptions::get(OptionKey::MaxErrors);
        let changed = if original == 11 { 12 } else { 11 };

        GlobalOptions::set(OptionKey::MaxErrors, changed).expect("set max errors");
        assert_eq!(GlobalOptions::get(OptionKey::MaxErrors), changed);
        assert_eq!(
            crate::options::mi_option_get(OptionKey::MaxErrors.raw_option()),
            changed
        );

        GlobalOptions::set(OptionKey::MaxErrors, original).expect("restore max errors");
        assert_eq!(GlobalOptions::get(OptionKey::MaxErrors), original);
    }

    #[test]
    fn stats_and_process_info_are_typed_and_nonprinting() {
        assert!(matches!(StatsError::Unavailable, StatsError::Unavailable));
        assert!(matches!(
            StatsError::HeapUnavailable,
            StatsError::HeapUnavailable
        ));
        assert!(matches!(
            StatsError::BufferTooSmall,
            StatsError::BufferTooSmall
        ));
        assert_eq!(StatCount::new(3).get(), 3);
        assert_eq!(StatTotal::new(4).get(), 4);

        let before = StatsSnapshot::capture().expect("stats before");
        let allocation = Allocation::with_size_align(1024, 16).expect("allocation");
        drop(allocation);
        let after = StatsSnapshot::capture().expect("stats after");
        assert!(after.requested_bytes_total().get() >= before.requested_bytes_total().get());

        let mut tiny = [0u8; 1];
        assert_eq!(after.write_json(&mut tiny), Err(StatsError::BufferTooSmall));
        let mut json = [0u8; 16 * 1024];
        let len = after.write_json(&mut json).expect("json");
        assert!(len > 0);
        assert!(json.starts_with(b"{"));

        let heap = Heap::new().expect("heap");
        let heap_stats = heap.stats().expect("heap stats");
        assert!(heap_stats.pages_current().get() >= 0);

        let main_stats = MainHeap::get()
            .expect("main heap")
            .stats()
            .expect("main stats");
        assert!(main_stats.pages_current().get() >= 0);

        let info = ProcessInfo::current();
        assert_eq!(info.elapsed().as_millis(), info.elapsed_msecs() as u128);
        assert_eq!(info.user_time().as_millis(), info.user_msecs() as u128);
        assert_eq!(info.system_time().as_millis(), info.system_msecs() as u128);
        assert!(info.page_size().is_some_and(|page_size| page_size > 0));
        assert!(info.page_faults().get() >= 0);
        // SAFETY[AUDIT:safe-process-info]: querying the platform page size
        // reads allocator primitive state and does not dereference caller data.
        let raw_page_size = unsafe { crate::os::_mi_os_page_size() };
        assert_eq!(info.page_size(), Some(raw_page_size));

        let mut raw_page_faults = 0usize;
        // SAFETY[AUDIT:safe-process-info]: null out-pointers are accepted by
        // the raw process-info API, and `raw_page_faults` is valid writable
        // local storage for the only non-null out-pointer.
        unsafe {
            crate::stats::mi_process_info(
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                &mut raw_page_faults,
            );
        }
        assert!(
            raw_page_faults.min(i64::MAX as usize) as i64 >= info.page_faults().get(),
            "raw process page-fault count should not go backward"
        );
    }

    static PREVIOUS_ERROR_CALLBACK_HITS: AtomicUsize = AtomicUsize::new(0);
    static PREVIOUS_DEFERRED_CALLBACK_HITS: AtomicUsize = AtomicUsize::new(0);

    // SAFETY[AUDIT:unsafe-callback]: test-only C ABI callback used as a
    // previous error registration fixture; it only updates an atomic counter.
    unsafe extern "C" fn previous_error_callback(_err: i32, _arg: *mut c_void) {
        PREVIOUS_ERROR_CALLBACK_HITS.fetch_add(1, Ordering::SeqCst);
    }

    // SAFETY[AUDIT:unsafe-callback]: test-only C ABI callback used as a
    // previous deferred-free registration fixture; it only updates an atomic
    // counter.
    unsafe extern "C" fn previous_deferred_free_callback(
        _force: bool,
        _heartbeat: u64,
        _arg: *mut c_void,
    ) {
        PREVIOUS_DEFERRED_CALLBACK_HITS.fetch_add(1, Ordering::SeqCst);
    }

    struct ErrorCallbackRestoreGuard {
        snapshot: (Option<crate::types::mi_error_fun>, *mut c_void),
    }

    impl ErrorCallbackRestoreGuard {
        fn new() -> Self {
            Self {
                snapshot: crate::options::registered_error_callback(),
            }
        }
    }

    impl Drop for ErrorCallbackRestoreGuard {
        fn drop(&mut self) {
            crate::options::restore_error_callback(self.snapshot);
        }
    }

    struct DeferredFreeCallbackRestoreGuard {
        snapshot: (Option<crate::types::mi_deferred_free_fun>, *mut c_void),
    }

    impl DeferredFreeCallbackRestoreGuard {
        fn new() -> Self {
            Self {
                // SAFETY[AUDIT:unsafe-callback]: tests snapshot the current
                // process-global callback state while holding the callback
                // serialization lock.
                snapshot: unsafe { crate::page::registered_deferred_free_callback() },
            }
        }
    }

    impl Drop for DeferredFreeCallbackRestoreGuard {
        fn drop(&mut self) {
            // SAFETY[AUDIT:unsafe-callback]: tests restore the exact callback
            // snapshot captured under the callback serialization lock.
            unsafe { crate::page::restore_deferred_free_callback(self.snapshot) };
        }
    }

    fn trigger_error_callback() {
        crate::options::_mi_error_message_fmt(
            crate::core_types::Errno::EINVAL,
            format_args!("safe error callback guard test\n"),
        );
    }

    fn with_test_thread_heap<R>(f: impl FnOnce(*mut crate::types::mi_theap_t) -> R) -> R {
        // SAFETY[AUDIT:unsafe-callback]: the zeroed values are local callback
        // test fixtures and are initialized only enough for deferred-free
        // reentry suppression through the `tld` field.
        let mut tld = unsafe { MaybeUninit::<crate::types::mi_tld_t>::zeroed().assume_init() };
        let mut theap = unsafe { MaybeUninit::<crate::types::mi_theap_t>::zeroed().assume_init() };
        theap.tld = ptr::from_mut(&mut tld);
        f(ptr::from_mut(&mut theap))
    }

    fn trigger_deferred_free_callback(theap: *mut crate::types::mi_theap_t, force: bool) {
        assert!(!theap.is_null());
        // SAFETY[AUDIT:unsafe-callback]: `theap` comes from
        // `with_test_thread_heap` and stays live for this synchronous callback
        // trigger.
        unsafe { crate::page::_mi_deferred_free(theap, force) };
    }

    #[test]
    #[ignore = "process-global callback registration must run serialized"]
    fn error_callback_guard_invokes_and_restores_previous_callback_quietly() {
        let _lock = CALLBACK_TEST_LOCK.lock().expect("callback test lock");
        let _restore = ErrorCallbackRestoreGuard::new();
        let _quiet_expected_errors = ShowErrorsGuard::disabled();
        PREVIOUS_ERROR_CALLBACK_HITS.store(0, Ordering::SeqCst);
        crate::options::mi_register_error(Some(previous_error_callback), ptr::null_mut());

        let mut hits = 0usize;
        {
            let mut callback = |_err: i32| {
                hits += 1;
            };
            // SAFETY[AUDIT:unsafe-callback]: this test holds the process-wide callback test lock,
            // triggers the callback synchronously, and restores the original
            // registration before releasing the lock.
            let _guard = unsafe { ErrorCallbackGuard::new(&mut callback) };
            trigger_error_callback();
        }
        assert_eq!(hits, 1);

        trigger_error_callback();
        assert_eq!(PREVIOUS_ERROR_CALLBACK_HITS.load(Ordering::SeqCst), 1);
    }

    #[test]
    #[ignore = "process-global callback registration must run serialized"]
    fn nested_error_callback_guards_restore_lifo_previous_callbacks() {
        let _lock = CALLBACK_TEST_LOCK.lock().expect("callback test lock");
        let _restore = ErrorCallbackRestoreGuard::new();
        let _quiet_expected_errors = ShowErrorsGuard::disabled();
        PREVIOUS_ERROR_CALLBACK_HITS.store(0, Ordering::SeqCst);
        crate::options::mi_register_error(Some(previous_error_callback), ptr::null_mut());

        let outer_hits = AtomicUsize::new(0);
        let inner_hits = AtomicUsize::new(0);
        let mut outer_callback = |_err: i32| {
            outer_hits.fetch_add(1, Ordering::SeqCst);
        };
        let mut inner_callback = |_err: i32| {
            inner_hits.fetch_add(1, Ordering::SeqCst);
        };

        // SAFETY[AUDIT:unsafe-callback]: this test serializes process-global callback registration,
        // invokes callbacks synchronously, and drops nested guards in LIFO
        // order before the borrowed callbacks go out of scope.
        let outer_guard = unsafe { ErrorCallbackGuard::new(&mut outer_callback) };
        trigger_error_callback();

        {
            // SAFETY[AUDIT:unsafe-callback]: same synchronization and LIFO nesting contract as above.
            let _inner_guard = unsafe { ErrorCallbackGuard::new(&mut inner_callback) };
            trigger_error_callback();
        }

        trigger_error_callback();
        drop(outer_guard);
        trigger_error_callback();

        assert_eq!(outer_hits.load(Ordering::SeqCst), 2);
        assert_eq!(inner_hits.load(Ordering::SeqCst), 1);
        assert_eq!(PREVIOUS_ERROR_CALLBACK_HITS.load(Ordering::SeqCst), 1);
    }

    #[test]
    #[ignore = "process-global callback registration must run serialized"]
    fn panicking_error_callback_is_caught_before_c_abi_return() {
        let _lock = CALLBACK_TEST_LOCK.lock().expect("callback test lock");
        let _restore = ErrorCallbackRestoreGuard::new();
        let _quiet_expected_errors = ShowErrorsGuard::disabled();
        let old_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));

        {
            let mut callback = |_err: i32| {
                panic!("callback panic should be caught");
            };
            // SAFETY[AUDIT:unsafe-callback]: this test holds the process-wide callback test lock,
            // triggers the callback synchronously, and restores the original
            // registration before releasing the lock.
            let _guard = unsafe { ErrorCallbackGuard::new(&mut callback) };
            trigger_error_callback();
        }

        std::panic::set_hook(old_hook);
    }

    #[test]
    #[ignore = "process-global callback registration must run serialized"]
    fn deferred_free_callback_guard_registers_and_restores() {
        let _lock = CALLBACK_TEST_LOCK.lock().expect("callback test lock");
        let restore = DeferredFreeCallbackRestoreGuard::new();
        {
            let mut callback = |_force: bool, _heartbeat: u64| {};
            // SAFETY[AUDIT:unsafe-callback]: this test holds the process-wide callback test lock and
            // only observes registration state without concurrent allocator
            // callback entry.
            let _guard = unsafe { DeferredFreeCallbackGuard::new(&mut callback) };
            let current = unsafe { crate::page::registered_deferred_free_callback() };
            assert!(current.0.is_some());
        }
        // SAFETY[AUDIT:unsafe-callback]: the callback test lock is still held
        // and the guard has restored the previous registration.
        let restored = unsafe { crate::page::registered_deferred_free_callback() };
        assert_eq!(callback_addr(restored.0), callback_addr(restore.snapshot.0));
        assert_eq!(restored.1, restore.snapshot.1);
    }

    #[test]
    #[ignore = "process-global callback registration must run serialized"]
    fn nested_deferred_free_callback_guards_restore_lifo_previous_callbacks() {
        let _lock = CALLBACK_TEST_LOCK.lock().expect("callback test lock");
        let _restore = DeferredFreeCallbackRestoreGuard::new();
        PREVIOUS_DEFERRED_CALLBACK_HITS.store(0, Ordering::SeqCst);
        // SAFETY[AUDIT:unsafe-callback]: this test holds the callback
        // serialization lock and installs a test-only previous callback.
        unsafe {
            crate::page::mi_register_deferred_free(
                Some(previous_deferred_free_callback),
                ptr::null_mut(),
            );
        }

        let outer_hits = AtomicUsize::new(0);
        let inner_hits = AtomicUsize::new(0);
        let mut outer_callback = |_force: bool, _heartbeat: u64| {
            outer_hits.fetch_add(1, Ordering::SeqCst);
        };
        let mut inner_callback = |_force: bool, _heartbeat: u64| {
            inner_hits.fetch_add(1, Ordering::SeqCst);
        };

        // SAFETY[AUDIT:unsafe-callback]: this test serializes process-global callback registration,
        // invokes callbacks synchronously on a local thread-heap fixture, and drops
        // nested guards in LIFO order before borrowed callbacks go out of scope.
        with_test_thread_heap(|theap| {
            let outer_guard = unsafe { DeferredFreeCallbackGuard::new(&mut outer_callback) };
            trigger_deferred_free_callback(theap, false);

            {
                // SAFETY[AUDIT:unsafe-callback]: same synchronization and LIFO nesting contract as above.
                let _inner_guard = unsafe { DeferredFreeCallbackGuard::new(&mut inner_callback) };
                trigger_deferred_free_callback(theap, true);
            }

            trigger_deferred_free_callback(theap, false);
            drop(outer_guard);
            trigger_deferred_free_callback(theap, false);
        });

        assert_eq!(outer_hits.load(Ordering::SeqCst), 2);
        assert_eq!(inner_hits.load(Ordering::SeqCst), 1);
        assert_eq!(PREVIOUS_DEFERRED_CALLBACK_HITS.load(Ordering::SeqCst), 1);
    }

    #[test]
    #[ignore = "process-global callback registration must run serialized"]
    fn error_callback_allows_allocator_reentry_without_recursive_error() {
        let _lock = CALLBACK_TEST_LOCK.lock().expect("callback test lock");
        let _restore = ErrorCallbackRestoreGuard::new();
        let _quiet_expected_errors = ShowErrorsGuard::disabled();
        let hits = AtomicUsize::new(0);
        let reentry_ok = AtomicBool::new(false);
        let reentry_failed = AtomicBool::new(false);
        let mut callback = |_err: i32| {
            hits.fetch_add(1, Ordering::SeqCst);
            match Allocation::with_size_align(16, 8) {
                Ok(allocation) if allocation.len() == 16 => {
                    reentry_ok.store(true, Ordering::SeqCst);
                }
                _ => {
                    reentry_failed.store(true, Ordering::SeqCst);
                }
            }
        };

        // SAFETY[AUDIT:unsafe-callback]: this test holds the process-wide callback lock, invokes the
        // callback synchronously, and records any callback allocation failure
        // instead of panicking inside the panic-catching trampoline.
        let _guard = unsafe { ErrorCallbackGuard::new(&mut callback) };
        trigger_error_callback();

        assert_eq!(hits.load(Ordering::SeqCst), 1);
        assert!(reentry_ok.load(Ordering::SeqCst));
        assert!(!reentry_failed.load(Ordering::SeqCst));
    }

    #[test]
    #[ignore = "process-global callback registration must run serialized"]
    fn deferred_free_callback_allows_allocator_reentry_and_suppresses_recursive_entry() {
        let _lock = CALLBACK_TEST_LOCK.lock().expect("callback test lock");
        let _restore = DeferredFreeCallbackRestoreGuard::new();
        let hits = AtomicUsize::new(0);
        let reentry_ok = AtomicBool::new(false);
        let reentry_failed = AtomicBool::new(false);
        with_test_thread_heap(|theap| {
            let mut callback = |_force: bool, _heartbeat: u64| {
                hits.fetch_add(1, Ordering::SeqCst);
                match Allocation::with_size_align(16, 8) {
                    Ok(allocation) if allocation.len() == 16 => {
                        reentry_ok.store(true, Ordering::SeqCst);
                    }
                    _ => {
                        reentry_failed.store(true, Ordering::SeqCst);
                    }
                }
                if theap.is_null() {
                    reentry_failed.store(true, Ordering::SeqCst);
                } else {
                    trigger_deferred_free_callback(theap, false);
                }
            };

            // SAFETY[AUDIT:unsafe-callback]: this test holds the process-wide callback lock and
            // invokes the deferred-free callback synchronously on a local
            // thread-heap fixture. The recursive trigger exercises the
            // allocator's per-thread reentry suppression before the borrowed
            // callback can be entered again.
            let _guard = unsafe { DeferredFreeCallbackGuard::new(&mut callback) };
            trigger_deferred_free_callback(theap, true);
        });

        assert_eq!(hits.load(Ordering::SeqCst), 1);
        assert!(reentry_ok.load(Ordering::SeqCst));
        assert!(!reentry_failed.load(Ordering::SeqCst));
    }

    #[test]
    #[ignore = "process-global callback registration must run serialized"]
    fn panicking_deferred_free_callback_is_caught_before_c_abi_return() {
        let _lock = CALLBACK_TEST_LOCK.lock().expect("callback test lock");
        let _restore = DeferredFreeCallbackRestoreGuard::new();
        let old_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));

        {
            let mut callback = |_force: bool, _heartbeat: u64| {
                panic!("deferred-free callback panic should be caught");
            };
            // SAFETY[AUDIT:unsafe-callback]: this test holds the process-wide callback test lock,
            // triggers the callback synchronously, and restores the original
            // registration before releasing the lock.
            let _guard = unsafe { DeferredFreeCallbackGuard::new(&mut callback) };
            with_test_thread_heap(|theap| trigger_deferred_free_callback(theap, false));
        }

        std::panic::set_hook(old_hook);
    }

    #[derive(Clone, Copy)]
    struct ModeledCallbackGuard {
        previous: usize,
        registered: usize,
    }

    fn modeled_callback_register(slot: &mut usize, registered: usize) -> ModeledCallbackGuard {
        let previous = *slot;
        *slot = registered;
        ModeledCallbackGuard {
            previous,
            registered,
        }
    }

    fn modeled_callback_drop(slot: &mut usize, guard: ModeledCallbackGuard) {
        assert_ne!(guard.registered, guard.previous);
        *slot = guard.previous;
    }

    #[test]
    fn callback_guard_registration_model_requires_serialized_lifo_drops() {
        let mut slot = 0usize;
        let outer = modeled_callback_register(&mut slot, 1);
        let inner = modeled_callback_register(&mut slot, 2);

        modeled_callback_drop(&mut slot, inner);
        assert_eq!(slot, 1);
        modeled_callback_drop(&mut slot, outer);
        assert_eq!(slot, 0);

        let outer = modeled_callback_register(&mut slot, 1);
        let inner = modeled_callback_register(&mut slot, 2);
        modeled_callback_drop(&mut slot, outer);
        assert_eq!(
            slot, 0,
            "dropping an older guard first restores a stale snapshot over the active registration"
        );
        modeled_callback_drop(&mut slot, inner);
        assert_eq!(
            slot, 1,
            "dropping the newer guard after out-of-order restore can resurrect a dropped registration"
        );
    }

    fn callback_addr<T>(callback: Option<T>) -> usize
    where
        T: Copy,
    {
        // SAFETY[AUDIT:unsafe-callback]: callback function pointer options are
        // copied to an integer address only for equality checks in tests.
        callback.map_or(0, |callback| unsafe {
            core::mem::transmute_copy::<T, usize>(&callback)
        })
    }

    struct FixedBuf {
        bytes: [u8; 256],
        len: usize,
    }

    impl FixedBuf {
        fn new() -> Self {
            Self {
                bytes: [0; 256],
                len: 0,
            }
        }

        fn as_str(&self) -> &str {
            core::str::from_utf8(&self.bytes[..self.len]).expect("utf8")
        }
    }

    impl Write for FixedBuf {
        fn write_str(&mut self, s: &str) -> fmt::Result {
            let next = self.len.checked_add(s.len()).ok_or(fmt::Error)?;
            if next > self.bytes.len() {
                return Err(fmt::Error);
            }
            self.bytes[self.len..next].copy_from_slice(s.as_bytes());
            self.len = next;
            Ok(())
        }
    }
}
