//! Compares two directories of same-named PNGs pixel by pixel.
//!
//! Used to prove that a change to the fit search did not move anything: render
//! the fixture before and after, then compare the two trees.
//!
//!   cargo run --release -p slide-typst --example png_diff -- <before-dir> <after-dir> [--align 4]
//!
//! Prints one line per image (mean absolute channel difference 0–255 and the
//! share of pixels where any channel differs by more than 16), then the five
//! worst images by mean difference, then the totals.
//!
//! With `--align <px>` every image is also compared at the integer offset that
//! minimizes the mean difference (`--align 4` searches -4..4 on both axes). A fit
//! factor a fraction of a percent smaller shifts the children of a vertically
//! centered block by a pixel or two, which is invisible but reads as a large mean
//! difference at offset (0, 0); if the aligned difference collapses, the page
//! moved rather than reflowed. Wrapped text does not collapse: a new line break
//! changes the shape of the paragraph.

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
    /// Mean difference at the best integer shift, and the shift itself.
    aligned: f64,
    offset: (i32, i32),
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut paths: Vec<PathBuf> = Vec::new();
    let mut align = 0i32;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--align" => align = args.next().and_then(|v| v.parse().ok()).unwrap_or(0),
            _ => paths.push(PathBuf::from(arg)),
        }
    }
    let (before, after) = match paths.as_slice() {
        [before, after, ..] => (before, after),
        _ => {
            eprintln!("usage: png_diff <before-dir> <after-dir> [--align <px>]");
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
            (Some(left), Some(right)) => compare(name, left, right, align),
            _ => Ok(Comparison {
                name: name.clone(),
                mean: f64::NAN,
                different: f64::NAN,
                size: (0, 0),
                missing: true,
                aligned: f64::NAN,
                offset: (0, 0),
            }),
        };
        match comparison {
            Ok(comparison) => comparisons.push(comparison),
            Err(err) => eprintln!("{name}: {err}"),
        }
    }

    println!(
        "{:<28} {:>9} {:>9} {:>9} {:>7}  size",
        "image", "mean", "diff%", "aligned", "shift"
    );
    for comparison in &comparisons {
        if comparison.missing {
            println!("{:<28} {:>9} {:>9}", comparison.name, "missing", "");
            continue;
        }
        println!(
            "{:<28} {:>9.3} {:>8.2}% {:>9} {:>7}  {}×{}",
            comparison.name,
            comparison.mean,
            comparison.different * 100.0,
            if align > 0 {
                format!("{:.3}", comparison.aligned)
            } else {
                "-".to_string()
            },
            if align > 0 {
                format!("{},{}", comparison.offset.0, comparison.offset.1)
            } else {
                "-".to_string()
            },
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
            "  {:<28} {:>8.3}  {:>6.2}%{}",
            comparison.name,
            comparison.mean,
            comparison.different * 100.0,
            if align > 0 {
                format!(
                    "   aligned {:.3} at ({}, {})",
                    comparison.aligned, comparison.offset.0, comparison.offset.1
                )
            } else {
                String::new()
            }
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
    if align > 0 {
        let aligned_max = measurable
            .iter()
            .map(|c| c.aligned)
            .fold(f64::NAN, f64::max);
        let aligned_mean =
            measurable.iter().map(|c| c.aligned).sum::<f64>() / measurable.len() as f64;
        let moved = measurable.iter().filter(|c| c.offset != (0, 0)).count();
        println!(
            "aligned (±{align}px) · max mean {aligned_max:.3} · average mean {aligned_mean:.3} · pages that moved: {moved}"
        );
    }
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
    align: i32,
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
    let (mean, different) = difference(&left, &right, 0, 0);
    let mut best = (mean, (0, 0));
    if align > 0 {
        for dx in -align..=align {
            for dy in -align..=align {
                if (dx, dy) == (0, 0) {
                    continue;
                }
                let (candidate, _) = difference(&left, &right, dx, dy);
                if candidate < best.0 {
                    best = (candidate, (dx, dy));
                }
            }
        }
    }
    Ok(Comparison {
        name: name.to_string(),
        mean,
        different,
        size: (lw, lh),
        missing: false,
        aligned: best.0,
        offset: best.1,
    })
}

/// Mean absolute channel difference and the share of divergent pixels when the
/// right image is shifted by `(dx, dy)` on top of the left one; pixels shifted in
/// from outside the canvas are skipped.
fn difference(left: &image::RgbaImage, right: &image::RgbaImage, dx: i32, dy: i32) -> (f64, f64) {
    let (width, height) = left.dimensions();
    let (mut total, mut divergent, mut pixels) = (0u64, 0u64, 0u64);
    for y in 0..height as i32 {
        let sy = y + dy;
        if sy < 0 || sy >= height as i32 {
            continue;
        }
        for x in 0..width as i32 {
            let sx = x + dx;
            if sx < 0 || sx >= width as i32 {
                continue;
            }
            let a = left.get_pixel(x as u32, y as u32);
            let b = right.get_pixel(sx as u32, sy as u32);
            let mut differs = false;
            for channel in 0..4 {
                let delta = a.0[channel].abs_diff(b.0[channel]);
                total += delta as u64;
                if delta > DIFFERENT as u8 {
                    differs = true;
                }
            }
            if differs {
                divergent += 1;
            }
            pixels += 1;
        }
    }
    let pixels = pixels.max(1);
    (
        total as f64 / (pixels * 4) as f64,
        divergent as f64 / pixels as f64,
    )
}
