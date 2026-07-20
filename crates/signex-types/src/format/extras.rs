//! Auxiliary "extras" sub-tables — the per-symbol / per-sheet and
//! per-footprint / per-pad / per-board fields that don't fit a flat
//! TSV row, plus the raw deserialization envelopes for the `[extras]`
//! TOML tree.
//!
//! Pure code motion out of `mod.rs`. Every type, field, and method is
//! `pub(in crate::format)` — visible to the whole `format` module tree
//! exactly as when it was module-private in the single file — so the
//! container types in `mod.rs` and the row-translation helpers in
//! `sch_rows` / `pcb_rows` can reach them. The `#[serde(...)]`
//! attributes and the `default_*` functions are unchanged (they define
//! the on-disk defaults; a changed default silently rewrites data).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::pcb::{Footprint, Pad, PcbBoard, Point as PcbPoint};
use crate::schematic::{SchematicSheet, Symbol};

#[derive(Debug, Clone, Default, Deserialize)]
pub(in crate::format) struct SchExtrasRaw {
    #[serde(default)]
    pub(in crate::format) symbols: BTreeMap<String, SymbolExtras>,
    #[serde(default)]
    pub(in crate::format) sheet: Option<SheetExtras>,
}

/// Per-symbol auxiliary fields that don't fit into [`SchComponentRow`].
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(in crate::format) struct SymbolExtras {
    #[serde(default)]
    pub(in crate::format) footprint: String,
    #[serde(default)]
    pub(in crate::format) datasheet: String,
    #[serde(default)]
    pub(in crate::format) mirror_x: bool,
    #[serde(default)]
    pub(in crate::format) mirror_y: bool,
    #[serde(default = "default_unit")]
    pub(in crate::format) unit: u32,
    #[serde(default)]
    pub(in crate::format) is_power: bool,
    #[serde(default)]
    pub(in crate::format) fields_autoplaced: bool,
    #[serde(default)]
    pub(in crate::format) fields_user_placed: bool,
    #[serde(default)]
    pub(in crate::format) dnp: bool,
    #[serde(default = "default_true")]
    pub(in crate::format) in_bom: bool,
    #[serde(default = "default_true")]
    pub(in crate::format) on_board: bool,
    #[serde(default)]
    pub(in crate::format) exclude_from_sim: bool,
    #[serde(default)]
    pub(in crate::format) locked: bool,
    #[serde(default)]
    pub(in crate::format) fields: BTreeMap<String, String>,
    #[serde(default)]
    pub(in crate::format) custom_properties: Vec<crate::property::SchematicProperty>,
    #[serde(default)]
    pub(in crate::format) pin_uuids: BTreeMap<String, Uuid>,
    #[serde(default)]
    pub(in crate::format) instances: Vec<crate::schematic::SymbolInstance>,
    #[serde(default)]
    pub(in crate::format) ref_text: Option<crate::schematic::TextProp>,
    #[serde(default)]
    pub(in crate::format) val_text: Option<crate::schematic::TextProp>,
}

impl SymbolExtras {
    pub(in crate::format) fn is_default(&self) -> bool {
        self.footprint.is_empty()
            && self.datasheet.is_empty()
            && !self.mirror_x
            && !self.mirror_y
            && self.unit == 1
            && !self.is_power
            && !self.fields_autoplaced
            && !self.fields_user_placed
            && !self.dnp
            && self.in_bom
            && self.on_board
            && !self.exclude_from_sim
            && !self.locked
            && self.fields.is_empty()
            && self.custom_properties.is_empty()
            && self.pin_uuids.is_empty()
            && self.instances.is_empty()
            && self.ref_text.is_none()
            && self.val_text.is_none()
    }

    pub(in crate::format) fn from_symbol(s: &Symbol) -> Self {
        SymbolExtras {
            footprint: s.footprint.clone(),
            datasheet: s.datasheet.clone(),
            mirror_x: s.mirror_x,
            mirror_y: s.mirror_y,
            unit: s.unit,
            is_power: s.is_power,
            fields_autoplaced: s.fields_autoplaced,
            fields_user_placed: s.fields_user_placed,
            dnp: s.dnp,
            in_bom: s.in_bom,
            on_board: s.on_board,
            exclude_from_sim: s.exclude_from_sim,
            locked: s.locked,
            fields: s
                .fields
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect(),
            custom_properties: s.custom_properties.clone(),
            pin_uuids: s
                .pin_uuids
                .iter()
                .map(|(key, value)| (key.clone(), *value))
                .collect(),
            instances: s.instances.clone(),
            ref_text: s.ref_text.clone(),
            val_text: s.val_text.clone(),
        }
    }
}

/// Fields on [`SchematicSheet`] that aren't yet TSV-tabularised
/// (rare in real designs, hierarchical or schema-rich): hierarchical
/// child sheets, no-connect markers, text notes, buses, bus entries,
/// drawing primitives, no-ERC directives, title block, and library
/// symbol cache.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(in crate::format) struct SheetExtras {
    #[serde(default)]
    pub(in crate::format) child_sheets: Vec<crate::schematic::ChildSheet>,
    #[serde(default)]
    pub(in crate::format) no_connects: Vec<crate::schematic::NoConnect>,
    #[serde(default)]
    pub(in crate::format) text_notes: Vec<crate::schematic::TextNote>,
    #[serde(default)]
    pub(in crate::format) buses: Vec<crate::schematic::Bus>,
    #[serde(default)]
    pub(in crate::format) bus_entries: Vec<crate::schematic::BusEntry>,
    #[serde(default)]
    pub(in crate::format) drawings: Vec<crate::schematic::SchDrawing>,
    #[serde(default)]
    pub(in crate::format) no_erc_directives: Vec<crate::schematic::NoConnect>,
    #[serde(default)]
    pub(in crate::format) title_block: BTreeMap<String, String>,
    #[serde(default)]
    pub(in crate::format) lib_symbols: BTreeMap<String, crate::schematic::LibSymbol>,
}

impl SheetExtras {
    pub(in crate::format) fn is_default(&self) -> bool {
        self.child_sheets.is_empty()
            && self.no_connects.is_empty()
            && self.text_notes.is_empty()
            && self.buses.is_empty()
            && self.bus_entries.is_empty()
            && self.drawings.is_empty()
            && self.no_erc_directives.is_empty()
            && self.title_block.is_empty()
            && self.lib_symbols.is_empty()
    }

    pub(in crate::format) fn from_sheet(s: &SchematicSheet) -> Self {
        SheetExtras {
            child_sheets: s.child_sheets.clone(),
            no_connects: s.no_connects.clone(),
            text_notes: s.text_notes.clone(),
            buses: s.buses.clone(),
            bus_entries: s.bus_entries.clone(),
            drawings: s.drawings.clone(),
            no_erc_directives: s.no_erc_directives.clone(),
            title_block: s
                .title_block
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect(),
            lib_symbols: s
                .lib_symbols
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect(),
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_unit() -> u32 {
    1
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(in crate::format) struct PcbExtrasRaw {
    #[serde(default)]
    pub(in crate::format) footprints: BTreeMap<String, FootprintExtras>,
    #[serde(default)]
    pub(in crate::format) pads: BTreeMap<String, PadExtras>,
    #[serde(default)]
    pub(in crate::format) board: Option<BoardExtras>,
}

pub(in crate::format) struct PcbExtras {
    pub(in crate::format) footprints: BTreeMap<String, FootprintExtras>,
    pub(in crate::format) pads: BTreeMap<String, PadExtras>,
    pub(in crate::format) outline: Vec<PcbPoint>,
    pub(in crate::format) graphics: Vec<crate::pcb::BoardGraphic>,
    pub(in crate::format) texts: Vec<crate::pcb::BoardText>,
}

impl PcbExtras {
    pub(in crate::format) fn from_board(board: &PcbBoard) -> Self {
        let mut footprints = BTreeMap::new();
        let mut pads = BTreeMap::new();
        for fp in &board.footprints {
            let fpe = FootprintExtras::from_footprint(fp);
            if !fpe.is_default() {
                footprints.insert(fp.uuid.to_string(), fpe);
            }
            for pad in &fp.pads {
                let pe = PadExtras::from_pad(pad);
                if !pe.is_default() {
                    pads.insert(pad.uuid.to_string(), pe);
                }
            }
        }
        PcbExtras {
            footprints,
            pads,
            outline: board.outline.clone(),
            graphics: board.graphics.clone(),
            texts: board.texts.clone(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(in crate::format) struct FootprintExtras {
    #[serde(default)]
    pub(in crate::format) footprint_id: String,
    #[serde(default)]
    pub(in crate::format) locked: bool,
    #[serde(default)]
    pub(in crate::format) graphics: Vec<crate::pcb::FpGraphic>,
    #[serde(default)]
    pub(in crate::format) properties: Vec<crate::property::PcbProperty>,
}

impl FootprintExtras {
    pub(in crate::format) fn is_default(&self) -> bool {
        self.footprint_id.is_empty()
            && !self.locked
            && self.graphics.is_empty()
            && self.properties.is_empty()
    }

    pub(in crate::format) fn from_footprint(fp: &Footprint) -> Self {
        FootprintExtras {
            footprint_id: fp.footprint_id.clone(),
            locked: fp.locked,
            graphics: fp.graphics.clone(),
            properties: fp.properties.clone(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(in crate::format) struct PadExtras {
    #[serde(default)]
    pub(in crate::format) drill_shape: String,
}

impl PadExtras {
    pub(in crate::format) fn is_default(&self) -> bool {
        self.drill_shape.is_empty()
    }

    pub(in crate::format) fn from_pad(pad: &Pad) -> Self {
        PadExtras {
            drill_shape: pad
                .drill
                .as_ref()
                .map(|d| d.shape.clone())
                .unwrap_or_default(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(in crate::format) struct BoardExtras {
    #[serde(default)]
    pub(in crate::format) outline: Vec<PcbPoint>,
    #[serde(default)]
    pub(in crate::format) graphics: Vec<crate::pcb::BoardGraphic>,
    #[serde(default)]
    pub(in crate::format) texts: Vec<crate::pcb::BoardText>,
}
