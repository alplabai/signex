//! The `[tables.components]` rows of `Stock.snxlib`: one placeable
//! component per stock symbol that is not a power port.
//!
//! Power ports (`GND`, `VCC`, `+3V3`, `+5V`) are placed by the Place
//! Power tool as power-port graphics, not as library components, so they
//! get no row. The mounting hole and the fiducial have no symbol, and a
//! row needs one, so they get none either.
//!
//! Every identity is pinned: the row ids below are minted once and never
//! regenerated, exactly like the footprint and symbol ids in [`super::ids`],
//! so a rerun writes identical bytes. A pair is only emitted when the
//! symbol's pin numbers equal the footprint's pad numbers, which keeps the
//! default 1:1 pin map correct without `pin_map_overrides`.

use std::collections::BTreeSet;
use std::error::Error;

use signex_library::component::{ComponentRow, DatasheetRef, PlmReserved};
use signex_library::identity::{ComponentClass, InternalPn};
use signex_library::lifecycle::LifecycleState;
use signex_library::manufacturer::ManufacturerPart;
use signex_library::primitive::PrimitiveRef;
use signex_sketch::entity::EntityKind;
use uuid::Uuid;

use super::ids::{STOCK_IDS, STOCK_LIBRARY_ID, StockIds, StockKind, symbol_ids};
use super::{build, stock_timestamp, symbols};

/// Revision every stock primitive and row ships at.
const STOCK_VERSION: &str = "0.0.1";

/// One pinned component row: which symbol it places, its class, and the
/// footprint it pairs with (`None` = symbol only; the user binds a package).
struct StockRow {
    symbol: &'static str,
    row_id: &'static str,
    class: &'static str,
    footprint: Option<StockKind>,
}

/// Rows in panel order. The `R0805` chip footprint is the generic
/// two-terminal 0805 land pattern, shared by every two-pin passive and
/// discrete here; `SOIC-8` pairs with the 8-pin generic IC.
const STOCK_ROWS: &[StockRow] = &[
    StockRow {
        symbol: "R",
        row_id: "01a18461-b555-75c3-995a-0a8a51e590a4",
        class: "resistor",
        footprint: Some(StockKind::R0805),
    },
    StockRow {
        symbol: "C",
        row_id: "01a18461-b55e-7523-b9c9-d63b68daae2d",
        class: "capacitor",
        footprint: Some(StockKind::R0805),
    },
    StockRow {
        symbol: "C_Polarized",
        row_id: "01a18461-b561-7339-8ca8-366a1e75f029",
        class: "capacitor",
        footprint: None,
    },
    StockRow {
        symbol: "L",
        row_id: "01a18461-b564-7742-8d1f-5970e27e2659",
        class: "inductor",
        footprint: Some(StockKind::R0805),
    },
    StockRow {
        symbol: "Ferrite",
        row_id: "01a18461-b567-7294-9054-dcaef701f93b",
        class: "inductor",
        footprint: Some(StockKind::R0805),
    },
    StockRow {
        symbol: "Fuse",
        row_id: "01a18461-b56a-71bb-9548-4f548c72ca99",
        class: "generic",
        footprint: Some(StockKind::R0805),
    },
    StockRow {
        symbol: "D",
        row_id: "01a18461-b56d-76ce-96b5-09c429eacb1e",
        class: "diode",
        footprint: Some(StockKind::R0805),
    },
    StockRow {
        symbol: "D_Schottky",
        row_id: "01a18461-b570-7585-ac70-cf199f9c6e3e",
        class: "diode",
        footprint: Some(StockKind::R0805),
    },
    StockRow {
        symbol: "D_Zener",
        row_id: "01a18461-b573-721f-92d5-ee2658a6d058",
        class: "diode",
        footprint: Some(StockKind::R0805),
    },
    StockRow {
        symbol: "LED",
        row_id: "01a18461-b577-73e1-8726-372141f949af",
        class: "led",
        footprint: Some(StockKind::R0805),
    },
    StockRow {
        symbol: "Q_NPN",
        row_id: "01a18461-b57a-7507-a154-3cd59262073f",
        class: "transistor",
        footprint: None,
    },
    StockRow {
        symbol: "Q_PNP",
        row_id: "01a18461-b57d-77f5-b360-9710bf084215",
        class: "transistor",
        footprint: None,
    },
    StockRow {
        symbol: "Q_NMOS",
        row_id: "01a18461-b580-72ec-a9cd-54fc454f7377",
        class: "transistor",
        footprint: None,
    },
    StockRow {
        symbol: "Q_PMOS",
        row_id: "01a18461-b583-7754-a605-75b147818921",
        class: "transistor",
        footprint: None,
    },
    StockRow {
        symbol: "Crystal",
        row_id: "01a18461-b586-751d-83ae-eb3cb7271b1e",
        class: "crystal",
        footprint: None,
    },
    StockRow {
        symbol: "IC_Generic",
        row_id: "01a18461-b58a-7109-98de-3517476e83d8",
        class: "ic",
        footprint: Some(StockKind::Soic8),
    },
    StockRow {
        symbol: "Conn_1x02",
        row_id: "01a18461-b58d-75dd-a724-5eb3dc524249",
        class: "connector",
        footprint: None,
    },
    StockRow {
        symbol: "Conn_1x03",
        row_id: "01a18461-b590-74fe-9267-fda68cbd460e",
        class: "connector",
        footprint: None,
    },
    StockRow {
        symbol: "Conn_1x04",
        row_id: "01a18461-b593-722e-b618-2aea834369a8",
        class: "connector",
        footprint: None,
    },
    StockRow {
        symbol: "Conn_1x05",
        row_id: "01a18461-b596-77b5-9358-f9dbd478797f",
        class: "connector",
        footprint: None,
    },
    StockRow {
        symbol: "Conn_1x06",
        row_id: "01a18461-b599-7183-bd6d-7cb2743c2871",
        class: "connector",
        footprint: None,
    },
    StockRow {
        symbol: "Conn_1x08",
        row_id: "01a18461-b59c-70cb-a1e1-c1163bafd166",
        class: "connector",
        footprint: None,
    },
    StockRow {
        symbol: "Conn_1x10",
        row_id: "01a18461-b59f-72f1-ad3b-1b1b4ad7d658",
        class: "connector",
        footprint: None,
    },
    StockRow {
        symbol: "Conn_2x05",
        row_id: "01a18461-b5a3-7719-9837-d621a7430117",
        class: "connector",
        footprint: None,
    },
    StockRow {
        symbol: "Conn_2x10",
        row_id: "01a18461-b5a6-7206-8fb7-3b4dfb26b5f6",
        class: "connector",
        footprint: None,
    },
    StockRow {
        symbol: "Screw_1x02",
        row_id: "01a18461-b5a9-71cc-a68c-d4a504c446d5",
        class: "connector",
        footprint: None,
    },
    StockRow {
        symbol: "Screw_1x03",
        row_id: "01a18461-b5ac-7226-8212-40535e0ceeb2",
        class: "connector",
        footprint: None,
    },
    StockRow {
        symbol: "74HC00",
        row_id: "01a18461-b5af-7262-b21e-aa1a5a4079de",
        class: "ic",
        footprint: None,
    },
    StockRow {
        symbol: "74HC02",
        row_id: "01a18461-b5b2-723c-9f23-77e1bed55ae6",
        class: "ic",
        footprint: None,
    },
    StockRow {
        symbol: "74HC04",
        row_id: "01a18461-b5b5-73ea-b833-411a906f34f3",
        class: "ic",
        footprint: None,
    },
    StockRow {
        symbol: "74HC08",
        row_id: "01a18461-b5b8-766c-8e63-40d3df715155",
        class: "ic",
        footprint: None,
    },
    StockRow {
        symbol: "74HC14",
        row_id: "01a18461-b5bb-7137-863b-5492b0a1fe63",
        class: "ic",
        footprint: None,
    },
    StockRow {
        symbol: "74HC32",
        row_id: "01a18461-b5be-752d-9461-dc77c0d1ddaa",
        class: "ic",
        footprint: None,
    },
    StockRow {
        symbol: "74HC86",
        row_id: "01a18461-b5c2-7105-9182-ccd452248d54",
        class: "ic",
        footprint: None,
    },
];

/// The pinned id row of a footprint kind.
fn ids_for(kind: StockKind) -> &'static StockIds {
    STOCK_IDS
        .iter()
        .find(|row| row.kind as u8 == kind as u8)
        .expect("every footprint kind has an id row")
}

/// Pad numbers a footprint authors in its sketch.
fn pad_numbers(kind: StockKind) -> BTreeSet<String> {
    build(ids_for(kind))
        .sketch
        .iter()
        .flat_map(|sketch| &sketch.entities)
        .filter(|e| matches!(e.kind, EntityKind::Point { .. }))
        .filter_map(|e| e.pad.as_ref().map(|pad| pad.number.clone()))
        .collect()
}

/// Build every stock component row with its content hash computed by the
/// library's own hashing code. Fails when a pair's pin numbers and pad
/// numbers disagree, so a bad pairing cannot be committed.
pub fn stock_rows() -> Result<Vec<ComponentRow>, Box<dyn Error>> {
    let library_id = Uuid::parse_str(STOCK_LIBRARY_ID)?;
    let stamp = stock_timestamp();
    let symbols = symbols::build_all();
    let mut rows = Vec::with_capacity(STOCK_ROWS.len());
    for spec in STOCK_ROWS {
        let symbol = symbols
            .iter()
            .find(|s| s.name == spec.symbol)
            .ok_or_else(|| format!("stock row {} names no stock symbol", spec.symbol))?;
        let mut description = symbol.description.clone();
        let mut footprint_ref = None;
        let mut footprint_version = String::new();
        if let Some(kind) = spec.footprint {
            let ids = ids_for(kind);
            let pins: BTreeSet<String> = symbol.pins.iter().map(|p| p.number.clone()).collect();
            let pads = pad_numbers(kind);
            if pins != pads {
                return Err(format!(
                    "stock row {}: symbol pins {pins:?} do not match footprint pads {pads:?}",
                    spec.symbol
                )
                .into());
            }
            description.push_str(&format!(" {} footprint.", build(ids).name));
            footprint_ref = Some(PrimitiveRef::new(library_id, ids.footprint_uuid()));
            footprint_version = STOCK_VERSION.to_string();
        }
        let mut row = ComponentRow {
            row_id: Uuid::parse_str(spec.row_id)?,
            internal_pn: InternalPn::new(format!("STOCK-{}", spec.symbol)),
            class: ComponentClass::new(spec.class),
            datasheet: DatasheetRef::default(),
            datasheet_label: String::new(),
            description,
            more_datasheets: Vec::new(),
            state: LifecycleState::Released,
            symbol_ref: PrimitiveRef::new(library_id, symbol_ids(spec.symbol).1),
            alternate_symbols: Vec::new(),
            footprint_ref,
            sim_ref: None,
            pin_map_overrides: Vec::new(),
            primary_mpn: ManufacturerPart::draft("", ""),
            alternates: Vec::new(),
            supply: Vec::new(),
            parameters: Default::default(),
            plm: PlmReserved::default(),
            version: STOCK_VERSION.to_string(),
            released: true,
            symbol_version: STOCK_VERSION.to_string(),
            footprint_version,
            sim_version: String::new(),
            created: stamp,
            updated: stamp,
            content_hash: [0; 32],
            server_revision: None,
        };
        row.refresh_content_hash()?;
        rows.push(row);
    }
    Ok(rows)
}
