//! Diagnostic binary: does repeated Typst compile + raster grow memory on its own?
//!
//! This is the same work the board's soak does (mutate one slide, compile the deck,
//! rasterize one page at full stage width) but without GPUI, so the two sources of
//! growth can be told apart:
//!
//! ```powershell
//! cargo run --release --bin raster-loop -- 300 1664
//! ```
//!
//! It prints the working set every 30 iterations. `slide_typst::Renderer::compile`
//! calls `typst::comemo::evict(50)` after every compile, exactly like the real app.

use std::time::Instant;

use serde_json::{Value, json};
use slide_typst::{ExportOptions, Renderer};
use windows_sys::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
use windows_sys::Win32::System::Threading::GetCurrentProcess;

fn working_set_mib() -> f64 {
    // SAFETY: `GetCurrentProcess` is a pseudo handle and the counter struct is a plain
    // out-parameter filled by the call.
    unsafe {
        let mut counters: PROCESS_MEMORY_COUNTERS = std::mem::zeroed();
        let size = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        if GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, size) == 0 {
            return f64::NAN;
        }
        counters.WorkingSetSize as f64 / (1024.0 * 1024.0)
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let iterations: usize = args
        .next()
        .and_then(|value| value.parse().ok())
        .unwrap_or(300);
    let width: u32 = args
        .next()
        .and_then(|value| value.parse().ok())
        .unwrap_or(1664);

    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/all-layout-blueprints.json");
    let deck: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let options = ExportOptions {
        theme: "light-notes".to_string(),
        aspect_ratio: "ratio_16x9".to_string(),
        ..Default::default()
    };

    let renderer = Renderer::with_system_fonts();
    eprintln!(
        "[raster-loop] {iterations} iterations, {width} px, {} pages in the deck; \
         start working set {:.1} MiB",
        deck.len(),
        working_set_mib()
    );
    let started = Instant::now();
    for iteration in 0..iterations {
        let mut slides = deck.clone();
        slides[0]["title"] = json!(format!("标题 {iteration}"));
        let pages = renderer
            .render_pixels(&slides, &options, width, Some(&[0]))
            .expect("render");
        drop(pages);
        if iteration % 30 == 0 || iteration + 1 == iterations {
            eprintln!(
                "[raster-loop] iteration {iteration}: {:.1} MiB ({:.0} ms elapsed)",
                working_set_mib(),
                started.elapsed().as_secs_f64() * 1000.0
            );
        }
    }
    eprintln!(
        "[raster-loop] done in {:.1} s, final working set {:.1} MiB",
        started.elapsed().as_secs_f64(),
        working_set_mib()
    );
}
