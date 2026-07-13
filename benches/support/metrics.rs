#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LogicalLiveBytes {
    current: usize,
    peak: usize,
}

impl LogicalLiveBytes {
    pub fn allocated(&mut self, bytes: usize) {
        self.current = self.current.saturating_add(bytes);
        self.peak = self.peak.max(self.current);
    }

    pub fn freed(&mut self, bytes: usize) {
        self.current = self
            .current
            .checked_sub(bytes)
            .expect("logical live-byte accounting underflow");
    }

    pub const fn current(self) -> usize {
        self.current
    }

    pub const fn peak(self) -> usize {
        self.peak
    }
}
