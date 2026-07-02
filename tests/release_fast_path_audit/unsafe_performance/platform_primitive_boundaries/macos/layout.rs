use super::*;

pub(super) fn assert_macos_zone_layout(macos_rs: &str) {
    assert!(
        macos_rs.contains("pub static mut mi_malloc_zone: malloc_zone_t = malloc_zone_t {")
            && macos_rs.contains("zone_name: MI_MALLOC_ZONE_NAME.as_ptr().cast(),")
            && macos_rs.contains("pub static _mi_zone_interposes: [mi_interpose_s; 27]")
            && macos_rs.contains("#[unsafe(link_section = \"__DATA,__interpose\")]")
            && macos_rs.contains("#[unsafe(link_section = \"__DATA,__mod_init_func\")]"),
        "macOS malloc-zone source should preserve zone metadata and section-based interposition shape"
    );
    for import in [
        "fn malloc_default_zone() -> *mut malloc_zone_t;",
        "fn malloc_default_purgeable_zone() -> *mut malloc_zone_t;",
        "fn malloc_get_all_zones(",
        "fn malloc_zone_register(zone: *mut malloc_zone_t);",
        "fn malloc_zone_unregister(zone: *mut malloc_zone_t);",
        "fn malloc_zone_batch_free(zone: *mut malloc_zone_t, ps: *mut *mut c_void, count: u32);",
        "fn malloc_zone_batch_malloc(",
    ] {
        assert!(
            macos_rs.contains(import),
            "macOS malloc-zone import missing: {import}"
        );
    }

    let zone_struct = source_item(
        "src/platform/macos.rs",
        macos_rs,
        "pub struct malloc_zone_t",
    );
    assert!(
        zone_struct.contains("pub size: Option<unsafe extern \"C\" fn")
            && zone_struct.contains("pub malloc: Option<unsafe extern \"C\" fn")
            && zone_struct.contains("pub calloc: Option<unsafe extern \"C\" fn")
            && zone_struct.contains("pub valloc: Option<unsafe extern \"C\" fn")
            && zone_struct.contains("pub free: Option<unsafe extern \"C\" fn")
            && zone_struct.contains("pub realloc:")
            && zone_struct.contains("pub destroy: Option<unsafe extern \"C\" fn")
            && zone_struct.contains("pub zone_name: *const c_char")
            && zone_struct.contains("pub batch_malloc:")
            && zone_struct.contains("pub batch_free: Option<unsafe extern \"C\" fn"),
        "malloc_zone_t should keep the Darwin callback layout and raw C ABI slots"
    );
}
