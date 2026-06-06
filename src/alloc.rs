//! Core allocation / free paths + libc overrides

use crate::page::mi_good_size;

pub extern "C" fn mi_malloc_good_size(size: usize) -> usize {
    mi_good_size(size)
}
