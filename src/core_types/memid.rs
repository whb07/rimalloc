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
    pub(crate) raw: crate::internal::mi_memid_t,
    _kind: PhantomData<K>,
}

pub(crate) enum MemIdView {
    None(MemId<NoneMem>),
    External(MemId<ExternalMem>),
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
            MemKind::External => MemIdView::External(MemId {
                raw,
                _kind: PhantomData,
            }),
            MemKind::Static => MemIdView::Static(MemId {
                raw,
                _kind: PhantomData,
            }),
            MemKind::Meta => {
                let m = unsafe { raw.mem.meta };
                let Some(block_count) = core::num::NonZeroU32::new(m.block_count) else {
                    return MemIdView::Unknown(code);
                };
                MemIdView::Meta(MetaMemId {
                    meta_page: unsafe { NonNull::new_unchecked(m.meta_page) },
                    block_index: BlockIndex(m.block_index),
                    block_count,
                })
            }
            MemKind::Os | MemKind::OsHuge | MemKind::OsRemap => {
                let os = unsafe { raw.mem.os };
                MemIdView::Os(OsMemId {
                    base: OsMemPtr::new(os.base),
                    size: ReservedBytes(os.size),
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
                MemIdView::Arena(ArenaMemId {
                    arena: unsafe { ArenaPtr::new_unchecked(a.arena) },
                    range: SliceRange::from_raw(a.slice_index, a.slice_count),
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

impl MemId<OsMem> {
    pub(crate) fn os(
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
                memkind: crate::internal::mi_memkind_t::MI_MEM_OS as _,
            },
            _kind: PhantomData,
        }
    }
}

impl MemId<ArenaMem> {
    pub(crate) fn arena(arena: ArenaPtr, range: SliceRange) -> Self {
        Self {
            raw: crate::internal::mi_memid_t {
                mem: crate::internal::mi_memid_mem_t {
                    arena: crate::internal::mi_memid_arena_info_t {
                        arena: arena.as_ptr(),
                        slice_index: range.start().0 as u32,
                        slice_count: range.count().0.get(),
                    },
                },
                is_pinned: false,
                initially_committed: false, // Default false until proven
                initially_zero: false,      // Default false until proven
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
                is_pinned: false,
                initially_committed: false,
                initially_zero: false,
                memkind: crate::internal::mi_memkind_t::MI_MEM_META as _,
            },
            _kind: PhantomData,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct BlockIndex(pub(crate) u32);
#[derive(Clone, Copy, Debug)]
pub(crate) struct ReservedBytes(pub(crate) usize);
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
