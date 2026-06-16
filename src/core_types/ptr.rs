//! DT-2 pointer domain types (zero-cost newtypes over NonNull).
//! All types are #[repr(transparent)], Copy, Clone, PartialEq, Eq, etc.
//! They do not implement Deref or DerefMut.

use core::ffi::c_void;
use core::ptr::NonNull;

use crate::types::{
    mi_arena_t, mi_block_t, mi_heap_t, mi_page_queue_t, mi_page_t, mi_subproc_t, mi_theap_t, mi_tld_t,
};

macro_rules! define_ptr_wrapper {
    ($name:ident, $raw_type:ty, $doc:expr) => {
        #[doc = $doc]
        #[repr(transparent)]
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
        pub(crate) struct $name(NonNull<$raw_type>);

        impl $name {
            #[inline(always)]
            pub(crate) fn new(ptr: *mut $raw_type) -> Option<Self> {
                NonNull::new(ptr).map(Self)
            }

            /// # Safety
            /// `ptr` must be non-null and point to valid metadata or data according to the type's invariants.
            #[inline(always)]
            pub(crate) unsafe fn new_unchecked(ptr: *mut $raw_type) -> Self {
                Self(NonNull::new_unchecked(ptr))
            }

            #[inline(always)]
            pub(crate) fn as_ptr(self) -> *mut $raw_type {
                self.0.as_ptr()
            }
            
            #[inline(always)]
            pub(crate) fn as_non_null(self) -> NonNull<$raw_type> {
                self.0
            }
        }
    };
}

define_ptr_wrapper!(PagePtr, mi_page_t, "Non-null, aligned raw page metadata");
define_ptr_wrapper!(BlockPtr, mi_block_t, "Non-null block pointer, block-aligned for its page");
define_ptr_wrapper!(HeapPtr, mi_heap_t, "Non-null raw heap metadata");
define_ptr_wrapper!(ThreadHeapPtr, mi_theap_t, "Non-null raw thread heap metadata");
define_ptr_wrapper!(TldPtr, mi_tld_t, "Non-null thread-local data");
define_ptr_wrapper!(ArenaPtr, mi_arena_t, "Non-null raw arena metadata");
define_ptr_wrapper!(SubprocPtr, mi_subproc_t, "Non-null raw subproc metadata");
define_ptr_wrapper!(PageQueuePtr, mi_page_queue_t, "Non-null page queue");
define_ptr_wrapper!(UserPtr, c_void, "Non-null pointer returned to/from user");
define_ptr_wrapper!(InteriorUserPtr, c_void, "User pointer may not equal block start");
define_ptr_wrapper!(PageStart, u8, "Start of page block area");
define_ptr_wrapper!(SliceStart, u8, "Aligned arena slice start");
define_ptr_wrapper!(OsMemPtr, c_void, "OS allocation base");

/// Non-null raw page metadata (read-only view).
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct PageConstPtr(NonNull<mi_page_t>);

impl PageConstPtr {
    #[inline(always)]
    pub(crate) fn new(ptr: *const mi_page_t) -> Option<Self> {
        NonNull::new(ptr as *mut mi_page_t).map(Self)
    }

    /// # Safety
    /// `ptr` must be non-null and point to valid page metadata.
    #[inline(always)]
    pub(crate) unsafe fn new_unchecked(ptr: *const mi_page_t) -> Self {
        Self(NonNull::new_unchecked(ptr as *mut mi_page_t))
    }

    #[inline(always)]
    pub(crate) fn as_ptr(self) -> *const mi_page_t {
        self.0.as_ptr() as *const mi_page_t
    }
}
