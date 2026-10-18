use super::*;
use crate::internal::mi_theap_is_initialized;

type Entry = unsafe fn(usize) -> *mut c_void;

fn empty() -> *mut mi_theap_t {
    ptr::addr_of_mut!(_mi_theap_empty)
}

fn empty_wrong() -> *mut mi_theap_t {
    ptr::addr_of_mut!(_mi_theap_empty_wrong)
}

// Every small entry point on the inline fast path. On a fresh thread both the default heap and
// the explicit `_mi_theap_empty` are the uninitialized empty sentinel.
const SMALL_ENTRIES: [(&str, Entry, bool); 12] = [
    ("mi_malloc", |n| mi_malloc(n), false),
    ("mi_zalloc", |n| mi_zalloc(n), true),
    ("mi_calloc", |n| mi_calloc(1, n), true),
    ("mi_malloc_small", |n| unsafe { mi_malloc_small(n) }, false),
    ("mi_zalloc_small", |n| unsafe { mi_zalloc_small(n) }, true),
    (
        "mi_umalloc_small",
        |n| unsafe { mi_umalloc_small(n, ptr::null_mut()) },
        false,
    ),
    (
        "mi_uzalloc_small",
        |n| unsafe { mi_uzalloc_small(n, ptr::null_mut()) },
        true,
    ),
    (
        "mi_theap_malloc(default)",
        |n| unsafe { mi_theap_malloc(_mi_theap_default(), n) },
        false,
    ),
    (
        "mi_theap_malloc(empty)",
        |n| unsafe { mi_theap_malloc(empty(), n) },
        false,
    ),
    (
        "mi_theap_zalloc(empty)",
        |n| unsafe { mi_theap_zalloc(empty(), n) },
        true,
    ),
    (
        "mi_theap_malloc_small(empty)",
        |n| unsafe { mi_theap_malloc_small(empty(), n) },
        false,
    ),
    (
        "mi_theap_zalloc_small(empty)",
        |n| unsafe { mi_theap_zalloc_small(empty(), n) },
        true,
    ),
];

/// The fields the small path could plausibly write: any change means a sentinel was written.
fn sentinel_state(theap: *const mi_theap_t) -> [usize; 6] {
    // SAFETY: the sentinels are statics that live for the whole process.
    unsafe {
        [
            (*theap).heap as usize,
            (*theap).heartbeat as usize,
            (*theap).generic_count as usize,
            (*theap).page_count,
            (*theap).pages_free_direct[1] as usize,
            #[cfg(feature = "guarded")]
            (*theap).guarded_sample_count,
            #[cfg(not(feature = "guarded"))]
            0,
        ]
    }
}

/// One first-allocation run: the entry point, whether it zeroes, and the first failed check.
type FirstAlloc = (Entry, bool, Option<&'static str>);

#[cfg(unix)]
extern "C" fn first_alloc(arg: *mut c_void) -> *mut c_void {
    // SAFETY: `arg` is the parent's `FirstAlloc`, borrowed until the join.
    let (entry, zeroed, failure) = unsafe { &mut *arg.cast::<FirstAlloc>() };
    // SAFETY: the default slot read is thread-local, and a nonnull `p` is a live 64-byte block.
    *failure = unsafe {
        if !ptr::eq(_mi_theap_default(), empty()) {
            Some("thread heap already initialized")
        } else {
            let p = entry(64);
            if p.is_null() {
                Some("first allocation returned null")
            } else {
                let bytes = core::slice::from_raw_parts(p.cast::<u8>(), 64);
                let failure = if !mi_theap_is_initialized(_mi_theap_default()) {
                    Some("thread heap not initialized")
                } else if *zeroed && bytes.iter().any(|&b| b != 0) {
                    Some("memory not zeroed")
                } else {
                    None
                };
                ptr::write_bytes(p.cast::<u8>(), 0xAB, 64);
                mi_free(p);
                failure
            }
        }
    };
    ptr::null_mut()
}

/// Make `entry` the first allocation of a raw pthread. A `std::thread` would already have
/// allocated through this test binary's `malloc` export and initialized its heap.
#[cfg(unix)]
fn first_alloc_on_fresh_thread(entry: Entry, zeroed: bool) -> Option<&'static str> {
    let mut run: FirstAlloc = (entry, zeroed, None);
    // SAFETY: `run` outlives the joined thread, which is its only other user.
    unsafe {
        let mut thread = core::mem::MaybeUninit::<libc::pthread_t>::uninit();
        let arg = ptr::addr_of_mut!(run).cast();
        assert_eq!(
            libc::pthread_create(thread.as_mut_ptr(), ptr::null(), first_alloc, arg),
            0
        );
        assert_eq!(libc::pthread_join(thread.assume_init(), ptr::null_mut()), 0);
    }
    run.2
}

#[cfg(unix)]
#[test]
#[cfg_attr(
    miri,
    ignore = "process init needs OS calls Miri lacks (MAP_NORESERVE mmap, /proc)"
)]
fn first_small_allocation_on_a_thread_resolves_the_empty_sentinel() {
    let empty_before = sentinel_state(empty());
    let wrong_before = sentinel_state(empty_wrong());
    for (name, entry, zeroed) in SMALL_ENTRIES {
        assert_eq!(first_alloc_on_fresh_thread(entry, zeroed), None, "{name}");
    }
    assert_eq!(sentinel_state(empty()), empty_before);
    assert_eq!(sentinel_state(empty_wrong()), wrong_before);
}

#[test]
fn small_allocation_from_the_wrong_thread_sentinel_returns_null() {
    let before = sentinel_state(empty_wrong());
    let entries: [unsafe fn(*mut mi_theap_t, usize) -> *mut c_void; 4] = [
        |t, n| unsafe { mi_theap_malloc(t, n) },
        |t, n| unsafe { mi_theap_zalloc(t, n) },
        |t, n| unsafe { mi_theap_malloc_small(t, n) },
        |t, n| unsafe { mi_theap_zalloc_small(t, n) },
    ];
    for entry in entries {
        // SAFETY: the wrong-thread sentinel is a supported input; it resolves to no heap.
        assert!(unsafe { entry(empty_wrong(), 64) }.is_null());
    }
    assert_eq!(sentinel_state(empty_wrong()), before);
}
