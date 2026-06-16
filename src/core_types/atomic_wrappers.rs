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
#[derive(Clone, Copy)]
pub(crate) struct AtomicThreadFree(pub(crate) *mut usize);

impl AtomicThreadFree {
    pub(crate) fn load_relaxed(self) -> ThreadFreeWord {
        let val = unsafe { core::sync::atomic::AtomicUsize::from_ptr(self.0).load(Ordering::Relaxed) };
        ThreadFreeWord(val)
    }
    
    pub(crate) fn compare_exchange_push(
        self,
        old: &mut ThreadFreeWord,
        block: BlockPtr,
    ) -> Result<ThreadFreeWord, ThreadFreeWord> {
        let new_tf = ThreadFreeWord::from_parts(Some(block), crate::core_types::ThreadFreeOwner::Owned);
        let atomic = unsafe { &*self.0.cast::<core::sync::atomic::AtomicUsize>() };
        match atomic.compare_exchange_weak(
            old.raw(),
            new_tf.raw(),
            Ordering::AcqRel,
            Ordering::Relaxed,
        ) {
            Ok(_) => Ok(new_tf),
            Err(val) => {
                *old = ThreadFreeWord::from_parts(ThreadFreeWord(val).head(), if ThreadFreeWord(val).is_owned() { crate::core_types::ThreadFreeOwner::Owned } else { crate::core_types::ThreadFreeOwner::Unowned });
                Err(*old)
            }
        }
    }
    
    pub(crate) fn claim_ownership(self) -> bool {
        unimplemented!()
    }
    
    pub(crate) fn release_ownership(self, _expected_head: Option<BlockPtr>) -> bool {
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
