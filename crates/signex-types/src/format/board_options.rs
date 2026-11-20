//! The optional `[board_options]` section of a `.snxpcb` (#2374): guides,
//! extra snap grids, the sheet, the polygon naming scheme, the
//! designator display, the board origin (#2384), the panel (#2410,
//! `board_panel`) and the layer colours (2026-11-18).
//!
//! Written only when the options differ from the default, so a board
//! without any re-saves byte-identical and a file from before the section
//! loads unchanged. Lengths are integer nm like every other on-disk
//! geometry; colours are `#rrggbbaa`. See `docs/PCB_BOARD_OPTIONS.md`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::FormatError;
use super::units::{NonFiniteField, mm_to_nm, nm_to_mm};
use crate::pcb::{PcbBoard, Point};
use crate::pcb_options::{
    BoardGrid, BoardGuide, BoardOptions, BoardOrigin, BoardSheet, DesignatorDisplay, DrcWaiver,
    GuideKind, LayerColor, PolygonNaming,
};
use crate::theme::Color;

const GUIDES: &str = "board_options.guides";
const GRIDS: &str = "board_options.grids";
const SHEET: &str = "board_options.sheet";
const ORIGIN: &str = "board_options.origin";
const LAYER_COLORS: &str = "board_options.layer_colors";

/// `[board_options]` as stored.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(in crate::format) struct BoardOptionsRaw {
    #[serde(default)]
    polygon_naming: PolygonNaming,
    #[serde(default)]
    designator_display: DesignatorDisplay,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sheet: Option<SheetRaw>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    guides: Vec<GuideRaw>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    grids: Vec<GridRaw>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    origin: Option<OriginRaw>,
    /// Waived design-rule findings (#2399), stored as held: the fingerprint
    /// is a hash, not geometry (#2307).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    drc_waivers: Vec<DrcWaiver>,
    /// The panel (#2410), `format::board_panel`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    panel: Option<super::board_panel::PanelRaw>,
    /// `[board_options.layer_colors]`: a layer token to `#rrggbbaa` (only in
    /// `snxpcb/1.5`, `format::snxpcb_token`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    layer_colors: BTreeMap<String, String>,
}

/// The layer colours of `raw`, in token order; a token this build does not
/// know or a colour it cannot read refuses the file, as a guide's colour
/// does.
fn layer_colors(raw: BTreeMap<String, String>) -> Result<Vec<LayerColor>, FormatError> {
    raw.into_iter()
        .enumerate()
        .map(|(row, (token, cell))| {
            let layer =
                super::parse_pcb_layer(&token).ok_or_else(|| FormatError::TsvFieldParse {
                    block: LAYER_COLORS.to_string(),
                    row,
                    field: "layer".to_string(),
                    message: format!("`{token}` is not a board layer"),
                })?;
            Ok(LayerColor {
                layer,
                color: color(&cell, LAYER_COLORS, row)?,
            })
        })
        .collect()
}

/// The board origin (#2384), nm.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct OriginRaw {
    x: i64,
    y: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SheetRaw {
    from_board: bool,
    x: i64,
    y: i64,
    width: i64,
    height: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GuideRaw {
    uuid: Uuid,
    kind: GuideKind,
    x: i64,
    y: i64,
    enabled: bool,
    color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GridRaw {
    uuid: Uuid,
    name: String,
    step: i64,
    origin_x: i64,
    origin_y: i64,
    color: String,
    components: bool,
    non_components: bool,
}

/// The board options of the file; the default when it has no section.
pub(in crate::format) fn read(raw: Option<BoardOptionsRaw>) -> Result<BoardOptions, FormatError> {
    let Some(raw) = raw else {
        return Ok(BoardOptions::default());
    };
    let point = |x: i64, y: i64| Point::new(nm_to_mm(x), nm_to_mm(y));
    let sheet = raw
        .sheet
        .map(|s| BoardSheet {
            from_board: s.from_board,
            origin: point(s.x, s.y),
            width: nm_to_mm(s.width),
            height: nm_to_mm(s.height),
        })
        .unwrap_or_default();
    let guides = raw
        .guides
        .into_iter()
        .enumerate()
        .map(|(row, g)| {
            Ok(BoardGuide {
                uuid: g.uuid,
                kind: g.kind,
                at: point(g.x, g.y),
                enabled: g.enabled,
                color: color(&g.color, GUIDES, row)?,
            })
        })
        .collect::<Result<_, FormatError>>()?;
    let grids = raw
        .grids
        .into_iter()
        .enumerate()
        .map(|(row, g)| {
            Ok(BoardGrid {
                uuid: g.uuid,
                name: g.name,
                step_mm: nm_to_mm(g.step),
                origin: point(g.origin_x, g.origin_y),
                color: color(&g.color, GRIDS, row)?,
                components: g.components,
                non_components: g.non_components,
            })
        })
        .collect::<Result<_, FormatError>>()?;
    Ok(BoardOptions {
        guides,
        grids,
        sheet,
        polygon_naming: raw.polygon_naming,
        designator_display: raw.designator_display,
        origin: raw
            .origin
            .map(|o| BoardOrigin::new(point(o.x, o.y)))
            .unwrap_or_default(),
        drc_waivers: raw.drc_waivers,
        panel: raw.panel.map(super::board_panel::read).transpose()?,
        layer_colors: layer_colors(raw.layer_colors)?,
    })
}

fn color(cell: &str, block: &str, row: usize) -> Result<Color, FormatError> {
    Color::parse_hex(cell).ok_or_else(|| FormatError::TsvFieldParse {
        block: block.to_string(),
        row,
        field: "color".to_string(),
        message: format!("{cell:?} is not #rrggbb or #rrggbbaa"),
    })
}

/// Append `[board_options]` to `out`; nothing for default options.
pub(in crate::format) fn write(out: &mut String, board: &PcbBoard) -> Result<(), FormatError> {
    let options = &board.options;
    if options.is_default() {
        return Ok(());
    }
    let refuse = |block: &str, row: usize, e: NonFiniteField| FormatError::NonFiniteField {
        block: block.to_string(),
        row,
        field: e.0.to_string(),
    };
    let sheet = &options.sheet;
    let sheet = (*sheet != BoardSheet::default())
        .then(|| {
            Ok::<_, NonFiniteField>(SheetRaw {
                from_board: sheet.from_board,
                x: mm_to_nm(sheet.origin.x, "x")?,
                y: mm_to_nm(sheet.origin.y, "y")?,
                width: mm_to_nm(sheet.width, "width")?,
                height: mm_to_nm(sheet.height, "height")?,
            })
        })
        .transpose()
        .map_err(|e| refuse(SHEET, 0, e))?;
    let guides = options
        .guides
        .iter()
        .enumerate()
        .map(|(row, g)| {
            Ok(GuideRaw {
                uuid: g.uuid,
                kind: g.kind,
                x: mm_to_nm(g.at.x, "x").map_err(|e| refuse(GUIDES, row, e))?,
                y: mm_to_nm(g.at.y, "y").map_err(|e| refuse(GUIDES, row, e))?,
                enabled: g.enabled,
                color: g.color.to_hex(),
            })
        })
        .collect::<Result<_, FormatError>>()?;
    let grids = options
        .grids
        .iter()
        .enumerate()
        .map(|(row, g)| {
            let nm = |mm: f64, field| mm_to_nm(mm, field).map_err(|e| refuse(GRIDS, row, e));
            Ok(GridRaw {
                uuid: g.uuid,
                name: g.name.clone(),
                step: nm(g.step_mm, "step")?,
                origin_x: nm(g.origin.x, "origin_x")?,
                origin_y: nm(g.origin.y, "origin_y")?,
                color: g.color.to_hex(),
                components: g.components,
                non_components: g.non_components,
            })
        })
        .collect::<Result<_, FormatError>>()?;
    let at = options.origin.at;
    let origin = (options.origin != BoardOrigin::default())
        .then(|| {
            Ok::<_, NonFiniteField>(OriginRaw {
                x: mm_to_nm(at.x, "x")?,
                y: mm_to_nm(at.y, "y")?,
            })
        })
        .transpose()
        .map_err(|e| refuse(ORIGIN, 0, e))?;
    let panel = options
        .panel
        .as_ref()
        .map(super::board_panel::write)
        .transpose()?;
    #[derive(Serialize)]
    struct Wrapper {
        board_options: BoardOptionsRaw,
    }
    let wrapper = Wrapper {
        board_options: BoardOptionsRaw {
            polygon_naming: options.polygon_naming,
            designator_display: options.designator_display,
            sheet,
            guides,
            grids,
            origin,
            drc_waivers: options.drc_waivers.clone(),
            panel,
            layer_colors: options
                .layer_colors
                .iter()
                .map(|c| (super::pcb_layer_token(c.layer), c.color.to_hex()))
                .collect(),
        },
    };
    out.push('\n');
    out.push_str(&toml::to_string_pretty(&wrapper)?);
    Ok(())
}
