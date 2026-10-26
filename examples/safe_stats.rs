use rimalloc::{ProcessInfo, StatsSnapshot};

fn main() -> Result<(), rimalloc::StatsError> {
    let stats = StatsSnapshot::capture()?;
    let _requested = stats.requested_bytes_total();

    let process = ProcessInfo::current();
    let _elapsed = process.elapsed();

    Ok(())
}
