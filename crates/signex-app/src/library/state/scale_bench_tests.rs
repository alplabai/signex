//! Library scale bench (#3115): a 25 000-part library, cold mount and
//! Components-panel search latency. `#[ignore]`d; method, thresholds and
//! results are in `docs/architecture/library-scale-bench.md`.
//!
//! ```text
//! cargo test -p signex-app --release --lib scale_bench -- --ignored --nocapture
//! ```
//!
//! `SIGNEX_BENCH_ROWS` sets the part count (default 25 000). One `BENCH`
//! line per measurement. The library is written by `LibraryWriter` and
//! mounted through the adapter and `LibraryState`, the calls the app makes;
//! the search is the panel's own `sync_view_caches` (`PanelQuery` over the
//! cached `SearchKeys`).

use std::path::Path;
use std::time::{Duration, Instant};

use chrono::{DateTime, TimeZone, Utc};
use signex_library::component::{ComponentRow, DatasheetRef, PlmReserved};
use signex_library::identity::{ComponentClass, InternalPn};
use signex_library::library_file::{FORMAT_TOKEN, LibrarySection, SnxlibManifest};
use signex_library::lifecycle::LifecycleState;
use signex_library::manifest::{LibraryMode, UsersConfig, WorkflowConfig};
use signex_library::manufacturer::{DistributorListing, ManufacturerPart};
use signex_library::param::{ParamMap, ParamValue};
use signex_library::primitive::{
    Footprint, FootprintFile, LayerId, Pad, PadKind, PadShape, PinDirection, PinOrientation,
    Polygon, PrimitiveRef, Symbol, SymbolFile, SymbolPin,
};
use signex_library::writer::LibraryWriter;

use super::*;
use crate::library::mount::prepare_mount;

const DEFAULT_ROWS: usize = 25_000;
/// One symbol envelope per ten parts, one footprint file per twenty.
const ROWS_PER_SYMBOL: usize = 10;
const ROWS_PER_FOOTPRINT: usize = 20;
const TABLES: [&str; 8] = [
    "resistors",
    "capacitors",
    "inductors",
    "diodes",
    "transistors",
    "ics",
    "connectors",
    "misc",
];
const MOUNT_LIMIT: Duration = Duration::from_secs(3);
const SEARCH_LIMIT: Duration = Duration::from_millis(150);
const LIBRARY_ID: Uuid = Uuid::from_u128(0x0194_0000_0000_7000_8000_0000_0000_0001);

fn id(kind: u128, n: usize) -> Uuid {
    Uuid::from_u128(0x0194_0000_0000_7000_8000_0000_0000_0000 | (kind << 40) | n as u128)
}

fn time() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 1, 2, 3, 4, 5).unwrap()
}

fn lib_ref(uuid: Uuid) -> PrimitiveRef {
    PrimitiveRef::new(LIBRARY_ID, uuid)
}

fn symbol_file(n: usize) -> SymbolFile {
    let pins = (1..=4)
        .map(|p| SymbolPin {
            electrical: PinDirection::Passive,
            position: [if p % 2 == 0 { 5.08 } else { -5.08 }, p as f64 * 2.54],
            orientation: if p % 2 == 0 {
                PinOrientation::Left
            } else {
                PinOrientation::Right
            },
            ..SymbolPin::new(p.to_string(), format!("P{p}"))
        })
        .collect();
    let symbol = Symbol {
        uuid: id(1, n),
        pins,
        created: time(),
        updated: time(),
        version: "1.0.0".into(),
        ..Symbol::empty(format!("SYM-{n:05}"))
    };
    SymbolFile {
        format: String::new(),
        file_uuid: id(5, n),
        display_name: format!("SYM-{n:05}"),
        copied_from: None,
        symbols: vec![symbol],
        created: time(),
        updated: time(),
    }
}

fn footprint_file(n: usize) -> FootprintFile {
    let mut fp = Footprint::with_identity(format!("FP-{n:05}"), id(2, n), time());
    fp.updated = time();
    fp.version = "1.0.0".into();
    fp.courtyard = Polygon::new(vec![[-2.0, -1.0], [2.0, -1.0], [2.0, 1.0], [-2.0, 1.0]]);
    fp.pads = (0..2 + n % 7)
        .map(|p| Pad {
            number: (p + 1).to_string(),
            kind: PadKind::Smd,
            shape: PadShape::Rect,
            size: [0.6, 0.7],
            position: [p as f64 * 0.65, 0.0],
            layers: ["Top Layer", "Top Solder", "Top Paste"]
                .iter()
                .map(|l| LayerId::new(*l))
                .collect(),
            ..Pad::default()
        })
        .collect();
    let mut file = FootprintFile::with_identity(fp, id(3, n), time());
    file.updated = time();
    file
}

/// E24 mantissas, so the values repeat as a real catalogue's do.
const E24: [f64; 24] = [
    1.0, 1.1, 1.2, 1.3, 1.5, 1.6, 1.8, 2.0, 2.2, 2.4, 2.7, 3.0, 3.3, 3.6, 3.9, 4.3, 4.7, 5.1, 5.6,
    6.2, 6.8, 7.5, 8.2, 9.1,
];
const PACKAGES: [&str; 6] = ["0201", "0402", "0603", "0805", "1206", "2512"];
const MAKERS: [&str; 6] = ["Murata", "Yageo", "Vishay", "Panasonic", "TDK", "Samsung"];

fn row(n: usize, symbols: usize, footprints: usize) -> ComponentRow {
    let table = n % TABLES.len();
    // The value walks the E24 mantissas, then the decades, so every value
    // repeats across parts as a catalogue's do.
    let decade = (n / (8 * E24.len()) % 7) as i32;
    let (unit, base, noun) = match table {
        0 => ("ohm", 10f64.powi(decade), "resistor"),
        1 => ("F", 1e-12 * 10f64.powi(decade), "capacitor"),
        2 => ("H", 1e-9 * 10f64.powi(decade), "inductor"),
        _ => ("V", 1.0, "part"),
    };
    let value = base * E24[n / 8 % E24.len()];
    let package = PACKAGES[n % PACKAGES.len()];
    let maker = MAKERS[n % MAKERS.len()];
    let mut parameters = ParamMap::new();
    parameters.insert(
        "value".into(),
        ParamValue::Measurement {
            value,
            unit: unit.into(),
        },
    );
    parameters.insert(
        "rated_voltage".into(),
        ParamValue::Measurement {
            value: [6.3, 10.0, 16.0, 25.0, 50.0, 100.0][n % 6],
            unit: "V".into(),
        },
    );
    parameters.insert(
        "tolerance".into(),
        ParamValue::Text(["1%", "5%", "10%"][n % 3].into()),
    );
    parameters.insert("package".into(), ParamValue::Text(package.into()));
    parameters.insert("aec_q200".into(), ParamValue::Bool(n.is_multiple_of(4)));
    ComponentRow {
        row_id: id(4, n),
        internal_pn: InternalPn::new(format!("{}-{n:06}", TABLES[table][..3].to_uppercase())),
        class: ComponentClass::new(noun),
        datasheet: DatasheetRef::url(format!("https://example.com/ds/{n}.pdf")),
        datasheet_label: String::new(),
        description: format!("{value:.3e} {unit} {noun} {package} {maker}"),
        more_datasheets: Vec::new(),
        state: LifecycleState::Released,
        symbol_ref: lib_ref(id(1, n / ROWS_PER_SYMBOL % symbols)),
        alternate_symbols: Vec::new(),
        footprint_ref: Some(lib_ref(id(2, n / ROWS_PER_FOOTPRINT % footprints))),
        alternate_footprints: Vec::new(),
        sim_ref: None,
        pin_map_overrides: Vec::new(),
        primary_mpn: ManufacturerPart::draft(
            maker,
            format!("{}-{n:06}-{package}", maker[..3].to_uppercase()),
        ),
        alternates: vec![ManufacturerPart::draft(
            MAKERS[(n + 1) % 6],
            format!("ALT-{n:06}"),
        )],
        supply: vec![DistributorListing::new(
            "ExampleDistributor",
            format!("ED-{n}"),
        )],
        parameters,
        plm: PlmReserved::default(),
        version: "1.0.0".into(),
        released: true,
        symbol_version: "1.0.0".into(),
        footprint_version: "1.0.0".into(),
        sim_version: String::new(),
        created: time(),
        updated: time(),
        content_hash: [0u8; 32],
        server_revision: None,
    }
}

fn write_library(dir: &Path, rows: usize) -> std::path::PathBuf {
    let manifest = SnxlibManifest {
        format: FORMAT_TOKEN.into(),
        library_id: LIBRARY_ID,
        library: LibrarySection {
            name: "Scale".into(),
            description: Some("Synthetic scale-bench library".into()),
            ..Default::default()
        },
        mode: LibraryMode::LocalGit,
        workflow: WorkflowConfig::default(),
        users: UsersConfig {
            default_role: "Designer".into(),
            ..UsersConfig::default()
        },
        classes: Vec::new(),
    };
    let symbols = (rows / ROWS_PER_SYMBOL).max(1);
    let footprints = (rows / ROWS_PER_FOOTPRINT).max(1);
    let mut writer = LibraryWriter::new("Scale", manifest);
    for n in 0..symbols {
        writer.add_symbol_file(symbol_file(n));
    }
    for n in 0..footprints {
        writer.add_footprint_file(footprint_file(n)).unwrap();
    }
    for n in 0..rows {
        writer.add_row(TABLES[n % TABLES.len()], row(n, symbols, footprints));
    }
    writer.write(&dir.join("Scale")).expect("write the library")
}

/// Peak resident set of this process, MiB (`VmHWM`), where `/proc` has it.
fn peak_rss_mib() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let kib = status.lines().find_map(|l| l.strip_prefix("VmHWM:"))?;
    Some(
        kib.trim()
            .trim_end_matches("kB")
            .trim()
            .parse::<u64>()
            .ok()?
            / 1024,
    )
}

fn median(mut samples: Vec<Duration>) -> Duration {
    samples.sort();
    samples[samples.len() / 2]
}

#[test]
#[ignore = "bench: see docs/architecture/library-scale-bench.md"]
fn scale_bench() {
    let rows: usize = std::env::var("SIGNEX_BENCH_ROWS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_ROWS);
    let dir = tempfile::tempdir().unwrap();
    let t = Instant::now();
    let snxlib = write_library(dir.path(), rows);
    println!(
        "BENCH generate rows={rows} write_ms={}",
        t.elapsed().as_millis()
    );

    // Cold: nothing of this library is in memory. `prepare_mount` is the
    // off-thread half (adapter open, tables read), `mount_prepared` the
    // `LibrarySet` mount, `sync_view_caches` the first panel list.
    let t0 = Instant::now();
    let prepared = prepare_mount(&snxlib).expect("prepare_mount");
    let prepare = t0.elapsed();
    let mut state = LibraryState::default();
    let t1 = Instant::now();
    state.mount_prepared(prepared).expect("mount_prepared");
    let mount = t1.elapsed();
    let t2 = Instant::now();
    state.sync_view_caches();
    let first_list = t2.elapsed();
    let cold = t0.elapsed();
    let listed = state.components_panel.rows.visible_count(&snxlib);
    println!(
        "BENCH cold_mount rows={rows} listed={listed} prepare_ms={} mount_ms={} first_list_ms={} total_ms={} peak_rss_mib={}",
        prepare.as_millis(),
        mount.as_millis(),
        first_list.as_millis(),
        cold.as_millis(),
        peak_rss_mib().unwrap_or(0)
    );

    // Search: each query is typed after another, so the filter really runs.
    let mut worst = Duration::ZERO;
    for (query, from) in [
        ("100n", "zz"),
        ("10k", "zz"),
        ("4k7", "zz"),
        ("resistor 0603", "zz"),
        ("TDK-001234", "zz"),
        ("1u 16V", "zz"),
        ("100", "zz"),
        ("murata 0805 capacitor", "zz"),
        ("no such part", "zz"),
    ] {
        let mut samples = Vec::new();
        for _ in 0..7 {
            state.components_panel.filter = from.into();
            state.sync_view_caches();
            state.components_panel.filter = query.into();
            let t = Instant::now();
            state.sync_view_caches();
            samples.push(t.elapsed());
        }
        let hits = state.components_panel.rows.visible_count(&snxlib);
        let (med, max) = (
            median(samples.clone()),
            samples.iter().max().copied().unwrap(),
        );
        worst = worst.max(max);
        println!(
            "BENCH search query={query:?} hits={hits} median_ms={:.1} max_ms={:.1}",
            med.as_secs_f64() * 1e3,
            max.as_secs_f64() * 1e3
        );
    }
    // Typing "100n" one key at a time: every keystroke refilters.
    let mut typing = Duration::ZERO;
    state.components_panel.filter.clear();
    state.sync_view_caches();
    for prefix in ["1", "10", "100", "100n"] {
        state.components_panel.filter = prefix.into();
        let t = Instant::now();
        state.sync_view_caches();
        typing = typing.max(t.elapsed());
    }
    println!(
        "BENCH typing worst_keystroke_ms={:.1}",
        typing.as_secs_f64() * 1e3
    );
    println!(
        "BENCH verdict mount_limit_ms={} mount_ok={} search_limit_ms={} search_ok={}",
        MOUNT_LIMIT.as_millis(),
        cold < MOUNT_LIMIT,
        SEARCH_LIMIT.as_millis(),
        worst.max(typing) < SEARCH_LIMIT
    );
    assert!(
        cold < MOUNT_LIMIT,
        "cold mount {cold:?} breaches {MOUNT_LIMIT:?}"
    );
    assert!(
        worst.max(typing) < SEARCH_LIMIT,
        "search {:?} breaches {SEARCH_LIMIT:?}",
        worst.max(typing)
    );
}
