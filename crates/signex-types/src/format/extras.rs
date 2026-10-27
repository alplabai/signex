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

use super::unknown::UnknownFields;
use crate::pcb::{Footprint, Pad, PcbBoard, Point as PcbPoint};
use crate::schematic::{Junction, SchematicSheet, Symbol};

#[derive(Debug, Clone, Default, Deserialize)]
pub(in crate::format) struct SchExtrasRaw {
    #[serde(default)]
    pub(in crate::format) symbols: BTreeMap<String, SymbolExtras>,
    #[serde(default)]
    pub(in crate::format) junctions: BTreeMap<String, JunctionExtras>,
    #[serde(default)]
    pub(in crate::format) sheet: Option<SheetExtras>,
}

/// Per-junction auxiliary fields that don't fit into [`SchJunctionRow`] —
/// today just the minted-vs-user-placed provenance bit (issue #422). Only
/// emitted for a junction where `minted` differs from the on-disk default
/// (`false`, i.e. user-placed), so a `.snxsch` predating this field round-
/// trips unchanged and every dot it names loads as user-placed.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(in crate::format) struct JunctionExtras {
    #[serde(default)]
    pub(in crate::format) minted: bool,
}

impl JunctionExtras {
    pub(in crate::format) fn is_default(&self) -> bool {
        !self.minted
    }

    pub(in crate::format) fn from_junction(j: &Junction) -> Self {
        JunctionExtras { minted: j.minted }
    }
}

/// Per-symbol auxiliary fields that don't fit into [`SchComponentRow`].
///
/// `Default` is hand-written, not derived: it must equal what serde
/// fills in for an empty table (`unit = 1`, `in_bom` and `on_board`
/// true). The writer omits a symbol whose extras are default and the
/// parser substitutes `Default` for the missing table, so any
/// disagreement silently rewrites the symbol on reload (#789).
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// Font families of the reference and value text. They live here, not
    /// inside `ref_text` / `val_text`, because a build that predates the
    /// field drops unknown keys nested in those typed tables on re-save but
    /// keeps unknown keys of this table (`unknown` below) attached to the
    /// symbol. That holds for builds since #658; v0.15.0 has no unknown-key
    /// capture and drops these keys on re-save.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(in crate::format) ref_font_family: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(in crate::format) val_font_family: Option<String>,
    /// Library row this symbol was placed from and the row version it is
    /// pinned to (#1338). Declared in key order (`library_id`,
    /// `library_version`, `row_id`) because an older build re-saves them
    /// sorted as unknown keys; this keeps its output byte-identical when the
    /// symbol has no other unknown keys. Written only when set, so a symbol
    /// with no link adds nothing to the file. A build keeps the keys as
    /// unknown keys of this table only if it has the unknown-key capture
    /// (trunk since #658); v0.15.0 drops them on re-save. A `library_id` or
    /// `row_id` that is not a UUID loads as no link and is reported with a
    /// `tracing::warn!` (it reaches Messages); the sheet still opens.
    #[serde(
        default,
        deserialize_with = "lenient_link_id::<_, 0>",
        skip_serializing_if = "Option::is_none"
    )]
    pub(in crate::format) library_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub(in crate::format) library_version: String,
    #[serde(
        default,
        deserialize_with = "lenient_link_id::<_, 1>",
        skip_serializing_if = "Option::is_none"
    )]
    pub(in crate::format) row_id: Option<Uuid>,
    /// Keys this build does not recognise (`[extras.symbols.<uuid>]`).
    #[serde(flatten)]
    pub(in crate::format) unknown: UnknownFields,
}

impl Default for SymbolExtras {
    fn default() -> Self {
        SymbolExtras {
            footprint: String::new(),
            datasheet: String::new(),
            mirror_x: false,
            mirror_y: false,
            unit: default_unit(),
            is_power: false,
            fields_autoplaced: false,
            fields_user_placed: false,
            dnp: false,
            in_bom: default_true(),
            on_board: default_true(),
            exclude_from_sim: false,
            locked: false,
            fields: BTreeMap::new(),
            custom_properties: Vec::new(),
            pin_uuids: BTreeMap::new(),
            instances: Vec::new(),
            ref_text: None,
            val_text: None,
            ref_font_family: None,
            val_font_family: None,
            library_id: None,
            row_id: None,
            library_version: String::new(),
            unknown: UnknownFields::default(),
        }
    }
}

impl SymbolExtras {
    /// True when nothing differs from [`SymbolExtras::default`], so the
    /// writer may omit the table and the parser's fallback restores it.
    pub(in crate::format) fn is_default(&self) -> bool {
        let d = Self::default();
        self.footprint == d.footprint
            && self.datasheet == d.datasheet
            && self.mirror_x == d.mirror_x
            && self.mirror_y == d.mirror_y
            && self.unit == d.unit
            && self.is_power == d.is_power
            && self.fields_autoplaced == d.fields_autoplaced
            && self.fields_user_placed == d.fields_user_placed
            && self.dnp == d.dnp
            && self.in_bom == d.in_bom
            && self.on_board == d.on_board
            && self.exclude_from_sim == d.exclude_from_sim
            && self.locked == d.locked
            && self.fields.is_empty()
            && self.custom_properties.is_empty()
            && self.pin_uuids.is_empty()
            && self.instances.is_empty()
            && self.ref_text.is_none()
            && self.val_text.is_none()
            && self.ref_font_family.is_none()
            && self.val_font_family.is_none()
            && self.library_id.is_none()
            && self.row_id.is_none()
            && self.library_version.is_empty()
            && self.unknown.is_empty()
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
            ref_text: s.ref_text.clone().map(without_font_family),
            val_text: s.val_text.clone().map(without_font_family),
            ref_font_family: s.ref_text.as_ref().and_then(|t| t.font_family.clone()),
            val_font_family: s.val_text.as_ref().and_then(|t| t.font_family.clone()),
            library_id: s.library_id,
            row_id: s.row_id,
            library_version: s.library_version.clone(),
            unknown: s.unknown.clone(),
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
    /// Net colour overrides: wire/junction uuid -> `#RRGGBBAA`. Skipped
    /// when empty so a sheet without overrides re-saves byte-identical, and
    /// an older build, which does not know the key, keeps it in `unknown`
    /// and writes it back.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(in crate::format) net_colors: BTreeMap<String, String>,
    /// Sheet-wide default font family; omitted when unset so a sheet that
    /// never chose one serialises exactly as it did before the field existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(in crate::format) font_family: Option<String>,
    /// Text-note font families by note uuid. Kept beside the notes, not in
    /// them: a build that predates the field drops unknown keys nested in
    /// the typed `text_notes` array on re-save but keeps unknown keys of this
    /// table (builds since #658; v0.15.0 drops them). Entries for notes that no longer exist are ignored on load and
    /// so are gone after the next save.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(in crate::format) note_font_families: BTreeMap<String, String>,
    /// Frame settings (`[extras.sheet.frame]`); omitted at the default so a
    /// sheet that never changed them re-saves byte-identical.
    #[serde(default, skip_serializing_if = "FrameExtras::is_default")]
    pub(in crate::format) frame: FrameExtras,
    /// Keys this build does not recognise (`[extras.sheet]`).
    #[serde(flatten)]
    pub(in crate::format) unknown: UnknownFields,
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
            && self.net_colors.is_empty()
            && self.font_family.is_none()
            && self.note_font_families.is_empty()
            && self.frame.is_default()
            && self.unknown.is_empty()
    }

    pub(in crate::format) fn from_sheet(s: &SchematicSheet) -> Self {
        SheetExtras {
            child_sheets: s.child_sheets.clone(),
            no_connects: s.no_connects.clone(),
            text_notes: s
                .text_notes
                .iter()
                .cloned()
                .map(|mut note| {
                    note.font_family = None;
                    note
                })
                .collect(),
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
            net_colors: s
                .net_colors
                .iter()
                .map(|(id, color)| (id.to_string(), color.to_hex()))
                .collect(),
            font_family: s.font_family.clone(),
            note_font_families: s
                .text_notes
                .iter()
                .filter_map(|note| Some((note.uuid.to_string(), note.font_family.clone()?)))
                .collect(),
            frame: FrameExtras {
                settings: s.frame,
                unknown: s.unknown.frame.clone(),
            },
            unknown: s.unknown.extras.clone(),
        }
    }
}

/// A reference/value text property without its family, which is stored in
/// the symbol's own table (see [`SymbolExtras::ref_font_family`]).
fn without_font_family(mut prop: crate::schematic::TextProp) -> crate::schematic::TextProp {
    prop.font_family = None;
    prop
}

/// Deserialize a library-link id, mapping a value that is not a UUID to
/// `None` and reporting it. `KEY` picks the name (0 `library_id`, 1
/// `row_id`) because `deserialize_with` receives no field name.
fn lenient_link_id<'de, D, const KEY: u8>(de: D) -> Result<Option<Uuid>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let Some(value) = Option::<toml::Value>::deserialize(de)? else {
        return Ok(None);
    };
    let parsed = value.as_str().and_then(|s| Uuid::parse_str(s).ok());
    if parsed.is_none() {
        let key = if KEY == 0 { "library_id" } else { "row_id" };
        tracing::warn!(
            target: "signex::format",
            "[extras.symbols] {key} = {value} is not a UUID; the symbol loads without its library link"
        );
    }
    Ok(parsed)
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
    /// Keys this build does not recognise (`[extras.footprints.<uuid>]`).
    #[serde(flatten)]
    pub(in crate::format) unknown: UnknownFields,
}

impl FootprintExtras {
    pub(in crate::format) fn is_default(&self) -> bool {
        self.footprint_id.is_empty()
            && !self.locked
            && self.graphics.is_empty()
            && self.properties.is_empty()
            && self.unknown.is_empty()
    }

    pub(in crate::format) fn from_footprint(fp: &Footprint) -> Self {
        FootprintExtras {
            footprint_id: fp.footprint_id.clone(),
            locked: fp.locked,
            graphics: fp.graphics.clone(),
            properties: fp.properties.clone(),
            unknown: fp.unknown.clone(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(in crate::format) struct PadExtras {
    #[serde(default)]
    pub(in crate::format) drill_shape: String,
    /// Keys this build does not recognise (`[extras.pads.<uuid>]`).
    #[serde(flatten)]
    pub(in crate::format) unknown: UnknownFields,
}

impl PadExtras {
    pub(in crate::format) fn is_default(&self) -> bool {
        self.drill_shape.is_empty() && self.unknown.is_empty()
    }

    pub(in crate::format) fn from_pad(pad: &Pad) -> Self {
        PadExtras {
            drill_shape: pad
                .drill
                .as_ref()
                .map(|d| d.shape.clone())
                .unwrap_or_default(),
            unknown: pad.unknown.clone(),
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
    /// Keys this build does not recognise (`[extras.board]`).
    #[serde(flatten)]
    pub(in crate::format) unknown: UnknownFields,
}

impl SheetExtras {
    /// The net colour overrides as the sheet holds them. An entry whose key
    /// is not a uuid or whose value is not `#RRGGBB[AA]` is dropped: it is a
    /// cosmetic override no build could draw.
    pub(in crate::format) fn parsed_net_colors(
        &self,
    ) -> std::collections::HashMap<Uuid, crate::theme::Color> {
        self.net_colors
            .iter()
            .filter_map(|(id, hex)| {
                Some((
                    Uuid::parse_str(id).ok()?,
                    crate::theme::Color::parse_hex(hex)?,
                ))
            })
            .collect()
    }
}

/// `[extras.sheet.frame]`: the sheet's frame settings, plus any key a newer
/// build wrote there, kept so a save does not drop it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub(in crate::format) struct FrameExtras {
    #[serde(flatten)]
    pub(in crate::format) settings: crate::schematic::SheetFrameSettings,
    #[serde(flatten)]
    pub(in crate::format) unknown: UnknownFields,
}

impl FrameExtras {
    pub(in crate::format) fn is_default(&self) -> bool {
        self.settings.is_default() && self.unknown.is_empty()
    }
}
