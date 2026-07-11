//! Signex native file formats — `.snxsch` (schematic) and `.snxpcb` (PCB).
//!
//! Wire format: TOML envelope + TSV bulk-block pattern (matches
//! `.snxlib` / `.snxsym` / `.snxfpt` from the v0.9 library refactor).
//! The first lines of every file are a TOML manifest (`format`, IDs).
//! For each bulk entity type, a single TOML table emits a `content`
//! key whose value is a literal multi-line TSV string — the first row
//! is the column header, subsequent rows are data, columns are
//! whitespace-separated. Hierarchical or rare-field data (zone
//! polygons, the stackup, custom properties) lives in regular TOML
//! sub-tables alongside the TSV blocks.
//!
//! `.snxprj` (project) is unchanged and uses its own pre-existing
//! format. Stays as-is.
//!
//! These types are the canonical Signex schema. Standard I/O — when it
//! returns via the `signex-standard-import` companion repo (GPL-3.0) —
//! translates to/from these types at the file-format boundary; no
//! Standard-shaped types live in this Apache codebase.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::pcb::{Footprint, PcbBoard, Point as PcbPoint, Zone};
use crate::schematic::SchematicSheet;

mod extras;
mod pcb_rows;
mod sch_rows;
mod tsv;
mod units;

#[cfg(test)]
mod tests;

// Public API re-exports — preserve the `format::…` import paths that
// signex-app / signex-engine / signex-output rely on.
pub use pcb_rows::{PcbFootprintRow, PcbPadRow, PcbTrackRow, PcbViaRow};
pub use sch_rows::{SchComponentRow, SchJunctionRow, SchLabelRow, SchWireRow};
pub use tsv::{parse_tsv_block, write_tsv_block};

// Subtree-internal helpers used by the container types below.
use extras::{
    BoardExtras, FootprintExtras, PadExtras, PcbExtras, PcbExtrasRaw, SchExtrasRaw, SheetExtras,
    SymbolExtras,
};
use pcb_rows::{
    footprint_to_row, pad_to_row, row_to_footprint, row_to_pad, row_to_track, row_to_via,
    track_to_row, via_to_row,
};
use sch_rows::{
    junction_to_row, label_to_row, row_to_junction, row_to_label, row_to_symbol, row_to_wire,
    symbol_to_row, wire_to_row,
};
use tsv::write_tsv_section;

// ---------------------------------------------------------------------------
// Format version tokens
// ---------------------------------------------------------------------------

/// Current `.snxsch` format version. Bumping this is a wire-format
/// break: older Signex versions refuse to open the file.
pub const SNXSCH_FORMAT_V1: &str = "snxsch/1";

/// Current `.snxpcb` format version.
pub const SNXPCB_FORMAT_V1: &str = "snxpcb/1";

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum FormatError {
    #[error("toml serialisation failed: {0}")]
    TomlSerialize(#[from] toml::ser::Error),
    #[error("toml deserialisation failed: {0}")]
    TomlDeserialize(#[from] toml::de::Error),
    #[error("unsupported format version: {found:?}; this build supports {expected:?}")]
    UnsupportedVersion { found: String, expected: String },
    #[error(
        "TSV block {block:?}: header has {expected} columns ({columns:?}), data row {row} has {got}"
    )]
    TsvCellCountMismatch {
        block: String,
        row: usize,
        got: usize,
        expected: usize,
        columns: Vec<String>,
    },
    #[error("TSV block {block:?}: header columns {got:?} do not match expected {expected:?}")]
    TsvHeaderMismatch {
        block: String,
        got: Vec<String>,
        expected: Vec<String>,
    },
    #[error("TSV block {block:?}: row {row} field {field:?} parse error: {message}")]
    TsvFieldParse {
        block: String,
        row: usize,
        field: String,
        message: String,
    },
    #[error("TSV block {block:?}: TSV body is empty — at minimum a header row is required")]
    TsvEmpty { block: String },
}

// ---------------------------------------------------------------------------
// SnxTable trait — manual row schemas (no derive macro for simplicity).
// ---------------------------------------------------------------------------

/// A row schema for TSV bulk blocks.
///
/// Implementors describe the TSV column order, how to render an
/// in-memory row to its column-cell strings, and how to parse a vector
/// of cell `&str` slices back into a row.
pub trait SnxTable: Sized {
    /// Static column ordering — what gets emitted as the TSV header
    /// row and what `from_row` expects to receive back.
    fn columns() -> &'static [&'static str];

    /// Emit one cell per declared column. Length must equal
    /// [`SnxTable::columns`]. Empty cells use `""`.
    fn to_row(&self) -> Vec<String>;

    /// Parse a row of cell strings (length matches [`SnxTable::columns`]).
    /// Implementations report parse failures via [`FormatError::TsvFieldParse`].
    fn from_row(values: &[&str], block: &str, row: usize) -> Result<Self, FormatError>;
}

// ---------------------------------------------------------------------------
// .snxsch — schematic file
// ---------------------------------------------------------------------------

/// On-disk representation of a `.snxsch` file.
///
/// Internally constructed from a [`SchematicSheet`] via
/// [`SnxSchematic::new`] (which decomposes the sheet into bulk TSV
/// rows + an extras-TOML auxiliary table that captures every field
/// the row schema doesn't cover) and rebuilt via [`SnxSchematic::parse`]
/// (which round-trips back to a fully-populated [`SchematicSheet`]).
///
/// Callers that just want the in-memory sheet read `self.sheet`.
#[derive(Debug, Clone)]
pub struct SnxSchematic {
    pub format: String,
    /// The reconstituted in-memory sheet. This is what callers
    /// consume; the TOML+TSV decomposition only matters at the disk
    /// boundary.
    pub sheet: SchematicSheet,
}

impl SnxSchematic {
    /// Wrap a `SchematicSheet` for serialisation as the current format version.
    pub fn new(sheet: SchematicSheet) -> Self {
        Self {
            format: SNXSCH_FORMAT_V1.to_string(),
            sheet,
        }
    }

    /// Serialise to a TOML+TSV string for writing to disk.
    pub fn write_string(&self) -> Result<String, FormatError> {
        let mut out = String::new();

        // Manifest header — emit as a small TOML document.
        let manifest = SchManifest {
            format: self.format.clone(),
            schematic_id: self.sheet.uuid,
            version: self.sheet.version,
            generator: self.sheet.generator.clone(),
            generator_version: self.sheet.generator_version.clone(),
            paper_size: self.sheet.paper_size.clone(),
            root_sheet_page: self.sheet.root_sheet_page.clone(),
        };
        out.push_str(&toml::to_string_pretty(&manifest)?);
        out.push('\n');

        // Bulk TSV blocks. Each is `[sheets.<entity>]` with a single
        // `content` key holding a literal multi-line string.
        let component_rows: Vec<SchComponentRow> =
            self.sheet.symbols.iter().map(symbol_to_row).collect();
        let wire_rows: Vec<SchWireRow> = self.sheet.wires.iter().map(wire_to_row).collect();
        let junction_rows: Vec<SchJunctionRow> =
            self.sheet.junctions.iter().map(junction_to_row).collect();
        let label_rows: Vec<SchLabelRow> = self.sheet.labels.iter().map(label_to_row).collect();

        write_tsv_section(&mut out, "sheets.components", &component_rows);
        write_tsv_section(&mut out, "sheets.wires", &wire_rows);
        write_tsv_section(&mut out, "sheets.junctions", &junction_rows);
        write_tsv_section(&mut out, "sheets.labels", &label_rows);

        // Extras: auxiliary TOML tables for fields the bulk row
        // doesn't carry. Wrap the whole extras tree in a single
        // serializable struct so toml::to_string_pretty emits the
        // correct nested-table headers (`[extras.symbols.<uuid>]`,
        // `[extras.symbols.<uuid>.fields]`, `[extras.sheet]`, …).
        // Hand-rolled per-section serialization breaks here because
        // the inner HashMaps render as their own `[fields]` sub-
        // tables which would attach to the wrong parent path.
        let symbols_extras: BTreeMap<String, SymbolExtras> = self
            .sheet
            .symbols
            .iter()
            .map(|s| (s.uuid.to_string(), SymbolExtras::from_symbol(s)))
            .filter(|(_, e)| !e.is_default())
            .collect();

        let sheet_extras = SheetExtras::from_sheet(&self.sheet);
        let sheet_extras_opt = if sheet_extras.is_default() {
            None
        } else {
            Some(sheet_extras)
        };

        if !symbols_extras.is_empty() || sheet_extras_opt.is_some() {
            #[derive(Serialize)]
            struct ExtrasWrapper {
                extras: ExtrasInner,
            }
            #[derive(Serialize)]
            struct ExtrasInner {
                #[serde(skip_serializing_if = "BTreeMap::is_empty")]
                symbols: BTreeMap<String, SymbolExtras>,
                #[serde(skip_serializing_if = "Option::is_none")]
                sheet: Option<SheetExtras>,
            }
            let body = toml::to_string_pretty(&ExtrasWrapper {
                extras: ExtrasInner {
                    symbols: symbols_extras,
                    sheet: sheet_extras_opt,
                },
            })?;
            out.push('\n');
            out.push_str(&body);
        }

        Ok(out)
    }

    /// Parse a TOML+TSV string from disk.
    pub fn parse(input: &str) -> Result<Self, FormatError> {
        // Stage 1: deserialise the document into the raw envelope —
        // manifest header + bulk blocks + extras.
        let raw: SchRaw = toml::from_str(input)?;

        if raw.format != SNXSCH_FORMAT_V1 {
            return Err(FormatError::UnsupportedVersion {
                found: raw.format,
                expected: SNXSCH_FORMAT_V1.to_string(),
            });
        }

        // Stage 2: parse each TSV block into adapter rows.
        let component_rows = match raw.sheets.components {
            Some(b) => parse_tsv_block::<SchComponentRow>("sheets.components", &b.content)?,
            None => Vec::new(),
        };
        let wire_rows = match raw.sheets.wires {
            Some(b) => parse_tsv_block::<SchWireRow>("sheets.wires", &b.content)?,
            None => Vec::new(),
        };
        let junction_rows = match raw.sheets.junctions {
            Some(b) => parse_tsv_block::<SchJunctionRow>("sheets.junctions", &b.content)?,
            None => Vec::new(),
        };
        let label_rows = match raw.sheets.labels {
            Some(b) => parse_tsv_block::<SchLabelRow>("sheets.labels", &b.content)?,
            None => Vec::new(),
        };

        // Stage 3: rebuild the SchematicSheet from rows + extras.
        let extras = raw.extras.unwrap_or_default();
        let sheet_extras = extras.sheet.unwrap_or_default();
        let symbols = component_rows
            .into_iter()
            .map(|row| {
                let key = row.uuid.to_string();
                let extra = extras.symbols.get(&key).cloned().unwrap_or_default();
                row_to_symbol(row, extra)
            })
            .collect();
        let wires = wire_rows.into_iter().map(row_to_wire).collect();
        let junctions = junction_rows.into_iter().map(row_to_junction).collect();
        let labels = label_rows.into_iter().map(row_to_label).collect();

        let sheet = SchematicSheet {
            uuid: raw.schematic_id,
            version: raw.version,
            generator: raw.generator,
            generator_version: raw.generator_version,
            paper_size: raw.paper_size,
            root_sheet_page: raw.root_sheet_page,
            symbols,
            wires,
            junctions,
            labels,
            child_sheets: sheet_extras.child_sheets,
            no_connects: sheet_extras.no_connects,
            text_notes: sheet_extras.text_notes,
            buses: sheet_extras.buses,
            bus_entries: sheet_extras.bus_entries,
            drawings: sheet_extras.drawings,
            no_erc_directives: sheet_extras.no_erc_directives,
            title_block: sheet_extras.title_block,
            lib_symbols: sheet_extras.lib_symbols,
        };

        Ok(SnxSchematic {
            format: raw.format,
            sheet,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SchManifest {
    format: String,
    schematic_id: Uuid,
    #[serde(default)]
    version: u32,
    #[serde(default)]
    generator: String,
    #[serde(default)]
    generator_version: String,
    #[serde(default)]
    paper_size: String,
    #[serde(default)]
    root_sheet_page: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct TsvBody {
    content: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct SchSheetsRaw {
    #[serde(default)]
    components: Option<TsvBody>,
    #[serde(default)]
    wires: Option<TsvBody>,
    #[serde(default)]
    junctions: Option<TsvBody>,
    #[serde(default)]
    labels: Option<TsvBody>,
}

#[derive(Debug, Clone, Deserialize)]
struct SchRaw {
    format: String,
    schematic_id: Uuid,
    #[serde(default)]
    version: u32,
    #[serde(default)]
    generator: String,
    #[serde(default)]
    generator_version: String,
    #[serde(default)]
    paper_size: String,
    #[serde(default)]
    root_sheet_page: String,
    #[serde(default)]
    sheets: SchSheetsRaw,
    #[serde(default)]
    extras: Option<SchExtrasRaw>,
}

// ---------------------------------------------------------------------------
// .snxpcb — PCB file
// ---------------------------------------------------------------------------

/// On-disk representation of a `.snxpcb` file.
///
/// Same shape as [`SnxSchematic`]: TOML manifest at the top, bulk
/// TSV blocks for footprints / pads / tracks / vias, regular TOML
/// for hierarchical or rare-field data (zone polygons, the stackup,
/// custom properties).
#[derive(Debug, Clone)]
pub struct SnxPcb {
    pub format: String,
    pub board: PcbBoard,
}

impl SnxPcb {
    /// Wrap a `PcbBoard` for serialisation as the current format version.
    pub fn new(board: PcbBoard) -> Self {
        Self {
            format: SNXPCB_FORMAT_V1.to_string(),
            board,
        }
    }

    /// Serialise to a TOML+TSV string for writing to disk.
    pub fn write_string(&self) -> Result<String, FormatError> {
        let mut out = String::new();

        // Manifest header.
        let manifest = PcbManifest {
            format: self.format.clone(),
            pcb_id: self.board.uuid,
            version: self.board.version,
            generator: self.board.generator.clone(),
            thickness: self.board.thickness,
        };
        out.push_str(&toml::to_string_pretty(&manifest)?);
        out.push('\n');

        // Stackup + setup + nets — emitted as one nested wrapper so
        // toml's serializer produces the correct `[stackup]`/`[nets]`
        // headers and any inner sub-tables (PcbSetup) attach to the
        // right parent path.
        if !self.board.layers.is_empty() || !self.board.nets.is_empty() {
            #[derive(Serialize)]
            struct StackupWrapper<'a> {
                #[serde(skip_serializing_if = "Option::is_none")]
                stackup: Option<StackBlock<'a>>,
                #[serde(skip_serializing_if = "Option::is_none")]
                nets: Option<NetsBlock<'a>>,
            }
            #[derive(Serialize)]
            struct StackBlock<'a> {
                layers: Vec<String>,
                #[serde(skip_serializing_if = "Option::is_none")]
                setup: Option<&'a crate::pcb::PcbSetup>,
            }
            #[derive(Serialize)]
            struct NetsBlock<'a> {
                entries: &'a [crate::pcb::NetDef],
            }
            let stackup = if self.board.layers.is_empty() {
                None
            } else {
                Some(StackBlock {
                    layers: self.board.layers.iter().map(|l| l.name.clone()).collect(),
                    setup: self.board.setup.as_ref(),
                })
            };
            let nets = if self.board.nets.is_empty() {
                None
            } else {
                Some(NetsBlock {
                    entries: &self.board.nets,
                })
            };
            out.push('\n');
            out.push_str(&toml::to_string_pretty(&StackupWrapper { stackup, nets })?);
        }

        // TSV blocks: footprints, pads, tracks, vias.
        let footprint_rows: Vec<PcbFootprintRow> =
            self.board.footprints.iter().map(footprint_to_row).collect();
        let mut pad_rows: Vec<PcbPadRow> = Vec::new();
        for fp in &self.board.footprints {
            for pad in &fp.pads {
                pad_rows.push(pad_to_row(pad, &fp.reference));
            }
        }
        let track_rows: Vec<PcbTrackRow> = self.board.segments.iter().map(track_to_row).collect();
        let via_rows: Vec<PcbViaRow> = self.board.vias.iter().map(via_to_row).collect();

        write_tsv_section(&mut out, "footprints", &footprint_rows);
        write_tsv_section(&mut out, "pads", &pad_rows);
        write_tsv_section(&mut out, "tracks", &track_rows);
        write_tsv_section(&mut out, "vias", &via_rows);

        // Zones — full struct array (each zone serialises naturally
        // under `[[zones]]` with its own uuid scalar).
        if !self.board.zones.is_empty() {
            #[derive(Serialize)]
            struct ZonesWrapper<'a> {
                zones: &'a [Zone],
            }
            out.push('\n');
            out.push_str(&toml::to_string_pretty(&ZonesWrapper {
                zones: &self.board.zones,
            })?);
        }

        // Footprint extras / pad extras / board extras — wrap in a
        // single `[extras]` tree so toml's serializer produces the
        // correct nested-table headers.
        let extras = PcbExtras::from_board(&self.board);
        let footprints = extras.footprints;
        let pads = extras.pads;
        let board_extras =
            if extras.outline.is_empty() && extras.graphics.is_empty() && extras.texts.is_empty() {
                None
            } else {
                Some(BoardExtras {
                    outline: extras.outline,
                    graphics: extras.graphics,
                    texts: extras.texts,
                })
            };
        if !footprints.is_empty() || !pads.is_empty() || board_extras.is_some() {
            #[derive(Serialize)]
            struct ExtrasWrapper {
                extras: ExtrasInner,
            }
            #[derive(Serialize)]
            struct ExtrasInner {
                #[serde(skip_serializing_if = "BTreeMap::is_empty")]
                footprints: BTreeMap<String, FootprintExtras>,
                #[serde(skip_serializing_if = "BTreeMap::is_empty")]
                pads: BTreeMap<String, PadExtras>,
                #[serde(skip_serializing_if = "Option::is_none")]
                board: Option<BoardExtras>,
            }
            out.push('\n');
            out.push_str(&toml::to_string_pretty(&ExtrasWrapper {
                extras: ExtrasInner {
                    footprints,
                    pads,
                    board: board_extras,
                },
            })?);
        }

        Ok(out)
    }

    /// Parse a TOML+TSV string from disk.
    pub fn parse(input: &str) -> Result<Self, FormatError> {
        let raw: PcbRaw = toml::from_str(input)?;

        if raw.format != SNXPCB_FORMAT_V1 {
            return Err(FormatError::UnsupportedVersion {
                found: raw.format,
                expected: SNXPCB_FORMAT_V1.to_string(),
            });
        }

        let footprint_rows = match raw.footprints {
            Some(b) => parse_tsv_block::<PcbFootprintRow>("footprints", &b.content)?,
            None => Vec::new(),
        };
        let pad_rows = match raw.pads {
            Some(b) => parse_tsv_block::<PcbPadRow>("pads", &b.content)?,
            None => Vec::new(),
        };
        let track_rows = match raw.tracks {
            Some(b) => parse_tsv_block::<PcbTrackRow>("tracks", &b.content)?,
            None => Vec::new(),
        };
        let via_rows = match raw.vias {
            Some(b) => parse_tsv_block::<PcbViaRow>("vias", &b.content)?,
            None => Vec::new(),
        };

        // Reconstruct footprints with their pads. Group pads by ref.
        let extras = raw.extras.unwrap_or_default();
        let board_extras = extras.board.unwrap_or_default();

        let mut footprints: Vec<Footprint> = footprint_rows
            .into_iter()
            .map(|row| {
                let key = row.uuid.to_string();
                let extra = extras.footprints.get(&key).cloned().unwrap_or_default();
                row_to_footprint(row, extra)
            })
            .collect();

        for prow in pad_rows {
            let extra = extras
                .pads
                .get(&prow.uuid.to_string())
                .cloned()
                .unwrap_or_default();
            let pad = row_to_pad(prow.clone(), extra);
            // attach to footprint by ref
            if let Some(fp) = footprints
                .iter_mut()
                .find(|f| f.reference == prow.footprint_ref)
            {
                fp.pads.push(pad);
            } else {
                // Orphan pad — preserve as a synthetic footprint
                // entry to avoid silent data loss.
                footprints.push(Footprint {
                    uuid: Uuid::nil(),
                    reference: prow.footprint_ref.clone(),
                    value: String::new(),
                    footprint_id: String::new(),
                    position: PcbPoint { x: 0.0, y: 0.0 },
                    rotation: 0.0,
                    layer: String::new(),
                    locked: false,
                    pads: vec![pad],
                    graphics: Vec::new(),
                    properties: Vec::new(),
                });
            }
        }

        let segments = track_rows.into_iter().map(row_to_track).collect();
        let vias = via_rows.into_iter().map(row_to_via).collect();

        // Reconstitute zones from `[[zones]]` array.
        let zones = raw.zones.unwrap_or_default();

        // Stackup → layers list (rebuild LayerDef with synthetic ids).
        let (layers, setup) = match raw.stackup {
            Some(s) => {
                let layers: Vec<crate::pcb::LayerDef> = s
                    .layers
                    .into_iter()
                    .enumerate()
                    .map(|(i, name)| crate::pcb::LayerDef {
                        id: i as u8,
                        name,
                        layer_type: String::new(),
                    })
                    .collect();
                (layers, s.setup)
            }
            None => (Vec::new(), None),
        };

        let board = PcbBoard {
            uuid: raw.pcb_id,
            version: raw.version,
            generator: raw.generator,
            thickness: raw.thickness,
            outline: board_extras.outline,
            layers,
            setup,
            nets: raw.nets.map(|n| n.entries).unwrap_or_default(),
            footprints,
            segments,
            vias,
            zones,
            graphics: board_extras.graphics,
            texts: board_extras.texts,
        };

        Ok(SnxPcb {
            format: raw.format,
            board,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PcbManifest {
    format: String,
    pcb_id: Uuid,
    #[serde(default)]
    version: u32,
    #[serde(default)]
    generator: String,
    #[serde(default)]
    thickness: f64,
}

#[derive(Debug, Clone, Deserialize)]
struct PcbRaw {
    format: String,
    pcb_id: Uuid,
    #[serde(default)]
    version: u32,
    #[serde(default)]
    generator: String,
    #[serde(default)]
    thickness: f64,
    #[serde(default)]
    stackup: Option<StackRaw>,
    #[serde(default)]
    nets: Option<NetsRaw>,
    #[serde(default)]
    footprints: Option<TsvBody>,
    #[serde(default)]
    pads: Option<TsvBody>,
    #[serde(default)]
    tracks: Option<TsvBody>,
    #[serde(default)]
    vias: Option<TsvBody>,
    #[serde(default)]
    zones: Option<Vec<Zone>>,
    #[serde(default)]
    extras: Option<PcbExtrasRaw>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct StackRaw {
    #[serde(default)]
    layers: Vec<String>,
    #[serde(default)]
    setup: Option<crate::pcb::PcbSetup>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct NetsRaw {
    #[serde(default)]
    entries: Vec<crate::pcb::NetDef>,
}
