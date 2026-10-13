//! Measures the edit latency of a single-slide change.
//!
//! The editor's live preview re-renders one page after every keystroke, so the
//! number that matters is "compile the whole deck again after touching one
//! field". This example prints that number per layout, plus a few controlled
//! experiments that isolate where the time goes.
//!
//!   cargo run --release -p slide-typst --example bench_edit -- [--theme light-notes] [--runs 20] [--fixture <path>]
//!
//! Every experiment compiles through `Renderer::compile`, which is what
//! `render_png` (thumbnails, the presentation stage) and `render_pdf` (export)
//! call; for a one-page deck the layout work is nearly the whole cost. Three
//! blocks are printed:
//!
//! - whole-deck: one compile per edit, warm caches, for scale;
//! - per layout: a one-page deck, `runs` edits, p50/p95. Each edit replaces one
//!   character with a different CJK character, so the page keeps its size and
//!   the sample measures one keystroke rather than a page creeping larger;
//! - cache isolation and the clip branch: see the comments at those blocks.
//!
//! A layout measured for the first time in a process (or one whose content, fit
//! factor and theme are all new) pays a cold layout of about 100ms; the timings
//! below that are warm, which is the state the editor is in after the first
//! keystroke on a page.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde_json::Value;
use slide_typst::{ExportOptions, Renderer};

/// The text field each layout actually draws, so "edit the body" changes
/// rendered content in every kind. The value names the array the content lives
/// in; the leaf is picked by `first_string_leaf`, which prefers `body`, `text`,
/// `description` … in the order the templates read them. Kinds without body
/// copy (cover, closing) pass an empty path and report `n/a`.
const BODY_FIELDS: &[(&str, &[&str])] = &[
    ("cover", &[]),
    ("overview", &["overview_items"]),
    ("feature_grid", &["cards"]),
    ("spotlight", &["panels"]),
    ("split_layers", &["left_items"]),
    ("section_list", &["list_items"]),
    ("focus_example", &["example_body"]),
    ("outcome_grid", &["cards"]),
    ("center_grid", &["center_items"]),
    ("timeline", &["timeline_events"]),
    ("step_flow", &["steps"]),
    ("process", &["phases"]),
    ("compare", &["compare_data"]),
    ("swot", &["swot_data"]),
    ("section_intro", &["cards"]),
    ("issue_stack", &["cards"]),
    ("chart_focus", &["points"]),
    ("closing", &[]),
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let mut theme = "light-notes".to_string();
    let mut runs = 20usize;
    let mut fixture: Option<PathBuf> = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--theme" => theme = args.next().unwrap_or(theme),
            "--runs" => runs = args.next().and_then(|v| v.parse().ok()).unwrap_or(runs),
            "--fixture" => fixture = args.next().map(PathBuf::from),
            _ => {}
        }
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixture = fixture.unwrap_or_else(|| root.join("fixtures/all-layout-blueprints.json"));
    let deck: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(&fixture)?)?;

    let started = Instant::now();
    let renderer = Renderer::with_system_fonts();
    println!(
        "theme {theme} · {} slides from {} · fonts in {:?}\n",
        deck.len(),
        fixture.display(),
        started.elapsed()
    );
    let opts = ExportOptions {
        title: Some("bench".into()),
        theme: theme.clone(),
        ..Default::default()
    };
    let compile = |slides: &[Value]| {
        let started = Instant::now();
        let result = renderer.compile(slides, &opts);
        let elapsed = started.elapsed();
        result.expect("compile");
        elapsed
    };

    // ── Whole-deck experiments ────────────────────────────────────────────────
    // Each line is one compile of the whole deck; the caches are warm from the
    // previous line, which is the state the editor is in after a keystroke.
    println!("== whole deck ({} slides), warm caches ==", deck.len());
    println!("cold compile                  {:?}", compile(&deck));
    println!("recompile, nothing changed    {:?}", compile(&deck));

    let mut edited = deck.clone();
    append_field(&mut edited[0], "title").expect("cover title");
    println!("edit title of slide 1         {:?}", compile(&edited));

    let last = deck.len() - 1;
    let mut edited = deck.clone();
    append_content(&mut edited[last]).expect("last slide text");
    println!("edit text of last slide       {:?}", compile(&edited));

    // Change something that is not rendered: `DeckInput` serializes the raw
    // `Value`s, so the deck bytes differ, but Typst's `json()` decodes the deck
    // once and the layout never reads this key. Cost equal to "nothing changed"
    // means the memoized layout is keyed on the *parsed* page, not on the bytes.
    let mut edited = deck.clone();
    edited[0]["__unused"] = Value::String("perturbation".into());
    println!("add never-rendered field      {:?}", compile(&edited));

    // Same byte length, different character: every comemo key that includes the
    // source bytes changes, the cluster count and the layout do not.
    let mut edited = deck.clone();
    same_length(&mut edited[0]["title"]).expect("cover title");
    println!("swap one title character      {:?}", compile(&edited));
    println!();

    // ── Where the cache stops helping ─────────────────────────────────────────
    // One `feature_grid` page. The first line is the same page and the same deck
    // JSON as the compiles above, so it only *looks* like a controlled
    // experiment: the page has never been laid out on its own, with this
    // `ctx.index` and this one-page deck, so it is a cold layout. That is how
    // much a page whose cache entry does not exist costs; the lines after it
    // change one property at a time.
    if let Some(slide) = deck.iter().find(|s| s["kind"] == "feature_grid") {
        let page = vec![slide.clone()];
        println!("== one feature_grid page, one property changed per compile ==");
        println!("this page alone, cold         {:?}", compile(&page));

        let mut variant = slide.clone();
        variant["cards"][0]["icon"] = Value::String("i-carbon:map".into());
        println!(
            "card icon changed             {:?}",
            compile(&vec![variant])
        );

        let mut variant = slide.clone();
        same_length(&mut variant["cards"][0]["body"]).expect("card body");
        println!(
            "one body character replaced   {:?}",
            compile(&vec![variant])
        );

        let mut variant = slide.clone();
        same_length(&mut variant["title"]).expect("title");
        println!(
            "one title character replaced  {:?}",
            compile(&vec![variant])
        );

        let mut variant = slide.clone();
        variant["cards"][0]["body"] = Value::String("测试文本".repeat(4));
        println!(
            "body rewritten, same length   {:?}",
            compile(&vec![variant])
        );
        println!();
    }

    // ── Per-layout experiments ────────────────────────────────────────────────
    // A deck of one slide, so the compile cost is that layout alone. Every
    // iteration replaces one character with a different one of the same byte
    // length, which keeps the page at a fixed size: the timings measure "one
    // keystroke changed this field", not "the page grew a little each time".
    println!("== per layout, {runs} edits, deck = one page ==");
    println!("title: one character of `title` replaced");
    println!("body:  one character of the kind's rendered body text replaced\n");
    let mut rows = Vec::new();
    for (kind, body_path) in BODY_FIELDS {
        let Some(slide) = deck.iter().find(|s| s["kind"] == *kind) else {
            println!("{kind:<14} (missing from fixture)");
            continue;
        };
        let pristine = vec![slide.clone()];
        let title = time_edits(&renderer, &opts, &pristine, runs, |slide, _| {
            same_length(&mut slide["title"])
        });
        let body = if body_path.is_empty() {
            None
        } else {
            Some(time_edits(&renderer, &opts, &pristine, runs, |slide, _| {
                same_length_in_body(slide, body_path)
            }))
        };
        rows.push((*kind, title, body));
        println!(
            "{kind:<14} cold {:>7}   title p50 {:>7} p95 {:>7}   body {}",
            ms(title.0),
            ms(title.1),
            ms(title.2),
            match body {
                Some((cold, p50, p95)) =>
                    format!("cold {:>7} p50 {:>7} p95 {:>7}", ms(cold), ms(p50), ms(p95)),
                None => "n/a".to_string(),
            }
        );
    }

    println!("\n== table ==");
    println!("kind | title cold | title p50 | title p95 | body cold | body p50 | body p95");
    for (kind, title, body) in &rows {
        let (bcold, bp50, bp95) = match body {
            Some((cold, p50, p95)) => (ms(*cold), ms(*p50), ms(*p95)),
            None => ("n/a".to_string(), "n/a".to_string(), "n/a".to_string()),
        };
        println!(
            "{} | {} | {} | {} | {} | {} | {}",
            kind,
            ms(title.0),
            ms(title.1),
            ms(title.2),
            bcold,
            bp50,
            bp95
        );
    }

    clip_experiment(&renderer, &opts, &deck, runs);
    Ok(())
}

/// `fit-height` has two branches: a successful fit (the search) and the clip
/// fallback (nothing fits, so the content is measured once at `fit-min` and the
/// overflow marker is emitted). This grows one `feature_grid` card until the
/// clip branch is definitely taken and prints the cost of each edit, so the
/// report can say which branch is slow.
fn clip_experiment(renderer: &Renderer, opts: &ExportOptions, deck: &[Value], runs: usize) {
    let Some(slide) = deck.iter().find(|s| s["kind"] == "feature_grid") else {
        return;
    };
    let mut base = slide.clone();
    for i in 0..=runs {
        let times = 4 + i * 3;
        base["cards"][0]["body"] = Value::String("测试文本".repeat(times));
        let page = vec![base.clone()];
        let started = Instant::now();
        let compiled = renderer.compile(&page, opts).expect("compile");
        let elapsed = started.elapsed();
        let width = compiled.document.pages().len();
        println!(
            "\nclip branch · feature_grid card body = 4×{times} characters · {} page(s) · {:?}",
            width, elapsed
        );
    }
}

/// Compiles `pristine` (a one-page deck) `runs + 1` times. Every iteration
/// rebuilds the slide from `pristine` and applies one `mutate`, so each measured
/// compile is the same size of edit. Returns `(cold, p50, p95)`: the first
/// compile of this page in the process is cold, which is the cost of the first
/// keystroke on a page; the rest are warm, which is every keystroke after that.
fn time_edits(
    renderer: &Renderer,
    opts: &ExportOptions,
    pristine: &[Value],
    runs: usize,
    mut mutate: impl FnMut(&mut Value, usize) -> Result<usize, String>,
) -> (Duration, Duration, Duration) {
    let mut samples = Vec::with_capacity(runs);
    let mut cold = Duration::ZERO;
    for i in 0..=runs {
        let mut slide = pristine[0].clone();
        mutate(&mut slide, i).expect("mutation");
        let deck = vec![slide];
        let started = Instant::now();
        renderer.compile(&deck, opts).expect("compile");
        let elapsed = started.elapsed();
        if i == 0 {
            cold = elapsed;
        } else {
            samples.push(elapsed);
        }
    }
    // The first compile of this page in the process is cold; `cold` reports it
    // so the warm p50 is not mistaken for the first-keystroke latency.
    let (p50, p95) = percentiles(samples);
    (cold, p50, p95)
}

fn percentiles(mut samples: Vec<Duration>) -> (Duration, Duration) {
    samples.sort();
    let p50 = samples[samples.len() / 2];
    let p95 = samples[(samples.len() * 95 / 100).min(samples.len() - 1)];
    (p50, p95)
}

/// Appends `suffix` to a string field, refusing non-strings.
fn append(value: &mut Value, suffix: &str) -> Result<usize, String> {
    let text = value
        .as_str()
        .ok_or_else(|| format!("not a string: {value}"))?
        .to_string();
    *value = Value::String(format!("{text}{suffix}"));
    Ok(text.chars().count())
}

/// Replaces the last character with a different CJK character: same UTF-8 byte
/// length, same cluster width, different text. Every comemo key that hashes the
/// source bytes changes; a cache keyed on "the page as laid out" would not.
fn same_length(value: &mut Value) -> Result<usize, String> {
    let text = value
        .as_str()
        .ok_or_else(|| format!("not a string: {value}"))?
        .to_string();
    let mut chars: Vec<char> = text.chars().collect();
    if chars.len() < 2 {
        return Err(format!("too short to swap: {text:?}"));
    }
    let last = chars.len() - 1;
    chars[last] = if chars[last] == '测' { '试' } else { '测' };
    let len = chars.len();
    *value = Value::String(chars.into_iter().collect());
    Ok(len)
}

/// Same as [`same_length`] but on the body text the kind renders.
fn same_length_in_body(slide: &mut Value, path: &[&str]) -> Result<usize, String> {
    let root = slide
        .get_mut(path[0])
        .ok_or_else(|| format!("missing {}", path[0]))?;
    let leaf = first_string_leaf(root).ok_or_else(|| format!("no string under {}", path[0]))?;
    same_length(leaf)
}

/// Appends a character to the first string under `key` (used for the title,
/// which is a plain top-level string).
fn append_field(slide: &mut Value, key: &str) -> Result<usize, String> {
    append(&mut slide[key], "字")
}

/// Appends a character to whichever body-ish string the slide has, so the
/// whole-deck experiment works on any fixture.
fn append_content(slide: &mut Value) -> Result<usize, String> {
    for key in [
        "note",
        "subtitle",
        "overview_items",
        "cards",
        "points",
        "left_items",
    ] {
        if let Some(root) = slide.get_mut(key) {
            if let Some(leaf) = first_string_leaf(root) {
                return append(leaf, "字");
            }
        }
    }
    // Closing slides only have a title: fall back to any string on the page.
    let root = slide.as_object_mut().ok_or("not an object")?;
    let key = root
        .keys()
        .find(|key| key.as_str() != "kind")
        .cloned()
        .ok_or("no editable text")?;
    let leaf = first_string_leaf(&mut root[&key]).ok_or("no editable text")?;
    append(leaf, "字")
}

fn first_string_leaf(value: &mut Value) -> Option<&mut Value> {
    match value {
        Value::String(_) => Some(value),
        Value::Array(items) => items.iter_mut().find_map(first_string_leaf),
        Value::Object(map) => {
            // Prefer the keys that carry body copy, in the order the templates
            // read them, so the edit lands on rendered text.
            const PREFERRED: &[&str] = &[
                "body",
                "text",
                "description",
                "detail",
                "conclusion",
                "title",
                "summary",
                "note",
            ];
            let key = PREFERRED
                .iter()
                .find(|key| map.contains_key(**key))
                .map(|key| (*key).to_string())
                .or_else(|| map.keys().next().cloned())?;
            first_string_leaf(map.get_mut(&key)?)
        }
        _ => None,
    }
}

fn ms(duration: Duration) -> String {
    format!("{:.1}ms", duration.as_secs_f64() * 1000.0)
}
