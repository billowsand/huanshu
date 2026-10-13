//! Compares two directories of same-named PNGs pixel by pixel.
//!
//! Used to prove that a change to the fit search did not move anything: render
//! the fixture before and after, then compare the two trees.
//!
//!   cargo run --release -p slide-typst --example png_diff -- <before-dir> <after-dir>
//!
//! Prints one line per image (mean absolute channel difference 0–255 and the
//! share of pixels where any channel differs by more than 16), then the five
//! worst images by mean difference, then the totals.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use image::GenericImageView;

/// A pixel counts as different when any channel differs by more than this.
const DIFFERENT: u16 = 16;

struct Comparison {
    name: String,
    mean: f64,
    different: f64,
    size: (u32, u32),
    missing: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<PathBuf> = std::env::args().skip(1).map(PathBuf::from).collect();
    let (before, after) = match args.as_slice() {
        [before, after, ..] => (before, after),
        _ => {
            eprintln!("usage: png_diff <before-dir> <after-dir>");
            std::process::exit(2);
        }
    };
    let before_files = pngs(before)?;
    let after_files = pngs(after)?;
    let mut names: Vec<String> = before_files
        .keys()
        .chain(after_files.keys())
        .cloned()
        .collect();
    names.sort();
    names.dedup();

    let mut comparisons = Vec::new();
    for name in &names {
        let paths = (before_files.get(name), after_files.get(name));
        let comparison = match paths {
            (Some(left), Some(right)) => compare(name, left, right),
            _ => Ok(Comparison {
                name: name.clone(),
                mean: f64::NAN,
                different: f64::NAN,
                size: (0, 0),
                missing: true,
            }),
        };
        match comparison {
            Ok(comparison) => comparisons.push(comparison),
            Err(err) => eprintln!("{name}: {err}"),
        }
    }

    println!("{:<28} {:>9} {:>9}  size", "image", "mean", "diff%");
    for comparison in &comparisons {
        if comparison.missing {
            println!("{:<28} {:>9} {:>9}", comparison.name, "missing", "");
            continue;
        }
        println!(
            "{:<28} {:>9.3} {:>8.2}%  {}×{}",
            comparison.name,
            comparison.mean,
            comparison.different * 100.0,
            comparison.size.0,
            comparison.size.1
        );
    }

    let mut worst: Vec<&Comparison> = comparisons
        .iter()
        .filter(|c| !c.missing && c.mean.is_finite())
        .collect();
    worst.sort_by(|a, b| b.mean.total_cmp(&a.mean));
    println!("\nworst 5 by mean difference:");
    for comparison in worst.iter().take(5) {
        println!(
            "  {:<28} {:>8.3}  {:>6.2}%",
            comparison.name,
            comparison.mean,
            comparison.different * 100.0
        );
    }

    let measurable: Vec<&Comparison> = comparisons
        .iter()
        .filter(|c| !c.missing && c.mean.is_finite())
        .collect();
    let max = measurable.iter().map(|c| c.mean).fold(f64::NAN, f64::max);
    let mean = if measurable.is_empty() {
        f64::NAN
    } else {
        measurable.iter().map(|c| c.mean).sum::<f64>() / measurable.len() as f64
    };
    let over = measurable.iter().filter(|c| c.mean >= 1.0).count();
    let missing = comparisons.iter().filter(|c| c.missing).count();
    println!(
        "\n{} images · max mean {max:.3} · average mean {mean:.3} · >= 1.0: {over} · missing pairs: {missing}",
        comparisons.len()
    );
    Ok(())
}

fn pngs(dir: &Path) -> Result<BTreeMap<String, PathBuf>, std::io::Error> {
    let mut files = BTreeMap::new();
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.extension().is_some_and(|ext| ext == "png") {
            if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
                files.insert(name.to_string(), path);
            }
        }
    }
    Ok(files)
}

fn compare(
    name: &str,
    before: &Path,
    after: &Path,
) -> Result<Comparison, Box<dyn std::error::Error>> {
    let left = image::open(before)?;
    let right = image::open(after)?;
    let (lw, lh) = left.dimensions();
    let (rw, rh) = right.dimensions();
    if (lw, lh) != (rw, rh) {
        return Err(format!("size changed: {lw}×{lh} → {rw}×{rh}").into());
    }
    let left = left.to_rgba8();
    let right = right.to_rgba8();
    let mut total = 0u64;
    let mut different = 0u64;
    let pixels = (lw as u64) * (lh as u64);
    for (a, b) in left.pixels().zip(right.pixels()) {
        let mut divergent = false;
        for channel in 0..4 {
            let delta = a.0[channel].abs_diff(b.0[channel]);
            total += delta as u64;
            if delta > DIFFERENT as u8 {
                divergent = true;
            }
        }
        if divergent {
            different += 1;
        }
    }
    Ok(Comparison {
        name: name.to_string(),
        mean: total as f64 / (pixels * 4) as f64,
        different: different as f64 / pixels.max(1) as f64,
        size: (lw, lh),
        missing: false,
    })
}
