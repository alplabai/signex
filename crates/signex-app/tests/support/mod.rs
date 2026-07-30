//! Synthetic `.snxlib` generator, shared by the library-open test targets:
//! `measure_library_open.rs` (timing probe) and `library_open_cache.rs`
//! (cache-priming regression). Each target `mod support;`s its own copy —
//! hence the blanket `allow(dead_code)`, since neither uses all of it.
//!
//! Everything on disk is produced by the *real* Signex writers —
//! `LocalGitAdapter::init` for the manifest, `SymbolFile::to_toml_string`
//! / `FootprintFile::to_toml_string` / `SimFile::to_toml_string` for the
//! primitive envelopes, `LocalGitAdapter::insert_row` for the component
//! rows, and `LocalGitAdapter::recover_init` to land the whole tree in one
//! git commit. No file layout or wire format is hand-rolled here.
//!
//! Why not `save_symbol` / `save_footprint` per primitive: each of those
//! does `scan_symbol_files` (O(n) re-parse of the whole library) plus a
//! git commit, so generating 2000 symbols that way is O(n²) and would take
//! longer than the entire measurement. The bytes on disk are identical —
//! `save_symbol_in_container`'s new-file branch is exactly
//! `SymbolFile::from_symbol(..).to_toml_string()` + write, and
//! `write_primitive`'s not-yet-exists branch is exactly
//! `FootprintFile::from_footprint(..).to_toml_string()` + write.

#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};

use signex_library::adapter::{LibraryAdapter, LibraryError};
use signex_library::adapters::local_git::{LibraryInitOptions, LocalGitAdapter};
use signex_library::component::{ComponentRow, DatasheetRef, PlmReserved};
use signex_library::identity::{ComponentClass, InternalPn};
use signex_library::library_file::{FORMAT_TOKEN, LibrarySection, SnxlibManifest};
use signex_library::lifecycle::LifecycleState;
use signex_library::manifest::{LibraryMode, UsersConfig, WorkflowConfig};
use signex_library::manufacturer::ManufacturerPart;
use signex_library::param::{ParamMap, ParamValue};
use signex_library::primitive::{
    Body3D, BodyShape, ComponentType, Footprint, FootprintFile, FpGraphic, FpGraphicKind, LayerId,
    Pad, PadKind, PadShape, PinDirection, PinOrientation, Polygon, PrimitiveRef, SimFile, SimKind,
    SimModel, Symbol, SymbolFile, SymbolGraphic, SymbolGraphicKind, SymbolPin,
};
use uuid::Uuid;

/// Pins per generated symbol / pads per generated footprint — a
/// 16-pin SOIC-class part, the median thing in a real library.
const PINS_PER_SYMBOL: usize = 16;
/// One sim model per this many symbols. Real libraries carry SPICE
/// models for a minority of parts.
const SIMS_PER_SYMBOLS: usize = 10;

#[derive(Clone)]
pub struct Scale {
    pub name: &'static str,
    pub symbols: usize,
    pub footprints: usize,
    pub sims: usize,
}

impl Scale {
    pub fn new(name: &'static str, symbols: usize, footprints: usize) -> Self {
        Self {
            name,
            symbols,
            footprints,
            sims: symbols / SIMS_PER_SYMBOLS,
        }
    }
}

// ── Realistic primitive fixtures ─────────────────────────────────────────

fn sample_params() -> ParamMap {
    let mut p = ParamMap::new();
    p.insert("Manufacturer".into(), ParamValue::Text("Acme Semi".into()));
    p.insert("Package".into(), ParamValue::Text("SOIC-16".into()));
    p.insert(
        "Supply Voltage".into(),
        ParamValue::Measurement {
            value: 3.3,
            unit: "V".into(),
        },
    );
    p.insert("RoHS".into(), ParamValue::Bool(true));
    p.insert("Temp Min".into(), ParamValue::Number(-40.0));
    p.insert("Temp Max".into(), ParamValue::Number(125.0));
    p
}

/// A 16-pin IC symbol: pins on both sides with names, descriptions and
/// alternate functions, plus a body rectangle, four leader lines and two
/// text labels.
fn make_symbol(index: usize) -> Symbol {
    let mut s = Symbol::empty(format!("IC-{index:05}-SOIC16"));
    s.description = "Generic 16-pin mixed-signal device used for library open-path timing".into();
    s.designator = "U?".into();
    s.comment = "*".into();
    s.component_type = ComponentType::Standard;
    s.schematic_params = sample_params();

    let half = PINS_PER_SYMBOL / 2;
    for i in 0..PINS_PER_SYMBOL {
        let n = i + 1;
        let mut p = SymbolPin::new(n.to_string(), format!("IO{n}"));
        p.electrical = if i % 4 == 0 {
            PinDirection::Input
        } else if i % 4 == 1 {
            PinDirection::Output
        } else if i % 4 == 2 {
            PinDirection::Bidirectional
        } else {
            PinDirection::Passive
        };
        let left = i < half;
        let row = if left { i } else { i - half } as f64;
        p.position = [if left { -12.7 } else { 12.7 }, 8.89 - row * 2.54];
        p.orientation = if left {
            PinOrientation::Right
        } else {
            PinOrientation::Left
        };
        p.length = 2.54;
        p.description = format!("General purpose I/O line {n}, 5 V tolerant");
        p.function = vec![format!("GPIO{n}"), format!("ALT{n}")];
        s.pins.push(p);
    }

    s.graphics.push(SymbolGraphic {
        kind: SymbolGraphicKind::Rectangle {
            from: [-10.16, 10.16],
            to: [10.16, -10.16],
        },
        stroke_width: 0.254,
        fill: Some([255, 255, 200, 255]),
        part_number: 0,
    });
    for k in 0..4 {
        let y = 6.35 - k as f64 * 1.27;
        s.graphics.push(SymbolGraphic {
            kind: SymbolGraphicKind::Line {
                from: [-8.89, y],
                to: [8.89, y],
            },
            stroke_width: 0.127,
            fill: None,
            part_number: 0,
        });
    }
    s.graphics.push(SymbolGraphic {
        kind: SymbolGraphicKind::Text {
            position: [0.0, 11.43],
            content: format!("IC-{index:05}"),
            size: 1.27,
        },
        stroke_width: 0.0,
        fill: None,
        part_number: 0,
    });
    s.graphics.push(SymbolGraphic {
        kind: SymbolGraphicKind::Text {
            position: [0.0, -11.43],
            content: "SOIC-16".into(),
            size: 1.27,
        },
        stroke_width: 0.0,
        fill: None,
        part_number: 0,
    });
    s
}

/// A SOIC-16 footprint: 16 SMD pads, a courtyard polygon, silkscreen
/// outline + pin-1 marker, and a fab-layer label.
fn make_footprint(index: usize) -> Footprint {
    let now = chrono::Utc::now();
    let half = PINS_PER_SYMBOL / 2;
    let mut pads = Vec::with_capacity(PINS_PER_SYMBOL);
    for i in 0..PINS_PER_SYMBOL {
        let n = i + 1;
        let left = i < half;
        let row = if left { i } else { PINS_PER_SYMBOL - 1 - i } as f64;
        pads.push(Pad {
            number: n.to_string(),
            kind: PadKind::Smd,
            shape: PadShape::RoundRect { radius_ratio: 0.25 },
            size: [1.55, 0.6],
            position: [if left { -2.7 } else { 2.7 }, 4.445 - row * 1.27],
            rotation: 0.0,
            layers: vec![
                LayerId::new("F.Cu"),
                LayerId::new("F.Mask"),
                LayerId::new("F.Paste"),
            ],
            drill: None,
            solder_mask_margin: Some(0.05),
            paste_margin: Some(-0.025),
            ..Pad::default()
        });
    }

    let silk = vec![
        FpGraphic {
            kind: FpGraphicKind::Line {
                from: [-1.95, 5.1],
                to: [1.95, 5.1],
            },
            stroke_width: 0.12,
            filled: false,
        },
        FpGraphic {
            kind: FpGraphicKind::Line {
                from: [-1.95, -5.1],
                to: [1.95, -5.1],
            },
            stroke_width: 0.12,
            filled: false,
        },
        FpGraphic {
            kind: FpGraphicKind::Circle {
                center: [-2.6, 5.4],
                radius: 0.15,
            },
            stroke_width: 0.12,
            filled: true,
        },
    ];

    Footprint {
        uuid: Uuid::now_v7(),
        name: format!("SOIC-16-{index:05}"),
        anchor: [0.0, 0.0],
        pads,
        courtyard: Polygon::new(vec![[-3.7, 5.6], [3.7, 5.6], [3.7, -5.6], [-3.7, -5.6]]),
        silk_f: silk,
        silk_b: Vec::new(),
        fab_f: vec![FpGraphic {
            kind: FpGraphicKind::Text {
                position: [0.0, 0.0],
                content: format!("SOIC-16-{index:05}"),
                size: 1.0,
                frame: Some((7.0, 1.2)),
            },
            stroke_width: 0.1,
            filled: false,
        }],
        fab_b: Vec::new(),
        body_3d: Body3D {
            shape: BodyShape::Extrude,
            height_mm: 1.75,
            offset_z_mm: 0.0,
            top_color: [0.1, 0.1, 0.1, 1.0],
            side_color: [0.2, 0.2, 0.2, 1.0],
            outline: None,
        },
        step_attachment: None,
        pcb_params: sample_params(),
        version: "0.0.1".into(),
        released: false,
        created: now,
        updated: now,
        schema_version: 2,
        sketch: None,
        pours: Vec::new(),
        keepouts: Vec::new(),
        cutouts: Vec::new(),
        v_scores: Vec::new(),
        mask_openings: Vec::new(),
        mask_excludes: Vec::new(),
        paste_apertures: Vec::new(),
        description: "16-lead small outline integrated circuit, 1.27 mm pitch".into(),
        default_designator: "U?".into(),
        component_type: signex_library::primitive::footprint::ComponentType::Standard,
        height_mm: Some(1.75),
    }
}

fn make_sim(index: usize) -> SimModel {
    let now = chrono::Utc::now();
    SimModel {
        uuid: Uuid::now_v7(),
        name: format!("SPICE-{index:05}"),
        kind: SimKind::Spice3,
        body: ".SUBCKT GENERIC IN OUT VCC GND\n\
               R1 IN 1 10k\n\
               C1 1 GND 100p\n\
               E1 OUT GND 1 GND 100\n\
               .ENDS GENERIC\n"
            .into(),
        default_node_map: Default::default(),
        version: "0.0.1".into(),
        released: false,
        created: now,
        updated: now,
    }
}

fn make_row(index: usize, lib_id: Uuid, sym: Uuid, fpt: Uuid) -> ComponentRow {
    let now = chrono::Utc::now();
    let pn = format!("SNX-IC-{index:05}");
    ComponentRow {
        row_id: Uuid::now_v7(),
        internal_pn: InternalPn::new(&pn),
        class: ComponentClass::new("IC"),
        datasheet: DatasheetRef::default(),
        state: LifecycleState::Draft,
        symbol_ref: PrimitiveRef::new(lib_id, sym),
        footprint_ref: Some(PrimitiveRef::new(lib_id, fpt)),
        sim_ref: None,
        pin_map_overrides: Vec::new(),
        primary_mpn: ManufacturerPart::draft("Acme Semi", &pn),
        alternates: Vec::new(),
        supply: Vec::new(),
        parameters: sample_params(),
        plm: PlmReserved::default(),
        version: "0.0.1".into(),
        released: false,
        symbol_version: "0.0.1".into(),
        footprint_version: "0.0.1".into(),
        sim_version: String::new(),
        created: now,
        updated: now,
        content_hash: [0u8; 32],
    }
}

// ── Generator ────────────────────────────────────────────────────────────

fn manifest(name: &str) -> SnxlibManifest {
    SnxlibManifest {
        format: FORMAT_TOKEN.into(),
        library_id: Uuid::now_v7(),
        library: LibrarySection {
            name: name.into(),
            description: Some("synthetic library for issue #99 timing".into()),
        },
        mode: LibraryMode::default(),
        workflow: WorkflowConfig::default(),
        users: UsersConfig::default(),
        classes: Vec::new(),
    }
}

/// Build a full `.snxlib` under `<root>/<name>/`, returning the path to the
/// `.snxlib` file itself. Verifies the result opens and lists correctly
/// through `LocalGitAdapter` before returning — a subtly invalid library
/// would silently invalidate every number downstream.
pub fn generate_library(root: &Path, name: &str, scale: &Scale) -> Result<PathBuf, LibraryError> {
    let dir = root.join(name);
    let snxlib = dir.join(format!("{name}.snxlib"));

    // Version control off during generation so the per-primitive writes
    // don't each try to commit; `recover_init` lands the whole tree in a
    // single real commit at the end.
    let adapter = LocalGitAdapter::init(
        &snxlib,
        manifest(name),
        LibraryInitOptions {
            enable_git: false,
            use_lfs: false,
        },
    )?;
    let lib_id = adapter.library_id();

    let sym_dir = dir.join("symbols");
    let fpt_dir = dir.join("footprints");
    let sim_dir = dir.join("sims");
    fs::create_dir_all(&sym_dir)?;
    fs::create_dir_all(&fpt_dir)?;
    fs::create_dir_all(&sim_dir)?;

    let mut sym_uuids = Vec::with_capacity(scale.symbols);
    for i in 0..scale.symbols {
        let sym = make_symbol(i);
        sym_uuids.push(sym.uuid);
        let file = SymbolFile::from_symbol(sym);
        let text = file
            .to_toml_string()
            .map_err(|e| LibraryError::Backend(format!("emit .snxsym: {e}")))?;
        fs::write(sym_dir.join(format!("ic-{i:05}-soic16.snxsym")), text)?;
    }

    let mut fpt_uuids = Vec::with_capacity(scale.footprints);
    for i in 0..scale.footprints {
        let fp = make_footprint(i);
        let uuid = fp.uuid;
        fpt_uuids.push(uuid);
        let file = FootprintFile::from_footprint(fp);
        let text = file
            .to_toml_string()
            .map_err(|e| LibraryError::Backend(format!("emit .snxfpt: {e}")))?;
        // `list_primitive_summaries` parses the file stem as a UUID and
        // skips anything else — the name is load-bearing, not cosmetic.
        fs::write(fpt_dir.join(format!("{uuid}.snxfpt")), text)?;
    }

    for i in 0..scale.sims {
        let sm = make_sim(i);
        let uuid = sm.uuid;
        let file = SimFile::from_model(sm);
        let text = file
            .to_toml_string()
            .map_err(|e| LibraryError::Backend(format!("emit .snxsim: {e}")))?;
        fs::write(sim_dir.join(format!("{uuid}.snxsim")), text)?;
    }

    // One component row per symbol — the DBLib model puts components in
    // `[tables.<name>]` inside the `.snxlib`, so this is what
    // `reload_tables` / `read_table` actually chew through.
    for i in 0..scale.symbols {
        let row = make_row(
            i,
            lib_id,
            sym_uuids[i],
            fpt_uuids[i % fpt_uuids.len().max(1)],
        );
        adapter.insert_row("ICs", row, "seed")?;
    }
    drop(adapter);

    // Land the whole working tree as one commit so `.git/` is present and
    // clean — `LocalGitAdapter::open` validates the repo, and the git2 cost
    // of that validation is explicitly in scope for this measurement.
    let adapter = LocalGitAdapter::recover_init(&snxlib)?;
    drop(adapter);

    verify(&snxlib, scale)?;
    Ok(snxlib)
}

/// Append one component — a symbol, a footprint and the row tying them
/// together — to an already-generated library, writing straight to disk
/// through a fresh adapter.
///
/// This is the out-of-band edit: another Signex window, a `git pull`, a
/// colleague's commit. Nothing in the caller's `LibraryState` knows it
/// happened, which is exactly the point — it is what the warm
/// (already-mounted) auto-mount path has to notice.
///
/// `index` must not collide with the indices `generate_library` already
/// wrote, so pass something >= `scale.symbols`. Returns the new row's
/// internal PN so the caller can assert on it by name.
pub fn append_component(snxlib: &Path, index: usize) -> Result<String, LibraryError> {
    let dir = snxlib
        .parent()
        .ok_or_else(|| LibraryError::Backend(format!("{snxlib:?} has no parent dir")))?;
    let adapter = LocalGitAdapter::open(snxlib)?;
    let lib_id = adapter.library_id();

    let sym = make_symbol(index);
    let sym_uuid = sym.uuid;
    let text = SymbolFile::from_symbol(sym)
        .to_toml_string()
        .map_err(|e| LibraryError::Backend(format!("emit .snxsym: {e}")))?;
    // Symbols are not stem-keyed by UUID (see `generate_library`), so a
    // human-readable name is fine — it just must not collide.
    fs::write(
        dir.join("symbols")
            .join(format!("ic-{index:05}-soic16.snxsym")),
        text,
    )?;

    let fp = make_footprint(index);
    let fpt_uuid = fp.uuid;
    let text = FootprintFile::from_footprint(fp)
        .to_toml_string()
        .map_err(|e| LibraryError::Backend(format!("emit .snxfpt: {e}")))?;
    // Footprints ARE stem-keyed by UUID — `list_primitive_summaries`
    // skips any file whose stem doesn't parse.
    fs::write(
        dir.join("footprints").join(format!("{fpt_uuid}.snxfpt")),
        text,
    )?;

    let row = make_row(index, lib_id, sym_uuid, fpt_uuid);
    let pn = row.internal_pn.to_string();
    adapter.insert_row("ICs", row, "out-of-band append")?;
    Ok(pn)
}

/// Fail loudly if the generated library does not open and enumerate
/// exactly what was written.
fn verify(snxlib: &Path, scale: &Scale) -> Result<(), LibraryError> {
    let adapter = LocalGitAdapter::open(snxlib)?;
    let syms = adapter.list_symbols()?;
    let fpts = adapter.list_footprints()?;
    let sims = adapter.list_sims()?;
    assert_eq!(
        syms.len(),
        scale.symbols,
        "symbol count mismatch in {snxlib:?}"
    );
    assert_eq!(
        fpts.len(),
        scale.footprints,
        "footprint count mismatch in {snxlib:?}"
    );
    assert_eq!(sims.len(), scale.sims, "sim count mismatch in {snxlib:?}");
    let tables = adapter.list_tables()?;
    assert_eq!(tables, vec!["ICs".to_string()], "table list mismatch");
    let rows = adapter.read_table("ICs")?;
    assert_eq!(
        rows.len(),
        scale.symbols,
        "row count mismatch in {snxlib:?}"
    );
    // Round-trip one symbol through the real reader so a malformed
    // envelope cannot slip past the summary listing.
    let one = adapter.get_symbol(syms[0].uuid)?;
    assert_eq!(one.pins.len(), PINS_PER_SYMBOL, "symbol pin count mismatch");
    assert!(snxlib.parent().unwrap().join(".git").is_dir(), "no .git/");
    Ok(())
}

// ── File-size reporting ──────────────────────────────────────────────────

pub struct Sizes {
    pub symbol: u64,
    pub footprint: u64,
    pub sim: u64,
    pub snxlib: u64,
    pub total: u64,
}

fn first_file_len(dir: &Path) -> u64 {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)
        .map(|rd| rd.filter_map(Result::ok).map(|e| e.path()).collect())
        .unwrap_or_default();
    entries.sort();
    entries
        .first()
        .and_then(|p| fs::metadata(p).ok())
        .map(|m| m.len())
        .unwrap_or(0)
}

fn dir_total(dir: &Path) -> u64 {
    fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(Result::ok)
                .filter_map(|e| e.metadata().ok())
                .filter(|m| m.is_file())
                .map(|m| m.len())
                .sum()
        })
        .unwrap_or(0)
}

/// Per-file byte weights for the generated primitives in `library_dir`.
pub fn primitive_file_sizes(library_dir: &Path) -> Sizes {
    let snxlib = fs::read_dir(library_dir)
        .map(|rd| {
            rd.filter_map(Result::ok)
                .find(|e| e.path().extension().is_some_and(|x| x == "snxlib"))
                .and_then(|e| e.metadata().ok())
                .map(|m| m.len())
                .unwrap_or(0)
        })
        .unwrap_or(0);
    let sym_dir = library_dir.join("symbols");
    let fpt_dir = library_dir.join("footprints");
    let sim_dir = library_dir.join("sims");
    Sizes {
        symbol: first_file_len(&sym_dir),
        footprint: first_file_len(&fpt_dir),
        sim: first_file_len(&sim_dir),
        snxlib,
        total: snxlib + dir_total(&sym_dir) + dir_total(&fpt_dir) + dir_total(&sim_dir),
    }
}
