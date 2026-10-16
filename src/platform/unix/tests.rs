use super::*;

#[test]
fn darwin_constants_and_timeval_match_libc() {
    assert_eq!(_SC_PAGESIZE, libc::_SC_PAGESIZE);
    assert_eq!(_SC_PHYS_PAGES, libc::_SC_PHYS_PAGES);
    assert_eq!(EAGAIN, libc::EAGAIN);
    assert_eq!(ENOSYS, libc::ENOSYS);
    assert_eq!(crate::types::EAGAIN, libc::EAGAIN);
    assert_eq!(crate::types::EOVERFLOW, libc::EOVERFLOW);
    assert_eq!(size_of::<mi_timeval>(), size_of::<libc::timeval>());
    assert_eq!(align_of::<mi_timeval>(), align_of::<libc::timeval>());
    assert_eq!(
        size_of_val(&mi_timeval::default().tv_usec),
        size_of::<libc::suseconds_t>()
    );
    assert_eq!(size_of::<rusage>(), size_of::<libc::rusage>());
    assert_eq!(
        core::mem::offset_of!(rusage, ru_maxrss),
        core::mem::offset_of!(libc::rusage, ru_maxrss)
    );
}

#[test]
fn darwin_memory_configuration_matches_host() {
    let mut config = mi_os_mem_config_t::default();
    // SAFETY: config is writable; sysconf takes scalar query identifiers.
    unsafe {
        _mi_prim_mem_init(&mut config);
        let page_size = libc::sysconf(libc::_SC_PAGESIZE) as usize;
        let pages = libc::sysconf(libc::_SC_PHYS_PAGES) as usize;
        assert_eq!(config.page_size, page_size);
        assert_eq!(config.alloc_granularity, page_size);
        assert_eq!(config.physical_memory_in_kib, pages * (page_size / 1024));
    }
}

#[test]
fn darwin_process_peak_rss_is_reported_in_bytes() {
    let mut before = core::mem::MaybeUninit::<libc::rusage>::uninit();
    let mut after = core::mem::MaybeUninit::<libc::rusage>::uninit();
    let mut info = mi_process_info_t::default();
    // SAFETY: all three output buffers are writable and have the required layouts.
    unsafe {
        assert_eq!(libc::getrusage(libc::RUSAGE_SELF, before.as_mut_ptr()), 0);
        _mi_prim_process_info(&mut info);
        assert_eq!(libc::getrusage(libc::RUSAGE_SELF, after.as_mut_ptr()), 0);
        assert!(info.peak_rss >= before.assume_init().ru_maxrss as usize);
        assert!(info.peak_rss <= after.assume_init().ru_maxrss as usize);
    }
}

unsafe extern "C" {
    static mach_task_self_: u32;
    fn mach_vm_page_query(
        task: u32,
        address: u64,
        disposition: *mut c_int,
        refs: *mut c_int,
    ) -> c_int;
}

fn page_is_reusable(p: *mut c_void) -> bool {
    let mut disposition = 0;
    let mut refs = 0;
    // SAFETY: query the current task with valid writable scalar output buffers.
    unsafe {
        assert_eq!(
            mach_vm_page_query(mach_task_self_, p as u64, &mut disposition, &mut refs),
            0
        );
    }
    disposition & 0x800 != 0 // VM_PAGE_QUERY_PAGE_REUSABLE from mach/vm_statistics.h
}

#[test]
fn darwin_decommit_marks_pages_reusable_and_reuse_clears_it() {
    // SAFETY: the test owns the complete mapping and releases it exactly once.
    unsafe {
        let size = libc::sysconf(libc::_SC_PAGESIZE) as usize;
        let mut p = ptr::null_mut();
        let mut large = false;
        let mut zero = false;
        assert_eq!(
            _mi_prim_alloc(
                ptr::null_mut(),
                size,
                size,
                true,
                false,
                &mut large,
                &mut zero,
                &mut p
            ),
            0
        );
        assert!(!p.is_null());
        p.cast::<u8>().write_bytes(0xa5, size);
        assert!(!page_is_reusable(p));
        let mut needs_recommit = false;
        assert_eq!(_mi_prim_decommit(p, size, &mut needs_recommit), 0);
        let reusable = page_is_reusable(p);
        if needs_recommit {
            assert_eq!(_mi_prim_commit(p, size, &mut zero), 0);
        }
        assert_eq!(_mi_prim_reuse(p, size), 0);
        let reused = !page_is_reusable(p);
        p.cast::<u8>().write_bytes(0x5a, size);
        assert_eq!(p.cast::<u8>().add(size - 1).read(), 0x5a);
        assert_eq!(_mi_prim_free(p, size), 0);
        assert!(reusable, "decommitted pages must be reclaimable by Darwin");
        assert!(reused, "reuse must restore normal page accounting");
    }
}
