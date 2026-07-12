use std::alloc::Layout;

pub struct PointerPool {
    slots: Box<[(*mut u8, Layout)]>,
}

unsafe impl Send for PointerPool {}

impl PointerPool {
    pub fn new(capacity: usize) -> Self {
        Self {
            slots: vec![(std::ptr::null_mut(), Layout::new::<u8>()); capacity].into_boxed_slice(),
        }
    }

    pub fn len(&self) -> usize {
        self.slots.len()
    }

    pub fn slot(&mut self, index: usize) -> &mut (*mut u8, Layout) {
        &mut self.slots[index]
    }

    pub fn all_empty(&self) -> bool {
        self.slots.iter().all(|(ptr, _)| ptr.is_null())
    }
}
