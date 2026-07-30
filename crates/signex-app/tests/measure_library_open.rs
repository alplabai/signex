//! Timing probe for the library-open path — issue #99 part 2.
//!
//! Not a pass/fail test: it prints numbers and asserts nothing about them,
//! so it is `#[ignore]`d and never runs in CI. It is committed so the
//! before/after of a change to the open path can be re-measured on demand
//! and a future regression has a ready-made probe. Run it explicitly:
//!
//! ```text
//! cargo test -p signex-app --release --test measure_library_open -- --ignored --nocapture
//! ```
//!
//! `--release` is not optional. The debug profile is >10× slower and the
//! generation step alone will not finish in a reasonable time.
//!
//! It generates synthetic `.snxlib` libraries at four scales in a tempdir
//! (via the real `LocalGitAdapter` / `SymbolFile` / `FootprintFile` /
//! `SimFile` writers, so nothing about the on-disk format is guessed) and
//! wall-clocks the project-open path against them. No production code is
//! touched; every call goes through an already-public API.
//!
//! Behavioural coverage of the same path — that `open_library` really does
//! prime all five caches — lives in `tests/library_open_cache.rs`.

mod support;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use signex_app::library::commands::auto_mount_project_libraries;
use signex_app::library::state::{LibraryDisplaySettings, LibraryState, OpenLibrary};
use signex_library::adapter::LibraryAdapter;
use signex_library::adapters::local_git::LocalGitAdapter;
use signex_types::project::{LibraryEntry, LibraryEntryKind, ProjectData, parse_project};

use support::{Scale, generate_library, primitive_file_sizes};

/// Timed repetitions kept per measurement (a further warm-up run is
/// discarded first, so each op actually executes `RUNS + 1` times).
const RUNS: usize = 5;

// ── Timing plumbing ──────────────────────────────────────────────────────

struct Stat {
    min_ms: f64,
    median_ms: f64,
    max_ms: f64,
}

impl Stat {
    fn from(mut samples: Vec<Duration>) -> Self {
        samples.sort();
        let ms = |d: Duration| d.as_secs_f64() * 1000.0;
        Self {
            min_ms: ms(samples[0]),
            median_ms: ms(samples[samples.len() / 2]),
            max_ms: ms(samples[samples.len() - 1]),
        }
    }
}

struct Row {
    scale: String,
    op: String,
    stat: Stat,
}

/// Run `f` `RUNS + 1` times, discard the first (warm-up), keep the rest.
/// `f` returns only the duration of the region under test so per-iteration
/// setup (fresh `LibraryState`, fresh adapter) stays out of the number.
fn measure(scale: &str, op: &str, rows: &mut Vec<Row>, mut f: impl FnMut() -> Duration) {
    let mut samples = Vec::with_capacity(RUNS);
    for i in 0..=RUNS {
        let d = f();
        if i > 0 {
            samples.push(d);
        }
    }
    let stat = Stat::from(samples);
    println!(
        "  {:<38} min {:>9.3} ms   median {:>9.3} ms   max {:>9.3} ms",
        op, stat.min_ms, stat.median_ms, stat.max_ms
    );
    rows.push(Row {
        scale: scale.to_string(),
        op: op.to_string(),
        stat,
    });
}

fn timed<T>(f: impl FnOnce() -> T) -> (T, Duration) {
    let t0 = Instant::now();
    let out = f();
    (out, t0.elapsed())
}

// ── Per-library measurements ─────────────────────────────────────────────

fn measure_scale(scale: &Scale, snxlib: &Path, rows: &mut Vec<Row>) {
    let name = scale.name;
    println!(
        "\n── {name} ({} symbols, {} footprints, {} sims) ──",
        scale.symbols, scale.footprints, scale.sims
    );

    // 1. LocalGitAdapter::open — reads the `.snxlib`, parses it, validates
    //    the git repo. NOTE: it takes the `.snxlib` FILE path, not the dir.
    measure(name, "LocalGitAdapter::open", rows, || {
        let (adapter, d) = timed(|| LocalGitAdapter::open(snxlib).expect("open"));
        drop(adapter);
        d
    });

    // 2. OpenLibrary::reload_tables on an already-open adapter.
    measure(name, "OpenLibrary::reload_tables", rows, || {
        let adapter = LocalGitAdapter::open(snxlib).expect("open");
        let mut entry = blank_open_library(snxlib, &adapter);
        let (_, d) = timed(|| entry.reload_tables(&adapter).expect("reload_tables"));
        d
    });

    // 3. The three listings — separately, then all three in sequence.
    measure(name, "list_symbols", rows, || {
        let a = LocalGitAdapter::open(snxlib).expect("open");
        timed(|| a.list_symbols().expect("list_symbols")).1
    });
    measure(name, "list_footprints", rows, || {
        let a = LocalGitAdapter::open(snxlib).expect("open");
        timed(|| a.list_footprints().expect("list_footprints")).1
    });
    measure(name, "list_sims", rows, || {
        let a = LocalGitAdapter::open(snxlib).expect("open");
        timed(|| a.list_sims().expect("list_sims")).1
    });
    measure(name, "list_* all three in sequence", rows, || {
        let a = LocalGitAdapter::open(snxlib).expect("open");
        timed(|| {
            a.list_symbols().expect("list_symbols");
            a.list_footprints().expect("list_footprints");
            a.list_sims().expect("list_sims");
        })
        .1
    });
    // Same adapter instance, list_symbols twice — isolates "is there a
    // cache on the adapter?" from "is the OS page cache warm?".
    measure(name, "list_symbols x2 (same adapter)", rows, || {
        let a = LocalGitAdapter::open(snxlib).expect("open");
        a.list_symbols().expect("warm");
        timed(|| a.list_symbols().expect("list_symbols")).1
    });

    // 4. LibraryState::open_library end to end.
    measure(name, "LibraryState::open_library", rows, || {
        let mut state = LibraryState::default();
        timed(|| {
            state
                .open_library(snxlib.to_path_buf())
                .expect("open_library")
        })
        .1
    });

    // 5. refresh_components immediately after a completed open_library —
    //    i.e. a full redundant rescan. No longer on the mount path (#99
    //    part 2a), but it is still what every post-write refresh in the
    //    dispatchers costs, so the number stays worth having.
    measure(name, "refresh_components after open", rows, || {
        let mut state = LibraryState::default();
        state
            .open_library(snxlib.to_path_buf())
            .expect("open_library");
        timed(|| {
            state
                .refresh_components(snxlib)
                .expect("refresh_components")
        })
        .1
    });
}

/// A fresh `OpenLibrary` display entry with empty caches — mirrors what
/// `LibraryState::open_library` builds before it calls `reload_tables`.
fn blank_open_library(snxlib: &Path, adapter: &LocalGitAdapter) -> OpenLibrary {
    OpenLibrary {
        root: snxlib.to_path_buf(),
        display_name: adapter.manifest().library.name.clone(),
        library_id: adapter.library_id(),
        tables: std::collections::HashMap::new(),
        cached_components: Vec::new(),
        cached_symbols: Vec::new(),
        cached_footprints: Vec::new(),
        cached_sims: Vec::new(),
        display: LibraryDisplaySettings::default(),
    }
}

// ── Project-level measurements ───────────────────────────────────────────

fn write_project(dir: &Path, name: &str, libs: &[PathBuf]) -> PathBuf {
    let path = dir.join(format!("{name}.snxprj"));
    let data = ProjectData {
        name: name.to_string(),
        dir: dir.to_string_lossy().to_string(),
        schematic_root: None,
        pcb_file: None,
        sheets: Vec::new(),
        variant_definitions: Vec::new(),
        active_variant: None,
        libraries: libs
            .iter()
            .map(|p| LibraryEntry {
                path: p.clone(),
                kind: LibraryEntryKind::Shared,
                library_id: None,
            })
            .collect(),
        enable_git: false,
    };
    signex_types::project::write_project(&path, &data).expect("write_project");
    path
}

// ── Entry point ──────────────────────────────────────────────────────────

#[test]
#[ignore = "timing probe, asserts nothing; run with --release ... -- --ignored --nocapture"]
fn measure_library_open() {
    let scales = [
        Scale::new("tiny", 10, 10),
        Scale::new("small", 100, 100),
        Scale::new("medium", 500, 500),
        Scale::new("large", 2000, 2000),
    ];

    let tmp = tempfile::Builder::new()
        .prefix("signex-measure-")
        .tempdir()
        .expect("tempdir");
    let root = tmp.path();

    println!(
        "=== generating synthetic libraries under {} ===",
        root.display()
    );
    let mut libs: Vec<(Scale, PathBuf)> = Vec::new();
    for scale in &scales {
        let (path, gen_ms) = timed(|| generate_library(root, scale.name, scale));
        let path = path.expect("generate_library");
        println!(
            "  {:<8} generated in {:>8.1} ms  ->  {}",
            scale.name,
            gen_ms.as_secs_f64() * 1000.0,
            path.display()
        );
        libs.push((scale.clone(), path));
    }

    println!("\n=== generated primitive file sizes (bytes) ===");
    for (scale, path) in &libs {
        let s = primitive_file_sizes(path.parent().unwrap());
        println!(
            "  {:<8} .snxsym {:>7}  .snxfpt {:>7}  .snxsim {:>7}  .snxlib {:>9}  (library total {:>10})",
            scale.name, s.symbol, s.footprint, s.sim, s.snxlib, s.total
        );
    }

    let mut rows: Vec<Row> = Vec::new();
    println!("\n=== per-library measurements ===");
    for (scale, path) in &libs {
        measure_scale(scale, path, &mut rows);
    }

    // ── parse_project, N = 1 and N = 6 ───────────────────────────────────
    println!("\n── project parse ──");
    let medium_path = libs
        .iter()
        .find(|(s, _)| s.name == "medium")
        .map(|(_, p)| p.clone())
        .expect("medium library");

    let proj1 = write_project(root, "proj1", std::slice::from_ref(&medium_path));
    measure("project", "parse_project (N=1 library)", &mut rows, || {
        timed(|| parse_project(&proj1).expect("parse_project")).1
    });

    // Six distinct medium libraries — LibrarySet::mount rejects duplicate
    // library_id, so the auto-mount case cannot reuse one path six times.
    println!("\n=== generating 6 additional medium libraries for the auto-mount case ===");
    let mut six: Vec<PathBuf> = Vec::new();
    let medium_scale = Scale::new("medium", 500, 500);
    for i in 0..6 {
        let nm = format!("mount{i}");
        let p = generate_library(root, &nm, &medium_scale).expect("generate_library");
        six.push(p);
    }
    let proj6 = write_project(root, "proj6", &six);
    measure(
        "project",
        "parse_project (N=6 libraries)",
        &mut rows,
        || timed(|| parse_project(&proj6).expect("parse_project")).1,
    );

    // ── auto_mount_project_libraries, 6 medium libraries ─────────────────
    let project_data = parse_project(&proj6).expect("parse_project");
    measure("project", "auto_mount (6x medium)", &mut rows, || {
        let mut state = LibraryState::default();
        let (n, d) = timed(|| auto_mount_project_libraries(&mut state, &project_data));
        assert_eq!(n, 6, "expected 6 libraries mounted, got {n}");
        d
    });

    // Control line: the same six libraries opened by hand, `open_library`
    // only. Before #99 part 2a, `auto_mount_project_libraries` chased every
    // open with a `refresh_components` that recomputed the identical five
    // caches, and this line came in ~795.7 ms below the one above. With the
    // duplicate gone the two lines should now read the same — a gap
    // reopening here means a second full scan crept back into the mount
    // loop.
    measure(
        "project",
        "open_library x6 (control, no refresh)",
        &mut rows,
        || {
            let mut state = LibraryState::default();
            let paths: Vec<PathBuf> = project_data
                .libraries
                .iter()
                .map(|e| project_data.resolve_library_path(e))
                .collect();
            timed(|| {
                for p in &paths {
                    state.open_library(p.clone()).expect("open_library");
                }
            })
            .1
        },
    );

    // ── Machine-readable summary table ───────────────────────────────────
    println!("\n=== SUMMARY (ms) ===");
    println!(
        "{:<10} {:<40} {:>10} {:>10} {:>10}",
        "scale", "operation", "min", "median", "max"
    );
    for r in &rows {
        println!(
            "{:<10} {:<40} {:>10.3} {:>10.3} {:>10.3}",
            r.scale, r.op, r.stat.min_ms, r.stat.median_ms, r.stat.max_ms
        );
    }
}
