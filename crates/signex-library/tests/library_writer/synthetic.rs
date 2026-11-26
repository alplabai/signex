//! A deterministic synthetic library of any size: fixed ids and times, so
//! two builds give the same bytes.

use std::collections::BTreeMap;

use chrono::{DateTime, TimeZone, Utc};
use uuid::Uuid;

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

pub const LIBRARY_ID: Uuid = Uuid::from_u128(0x0194_0000_0000_7000_8000_0000_0000_0001);
const SYMBOLS_PER_ENVELOPE: usize = 10;
const TABLES: [&str; 5] = ["capacitors", "connectors", "ics", "inductors", "resistors"];

fn id(kind: u128, n: usize) -> Uuid {
    Uuid::from_u128(0x0194_0000_0000_7000_8000_0000_0000_0000 | (kind << 40) | n as u128)
}

pub fn row_id(n: usize) -> Uuid {
    id(4, n)
}

pub fn time() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 1, 2, 3, 4, 5).unwrap()
}

pub fn manifest(name: &str) -> SnxlibManifest {
    SnxlibManifest {
        format: FORMAT_TOKEN.into(),
        library_id: LIBRARY_ID,
        library: LibrarySection {
            name: name.into(),
            description: Some("Synthetic library".into()),
            ..Default::default()
        },
        mode: LibraryMode::LocalGit,
        workflow: WorkflowConfig::default(),
        users: UsersConfig {
            default_role: "Designer".into(),
            ..UsersConfig::default()
        },
        classes: Vec::new(),
    }
}

pub fn lib_ref(uuid: Uuid) -> PrimitiveRef {
    PrimitiveRef::new(LIBRARY_ID, uuid)
}

pub fn symbol(n: usize) -> Symbol {
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
    Symbol {
        uuid: id(1, n),
        pins,
        created: time(),
        updated: time(),
        version: "1.0.0".into(),
        ..Symbol::empty(format!("SYM-{n}"))
    }
}

pub fn footprint(n: usize) -> FootprintFile {
    let mut fp = Footprint::with_identity(format!("FP-{n}"), id(2, n), time());
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

pub fn row(n: usize, symbols: usize, footprints: usize) -> ComponentRow {
    let mut parameters = ParamMap::new();
    parameters.insert(
        "resistance".into(),
        ParamValue::Measurement {
            value: 10.0 * (n % 1000 + 1) as f64,
            unit: "ohm".into(),
        },
    );
    parameters.insert("tolerance".into(), ParamValue::Text("1%".into()));
    ComponentRow {
        row_id: row_id(n),
        internal_pn: InternalPn::new(format!("PN-{n:06}")),
        class: ComponentClass::new("part"),
        datasheet: DatasheetRef::url(format!("https://example.com/ds/{n}.pdf")),
        datasheet_label: String::new(),
        description: format!("Synthetic part {n}"),
        more_datasheets: Vec::new(),
        state: LifecycleState::Released,
        symbol_ref: lib_ref(id(1, n % symbols)),
        alternate_symbols: Vec::new(),
        footprint_ref: Some(lib_ref(id(2, n % footprints))),
        alternate_footprints: Vec::new(),
        sim_ref: None,
        pin_map_overrides: Vec::new(),
        primary_mpn: ManufacturerPart::draft("Example Corp", format!("EX-{n:06}")),
        alternates: Vec::new(),
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

/// `rows` part rows over five tables, one symbol per 50 rows in envelopes
/// of ten, one footprint per 10 rows.
pub fn library(rows: usize) -> LibraryWriter {
    let symbols = (rows / 50).max(1);
    let footprints = (rows / 10).max(1);
    let mut w = LibraryWriter::new("Synthetic", manifest("Synthetic"));
    let mut envelopes: BTreeMap<usize, Vec<Symbol>> = BTreeMap::new();
    for n in 0..symbols {
        envelopes
            .entry(n / SYMBOLS_PER_ENVELOPE)
            .or_default()
            .push(symbol(n));
    }
    for (e, symbols) in envelopes {
        w.add_symbol_file(envelope(e, symbols));
    }
    for n in 0..footprints {
        w.add_footprint_file(footprint(n)).unwrap();
    }
    for n in 0..rows {
        w.add_row(table_of(n), row(n, symbols, footprints));
    }
    w
}

pub fn envelope(e: usize, symbols: Vec<Symbol>) -> SymbolFile {
    SymbolFile {
        format: String::new(),
        file_uuid: id(5, e),
        display_name: format!("Symbols {e}"),
        copied_from: None,
        symbols,
        created: time(),
        updated: time(),
    }
}

pub fn table_of(n: usize) -> &'static str {
    TABLES[n % TABLES.len()]
}
