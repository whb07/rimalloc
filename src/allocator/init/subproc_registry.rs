use core::ptr::{self, NonNull};

use crate::internal::atomic::{mi_lock_acquire, mi_lock_release};
use crate::types::{
    HeapPtr, HeapVisitCallback, HeapVisitCallbackArg, MainSubproc, MemoryOrder, MiLockGuard,
    MiLockPtr, SubprocPtr, ThreadSeq, TldPtr, mi_subproc_t,
};

use super::{MiHeapMainPrefix, mi_subproc_prefix_typed, subprocs, subprocs_lock};

/// Exclusive access to the process-global intrusive subproc registry.
pub(super) struct SubprocRegistryGuard<'a> {
    _lock: MiLockGuard<'a>,
    head: NonNull<*mut mi_subproc_t>,
}

/// A live subproc removed exactly once from the process registry.
#[must_use = "detached subproc ownership must be consumed by destruction"]
pub(super) struct DetachedSubproc(SubprocPtr);

pub(super) enum AssignCurrentThreadResult {
    NotLinked,
    AlreadyAssigned,
    Assigned,
}

/// A registry-validated subproc whose heap list lock is held.
pub(super) struct LockedSubprocHeaps {
    lock: MiLockPtr,
    subproc: SubprocPtr,
}

impl DetachedSubproc {
    #[inline(always)]
    pub(super) fn ptr(&self) -> SubprocPtr {
        self.0
    }
}

impl SubprocRegistryGuard<'static> {
    /// # Safety
    /// The process-global subproc lock must be initialized and remain live.
    pub(super) unsafe fn acquire() -> Self {
        // SAFETY: required by this constructor's contract; the guard releases
        // the same process-global lock on drop.
        let lock = MiLockPtr::from_nonnull(unsafe {
            NonNull::new_unchecked(ptr::addr_of_mut!(subprocs_lock))
        });
        Self {
            // SAFETY: the process-global lock is initialized by `main_init`.
            _lock: unsafe { MiLockGuard::acquire(lock) },
            // SAFETY: a static variable has a stable non-null address.
            head: unsafe { NonNull::new_unchecked(ptr::addr_of_mut!(subprocs)) },
        }
    }

    /// # Safety
    /// `subproc` must be initialized, live, and not currently linked.
    pub(super) unsafe fn push_front(&self, subproc: SubprocPtr) {
        // SAFETY: this guard exclusively owns the registry and the caller
        // provides an initialized unlinked node.
        unsafe { push_front(self.head.as_ptr(), subproc) };
    }

    /// Looks up `candidate` by address without dereferencing it, then unlinks
    /// the matching live registry node exactly once.
    pub(super) fn detach(&self, candidate: *mut mi_subproc_t) -> Option<DetachedSubproc> {
        // SAFETY: this guard exclusively owns the initialized registry. The
        // candidate is compared as an address and dereferenced only after it
        // equals a node reached from the registry head.
        unsafe { detach_from(self.head.as_ptr(), candidate) }
    }

    pub(super) fn detach_first_except(
        &self,
        excluded: *mut mi_subproc_t,
    ) -> Option<DetachedSubproc> {
        // SAFETY: this guard exclusively owns the initialized registry.
        unsafe { detach_first_except_from(self.head.as_ptr(), excluded) }
    }

    /// # Safety
    /// `tld` must be the live writable TLD for the current thread. `main` must
    /// identify the process-global main subproc; association is checked before
    /// mutation.
    pub(super) unsafe fn assign_current_thread(
        &self,
        candidate: *mut mi_subproc_t,
        tld: TldPtr,
        main: MainSubproc,
    ) -> AssignCurrentThreadResult {
        // SAFETY: this guard exclusively owns the initialized registry and the
        // caller provides the live writable current-thread TLD.
        unsafe { assign_current_thread_from(self.head.as_ptr(), candidate, tld, main.ptr()) }
    }

    /// Validates `candidate` and acquires its heap-list lock before the
    /// registry lock can be released.
    pub(super) fn lock_heaps(&self, candidate: *mut mi_subproc_t) -> Option<LockedSubprocHeaps> {
        // SAFETY: this guard exclusively owns the initialized registry.
        unsafe { lock_heaps_from(self.head.as_ptr(), candidate) }
    }
}

unsafe fn assign_current_thread_from(
    head: *mut *mut mi_subproc_t,
    candidate: *mut mi_subproc_t,
    tld: TldPtr,
    main: SubprocPtr,
) -> AssignCurrentThreadResult {
    // SAFETY: caller exclusively owns the initialized registry, and candidate
    // is dereferenced only after address validation.
    let Some(subproc) = (unsafe { find_from(head, candidate) }) else {
        return AssignCurrentThreadResult::NotLinked;
    };
    // SAFETY: caller provides a live writable current-thread TLD.
    if unsafe { tld.subproc() } != Some(main) {
        return AssignCurrentThreadResult::AlreadyAssigned;
    }

    // SAFETY: registry membership keeps both subprocs live, and the caller
    // provides exclusive access to its current-thread TLD.
    unsafe {
        tld.set_subproc(Some(subproc));
        let thread_seq = subproc
            .thread_total_count_cell()
            .fetch_add(1, MemoryOrder::Relaxed);
        tld.set_thread_seq(ThreadSeq::new(thread_seq));
        main.thread_count_cell().fetch_sub(1, MemoryOrder::Relaxed);
        subproc
            .thread_count_cell()
            .fetch_add(1, MemoryOrder::Relaxed);
    }
    AssignCurrentThreadResult::Assigned
}

unsafe fn lock_heaps_from(
    head: *mut *mut mi_subproc_t,
    candidate: *mut mi_subproc_t,
) -> Option<LockedSubprocHeaps> {
    // SAFETY: caller exclusively owns the initialized registry, and candidate
    // is dereferenced only after address validation.
    let subproc = unsafe { find_from(head, candidate) }?;
    // SAFETY: registry membership keeps the dynamic lock live until acquired.
    let lock = unsafe { subproc.heaps_lock() };
    // SAFETY: destruction must acquire this same lock before freeing the
    // subproc's heaps and metadata, so the token keeps it live until Drop.
    unsafe { mi_lock_acquire(lock.as_ptr()) };
    Some(LockedSubprocHeaps { lock, subproc })
}

impl Drop for LockedSubprocHeaps {
    fn drop(&mut self) {
        // SAFETY: construction acquired this exact lock once. The destruction
        // protocol cannot free the subproc metadata until this release.
        unsafe { mi_lock_release(self.lock.as_ptr()) };
    }
}

impl LockedSubprocHeaps {
    /// Invokes `visitor` over the locked heap list without exposing the
    /// registry-validated subproc capability.
    ///
    /// # Safety
    /// The callback and argument must remain valid for the traversal, the
    /// callback must uphold the public heap-visitor reentrancy contract, and it
    /// must not unwind across the C ABI.
    pub(super) unsafe fn visit(
        &self,
        visitor: HeapVisitCallback,
        arg: HeapVisitCallbackArg,
    ) -> bool {
        // SAFETY: this token holds the target heap-list lock and keeps the
        // subproc metadata alive against concurrent destruction.
        let mut heap = unsafe { self.subproc.heaps() };
        let mut ok = true;
        while let Some(current) = heap
            && ok
        {
            // SAFETY: current is a node reached from the locked live heap list.
            let next = unsafe { (*(current.as_ptr().cast::<MiHeapMainPrefix>())).next };
            // SAFETY: current is a live heap node reached from the locked heap list; visitor invocation respects heap list protocol.
            ok = unsafe { visitor.invoke(current, arg) };
            heap = HeapPtr::new(next);
        }
        ok
    }
}

unsafe fn push_front(head: *mut *mut mi_subproc_t, subproc: SubprocPtr) {
    // SAFETY: caller owns the registry and provides an initialized unlinked node.
    unsafe {
        let prefix = mi_subproc_prefix_typed(subproc);
        (*prefix).prev = ptr::null_mut();
        (*prefix).next = *head;
        if let Some(old_head) = SubprocPtr::new(*head) {
            (*mi_subproc_prefix_typed(old_head)).prev = subproc.as_ptr();
        }
        *head = subproc.as_ptr();
    }
}

unsafe fn detach_from(
    head: *mut *mut mi_subproc_t,
    candidate: *mut mi_subproc_t,
) -> Option<DetachedSubproc> {
    // SAFETY: caller provides exclusive access to the initialized registry.
    let linked = unsafe { find_from(head, candidate) }?;
    // SAFETY: `linked` was reached from this exclusively owned registry.
    unsafe { unlink(head, linked) };
    Some(DetachedSubproc(linked))
}

unsafe fn find_from(
    head: *mut *mut mi_subproc_t,
    candidate: *mut mi_subproc_t,
) -> Option<SubprocPtr> {
    // SAFETY: caller provides exclusive access to the initialized head word.
    let mut current = SubprocPtr::new(unsafe { *head });
    while let Some(linked) = current {
        if ptr::eq(linked.as_ptr(), candidate) {
            return Some(linked);
        }
        // SAFETY: `linked` was reached from this exclusively owned registry.
        current = SubprocPtr::new(unsafe { (*mi_subproc_prefix_typed(linked)).next });
    }
    None
}

unsafe fn detach_first_except_from(
    head: *mut *mut mi_subproc_t,
    excluded: *mut mi_subproc_t,
) -> Option<DetachedSubproc> {
    // SAFETY: caller provides exclusive access to the initialized head word.
    let mut current = SubprocPtr::new(unsafe { *head });
    while let Some(linked) = current {
        if !ptr::eq(linked.as_ptr(), excluded) {
            // SAFETY: `linked` was reached from this exclusively owned registry.
            unsafe { unlink(head, linked) };
            return Some(DetachedSubproc(linked));
        }
        // SAFETY: `linked` was reached from this exclusively owned registry.
        current = SubprocPtr::new(unsafe { (*mi_subproc_prefix_typed(linked)).next });
    }
    None
}

unsafe fn unlink(head: *mut *mut mi_subproc_t, subproc: SubprocPtr) {
    // SAFETY: caller owns the registry and `subproc` is a node reached from it.
    unsafe {
        let prefix = mi_subproc_prefix_typed(subproc);
        let next = SubprocPtr::new((*prefix).next);
        let prev = SubprocPtr::new((*prefix).prev);

        if let Some(next) = next {
            (*mi_subproc_prefix_typed(next)).prev = (*prefix).prev;
        }
        if let Some(prev) = prev {
            (*mi_subproc_prefix_typed(prev)).next = (*prefix).next;
        } else {
            *head = (*prefix).next;
        }
        (*prefix).next = ptr::null_mut();
        (*prefix).prev = ptr::null_mut();
    }
}

#[cfg(test)]
mod tests {
    use core::ptr;
    use core::sync::atomic::Ordering;

    use super::*;
    use crate::internal::atomic::{mi_lock_init, mi_lock_release, mi_lock_t, mi_lock_try_acquire};
    use crate::internal::memid::mi_memid_t;
    use crate::types::{SubprocSeq, mi_tld_t};

    fn node(seq: usize) -> super::super::MiSubprocPrefix {
        super::super::MiSubprocPrefix::new_with_memid(SubprocSeq::new(seq), mi_memid_t::default())
    }

    unsafe fn node_ptr(node: &mut super::super::MiSubprocPrefix) -> SubprocPtr {
        // SAFETY: the caller keeps this stack node live for the whole list operation.
        unsafe { SubprocPtr::new_unchecked(ptr::from_mut(node).cast::<mi_subproc_t>()) }
    }

    unsafe fn links(node: SubprocPtr) -> (*mut mi_subproc_t, *mut mi_subproc_t) {
        let prefix = mi_subproc_prefix_typed(node);
        // SAFETY: the node's link fields are initialized and readable.
        unsafe {
            (
                ptr::read(ptr::addr_of!((*prefix).next)),
                ptr::read(ptr::addr_of!((*prefix).prev)),
            )
        }
    }

    fn tld(subproc: SubprocPtr) -> mi_tld_t {
        mi_tld_t {
            thread_id: 1,
            thread_seq: 0,
            numa_node: 0,
            subproc: subproc.as_ptr(),
            theaps: ptr::null_mut(),
            theaps_lock: mi_lock_t::new(),
            recurse: false,
            is_in_threadpool: false,
            memid: mi_memid_t::default(),
        }
    }

    #[test]
    fn detach_rejects_forged_stale_and_double_ids_without_dereference() {
        let mut first = node(1);
        let mut head = ptr::null_mut();
        // SAFETY: the stack node is initialized, live, and initially unlinked.
        let first_ptr = unsafe { node_ptr(&mut first) };
        // SAFETY: this test exclusively owns the local head and node.
        unsafe { push_front(ptr::addr_of_mut!(head), first_ptr) };

        let forged = NonNull::<mi_subproc_t>::dangling().as_ptr();
        // SAFETY: the local registry is initialized and exclusively owned;
        // `forged` is compared only by address.
        assert!(unsafe { detach_from(ptr::addr_of_mut!(head), forged) }.is_none());
        // SAFETY: the local registry is initialized and exclusively owned.
        let detached = unsafe { detach_from(ptr::addr_of_mut!(head), first_ptr.as_ptr()) };
        assert_eq!(
            detached.map(|node| node.ptr().as_ptr()),
            Some(first_ptr.as_ptr())
        );
        // SAFETY: the now-empty local registry is initialized and exclusively owned.
        assert!(unsafe { detach_from(ptr::addr_of_mut!(head), first_ptr.as_ptr()) }.is_none());
        assert!(head.is_null());
    }

    #[test]
    fn detach_preserves_head_middle_and_tail_topology() {
        let mut first = node(1);
        let mut middle = node(2);
        let mut last = node(3);
        let mut head = ptr::null_mut();
        // SAFETY: the stack nodes remain live and initially unlinked.
        let (first_ptr, middle_ptr, last_ptr) = unsafe {
            (
                node_ptr(&mut first),
                node_ptr(&mut middle),
                node_ptr(&mut last),
            )
        };
        // SAFETY: this test exclusively owns the local head and nodes.
        unsafe {
            push_front(ptr::addr_of_mut!(head), first_ptr);
            push_front(ptr::addr_of_mut!(head), middle_ptr);
            push_front(ptr::addr_of_mut!(head), last_ptr);
        }

        // SAFETY: the local registry is initialized and exclusively owned.
        assert!(unsafe { detach_from(ptr::addr_of_mut!(head), middle_ptr.as_ptr()) }.is_some());
        assert_eq!(head, last_ptr.as_ptr());
        // SAFETY: all stack nodes remain initialized and live.
        let (last_next, last_prev) = unsafe { links(last_ptr) };
        // SAFETY: all stack nodes remain initialized and live.
        let (_, first_prev) = unsafe { links(first_ptr) };
        // SAFETY: all stack nodes remain initialized and live.
        let (middle_next, middle_prev) = unsafe { links(middle_ptr) };
        assert_eq!(last_next, first_ptr.as_ptr());
        assert!(last_prev.is_null());
        assert_eq!(first_prev, last_ptr.as_ptr());
        assert!(middle_next.is_null());
        assert!(middle_prev.is_null());

        // SAFETY: the local registry is initialized and exclusively owned.
        let detached =
            unsafe { detach_first_except_from(ptr::addr_of_mut!(head), last_ptr.as_ptr()) };
        assert_eq!(
            detached.map(|node| node.ptr().as_ptr()),
            Some(first_ptr.as_ptr())
        );
        assert_eq!(head, last_ptr.as_ptr());
        // SAFETY: the stack node remains initialized and live.
        let (last_next, last_prev) = unsafe { links(last_ptr) };
        assert!(last_next.is_null());
        assert!(last_prev.is_null());

        // SAFETY: the local registry is initialized and exclusively owned.
        assert!(unsafe { detach_from(ptr::addr_of_mut!(head), last_ptr.as_ptr()) }.is_some());
        assert!(head.is_null());
    }

    #[test]
    fn assignment_validates_membership_before_mutating_current_tld() {
        let mut main = node(1);
        let mut target = node(2);
        let mut head = ptr::null_mut();
        main.thread_count.store(1, Ordering::Relaxed);
        // SAFETY: both stack nodes remain initialized and live.
        let (main_ptr, target_ptr) = unsafe { (node_ptr(&mut main), node_ptr(&mut target)) };
        // SAFETY: this test exclusively owns the local head and nodes.
        unsafe {
            push_front(ptr::addr_of_mut!(head), main_ptr);
            push_front(ptr::addr_of_mut!(head), target_ptr);
        }
        let mut tld = tld(main_ptr);
        // SAFETY: the stack TLD remains live and exclusively owned.
        let tld_ptr = unsafe { TldPtr::new_unchecked(ptr::from_mut(&mut tld)) };
        let forged = NonNull::<mi_subproc_t>::dangling().as_ptr();

        // SAFETY: the local registry and current-thread TLD are exclusively owned.
        assert!(matches!(
            unsafe {
                assign_current_thread_from(ptr::addr_of_mut!(head), forged, tld_ptr, main_ptr)
            },
            AssignCurrentThreadResult::NotLinked
        ));
        // SAFETY: the local registry and current-thread TLD are exclusively owned.
        assert!(matches!(
            unsafe {
                assign_current_thread_from(
                    ptr::addr_of_mut!(head),
                    target_ptr.as_ptr(),
                    tld_ptr,
                    main_ptr,
                )
            },
            AssignCurrentThreadResult::Assigned
        ));
        // SAFETY: the stack TLD remains initialized and live.
        assert_eq!(unsafe { tld_ptr.subproc() }, Some(target_ptr));
        // SAFETY: both stack nodes remain initialized and live.
        assert_eq!(
            unsafe { main_ptr.thread_count_cell().load(MemoryOrder::Relaxed) },
            0
        );
        // SAFETY: both stack nodes remain initialized and live.
        assert_eq!(
            unsafe { target_ptr.thread_count_cell().load(MemoryOrder::Relaxed) },
            1
        );
        // SAFETY: both stack nodes remain initialized and live.
        assert_eq!(
            unsafe {
                target_ptr
                    .thread_total_count_cell()
                    .load(MemoryOrder::Relaxed)
            },
            1
        );

        // SAFETY: association validation prevents a second mutation.
        assert!(matches!(
            unsafe {
                assign_current_thread_from(
                    ptr::addr_of_mut!(head),
                    target_ptr.as_ptr(),
                    tld_ptr,
                    main_ptr,
                )
            },
            AssignCurrentThreadResult::AlreadyAssigned
        ));
    }

    #[test]
    fn heaps_lock_handoff_rejects_forged_and_stale_nodes() {
        let mut target = node(1);
        let mut head = ptr::null_mut();
        // SAFETY: the stack node remains initialized and live.
        let target_ptr = unsafe { node_ptr(&mut target) };
        // SAFETY: initialize the test node's dynamic heap-list lock.
        unsafe { mi_lock_init(target_ptr.heaps_lock().as_ptr()) };
        // SAFETY: this test exclusively owns the local head and node.
        unsafe { push_front(ptr::addr_of_mut!(head), target_ptr) };

        let forged = NonNull::<mi_subproc_t>::dangling().as_ptr();
        // SAFETY: forged is compared only by address in the local registry.
        assert!(unsafe { lock_heaps_from(ptr::addr_of_mut!(head), forged) }.is_none());
        // SAFETY: the local registry keeps the target lock live through acquisition.
        let locked = unsafe { lock_heaps_from(ptr::addr_of_mut!(head), target_ptr.as_ptr()) }
            .expect("linked target must hand off its heap-list lock");
        // SAFETY: the test node's lock is initialized and currently held.
        assert!(!unsafe { mi_lock_try_acquire(locked.lock.as_ptr()) });
        drop(locked);
        // SAFETY: dropping the handoff token released the initialized lock.
        assert!(unsafe { mi_lock_try_acquire(target_ptr.heaps_lock().as_ptr()) });
        // SAFETY: release the successful test acquisition.
        unsafe { mi_lock_release(target_ptr.heaps_lock().as_ptr()) };

        // SAFETY: the local registry is initialized and exclusively owned.
        assert!(unsafe { detach_from(ptr::addr_of_mut!(head), target_ptr.as_ptr()) }.is_some());
        // SAFETY: stale is compared only by address in the now-empty registry.
        assert!(unsafe { lock_heaps_from(ptr::addr_of_mut!(head), target_ptr.as_ptr()) }.is_none());
        // SAFETY: the initialized stack lock is no longer held or reachable
        // from the local registry.
        unsafe { crate::internal::atomic::mi_lock_done(target_ptr.heaps_lock().as_ptr()) };
    }
}
