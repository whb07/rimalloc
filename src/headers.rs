//! Mechanical translation of header-level override macros.

#![allow(dead_code)]

#[macro_export]
macro_rules! malloc {
    ($n:expr) => {
        $crate::alloc::mi_malloc($n)
    };
}

#[macro_export]
macro_rules! realloc {
    ($p:expr, $n:expr) => {
        $crate::mimalloc::mi_realloc($p, $n)
    };
}

#[macro_export]
macro_rules! strdup {
    ($s:expr) => {
        $crate::alloc::mi_strdup($s)
    };
}

#[macro_export]
macro_rules! _strdup {
    ($s:expr) => {
        $crate::alloc::mi_strdup($s)
    };
}

#[macro_export]
macro_rules! realpath {
    ($f:expr, $n:expr) => {
        $crate::alloc::mi_theap_realpath($crate::prim::_mi_theap_default(), $f, $n)
    };
}

#[macro_export]
macro_rules! reallocarray {
    ($p:expr, $c:expr, $n:expr) => {
        $crate::alloc_posix::mi_reallocarray($p, $c, $n)
    };
}

#[macro_export]
macro_rules! posix_memalign {
    ($p:expr, $a:expr, $n:expr) => {
        $crate::alloc_posix::mi_posix_memalign($p, $a, $n)
    };
}

#[macro_export]
macro_rules! _expand {
    ($p:expr, $n:expr) => {
        $crate::alloc_posix::mi__expand($p, $n)
    };
}

#[macro_export]
macro_rules! _recalloc {
    ($p:expr, $c:expr, $n:expr) => {
        $crate::mimalloc::mi_recalloc($p, $c, $n)
    };
}

#[macro_export]
macro_rules! _aligned_recalloc {
    ($p:expr, $c:expr, $n:expr, $a:expr) => {
        $crate::alloc_aligned::mi_aligned_recalloc($p, $c, $n, $a)
    };
}

#[macro_export]
macro_rules! _wcsdup {
    ($s:expr) => {
        $crate::alloc_posix::mi_wcsdup($s)
    };
}

#[macro_export]
macro_rules! _dupenv_s {
    ($buf:expr, $n:expr, $nm:expr) => {
        $crate::alloc_posix::mi_dupenv_s($buf, $n, $nm)
    };
}

#[macro_export]
macro_rules! reallocf {
    ($p:expr, $n:expr) => {
        $crate::alloc::mi_reallocf($p, $n)
    };
}

#[macro_export]
macro_rules! malloc_usable_size {
    ($p:expr) => {
        $crate::free::mi_usable_size($p)
    };
}

#[macro_export]
macro_rules! cfree {
    ($p:expr) => {
        $crate::alloc_posix::mi_cfree($p)
    };
}

#[macro_export]
macro_rules! valloc {
    ($n:expr) => {
        $crate::alloc_posix::mi_valloc($n)
    };
}

#[macro_export]
macro_rules! memalign {
    ($a:expr, $n:expr) => {
        $crate::alloc_posix::mi_memalign($a, $n)
    };
}

#[macro_export]
macro_rules! _aligned_malloc {
    ($n:expr, $a:expr) => {
        $crate::mimalloc::mi_malloc_aligned($n, $a)
    };
}

#[macro_export]
macro_rules! _aligned_free {
    ($p:expr) => {
        $crate::mimalloc::mi_free($p)
    };
}

#[macro_export]
macro_rules! _aligned_offset_realloc {
    ($p:expr, $n:expr, $a:expr, $o:expr) => {
        $crate::alloc_aligned::mi_realloc_aligned_at($p, $n, $a, $o)
    };
}
