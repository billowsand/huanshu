//! Process metrics for the resource section of the report: memory at each phase and
//! small timing helpers that print one comparable line per measurement.

use std::time::Duration;
use windows_sys::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
use windows_sys::Win32::System::Threading::GetCurrentProcess;

/// Working set of this process in bytes, or `None` if the query fails.
pub fn working_set() -> Option<u64> {
    // SAFETY: `GetCurrentProcess` is a pseudo handle, and the counter struct is a plain
    // out-parameter that the call fills in; `cb` is its size as the API requires.
    unsafe {
        let mut counters: PROCESS_MEMORY_COUNTERS = std::mem::zeroed();
        let size = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        if GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, size) == 0 {
            return None;
        }
        Some(counters.WorkingSetSize as u64)
    }
}

pub fn mib(bytes: u64) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}

/// Prints `[mem] <phase>: <working set> MiB (peak <peak> MiB)`.
pub fn report(phase: &str) {
    match working_set() {
        Some(bytes) => eprintln!("[mem] {phase}: {:.1} MiB", mib(bytes)),
        None => eprintln!("[mem] {phase}: unavailable"),
    }
}

/// Prints one measurement line: `[time] <label>: <ms>`.
pub fn time(label: &str, duration: Duration) {
    eprintln!("[time] {label}: {:.1} ms", duration.as_secs_f64() * 1000.0);
}

/// Prints the median of a sample set.
pub fn time_median(label: &str, samples: &[f64]) {
    eprintln!(
        "[time] {label}: median {:.1} ms over {} samples ({})",
        crate::deck::median(samples.to_vec()),
        samples.len(),
        samples
            .iter()
            .map(|value| format!("{value:.1}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
}
