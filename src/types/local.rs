//! Short loans of current-thread fields, excluding shared atomic metadata.

use core::marker::PhantomData;

use super::{Heartbeat, ThreadHeapLocal, ThreadHeapPtr};
use crate::allocator::stats::mi_stats_t;
use crate::types::mi_random_ctx_t;

#[repr(transparent)]
pub(crate) struct LocalHeapAccess<'scope> {
    heap: ThreadHeapPtr,
    _scope: PhantomData<fn(&'scope mut ()) -> &'scope mut ()>,
}

impl ThreadHeapLocal {
    /// Borrow the local fields during one callback-free phase.
    ///
    /// # Safety
    /// The thread heap must remain live and initialized throughout `f`, with
    /// exclusive access to its local fields. No callback or other allocator
    /// entry may access those fields until the loan ends. Shared atomic fields
    /// may still be accessed through their existing protocols.
    #[inline(always)]
    pub(crate) unsafe fn with_local_access<R>(
        self,
        f: impl for<'scope> FnOnce(&mut LocalHeapAccess<'scope>) -> R,
    ) -> R {
        f(&mut LocalHeapAccess {
            heap: self.ptr(),
            _scope: PhantomData,
        })
    }
}

impl LocalHeapAccess<'_> {
    #[inline(always)]
    pub(crate) fn stats(&mut self) -> &mut mi_stats_t {
        unsafe { &mut (*self.heap.as_ptr()).stats }
    }

    #[inline(always)]
    pub(crate) fn random(&mut self) -> &mut mi_random_ctx_t {
        unsafe { &mut (*self.heap.as_ptr()).random }
    }

    #[inline(always)]
    pub(crate) fn increment_heartbeat(&mut self) -> Heartbeat {
        let heartbeat = unsafe { &mut (*self.heap.as_ptr()).heartbeat };
        let next = Heartbeat::new(*heartbeat).increment_wrapping();
        *heartbeat = next.get();
        next
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn local_field_loans_leave_shared_atomic_storage_accessible() {
        // SAFETY: the synthetic thread heap contains only zero-valid scalar,
        // pointer, atomic, and MemId storage; no OS/TLS initialization is used.
        let mut heap = Box::new(unsafe { core::mem::zeroed::<crate::types::mi_theap_t>() });
        let raw = core::ptr::addr_of_mut!(*heap);
        // SAFETY: refcount is initialized, aligned atomic-compatible storage.
        // Only this field is borrowed; local fields remain disjoint.
        let refcount = unsafe { &*core::ptr::addr_of!((*raw).refcount).cast::<AtomicUsize>() };
        let local = unsafe { ThreadHeapLocal::new_unchecked(ThreadHeapPtr::new_unchecked(raw)) };
        // SAFETY: the fixture remains live; these operations never call out.
        unsafe {
            local.with_local_access(|access| {
                let stats = access.stats();
                stats.malloc_normal.current = 7;
                refcount.fetch_add(1, Ordering::Relaxed);
                access.random().weak = true;
                assert_eq!(access.increment_heartbeat().get(), 1);
            });
        }
        assert_eq!(refcount.load(Ordering::Relaxed), 1);
        assert_eq!(heap.stats.malloc_normal.current, 7);
        assert!(heap.random.weak);
    }
}
