use core::alloc::Layout;
use core::mem::MaybeUninit;

use mechanical_mimalloc::{Allocation, TryAllocError};

fn main() -> Result<(), TryAllocError> {
    let layout = Layout::from_size_align(4, 1).expect("valid example layout");
    let mut allocation = Allocation::new(layout).map_err(TryAllocError::Alloc)?;
    assert_eq!(allocation.layout(), layout);

    let slots: &mut [MaybeUninit<u8>] = allocation.as_uninit_slice_mut();
    for (slot, byte) in slots.iter_mut().zip([1, 2, 3, 4]) {
        slot.write(byte);
    }

    let initialized = unsafe { allocation.assume_initialized() };
    assert_eq!(initialized.as_bytes(), &[1, 2, 3, 4]);

    let zeroed = Allocation::zeroed_with_size_align(256, 64)?;
    assert_eq!(zeroed.len(), 256);
    assert_eq!(zeroed.as_ptr() as usize % 64, 0);
    assert!(zeroed.as_bytes().iter().all(|byte| *byte == 0));

    Ok(())
}
