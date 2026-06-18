use core::alloc::Layout;

use mechanical_mimalloc::Allocation;

fn main() -> Result<(), mechanical_mimalloc::TryAllocError> {
    let allocation = Allocation::zeroed_with_size_align(256, 64)?;
    assert_eq!(allocation.len(), 256);
    assert_eq!(allocation.as_ptr() as usize % 64, 0);

    let layout = Layout::from_size_align(128, 16).expect("valid example layout");
    let allocation = Allocation::new(layout).map_err(mechanical_mimalloc::TryAllocError::Alloc)?;
    assert_eq!(allocation.layout(), layout);

    Ok(())
}
