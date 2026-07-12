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
pub use stats::{PageOccupancy, ProcessInfo, StatCount, StatTotal, StatsError, StatsSnapshot};

#[cfg(test)]
mod tests;
