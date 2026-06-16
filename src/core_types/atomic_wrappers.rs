//! DT-13: Atomic field wrappers.

use core::marker::PhantomData;
use core::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};
use core::ffi::c_void;
use core::ptr::NonNull;

use crate::core_types::{BlockPtr, PageFlags, PageOwnerId, PageThreadId, RemoteFreeList, ThreadFreeWord};

pub(crate) type MemoryOrder = Ordering;

#[repr(transparent)]
pub(crate) struct AtomicUsizeField(pub(crate) *mut usize);

#[repr(transparent)]
pub(crate) struct AtomicPageThreadId(pub(crate) AtomicUsizeField);

impl AtomicPageThreadId {
    pub(crate) fn load_relaxed(self) -> PageThreadId {
        unimplemented!()
    }
    
    pub(crate) fn set_flags(self, _flags: PageFlags, _set: bool) -> PageThreadId {
        unimplemented!()
    }
    
    pub(crate) fn set_owner_release(self, _owner: PageOwnerId) {
        unimplemented!()
    }
}

pub(crate) enum RemotePushOutcome {
    Success,
    Failed(BlockPtr)
}

#[repr(transparent)]
pub(crate) struct AtomicThreadFree(pub(crate) AtomicUsizeField);

impl AtomicThreadFree {
    pub(crate) fn load_relaxed(self) -> ThreadFreeWord {
        unimplemented!()
    }
    
    pub(crate) fn push_remote_acq_rel(self, _block: BlockPtr) -> RemotePushOutcome {
        unimplemented!()
    }
    
    pub(crate) fn collect_acq_rel(self) -> Option<RemoteFreeList> {
        unimplemented!()
    }
    
    pub(crate) fn claim_acq_rel(self) -> bool {
        unimplemented!()
    }
}

#[repr(transparent)]
pub(crate) struct AtomicAbandonedCount(pub(crate) *const AtomicUsize);

#[repr(transparent)]
pub(crate) struct AtomicArenaPage(pub(crate) *const AtomicPtr<c_void>);

pub(crate) struct AtomicGuard<'a> {
    pub(crate) field: AtomicUsizeField,
    pub(crate) _borrow: PhantomData<&'a ()>,
}

#[repr(transparent)]
pub(crate) struct OnceStatePtr(pub(crate) *mut usize);

pub(crate) struct OnceGuard<'a> {
    pub(crate) once: OnceStatePtr,
    pub(crate) _borrow: PhantomData<&'a ()>,
}

#[repr(transparent)]
pub(crate) struct MiLockPtr(pub(crate) NonNull<crate::types::mi_lock_t_pub>);

pub(crate) struct MiLockGuard<'a> {
    pub(crate) lock: MiLockPtr,
    pub(crate) _borrow: PhantomData<&'a ()>,
}
