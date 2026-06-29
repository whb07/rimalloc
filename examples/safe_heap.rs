use core::alloc::Layout;

use mechanical_mimalloc::Heap;

fn main() {
    let heap = Heap::new().expect("heap creation should succeed in example");
    let mut allocation = heap
        .allocate_zeroed(Layout::from_size_align(64, 16).expect("layout"))
        .expect("heap allocation should succeed in example");
    allocation.as_mut()[0] = 42;
    assert_eq!(allocation.as_ref()[0], 42);
}
