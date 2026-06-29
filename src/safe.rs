#![deny(clippy::must_use_candidate)]

mod allocations;
mod errors;
mod heaps;
mod options;
mod stats;

pub use allocations::{Allocation, InitializedAllocation};
pub use errors::{AllocError, LayoutError, ResizeError, ResizeErrorKind, TryAllocError};
pub use heaps::{Heap, HeapAllocation, HeapError, HeapRef, InitializedHeapAllocation, MainHeap};
pub use options::{OptionError, OptionKey, Options};
pub use stats::{ProcessInfo, StatCount, StatTotal, StatsError, StatsSnapshot};

#[cfg(test)]
mod tests {
    use super::*;
    use core::alloc::Layout;
    use core::ffi::c_void;
    use core::fmt::{self, Write};
    use core::ptr;
    use core::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    use crate::unsafe_callbacks::{DeferredFreeCallbackGuard, ErrorCallbackGuard};

    static CALLBACK_TEST_LOCK: Mutex<()> = Mutex::new(());

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
            unsafe { allocation.as_mut_ptr().add(i).write((i % 251) as u8) };
        }

        let bytes = unsafe { core::slice::from_raw_parts(allocation.as_ptr(), allocation.len()) };
        assert_eq!(bytes[0], 0);
        assert_eq!(bytes[31], 31);
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
    fn allocation_is_move_only_and_drops_once() {
        let mut allocation = Allocation::with_size_align(64, 16).expect("allocation");
        let ptr = allocation.as_mut_ptr();
        unsafe { ptr.write(42) };

        let moved = allocation;
        assert_eq!(moved.as_ptr(), ptr.cast_const());
        assert_eq!(unsafe { moved.as_ptr().read() }, 42);
    }

    #[test]
    fn allocation_has_expected_thread_traits() {
        fn assert_traits<T: Send + Sync>() -> bool {
            let _ = core::marker::PhantomData::<T>;
            true
        }
        assert!(assert_traits::<Allocation>());
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
            unsafe { allocation.as_mut_ptr().add(idx).write(idx as u8) };
        }

        let grown_layout = Layout::from_size_align(64, 8).expect("layout");
        let grown = allocation.resize(grown_layout).expect("grow");
        let grown_bytes = unsafe { core::slice::from_raw_parts(grown.as_ptr(), 16) };
        assert_eq!(grown_bytes, (0u8..16).collect::<Vec<_>>().as_slice());

        let shrunk_layout = Layout::from_size_align(8, 8).expect("layout");
        let shrunk = grown.resize(shrunk_layout).expect("shrink");
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
            unsafe { allocation.as_mut_ptr().add(idx).write((idx + 1) as u8) };
        }
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

        let mut allocation = zeroed.into_allocation();
        for idx in 0..allocation.len() {
            unsafe { allocation.as_mut_ptr().add(idx).write(idx as u8) };
        }
        let grown = allocation
            .resize(Layout::from_size_align(32, 8).expect("layout"))
            .expect("grow");
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
    fn main_heap_allocates_without_owning_heap() {
        let main = MainHeap::get().expect("main heap");
        let allocation = main
            .allocate(Layout::from_size_align(32, 8).expect("layout"))
            .expect("main heap allocation");
        assert_eq!(allocation.len(), 32);
    }

    #[test]
    fn safe_options_cover_supported_invalid_and_immutable_keys() {
        assert!(matches!(OptionError::InvalidKey, OptionError::InvalidKey));
        assert!(matches!(OptionError::Unsupported, OptionError::Unsupported));

        for key in OptionKey::ALL {
            assert_eq!(OptionKey::from_raw(key.as_raw()), Ok(key));
            assert_eq!(OptionKey::try_from(key.as_raw()), Ok(key));
            let _ = Options::get(key);
        }
        assert_eq!(OptionKey::from_raw(-1), Err(OptionError::InvalidKey));
        assert_eq!(OptionKey::try_from(-1), Err(OptionError::InvalidKey));
        assert_eq!(OptionKey::from_raw(2), Err(OptionError::Unsupported));
        assert_eq!(
            Options::set(OptionKey::ShowStats, 1),
            Err(OptionError::Unsupported)
        );

        let original = Options::get(OptionKey::MaxWarnings);
        let changed = if original == 7 { 8 } else { 7 };
        Options::set(OptionKey::MaxWarnings, changed).expect("set");
        assert_eq!(Options::get(OptionKey::MaxWarnings), changed);
        Options::reset(OptionKey::MaxWarnings).expect("reset");
        assert_eq!(
            Options::get(OptionKey::MaxWarnings),
            OptionKey::MaxWarnings.default_value()
        );
        Options::set(OptionKey::MaxWarnings, original).expect("restore");
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
        assert!(after.malloc_requested_total().get() >= before.malloc_requested_total().get());

        let mut tiny = [0u8; 1];
        assert_eq!(after.write_json(&mut tiny), Err(StatsError::BufferTooSmall));
        let mut json = [0u8; 16 * 1024];
        let len = after.write_json(&mut json).expect("json");
        assert!(len > 0);
        assert!(json.starts_with(b"{"));

        let heap = Heap::new().expect("heap");
        let heap_stats = heap.as_ref().stats().expect("heap stats");
        assert!(heap_stats.pages_current().get() >= 0);

        let info = ProcessInfo::current();
        assert_eq!(info.elapsed().as_millis(), info.elapsed_msecs() as u128);
        assert_eq!(info.user_time().as_millis(), info.user_msecs() as u128);
        assert_eq!(info.system_time().as_millis(), info.system_msecs() as u128);
        assert!(info.page_size().is_some_and(|page_size| page_size > 0));
        assert!(info.page_faults().is_some());
        let raw_page_size = unsafe { crate::os::_mi_os_page_size() };
        assert_eq!(info.page_size(), Some(raw_page_size));
    }

    static PREVIOUS_ERROR_CALLBACK_HITS: AtomicUsize = AtomicUsize::new(0);

    unsafe extern "C" fn previous_error_callback(_err: i32, _arg: *mut c_void) {
        PREVIOUS_ERROR_CALLBACK_HITS.fetch_add(1, Ordering::SeqCst);
    }

    #[test]
    fn error_callback_guard_invokes_and_restores_previous_callback_quietly() {
        let _lock = CALLBACK_TEST_LOCK.lock().expect("callback test lock");
        let original = crate::options::registered_error_callback();
        let original_show_errors = Options::get(OptionKey::ShowErrors);
        Options::set(OptionKey::ShowErrors, 0).expect("disable error output");
        PREVIOUS_ERROR_CALLBACK_HITS.store(0, Ordering::SeqCst);
        crate::options::mi_register_error(Some(previous_error_callback), ptr::null_mut());

        let mut hits = 0usize;
        {
            let mut callback = |_err: i32| {
                hits += 1;
            };
            // SAFETY: this test holds the process-wide callback test lock,
            // triggers the callback synchronously, and restores the original
            // registration before releasing the lock.
            let _guard = unsafe { ErrorCallbackGuard::new(&mut callback) };
            crate::options::_mi_error_message_fmt(
                crate::core_types::Errno::EINVAL,
                format_args!("safe error callback guard test\n"),
            );
        }
        assert_eq!(hits, 1);

        crate::options::_mi_error_message_fmt(
            crate::core_types::Errno::EINVAL,
            format_args!("safe error callback restore test\n"),
        );
        assert_eq!(PREVIOUS_ERROR_CALLBACK_HITS.load(Ordering::SeqCst), 1);

        crate::options::restore_error_callback(original);
        Options::set(OptionKey::ShowErrors, original_show_errors).expect("restore show errors");
    }

    #[test]
    fn panicking_error_callback_is_caught_before_c_abi_return() {
        let _lock = CALLBACK_TEST_LOCK.lock().expect("callback test lock");
        let original = crate::options::registered_error_callback();
        let original_show_errors = Options::get(OptionKey::ShowErrors);
        Options::set(OptionKey::ShowErrors, 0).expect("disable error output");
        let old_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));

        {
            let mut callback = |_err: i32| {
                panic!("callback panic should be caught");
            };
            // SAFETY: this test holds the process-wide callback test lock,
            // triggers the callback synchronously, and restores the original
            // registration before releasing the lock.
            let _guard = unsafe { ErrorCallbackGuard::new(&mut callback) };
            crate::options::_mi_error_message_fmt(
                crate::core_types::Errno::EINVAL,
                format_args!("safe panic callback guard test\n"),
            );
        }

        std::panic::set_hook(old_hook);
        crate::options::restore_error_callback(original);
        Options::set(OptionKey::ShowErrors, original_show_errors).expect("restore show errors");
    }

    #[test]
    fn deferred_free_callback_guard_registers_and_restores() {
        let _lock = CALLBACK_TEST_LOCK.lock().expect("callback test lock");
        let original = unsafe { crate::page::registered_deferred_free_callback() };
        {
            let mut callback = |_force: bool, _heartbeat: u64| {};
            // SAFETY: this test holds the process-wide callback test lock and
            // only observes registration state without concurrent allocator
            // callback entry.
            let _guard = unsafe { DeferredFreeCallbackGuard::new(&mut callback) };
            let current = unsafe { crate::page::registered_deferred_free_callback() };
            assert!(current.0.is_some());
        }
        let restored = unsafe { crate::page::registered_deferred_free_callback() };
        assert_eq!(callback_addr(restored.0), callback_addr(original.0));
        assert_eq!(restored.1, original.1);
        unsafe { crate::page::restore_deferred_free_callback(original) };
    }

    fn callback_addr<T>(callback: Option<T>) -> usize
    where
        T: Copy,
    {
        callback
            .map(|callback| unsafe { core::mem::transmute_copy::<T, usize>(&callback) })
            .unwrap_or(0)
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
