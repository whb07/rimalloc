pub(super) fn assert_scalar_domain_wrappers(
    ptr_rs: &str,
    scalar_rs: &str,
    unsafe_migration_allowlist: &str,
) {
    assert!(
        scalar_rs.contains("pub(crate) struct SubprocSeq(pub(crate) usize);")
            && scalar_rs.contains("pub(crate) struct ArenaCount(pub(crate) usize);")
            && scalar_rs.contains("pub(crate) struct ThreadCount(pub(crate) usize);")
            && scalar_rs.contains("pub(crate) struct HeapCount(pub(crate) usize);")
            && ptr_rs.contains(
                "pub(crate) unsafe fn arena_count(self, order: MemoryOrder) -> ArenaCount"
            )
            && ptr_rs.contains(
                "pub(crate) unsafe fn set_arena_count(self, count: ArenaCount, order: MemoryOrder)"
            )
            && ptr_rs.contains(
                "pub(crate) unsafe fn thread_count(self, order: MemoryOrder) -> ThreadCount"
            )
            && ptr_rs.contains(
                "pub(crate) unsafe fn thread_total_count(self, order: MemoryOrder) -> ThreadCount"
            )
            && ptr_rs
                .contains("pub(crate) unsafe fn heap_count(self, order: MemoryOrder) -> HeapCount")
            && ptr_rs.contains(
                "pub(crate) unsafe fn heap_total_count(self, order: MemoryOrder) -> HeapCount"
            ),
        "subproc sequence and count fields should have scalar wrappers plus typed SubprocPtr accessors"
    );
    assert!(
        !unsafe_migration_allowlist.contains("src/types/scalar.rs")
            && scalar_rs
                .matches("SAFETY[AUDIT:scalar-size-domain-wrappers]")
                .count()
                >= 6
            && scalar_rs
                .matches("SAFETY[AUDIT:scalar-alignment-domain-wrappers]")
                .count()
                >= 3
            && scalar_rs
                .matches("SAFETY[AUDIT:scalar-page-count-domain-wrappers]")
                .count()
                >= 2
            && scalar_rs
                .matches("SAFETY[AUDIT:scalar-index-domain-wrappers]")
                .count()
                >= 8
            && scalar_rs
                .matches("SAFETY[AUDIT:scalar-lint-fixture-wrappers]")
                .count()
                >= 4
            && scalar_rs
                .contains("Scalar domain wrappers for allocator sizes, counts, indexes, and bins.")
            && scalar_rs.contains("pub(crate) struct UsableOut(*mut usize);")
            && scalar_rs.contains("pub(crate) unsafe fn write_if_present")
            && scalar_rs.contains("pub(crate) const unsafe fn new_unchecked(size: usize) -> Self")
            && scalar_rs.contains("pub(crate) const unsafe fn new_unchecked(index: usize) -> Self")
            && scalar_rs.contains("pub(crate) const unsafe fn new_unchecked(count: u32) -> Self")
            && scalar_rs.contains("pub(crate) const unsafe fn new_unchecked(idx: u16) -> Self"),
        "scalar domain wrappers should stay locally audited and off the unsafe migration allowlist"
    );
}
