use core::ffi::c_void;

#[cfg(miri)]
mod miri {
    use super::c_void;
    use std::collections::BTreeMap;
    use std::sync::{LazyLock, RwLock};

    #[derive(Clone, Copy)]
    struct ProvenancePtr(*mut c_void);

    unsafe impl Send for ProvenancePtr {}
    unsafe impl Sync for ProvenancePtr {}

    #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    struct EncodedKey {
        word: usize,
        null_addr: usize,
        key0: usize,
        key1: usize,
    }

    static ADDRESS_PROVENANCE: LazyLock<RwLock<BTreeMap<usize, ProvenancePtr>>> =
        LazyLock::new(|| RwLock::new(BTreeMap::new()));
    static ENCODED_PROVENANCE: LazyLock<RwLock<BTreeMap<EncodedKey, ProvenancePtr>>> =
        LazyLock::new(|| RwLock::new(BTreeMap::new()));

    macro_rules! provenance_abort {
        ($message:literal) => {{
            crate::internal::_mi_rust_assert_fail(
                concat!($message, "\0").as_ptr().cast(),
                concat!(file!(), "\0").as_ptr().cast(),
                line!(),
            )
        }};
    }

    fn provenance_ptr<T>(ptr: *const T) -> ProvenancePtr {
        ProvenancePtr(ptr.cast_mut().cast::<c_void>())
    }

    fn encoded_key(word: usize, null_context: *const c_void, keys: &[usize; 2]) -> EncodedKey {
        EncodedKey {
            word,
            null_addr: null_context.addr(),
            key0: keys[0],
            key1: keys[1],
        }
    }

    pub(crate) fn remember_address<T>(addr: usize, ptr: *const T) {
        if ptr.is_null() {
            return;
        }
        match ADDRESS_PROVENANCE.write() {
            Ok(mut registry) => {
                registry.insert(addr, provenance_ptr(ptr));
            }
            Err(_) => provenance_abort!("Miri provenance address registry poisoned"),
        }
    }

    pub(crate) fn resolve_address<T>(addr: usize) -> Option<*mut T> {
        match ADDRESS_PROVENANCE.read() {
            Ok(registry) => registry.get(&addr).map(|ptr| ptr.0.cast::<T>()),
            Err(_) => provenance_abort!("Miri provenance address registry poisoned"),
        }
    }

    pub(crate) fn remember_encoded<T>(
        word: usize,
        null_context: *const c_void,
        keys: &[usize; 2],
        ptr: *const T,
    ) {
        if ptr.is_null() {
            return;
        }
        match ENCODED_PROVENANCE.write() {
            Ok(mut registry) => {
                registry.insert(encoded_key(word, null_context, keys), provenance_ptr(ptr));
            }
            Err(_) => provenance_abort!("Miri provenance encoded registry poisoned"),
        }
    }

    pub(crate) fn resolve_encoded<T>(
        word: usize,
        null_context: *const c_void,
        keys: &[usize; 2],
    ) -> Option<*mut T> {
        match ENCODED_PROVENANCE.read() {
            Ok(registry) => registry
                .get(&encoded_key(word, null_context, keys))
                .map(|ptr| ptr.0.cast::<T>()),
            Err(_) => provenance_abort!("Miri provenance encoded registry poisoned"),
        }
    }

    pub(crate) fn missing_address<T>(addr: usize) -> *mut T {
        let _ = addr;
        provenance_abort!("missing Miri strict-provenance address entry")
    }

    pub(crate) fn missing_encoded<T>(
        word: usize,
        null_context: *const c_void,
        keys: &[usize; 2],
    ) -> *mut T {
        let _ = (word, null_context, keys);
        provenance_abort!("missing Miri strict-provenance encoded entry")
    }
}

#[cfg(miri)]
pub(crate) use miri::*;

#[cfg(not(miri))]
#[inline(always)]
pub(crate) fn remember_address<T>(addr: usize, ptr: *const T) {
    let _ = (addr, ptr);
}

#[cfg(not(miri))]
#[inline(always)]
pub(crate) fn remember_encoded<T>(
    word: usize,
    null_context: *const c_void,
    keys: &[usize; 2],
    ptr: *const T,
) {
    let _ = (word, null_context, keys, ptr);
}
