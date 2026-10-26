use rimalloc::raw::headers::{
    mi_stl_allocator, mi_stl_allocator_operator_eq, mi_stl_allocator_operator_ne,
};
use rimalloc::raw::{
    MI_MAX_ALIGN_SIZE, mi_free, mi_option_disable, mi_option_t, mi_zalloc_aligned,
};

#[derive(Clone, Copy, Default)]
struct SomeStruct {
    i: i32,
    j: i32,
    z: f64,
}

fn test_stl_allocator1() -> bool {
    let _ = core::hint::black_box((
        mi_stl_allocator,
        mi_stl_allocator_operator_eq,
        mi_stl_allocator_operator_ne,
    ));

    let mut vec = Vec::<i32>::new();
    vec.push(1);
    vec.pop();
    vec.is_empty()
}

fn test_stl_allocator2() -> bool {
    core::hint::black_box(mi_stl_allocator);

    let item = SomeStruct::default();
    let _ = item.i + item.j + item.z as i32;

    let mut vec = Vec::<SomeStruct>::new();
    vec.push(item);
    vec.pop();
    vec.is_empty()
}

fn mem_is_zero(p: *const u8, size: usize) -> bool {
    if p.is_null() {
        return false;
    }
    unsafe { core::slice::from_raw_parts(p, size) }
        .iter()
        .all(|byte| *byte == 0)
}

fn test_api_main() -> i32 {
    unsafe { mi_option_disable(mi_option_t::mi_option_verbose) };
    let mut ok = true;
    ok = ok && test_stl_allocator1();
    ok = ok && test_stl_allocator2();

    unsafe {
        let zalloc_size = 1024 * 1024;
        let alignment = MI_MAX_ALIGN_SIZE * 2;
        let p = mi_zalloc_aligned(zalloc_size, alignment).cast::<u8>();
        ok = ok && mem_is_zero(p, zalloc_size);
        mi_free(p.cast());
    }

    if ok { 0 } else { 1 }
}

#[test]
fn original_test_api_stl_allocator1() {
    assert!(test_stl_allocator1());
}

#[test]
fn original_test_api_stl_allocator2() {
    assert!(test_stl_allocator2());
}

#[test]
fn original_test_api_mem_is_zero() {
    let bytes = [0_u8, 0, 0, 0];
    assert!(mem_is_zero(bytes.as_ptr(), bytes.len()));
    assert!(!mem_is_zero(core::ptr::null(), bytes.len()));
    assert!(!mem_is_zero([0_u8, 1].as_ptr(), 2));
}

#[test]
fn original_test_api_main() {
    assert_eq!(test_api_main(), 0);
}
