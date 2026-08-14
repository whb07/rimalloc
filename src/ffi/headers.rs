//! Mechanical translation of header-level override macros.

#[cfg(feature = "c-header-macros")]
#[macro_export]
macro_rules! malloc {
    ($n:expr) => {
        $crate::raw::mi_malloc($n)
    };
}

#[cfg(feature = "c-header-macros")]
#[macro_export]
macro_rules! realloc {
    ($p:expr, $n:expr) => {
        $crate::raw::mi_realloc($p, $n)
    };
}

#[cfg(feature = "c-header-macros")]
#[macro_export]
macro_rules! strdup {
    ($s:expr) => {
        $crate::raw::mi_strdup($s)
    };
}

#[cfg(feature = "c-header-macros")]
#[macro_export]
macro_rules! _strdup {
    ($s:expr) => {
        $crate::raw::mi_strdup($s)
    };
}

#[cfg(feature = "c-header-macros")]
#[macro_export]
macro_rules! realpath {
    ($f:expr, $n:expr) => {
        $crate::raw::mi_realpath($f, $n)
    };
}

#[cfg(feature = "c-header-macros")]
#[macro_export]
macro_rules! reallocarray {
    ($p:expr, $c:expr, $n:expr) => {
        $crate::raw::mi_reallocarray($p, $c, $n)
    };
}

#[cfg(feature = "c-header-macros")]
#[macro_export]
macro_rules! posix_memalign {
    ($p:expr, $a:expr, $n:expr) => {
        $crate::raw::mi_posix_memalign($p, $a, $n)
    };
}

#[cfg(feature = "c-header-macros")]
#[macro_export]
macro_rules! _expand {
    ($p:expr, $n:expr) => {
        $crate::raw::mi__expand($p, $n)
    };
}

#[cfg(feature = "c-header-macros")]
#[macro_export]
macro_rules! _recalloc {
    ($p:expr, $c:expr, $n:expr) => {
        $crate::raw::mi_recalloc($p, $c, $n)
    };
}

#[cfg(feature = "c-header-macros")]
#[macro_export]
macro_rules! _aligned_recalloc {
    ($p:expr, $c:expr, $n:expr, $a:expr) => {
        $crate::raw::mi_aligned_recalloc($p, $c, $n, $a)
    };
}

#[cfg(feature = "c-header-macros")]
#[macro_export]
macro_rules! _wcsdup {
    ($s:expr) => {
        $crate::raw::mi_wcsdup($s)
    };
}

#[cfg(feature = "c-header-macros")]
#[macro_export]
macro_rules! _dupenv_s {
    ($buf:expr, $n:expr, $nm:expr) => {
        $crate::raw::mi_dupenv_s($buf, $n, $nm)
    };
}

#[cfg(feature = "c-header-macros")]
#[macro_export]
macro_rules! reallocf {
    ($p:expr, $n:expr) => {
        $crate::raw::mi_reallocf($p, $n)
    };
}

#[cfg(feature = "c-header-macros")]
#[macro_export]
macro_rules! malloc_usable_size {
    ($p:expr) => {
        $crate::raw::mi_malloc_usable_size($p)
    };
}

#[cfg(feature = "c-header-macros")]
#[macro_export]
macro_rules! cfree {
    ($p:expr) => {
        $crate::raw::mi_cfree($p)
    };
}

#[cfg(feature = "c-header-macros")]
#[macro_export]
macro_rules! valloc {
    ($n:expr) => {
        $crate::raw::mi_valloc($n)
    };
}

#[cfg(feature = "c-header-macros")]
#[macro_export]
macro_rules! memalign {
    ($a:expr, $n:expr) => {
        $crate::raw::mi_memalign($a, $n)
    };
}

#[cfg(feature = "c-header-macros")]
#[macro_export]
macro_rules! _aligned_malloc {
    ($n:expr, $a:expr) => {
        $crate::raw::mi_malloc_aligned($n, $a)
    };
}

#[cfg(feature = "c-header-macros")]
#[macro_export]
macro_rules! _aligned_free {
    ($p:expr) => {
        $crate::raw::mi_free($p)
    };
}

#[cfg(feature = "c-header-macros")]
#[macro_export]
macro_rules! _aligned_offset_realloc {
    ($p:expr, $n:expr, $a:expr, $o:expr) => {
        $crate::raw::mi_realloc_aligned_at($p, $n, $a, $o)
    };
}

#[allow(non_upper_case_globals)]
pub const mi_decl_new: () = ();

#[allow(non_upper_case_globals)]
pub const mi_decl_new_nothrow: () = ();

#[cfg(feature = "c-header-macros")]
#[macro_export]
macro_rules! operatordelete {
    ($p:expr) => {
        $crate::raw::mi_free($p)
    };
}

#[cfg(feature = "c-header-macros")]
#[macro_export]
macro_rules! operatordelete_array {
    ($p:expr) => {
        $crate::raw::mi_free($p)
    };
}

#[cfg(feature = "c-header-macros")]
#[macro_export]
macro_rules! operatordelete_nothrow {
    ($p:expr, $tag:expr) => {{
        let _ = &$tag;
        $crate::raw::mi_free($p)
    }};
}

#[cfg(feature = "c-header-macros")]
#[macro_export]
macro_rules! operatordelete_array_nothrow {
    ($p:expr, $tag:expr) => {{
        let _ = &$tag;
        $crate::raw::mi_free($p)
    }};
}

#[cfg(feature = "c-header-macros")]
#[macro_export]
macro_rules! operatornew {
    ($n:expr) => {
        $crate::raw::mi_new($n)
    };
}

#[cfg(feature = "c-header-macros")]
#[macro_export]
macro_rules! operatornew_array {
    ($n:expr) => {
        $crate::raw::mi_new($n)
    };
}

#[cfg(feature = "c-header-macros")]
#[macro_export]
macro_rules! operatornew_nothrow {
    ($n:expr, $tag:expr) => {{
        let _ = &$tag;
        $crate::raw::mi_new_nothrow($n)
    }};
}

#[cfg(feature = "c-header-macros")]
#[macro_export]
macro_rules! operatornew_array_nothrow {
    ($n:expr, $tag:expr) => {{
        let _ = &$tag;
        $crate::raw::mi_new_nothrow($n)
    }};
}

#[cfg(feature = "c-header-macros")]
#[macro_export]
macro_rules! operatordelete_sized {
    ($p:expr, $n:expr) => {
        $crate::raw::mi_free_size($p, $n)
    };
}

#[cfg(feature = "c-header-macros")]
#[macro_export]
macro_rules! operatordelete_array_sized {
    ($p:expr, $n:expr) => {
        $crate::raw::mi_free_size($p, $n)
    };
}

#[cfg(feature = "c-header-macros")]
#[macro_export]
macro_rules! operatordelete_aligned {
    ($p:expr, $al:expr) => {
        $crate::raw::mi_free_aligned($p, $al as usize)
    };
}

#[cfg(feature = "c-header-macros")]
#[macro_export]
macro_rules! operatordelete_array_aligned {
    ($p:expr, $al:expr) => {
        $crate::raw::mi_free_aligned($p, $al as usize)
    };
}

#[cfg(feature = "c-header-macros")]
#[macro_export]
macro_rules! operatordelete_sized_aligned {
    ($p:expr, $n:expr, $al:expr) => {
        $crate::raw::mi_free_size_aligned($p, $n, $al as usize)
    };
}

#[cfg(feature = "c-header-macros")]
#[macro_export]
macro_rules! operatordelete_array_sized_aligned {
    ($p:expr, $n:expr, $al:expr) => {
        $crate::raw::mi_free_size_aligned($p, $n, $al as usize)
    };
}

#[cfg(feature = "c-header-macros")]
#[macro_export]
macro_rules! operatordelete_aligned_nothrow {
    ($p:expr, $al:expr, $tag:expr) => {{
        let _ = &$tag;
        $crate::raw::mi_free_aligned($p, $al as usize)
    }};
}

#[cfg(feature = "c-header-macros")]
#[macro_export]
macro_rules! operatordelete_array_aligned_nothrow {
    ($p:expr, $al:expr, $tag:expr) => {{
        let _ = &$tag;
        $crate::raw::mi_free_aligned($p, $al as usize)
    }};
}

#[cfg(feature = "c-header-macros")]
#[macro_export]
macro_rules! operatornew_aligned {
    ($n:expr, $al:expr) => {
        $crate::raw::mi_new_aligned($n, $al as usize)
    };
}

#[cfg(feature = "c-header-macros")]
#[macro_export]
macro_rules! operatornew_array_aligned {
    ($n:expr, $al:expr) => {
        $crate::raw::mi_new_aligned($n, $al as usize)
    };
}

#[cfg(feature = "c-header-macros")]
#[macro_export]
macro_rules! operatornew_aligned_nothrow {
    ($n:expr, $al:expr, $tag:expr) => {{
        let _ = &$tag;
        $crate::raw::mi_new_aligned_nothrow($n, $al as usize)
    }};
}

#[cfg(feature = "c-header-macros")]
#[macro_export]
macro_rules! operatornew_array_aligned_nothrow {
    ($n:expr, $al:expr, $tag:expr) => {{
        let _ = &$tag;
        $crate::raw::mi_new_aligned_nothrow($n, $al as usize)
    }};
}

#[allow(non_upper_case_globals)]
pub const mi_stl_allocator: () = ();

#[allow(non_upper_case_globals)]
pub const mi_stl_allocator_operator_eq: () = ();

#[allow(non_upper_case_globals)]
pub const mi_stl_allocator_operator_ne: () = ();

#[allow(non_upper_case_globals)]
pub const _mi_heap_stl_allocator_common: () = ();

#[allow(non_upper_case_globals)]
pub const mi_heap_stl_allocator: () = ();

#[allow(non_upper_case_globals)]
pub const mi_heap_stl_allocator_operator_eq: () = ();

#[allow(non_upper_case_globals)]
pub const mi_heap_stl_allocator_operator_eq_decl: () = ();

#[allow(non_upper_case_globals)]
pub const mi_heap_stl_allocator_operator_ne: () = ();

#[allow(non_upper_case_globals)]
pub const mi_heap_destroy_stl_allocator: () = ();

#[allow(non_upper_case_globals)]
pub const mi_heap_destroy_stl_allocator_operator_eq: () = ();

#[allow(non_upper_case_globals)]
pub const mi_heap_destroy_stl_allocator_operator_eq_decl: () = ();

#[allow(non_upper_case_globals)]
pub const mi_heap_destroy_stl_allocator_operator_ne: () = ();
