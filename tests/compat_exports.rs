use core::ffi::{c_uchar, c_void};
use core::ptr;
use std::ffi::CString;
use std::os::raw::c_long;

use mechanical_mimalloc::raw::mimalloc::*;

struct ShowErrorsGuard(c_long);

impl ShowErrorsGuard {
    unsafe fn disabled() -> Self {
        let option = mi_option_t::mi_option_show_errors;
        let previous = unsafe { mi_option_get(option) };
        unsafe { mi_option_set(option, 0) };
        Self(previous)
    }
}

impl Drop for ShowErrorsGuard {
    fn drop(&mut self) {
        unsafe { mi_option_set(mi_option_t::mi_option_show_errors, self.0) };
    }
}

fn assert_aligned_at(p: *mut c_void, alignment: usize, offset: usize) {
    assert!(!p.is_null());
    assert_eq!((p as usize + offset) % alignment, 0);
}

unsafe fn assert_zeroed(p: *mut c_void, start: usize, end: usize) {
    let bytes = unsafe { core::slice::from_raw_parts(p.cast::<u8>().add(start), end - start) };
    assert!(bytes.iter().all(|byte| *byte == 0));
}

#[test]
fn mbsdup_export_matches_strdup_byte_copy_behavior() {
    unsafe {
        assert!(mi_mbsdup(ptr::null()).is_null());

        let empty = c"";
        let p = mi_mbsdup(empty.as_ptr().cast::<c_uchar>());
        assert!(!p.is_null());
        assert_eq!(*p, 0);
        mi_free(p.cast());

        let source = b"multi-byte-\xc3\xa9\x00";
        let p = mi_mbsdup(source.as_ptr());
        assert!(!p.is_null());
        let copied = core::slice::from_raw_parts(p, source.len());
        assert_eq!(copied, source);
        mi_free(p.cast());
    }
}

#[test]
fn aligned_recalloc_compat_exports_preserve_and_zero_extend() {
    unsafe {
        let p = mi_aligned_recalloc(ptr::null_mut(), 4, 8, 64);
        assert_aligned_at(p, 64, 0);
        assert_zeroed(p, 0, 32);
        for i in 0..32 {
            *p.cast::<u8>().add(i) = i as u8;
        }

        let q = mi_aligned_recalloc(p, 8, 8, 64);
        assert_aligned_at(q, 64, 0);
        let bytes = core::slice::from_raw_parts(q.cast::<u8>(), 64);
        assert_eq!(&bytes[..32], &(0_u8..32).collect::<Vec<_>>()[..]);
        assert!(bytes[32..].iter().all(|byte| *byte == 0));
        mi_free(q);

        let p = mi_aligned_offset_recalloc(ptr::null_mut(), 4, 8, 64, 16);
        assert_aligned_at(p, 64, 16);
        assert_zeroed(p, 0, 32);
        mi_free(p);

        let _quiet_expected_errors = ShowErrorsGuard::disabled();
        assert!(mi_aligned_recalloc(ptr::null_mut(), usize::MAX, 2, 64).is_null());
        assert!(mi_aligned_offset_recalloc(ptr::null_mut(), usize::MAX, 2, 64, 16).is_null());
    }
}

#[test]
fn posix_exports_preserve_error_and_out_pointer_contracts() {
    unsafe {
        let mut p = ptr::null_mut();
        assert_eq!(
            mi_posix_memalign(ptr::null_mut::<*mut c_void>(), 16, 16),
            libc::EINVAL
        );
        assert_eq!(mi_posix_memalign(&mut p, 3, 16), libc::EINVAL);
        assert_eq!(mi_posix_memalign(&mut p, 16, 64), 0);
        assert_aligned_at(p, 16, 0);
        mi_free(p);

        let _quiet_expected_errors = ShowErrorsGuard::disabled();
        assert!(mi_reallocarray(ptr::null_mut(), usize::MAX, 2).is_null());

        let mut env_buf = ptr::null_mut();
        let mut env_size = usize::MAX;
        assert_eq!(
            mi_dupenv_s(&mut env_buf, &mut env_size, ptr::null()),
            libc::EINVAL
        );
        assert!(env_buf.is_null());
        assert_eq!(env_size, 0);
    }
}

#[test]
fn new_and_heap_alloc_new_n_exports_allocate_from_expected_heap() {
    unsafe {
        let p = mi_new(32);
        assert!(!p.is_null());
        assert!(mi_usable_size(p) >= 32);
        mi_free(p);

        let heap = mi_heap_new();
        assert!(!heap.is_null());
        let p = mi_heap_alloc_new_n(heap, 4, 16);
        assert!(!p.is_null());
        assert!(mi_usable_size(p) >= 64);
        assert!(mi_heap_contains(heap, p));
        mi_free(p);
        mi_heap_delete(heap);
    }
}

#[test]
fn reallocarr_export_updates_pointer_slot_contract() {
    unsafe {
        let mut p = mi_malloc(16);
        assert!(!p.is_null());
        p.cast::<u8>().write(0xA5);

        assert_eq!(mi_reallocarr(&mut p, 2, 16), 0);
        assert!(!p.is_null());
        assert!(mi_usable_size(p) >= 32);
        assert_eq!(p.cast::<u8>().read(), 0xA5);

        assert_eq!(mi_reallocarr(&mut p, 0, 16), 0);
        assert!(p.is_null());
    }
}

#[cfg(all(unix, not(target_os = "wasi")))]
#[test]
fn realpath_export_matches_resolved_buffer_and_allocated_result_contracts() {
    unsafe {
        let cwd = std::env::current_dir().expect("current dir");
        let cwd = CString::new(cwd.as_os_str().as_encoded_bytes()).expect("cwd has no nul");

        let mut resolved = vec![0_i8; 4096];
        let p = mi_realpath(cwd.as_ptr(), resolved.as_mut_ptr());
        assert_eq!(p, resolved.as_mut_ptr());

        let allocated = mi_realpath(cwd.as_ptr(), ptr::null_mut());
        assert!(!allocated.is_null());
        assert!(mi_check_owned(allocated.cast()));
        mi_free(allocated.cast());

        let missing = c"/definitely/not/a/mechanical_mimalloc/path";
        assert!(mi_realpath(missing.as_ptr(), ptr::null_mut()).is_null());
    }
}
