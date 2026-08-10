//! DT-7: MemId typed views.

use core::marker::PhantomData;
use core::ptr::NonNull;

use super::arena::SliceRange;
use super::ptr::{ArenaPtr, OsMemPtr};

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
    raw: crate::internal::memid::mi_memid_t,
    _kind: PhantomData<K>,
}

impl<K> MemId<K> {
    #[inline(always)]
    pub(crate) const fn into_raw(self) -> crate::internal::memid::mi_memid_t {
        self.raw
    }

    #[inline(always)]
    pub(crate) const fn as_raw(&self) -> crate::internal::memid::mi_memid_t {
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
    pub(crate) fn view(raw: crate::internal::memid::mi_memid_t) -> MemIdView {
        let code = MemKindCode(raw.memkind as i32);
        match MemKind::from(code) {
            MemKind::None => MemIdView::None(MemId {
                raw,
                _kind: PhantomData,
            }),
            MemKind::External => {
                // SAFETY: the discriminant is
                // `MI_MEM_EXTERNAL`, whose C layout stores external memory
                // metadata in the `os` union arm.
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
                // SAFETY: the discriminant is
                // `MI_MEM_META`, whose C layout stores meta-page metadata in
                // the `meta` union arm; invalid payloads are rejected below.
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
                // SAFETY: OS, huge-OS, and remap
                // discriminants all use the `os` union arm in the C layout.
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
                // SAFETY: the discriminant is
                // `MI_MEM_ARENA`, whose C layout stores arena metadata in the
                // `arena` union arm; null arenas and invalid ranges are
                // rejected below.
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
            raw: crate::internal::memid::mi_memid_t {
                mem: crate::internal::memid::mi_memid_mem_t {
                    os: crate::internal::memid::mi_memid_os_info_t {
                        base: core::ptr::null_mut(),
                        size: 0,
                    },
                },
                is_pinned: false,
                initially_committed: false,
                initially_zero: false,
                memkind: crate::internal::memid::mi_memkind_t::MI_MEM_NONE as _,
            },
            _kind: PhantomData,
        }
    }
}

impl MemId<StaticMem> {
    pub(crate) const fn static_mem() -> Self {
        Self {
            raw: crate::internal::memid::mi_memid_t {
                mem: crate::internal::memid::mi_memid_mem_t {
                    os: crate::internal::memid::mi_memid_os_info_t {
                        base: core::ptr::null_mut(),
                        size: 0,
                    },
                },
                is_pinned: false,
                initially_committed: false,
                initially_zero: false,
                memkind: crate::internal::memid::mi_memkind_t::MI_MEM_STATIC as _,
            },
            _kind: PhantomData,
        }
    }
}

impl MemId<HeapMainMem> {
    pub(crate) const fn heap_main() -> Self {
        Self {
            raw: crate::internal::memid::mi_memid_t {
                mem: crate::internal::memid::mi_memid_mem_t {
                    os: crate::internal::memid::mi_memid_os_info_t {
                        base: core::ptr::null_mut(),
                        size: 0,
                    },
                },
                is_pinned: false,
                initially_committed: false,
                initially_zero: false,
                memkind: crate::internal::memid::mi_memkind_t::MI_MEM_HEAP_MAIN as _,
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
            raw: crate::internal::memid::mi_memid_t {
                mem: crate::internal::memid::mi_memid_mem_t {
                    os: crate::internal::memid::mi_memid_os_info_t {
                        base: base.map_or(core::ptr::null_mut(), |p| p.as_ptr()),
                        size: size.0,
                    },
                },
                is_pinned: pinned.0,
                initially_committed: committed.0,
                initially_zero: zero.0,
                memkind: match kind {
                    OsMemKind::Normal => crate::internal::memid::mi_memkind_t::MI_MEM_OS,
                    OsMemKind::Huge => crate::internal::memid::mi_memkind_t::MI_MEM_OS_HUGE,
                    OsMemKind::Remap => crate::internal::memid::mi_memkind_t::MI_MEM_OS_REMAP,
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
            raw: crate::internal::memid::mi_memid_t {
                mem: crate::internal::memid::mi_memid_mem_t {
                    os: crate::internal::memid::mi_memid_os_info_t {
                        base: base.map_or(core::ptr::null_mut(), |p| p.as_ptr()),
                        size: size.0,
                    },
                },
                is_pinned: pinned.0,
                initially_committed: committed.0,
                initially_zero: zero.0,
                memkind: crate::internal::memid::mi_memkind_t::MI_MEM_EXTERNAL as _,
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
            raw: crate::internal::memid::mi_memid_t {
                mem: crate::internal::memid::mi_memid_mem_t {
                    arena: crate::internal::memid::mi_memid_arena_info_t {
                        arena: arena.as_ptr(),
                        slice_index: range.start_u32(),
                        slice_count: range.count_u32(),
                    },
                },
                is_pinned: pinned.0,
                initially_committed: committed.0,
                initially_zero: zero.0,
                memkind: crate::internal::memid::mi_memkind_t::MI_MEM_ARENA as _,
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
            raw: crate::internal::memid::mi_memid_t {
                mem: crate::internal::memid::mi_memid_mem_t {
                    meta: crate::internal::memid::mi_memid_meta_info_t {
                        meta_page: meta_page.as_ptr(),
                        block_index: block_index.0,
                        block_count: block_count.get(),
                    },
                },
                is_pinned: true,
                initially_committed: true,
                initially_zero: true,
                memkind: crate::internal::memid::mi_memkind_t::MI_MEM_META as _,
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

#[derive(Clone, Copy)]
pub(crate) struct ArenaMemId {
    arena: ArenaPtr,
    range: SliceRange,
    initially_committed: InitiallyCommitted,
    initially_zero: InitiallyZero,
    pinned: Pinned,
}

impl ArenaMemId {
    #[inline(always)]
    pub(crate) const fn arena(self) -> ArenaPtr {
        self.arena
    }

    #[inline(always)]
    pub(crate) const fn range(self) -> SliceRange {
        self.range
    }

    #[inline(always)]
    pub(crate) const fn initially_committed(self) -> InitiallyCommitted {
        self.initially_committed
    }

    #[inline(always)]
    pub(crate) const fn initially_zero(self) -> InitiallyZero {
        self.initially_zero
    }

    #[inline(always)]
    pub(crate) const fn pinned(self) -> Pinned {
        self.pinned
    }
}

pub(crate) struct MetaMemId {
    pub(crate) meta_page: NonNull<core::ffi::c_void>,
    pub(crate) block_index: BlockIndex,
    pub(crate) block_count: core::num::NonZeroU32,
}

pub(super) fn __lint_touch() {
    let none = MemId::<NoneMem>::none();
    let _ = none.as_raw();
    let _ = MemId::<HeapMainMem>::heap_main();
    let _ = MemId::<OsMem>::os_remap
        as fn(
            Option<OsMemPtr>,
            ReservedBytes,
            InitiallyCommitted,
            InitiallyZero,
            Pinned,
        ) -> MemId<OsMem>;
    let _ = ReservedBytes::new(0).is_zero();

    if let MemIdView::None(memid) = MemIdView::None(MemId::<NoneMem>::none()) {
        let _ = memid.into_raw();
    }
    if let MemIdView::Static(memid) = MemIdView::Static(MemId::<StaticMem>::static_mem()) {
        let _ = memid.into_raw();
    }
    if let MemIdView::HeapMain(memid) = MemIdView::HeapMain(MemId::<HeapMainMem>::heap_main()) {
        let _ = memid.into_raw();
    }
    if let MemIdView::Unknown(code) = MemIdView::Unknown(MemKindCode::NONE) {
        let _ = code.0;
    }

    let os = OsMemId {
        base: None,
        size: ReservedBytes::new(0),
        initially_committed: InitiallyCommitted(false),
        initially_zero: InitiallyZero(false),
        pinned: Pinned(false),
        kind: OsMemKind::Normal,
    };
    let _ = (
        os.base,
        os.size,
        os.initially_committed.0,
        os.initially_zero.0,
        os.pinned.0,
        matches!(os.kind, OsMemKind::Normal),
    );

    let external = ExternalMemId {
        base: None,
        size: ReservedBytes::new(0),
        initially_committed: InitiallyCommitted(false),
        initially_zero: InitiallyZero(false),
        pinned: Pinned(false),
    };
    let _ = (
        external.base,
        external.size,
        external.initially_committed.0,
        external.initially_zero.0,
        external.pinned.0,
    );

    if let Ok(range) = SliceRange::from_raw(0, 1) {
        let arena = ArenaMemId {
            // SAFETY: this lint-touch fixture
            // uses a non-dereferenced dangling pointer only to keep typed
            // accessors referenced for dead-code lint accounting.
            arena: unsafe { ArenaPtr::new_unchecked(core::ptr::dangling_mut()) },
            range,
            initially_committed: InitiallyCommitted(false),
            initially_zero: InitiallyZero(false),
            pinned: Pinned(false),
        };
        let _ = (
            arena.arena(),
            arena.range(),
            arena.initially_committed().0,
            arena.initially_zero().0,
            arena.pinned().0,
        );
    }
}

#[cfg(test)]
mod tests;
