//! DT-7: MemId typed views.

use core::marker::PhantomData;
use core::ptr::NonNull;

use crate::core_types::{ArenaPtr, OsMemPtr, SliceRange};

/// The internal C-compatible memkind code.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct MemKindCode(pub(crate) i32);

impl MemKindCode {
    pub(crate) const NONE: Self = Self(0);
    pub(crate) const EXTERNAL: Self = Self(1);
    pub(crate) const STATIC: Self = Self(2);
    pub(crate) const META: Self = Self(3);
    pub(crate) const OS: Self = Self(4);
    pub(crate) const OS_HUGE: Self = Self(5);
    pub(crate) const OS_REMAP: Self = Self(6);
    pub(crate) const ARENA: Self = Self(7);
    pub(crate) const HEAP_MAIN: Self = Self(8);
}

/// A typed interpretation of a MemKindCode.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum MemKind {
    None,
    External,
    Static,
    Meta,
    Os,
    OsHuge,
    OsRemap,
    Arena,
    HeapMain,
    Unknown(MemKindCode),
}

impl From<MemKindCode> for MemKind {
    fn from(code: MemKindCode) -> Self {
        match code {
            MemKindCode::NONE => Self::None,
            MemKindCode::EXTERNAL => Self::External,
            MemKindCode::STATIC => Self::Static,
            MemKindCode::META => Self::Meta,
            MemKindCode::OS => Self::Os,
            MemKindCode::OS_HUGE => Self::OsHuge,
            MemKindCode::OS_REMAP => Self::OsRemap,
            MemKindCode::ARENA => Self::Arena,
            MemKindCode::HEAP_MAIN => Self::HeapMain,
            _ => Self::Unknown(code),
        }
    }
}

pub(crate) enum AnyMem {}
pub(crate) enum NoneMem {}
pub(crate) enum ExternalMem {}
pub(crate) enum StaticMem {}
pub(crate) enum MetaMem {}
pub(crate) enum OsMem {}
pub(crate) enum ArenaMem {}
pub(crate) enum HeapMainMem {}

/// Safely wraps the raw `mi_memid_t` with a phantom tag proving its valid discriminator.
#[repr(transparent)]
pub(crate) struct MemId<K = AnyMem> {
    raw: crate::internal::mi_memid_t,
    _kind: PhantomData<K>,
}

impl<K> MemId<K> {
    #[inline(always)]
    pub(crate) const fn into_raw(self) -> crate::internal::mi_memid_t {
        self.raw
    }

    #[inline(always)]
    pub(crate) const fn as_raw(&self) -> crate::internal::mi_memid_t {
        self.raw
    }
}

pub(crate) enum MemIdView {
    None(MemId<NoneMem>),
    External(ExternalMemId),
    Static(MemId<StaticMem>),
    Meta(MetaMemId),
    Os(OsMemId),
    Arena(ArenaMemId),
    HeapMain(MemId<HeapMainMem>),
    Unknown(MemKindCode),
}

impl MemId<AnyMem> {
    pub(crate) fn view(raw: crate::internal::mi_memid_t) -> MemIdView {
        let code = MemKindCode(raw.memkind as i32);
        match MemKind::from(code) {
            MemKind::None => MemIdView::None(MemId {
                raw,
                _kind: PhantomData,
            }),
            MemKind::External => {
                let os = unsafe { raw.mem.os };
                MemIdView::External(ExternalMemId {
                    base: OsMemPtr::new(os.base),
                    size: ReservedBytes::new(os.size),
                    initially_committed: InitiallyCommitted(raw.initially_committed),
                    initially_zero: InitiallyZero(raw.initially_zero),
                    pinned: Pinned(raw.is_pinned),
                })
            }
            MemKind::Static => MemIdView::Static(MemId {
                raw,
                _kind: PhantomData,
            }),
            MemKind::Meta => {
                let m = unsafe { raw.mem.meta };
                let Some(block_count) = core::num::NonZeroU32::new(m.block_count) else {
                    return MemIdView::Unknown(code);
                };
                let Some(meta_page) = NonNull::new(m.meta_page) else {
                    return MemIdView::Unknown(code);
                };
                MemIdView::Meta(MetaMemId {
                    meta_page,
                    block_index: BlockIndex(m.block_index),
                    block_count,
                })
            }
            MemKind::Os | MemKind::OsHuge | MemKind::OsRemap => {
                let os = unsafe { raw.mem.os };
                MemIdView::Os(OsMemId {
                    base: OsMemPtr::new(os.base),
                    size: ReservedBytes::new(os.size),
                    initially_committed: InitiallyCommitted(raw.initially_committed),
                    initially_zero: InitiallyZero(raw.initially_zero),
                    pinned: Pinned(raw.is_pinned),
                    kind: match MemKind::from(code) {
                        MemKind::OsHuge => OsMemKind::Huge,
                        MemKind::OsRemap => OsMemKind::Remap,
                        _ => OsMemKind::Normal,
                    },
                })
            }
            MemKind::Arena => {
                let a = unsafe { raw.mem.arena };
                let Some(arena) = ArenaPtr::new(a.arena) else {
                    return MemIdView::Unknown(code);
                };
                let Ok(range) = SliceRange::from_raw(a.slice_index, a.slice_count) else {
                    return MemIdView::Unknown(code);
                };
                MemIdView::Arena(ArenaMemId {
                    arena,
                    range,
                    initially_committed: InitiallyCommitted(raw.initially_committed),
                    initially_zero: InitiallyZero(raw.initially_zero),
                    pinned: Pinned(raw.is_pinned),
                })
            }
            MemKind::HeapMain => MemIdView::HeapMain(MemId {
                raw,
                _kind: PhantomData,
            }),
            MemKind::Unknown(c) => MemIdView::Unknown(c),
        }
    }
}

impl MemId<NoneMem> {
    pub(crate) const fn none() -> Self {
        Self {
            raw: crate::internal::mi_memid_t {
                mem: crate::internal::mi_memid_mem_t {
                    os: crate::internal::mi_memid_os_info_t {
                        base: core::ptr::null_mut(),
                        size: 0,
                    },
                },
                is_pinned: false,
                initially_committed: false,
                initially_zero: false,
                memkind: crate::internal::mi_memkind_t::MI_MEM_NONE as _,
            },
            _kind: PhantomData,
        }
    }
}

impl MemId<StaticMem> {
    pub(crate) const fn static_mem() -> Self {
        Self {
            raw: crate::internal::mi_memid_t {
                mem: crate::internal::mi_memid_mem_t {
                    os: crate::internal::mi_memid_os_info_t {
                        base: core::ptr::null_mut(),
                        size: 0,
                    },
                },
                is_pinned: false,
                initially_committed: false,
                initially_zero: false,
                memkind: crate::internal::mi_memkind_t::MI_MEM_STATIC as _,
            },
            _kind: PhantomData,
        }
    }
}

impl MemId<HeapMainMem> {
    pub(crate) const fn heap_main() -> Self {
        Self {
            raw: crate::internal::mi_memid_t {
                mem: crate::internal::mi_memid_mem_t {
                    os: crate::internal::mi_memid_os_info_t {
                        base: core::ptr::null_mut(),
                        size: 0,
                    },
                },
                is_pinned: false,
                initially_committed: false,
                initially_zero: false,
                memkind: crate::internal::mi_memkind_t::MI_MEM_HEAP_MAIN as _,
            },
            _kind: PhantomData,
        }
    }
}

impl MemId<OsMem> {
    pub(crate) fn os(
        base: Option<OsMemPtr>,
        size: ReservedBytes,
        committed: InitiallyCommitted,
        zero: InitiallyZero,
        pinned: Pinned,
    ) -> Self {
        Self::os_with_kind(base, size, committed, zero, pinned, OsMemKind::Normal)
    }

    pub(crate) fn os_huge(
        base: Option<OsMemPtr>,
        size: ReservedBytes,
        committed: InitiallyCommitted,
        zero: InitiallyZero,
        pinned: Pinned,
    ) -> Self {
        Self::os_with_kind(base, size, committed, zero, pinned, OsMemKind::Huge)
    }

    pub(crate) fn os_remap(
        base: Option<OsMemPtr>,
        size: ReservedBytes,
        committed: InitiallyCommitted,
        zero: InitiallyZero,
        pinned: Pinned,
    ) -> Self {
        Self::os_with_kind(base, size, committed, zero, pinned, OsMemKind::Remap)
    }

    fn os_with_kind(
        base: Option<OsMemPtr>,
        size: ReservedBytes,
        committed: InitiallyCommitted,
        zero: InitiallyZero,
        pinned: Pinned,
        kind: OsMemKind,
    ) -> Self {
        Self {
            raw: crate::internal::mi_memid_t {
                mem: crate::internal::mi_memid_mem_t {
                    os: crate::internal::mi_memid_os_info_t {
                        base: base.map_or(core::ptr::null_mut(), |p| p.as_ptr()),
                        size: size.0,
                    },
                },
                is_pinned: pinned.0,
                initially_committed: committed.0,
                initially_zero: zero.0,
                memkind: match kind {
                    OsMemKind::Normal => crate::internal::mi_memkind_t::MI_MEM_OS,
                    OsMemKind::Huge => crate::internal::mi_memkind_t::MI_MEM_OS_HUGE,
                    OsMemKind::Remap => crate::internal::mi_memkind_t::MI_MEM_OS_REMAP,
                },
            },
            _kind: PhantomData,
        }
    }
}

impl MemId<ExternalMem> {
    pub(crate) fn external(
        base: Option<OsMemPtr>,
        size: ReservedBytes,
        committed: InitiallyCommitted,
        zero: InitiallyZero,
        pinned: Pinned,
    ) -> Self {
        Self {
            raw: crate::internal::mi_memid_t {
                mem: crate::internal::mi_memid_mem_t {
                    os: crate::internal::mi_memid_os_info_t {
                        base: base.map_or(core::ptr::null_mut(), |p| p.as_ptr()),
                        size: size.0,
                    },
                },
                is_pinned: pinned.0,
                initially_committed: committed.0,
                initially_zero: zero.0,
                memkind: crate::internal::mi_memkind_t::MI_MEM_EXTERNAL as _,
            },
            _kind: PhantomData,
        }
    }
}

impl MemId<ArenaMem> {
    pub(crate) fn arena(
        arena: ArenaPtr,
        range: SliceRange,
        committed: InitiallyCommitted,
        zero: InitiallyZero,
        pinned: Pinned,
    ) -> Self {
        Self {
            raw: crate::internal::mi_memid_t {
                mem: crate::internal::mi_memid_mem_t {
                    arena: crate::internal::mi_memid_arena_info_t {
                        arena: arena.as_ptr(),
                        slice_index: range.start_u32(),
                        slice_count: range.count_u32(),
                    },
                },
                is_pinned: pinned.0,
                initially_committed: committed.0,
                initially_zero: zero.0,
                memkind: crate::internal::mi_memkind_t::MI_MEM_ARENA as _,
            },
            _kind: PhantomData,
        }
    }
}

impl MemId<MetaMem> {
    pub(crate) fn meta(
        meta_page: NonNull<core::ffi::c_void>,
        block_index: BlockIndex,
        block_count: core::num::NonZeroU32,
    ) -> Self {
        Self {
            raw: crate::internal::mi_memid_t {
                mem: crate::internal::mi_memid_mem_t {
                    meta: crate::internal::mi_memid_meta_info_t {
                        meta_page: meta_page.as_ptr(),
                        block_index: block_index.0,
                        block_count: block_count.get(),
                    },
                },
                is_pinned: true,
                initially_committed: true,
                initially_zero: true,
                memkind: crate::internal::mi_memkind_t::MI_MEM_META as _,
            },
            _kind: PhantomData,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct BlockIndex(pub(crate) u32);

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct ReservedBytes(pub(crate) usize);

impl ReservedBytes {
    #[inline(always)]
    pub(crate) const fn new(size: usize) -> Self {
        Self(size)
    }

    #[inline(always)]
    pub(crate) const fn get(self) -> usize {
        self.0
    }

    #[inline(always)]
    pub(crate) const fn is_zero(self) -> bool {
        self.0 == 0
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct CommittedBytes(pub(crate) usize);

impl CommittedBytes {
    #[inline(always)]
    pub(crate) const fn new(size: usize) -> Self {
        Self(size)
    }

    #[inline(always)]
    pub(crate) const fn get(self) -> usize {
        self.0
    }

    #[inline(always)]
    pub(crate) const fn is_zero(self) -> bool {
        self.0 == 0
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct InitiallyCommitted(pub(crate) bool);
#[derive(Clone, Copy, Debug)]
pub(crate) struct InitiallyZero(pub(crate) bool);
#[derive(Clone, Copy, Debug)]
pub(crate) struct Pinned(pub(crate) bool);

pub(crate) struct OsMemId {
    pub(crate) base: Option<OsMemPtr>,
    pub(crate) size: ReservedBytes,
    pub(crate) initially_committed: InitiallyCommitted,
    pub(crate) initially_zero: InitiallyZero,
    pub(crate) pinned: Pinned,
    pub(crate) kind: OsMemKind,
}

pub(crate) enum OsMemKind {
    Normal,
    Huge,
    Remap,
}

pub(crate) struct ExternalMemId {
    pub(crate) base: Option<OsMemPtr>,
    pub(crate) size: ReservedBytes,
    pub(crate) initially_committed: InitiallyCommitted,
    pub(crate) initially_zero: InitiallyZero,
    pub(crate) pinned: Pinned,
}

pub(crate) struct ArenaMemId {
    pub(crate) arena: ArenaPtr,
    pub(crate) range: SliceRange,
    pub(crate) initially_committed: InitiallyCommitted,
    pub(crate) initially_zero: InitiallyZero,
    pub(crate) pinned: Pinned,
}

pub(crate) struct MetaMemId {
    pub(crate) meta_page: NonNull<core::ffi::c_void>,
    pub(crate) block_index: BlockIndex,
    pub(crate) block_count: core::num::NonZeroU32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::ffi::c_void;
    use core::mem::MaybeUninit;
    use core::ptr;

    #[test]
    fn memkind_code_maps_known_and_unknown_values() {
        assert_eq!(MemKind::from(MemKindCode::NONE), MemKind::None);
        assert_eq!(MemKind::from(MemKindCode::EXTERNAL), MemKind::External);
        assert_eq!(MemKind::from(MemKindCode::STATIC), MemKind::Static);
        assert_eq!(MemKind::from(MemKindCode::META), MemKind::Meta);
        assert_eq!(MemKind::from(MemKindCode::OS), MemKind::Os);
        assert_eq!(MemKind::from(MemKindCode::OS_HUGE), MemKind::OsHuge);
        assert_eq!(MemKind::from(MemKindCode::OS_REMAP), MemKind::OsRemap);
        assert_eq!(MemKind::from(MemKindCode::ARENA), MemKind::Arena);
        assert_eq!(MemKind::from(MemKindCode::HEAP_MAIN), MemKind::HeapMain);
        assert_eq!(
            MemKind::from(MemKindCode(99)),
            MemKind::Unknown(MemKindCode(99))
        );
    }

    #[test]
    fn os_memid_round_trips_through_view() {
        let mut byte = 0u8;
        let base = unsafe { OsMemPtr::new_unchecked((&mut byte as *mut u8).cast::<c_void>()) };
        let normal = MemId::<OsMem>::os(
            Some(base),
            ReservedBytes::new(2048),
            InitiallyCommitted(false),
            InitiallyZero(true),
            Pinned(false),
        );

        let MemIdView::Os(view) = MemId::<AnyMem>::view(normal.as_raw()) else {
            panic!("expected normal OS memid view");
        };
        assert_eq!(view.base.map(OsMemPtr::as_ptr), Some(base.as_ptr()));
        assert_eq!(view.size.get(), 2048);
        assert!(!view.initially_committed.0);
        assert!(view.initially_zero.0);
        assert!(!view.pinned.0);
        assert!(matches!(view.kind, OsMemKind::Normal));

        let memid = MemId::<OsMem>::os_huge(
            Some(base),
            ReservedBytes::new(4096),
            InitiallyCommitted(true),
            InitiallyZero(false),
            Pinned(true),
        );

        let MemIdView::Os(view) = MemId::<AnyMem>::view(memid.as_raw()) else {
            panic!("expected OS memid view");
        };
        assert_eq!(view.base.map(OsMemPtr::as_ptr), Some(base.as_ptr()));
        assert_eq!(view.size.get(), 4096);
        assert!(view.initially_committed.0);
        assert!(!view.initially_zero.0);
        assert!(view.pinned.0);
        assert!(matches!(view.kind, OsMemKind::Huge));
    }

    #[test]
    fn os_remap_memid_round_trips_through_view() {
        let mut byte = 0u8;
        let base = unsafe { OsMemPtr::new_unchecked((&mut byte as *mut u8).cast::<c_void>()) };
        let memid = MemId::<OsMem>::os_remap(
            Some(base),
            ReservedBytes::new(16384),
            InitiallyCommitted(false),
            InitiallyZero(true),
            Pinned(false),
        );

        let MemIdView::Os(view) = MemId::<AnyMem>::view(memid.as_raw()) else {
            panic!("expected OS remap memid view");
        };
        assert_eq!(view.base.map(OsMemPtr::as_ptr), Some(base.as_ptr()));
        assert_eq!(view.size.get(), 16384);
        assert!(!view.initially_committed.0);
        assert!(view.initially_zero.0);
        assert!(!view.pinned.0);
        assert!(matches!(view.kind, OsMemKind::Remap));
    }

    #[test]
    fn external_and_meta_memids_round_trip_through_view() {
        let mut byte = 0u8;
        let base = unsafe { OsMemPtr::new_unchecked((&mut byte as *mut u8).cast::<c_void>()) };
        let external = MemId::<ExternalMem>::external(
            Some(base),
            ReservedBytes::new(8192),
            InitiallyCommitted(false),
            InitiallyZero(true),
            Pinned(false),
        );

        let MemIdView::External(view) = MemId::<AnyMem>::view(external.as_raw()) else {
            panic!("expected external memid view");
        };
        assert_eq!(view.base.map(OsMemPtr::as_ptr), Some(base.as_ptr()));
        assert_eq!(view.size.get(), 8192);
        assert!(!view.initially_committed.0);
        assert!(view.initially_zero.0);
        assert!(!view.pinned.0);

        let mut meta_byte = 0u8;
        let meta_page = NonNull::new((&mut meta_byte as *mut u8).cast::<c_void>()).unwrap();
        let meta = MemId::<MetaMem>::meta(
            meta_page,
            BlockIndex(7),
            core::num::NonZeroU32::new(3).unwrap(),
        );

        let MemIdView::Meta(view) = MemId::<AnyMem>::view(meta.as_raw()) else {
            panic!("expected meta memid view");
        };
        assert_eq!(view.meta_page, meta_page);
        assert_eq!(view.block_index.0, 7);
        assert_eq!(view.block_count.get(), 3);
    }

    #[test]
    fn arena_memid_round_trips_slice_metadata() {
        let mut arena = MaybeUninit::<crate::types::mi_arena_t>::uninit();
        let arena_ptr = unsafe { ArenaPtr::new_unchecked(arena.as_mut_ptr()) };
        unsafe {
            arena_ptr.set_slice_count(crate::core_types::SliceCount::from_usize(32).unwrap());
        }
        let range = SliceRange::new(11, 5, arena_ptr).unwrap();
        let memid = MemId::<ArenaMem>::arena(
            arena_ptr,
            range,
            InitiallyCommitted(true),
            InitiallyZero(false),
            Pinned(true),
        );

        let MemIdView::Arena(view) = MemId::<AnyMem>::view(memid.as_raw()) else {
            panic!("expected arena memid view");
        };
        assert_eq!(view.arena.as_ptr(), arena_ptr.as_ptr());
        assert_eq!(view.range.start_usize(), 11);
        assert_eq!(view.range.count_usize(), 5);
        assert!(view.initially_committed.0);
        assert!(!view.initially_zero.0);
        assert!(view.pinned.0);
    }

    #[test]
    fn heap_main_memid_round_trips_through_view() {
        let memid = MemId::<HeapMainMem>::heap_main();

        let MemIdView::HeapMain(view) = MemId::<AnyMem>::view(memid.as_raw()) else {
            panic!("expected heap-main memid view");
        };
        assert_eq!(
            view.as_raw().memkind,
            crate::internal::mi_memkind_t::MI_MEM_HEAP_MAIN as _
        );
    }

    #[test]
    fn memid_view_returns_unknown_for_null_meta_or_arena_union_pointers() {
        let null_meta = crate::internal::mi_memid_t {
            mem: crate::internal::mi_memid_mem_t {
                meta: crate::internal::mi_memid_meta_info_t {
                    meta_page: ptr::null_mut(),
                    block_index: 0,
                    block_count: 1,
                },
            },
            is_pinned: true,
            initially_committed: true,
            initially_zero: true,
            memkind: crate::internal::mi_memkind_t::MI_MEM_META as _,
        };
        let MemIdView::Unknown(code) = MemId::<AnyMem>::view(null_meta) else {
            panic!("expected unknown view for null meta page");
        };
        assert_eq!(code, MemKindCode::META);

        let null_arena = crate::internal::mi_memid_t {
            mem: crate::internal::mi_memid_mem_t {
                arena: crate::internal::mi_memid_arena_info_t {
                    arena: ptr::null_mut(),
                    slice_index: 0,
                    slice_count: 1,
                },
            },
            is_pinned: false,
            initially_committed: false,
            initially_zero: false,
            memkind: crate::internal::mi_memkind_t::MI_MEM_ARENA as _,
        };
        let MemIdView::Unknown(code) = MemId::<AnyMem>::view(null_arena) else {
            panic!("expected unknown view for null arena pointer");
        };
        assert_eq!(code, MemKindCode::ARENA);
    }

    #[test]
    fn memid_view_returns_unknown_for_invalid_arena_slice_range() {
        let mut arena = MaybeUninit::<crate::types::mi_arena_t>::uninit();
        let arena = arena.as_mut_ptr();

        let zero_count = crate::internal::mi_memid_t {
            mem: crate::internal::mi_memid_mem_t {
                arena: crate::internal::mi_memid_arena_info_t {
                    arena,
                    slice_index: 0,
                    slice_count: 0,
                },
            },
            is_pinned: false,
            initially_committed: false,
            initially_zero: false,
            memkind: crate::internal::mi_memkind_t::MI_MEM_ARENA as _,
        };
        let MemIdView::Unknown(code) = MemId::<AnyMem>::view(zero_count) else {
            panic!("expected unknown view for zero arena slice count");
        };
        assert_eq!(code, MemKindCode::ARENA);

        let overflow = crate::internal::mi_memid_t {
            mem: crate::internal::mi_memid_mem_t {
                arena: crate::internal::mi_memid_arena_info_t {
                    arena,
                    slice_index: u32::MAX,
                    slice_count: 1,
                },
            },
            is_pinned: false,
            initially_committed: false,
            initially_zero: false,
            memkind: crate::internal::mi_memkind_t::MI_MEM_ARENA as _,
        };
        let MemIdView::Unknown(code) = MemId::<AnyMem>::view(overflow) else {
            panic!("expected unknown view for overflowing arena slice range");
        };
        assert_eq!(code, MemKindCode::ARENA);
    }
}
