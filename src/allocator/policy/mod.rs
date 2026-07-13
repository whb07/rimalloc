//! Concrete compatibility policy for allocator tuning decisions.

mod compat;
mod decisions;
#[cfg(test)]
mod reference;
#[cfg(test)]
mod tests;

pub(crate) use compat::CompatPolicy;
pub(crate) use decisions::{
    GenericMaintenanceInterval, PageExtendCount, PageFullRetain, ReclaimContext, ReclaimDecision,
    RemoteCollectMode, ThreadHeapPolicy,
};

pub(crate) type ActivePolicy = CompatPolicy;
