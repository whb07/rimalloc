use core::ffi::c_void;
use core::ptr;

fn main() {
    unsafe {
        let ptr = rimalloc::raw::mi_malloc(64).cast::<u8>();
        assert!(!ptr.is_null());

        for index in 0..64 {
            ptr.add(index).write(index as u8);
        }

        assert!(rimalloc::raw::mi_usable_size(ptr.cast::<c_void>()) >= 64);
        assert_eq!(ptr.add(63).read(), 63);

        rimalloc::raw::mi_free(ptr.cast::<c_void>());
        rimalloc::raw::mi_free(ptr::null_mut());
    }
}
