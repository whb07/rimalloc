use mechanical_mimalloc::{ProcessInfo, StatsSnapshot};

fn main() -> Result<(), mechanical_mimalloc::StatsError> {
    let stats = StatsSnapshot::capture()?;
    let _requested = stats.malloc_requested_total();

    let process = ProcessInfo::current();
    let _elapsed = process.elapsed();

    Ok(())
}
