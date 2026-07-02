use super::*;

#[test]
fn generic_malloc_refill_helper_is_force_inlined() {
    let page_rs =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/allocator/page.rs"))
            .expect("read src/allocator/page.rs");

    assert!(
        page_rs.contains("#[inline(always)]\nunsafe fn mi_find_page("),
        "mi_find_page should stay force-inlined into _mi_malloc_generic like C's generic path"
    );
}

#[test]
fn release_page_stat_writes_follow_c_mimalloc_gates() {
    let page_rs =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/allocator/page.rs"))
            .expect("read src/allocator/page.rs");

    for (stat, needle) in [
        (
            "pages_retire",
            "if MI_STAT > 0 {\n                __mi_stat_counter_increase(ptr::addr_of_mut!((*theap).stats.pages_retire), 1);",
        ),
        (
            "pages_extended",
            "if MI_STAT > 0 {\n            __mi_stat_counter_increase(ptr::addr_of_mut!((*theap).stats.pages_extended), 1);",
        ),
        (
            "page_committed",
            "if MI_STAT > 0 {\n            __mi_stat_increase(\n                ptr::addr_of_mut!((*theap).stats.page_committed),",
        ),
    ] {
        assert!(
            page_rs.contains(needle),
            "{stat} updates should stay behind MI_STAT > 0 like C mimalloc"
        );
    }
}

#[test]
fn public_option_domain_is_transparent_not_rust_enum() {
    let options_rs =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/allocator/options.rs"))
            .expect("read src/allocator/options.rs");

    assert!(
        options_rs.contains("#[repr(transparent)]\n#[derive(Clone, Copy, Debug, PartialEq, Eq)]\npub struct mi_option_t(i32);")
            && options_rs.contains("pub const fn from_raw(raw: i32) -> Self")
            && options_rs.contains("pub const fn as_i32(self) -> i32")
            && options_rs.contains("pub type mi_option_e = mi_option_t;")
            && !options_rs.contains("pub enum mi_option_e")
            && !options_rs.contains("transmute::<i32, mi_option_t>"),
        "public mi_option_t must allow unknown C integer values without Rust enum UB"
    );
}
