//! Board options (#2374): snap guides, the extra snap grids, the sheet, the
//! polygon naming scheme and the designator display. Document data, saved
//! in the `.snxpcb` `[board_options]` section and changed only through the
//! board engine's `SetBoardOptions` command. The board's origin (#2384) and
//! its waived design-rule findings (#2399) are kept here too, and the panel
//! (#2410). See `docs/PCB_BOARD_OPTIONS.md`.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::pcb::Point;
use crate::pcb_layer::PcbLayer;
pub use crate::pcb_origin::BoardOrigin;
use crate::theme::Color;

/// The colours a guide or grid swatch steps through, first one the default
/// of a new guide or grid.
pub const MARK_COLORS: [Color; 6] = [
    Color::new(0x2E, 0xC4, 0xD8, 0xFF),
    Color::new(0xE8, 0xC4, 0x3A, 0xFF),
    Color::new(0x7C, 0xD9, 0x6A, 0xFF),
    Color::new(0xF0, 0x7A, 0x4A, 0xFF),
    Color::new(0xB4, 0x8A, 0xF0, 0xFF),
    Color::new(0xE8, 0xE8, 0xE8, 0xFF),
];

/// The swatch colour after `color` in [`MARK_COLORS`]; a colour not in the
/// list steps to the first.
pub fn next_mark_color(color: Color) -> Color {
    let at = MARK_COLORS.iter().position(|c| *c == color);
    match at {
        Some(i) => MARK_COLORS[(i + 1) % MARK_COLORS.len()],
        None => MARK_COLORS[0],
    }
}

/// Everything the board options hold. The default is what a board without
/// a `[board_options]` section has.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct BoardOptions {
    #[serde(default)]
    pub guides: Vec<BoardGuide>,
    /// Snap grids above the board's own (`PcbSetup::grid_size`), first the
    /// highest priority.
    #[serde(default)]
    pub grids: Vec<BoardGrid>,
    #[serde(default)]
    pub sheet: BoardSheet,
    #[serde(default)]
    pub polygon_naming: PolygonNaming,
    #[serde(default)]
    pub designator_display: DesignatorDisplay,
    /// The point shown as (0, 0), Y up (#2384). A board saved without one
    /// counts from the model's zero.
    #[serde(default)]
    pub origin: BoardOrigin,
    /// Design-rule findings the designer accepted, each with a note (#2399).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub drc_waivers: Vec<DrcWaiver>,
    /// The panel the board is arrayed into for manufacture (#2410); `None`
    /// for a board made on its own. Changed only through the board engine's
    /// `SetBoardPanel` command.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub panel: Option<crate::pcb_panel::BoardPanel>,
    /// The colours the user gave layers (Layer colour..., 2026-11-18),
    /// sorted by layer token; a layer not listed takes the palette's.
    /// Saved as `[board_options.layer_colors]` (`snxpcb/1.5`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub layer_colors: Vec<LayerColor>,
}

/// A colour the user gave one board layer: the canvas, the layer tabs and
/// the Layers panel draw the layer in it instead of the palette's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "LayerColorRaw", into = "LayerColorRaw")]
pub struct LayerColor {
    pub layer: PcbLayer,
    pub color: Color,
}

/// [`LayerColor`] as serde holds it: the layer by its `.snxpcb` token.
#[derive(Serialize, Deserialize)]
struct LayerColorRaw {
    layer: String,
    color: String,
}

impl From<LayerColor> for LayerColorRaw {
    fn from(c: LayerColor) -> Self {
        Self {
            layer: crate::format::pcb_layer_token(c.layer),
            color: c.color.to_hex(),
        }
    }
}

impl TryFrom<LayerColorRaw> for LayerColor {
    type Error = String;

    fn try_from(raw: LayerColorRaw) -> Result<Self, String> {
        let layer = crate::format::parse_pcb_layer(&raw.layer)
            .ok_or_else(|| format!("`{}` is not a board layer", raw.layer))?;
        let color = Color::parse_hex(&raw.color)
            .ok_or_else(|| format!("{:?} is not #rrggbb or #rrggbbaa", raw.color))?;
        Ok(Self { layer, color })
    }
}

/// One accepted design-rule finding. `key` names the finding the same way
/// in every run (`signex_drc::waiver_key`): its rule and the objects, not
/// where they are. `fingerprint` is the objects' geometry when it was
/// waived (`signex_drc::fingerprint`, #2307): moving, resizing or re-netting
/// one of them lapses the waiver and the finding comes back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DrcWaiver {
    pub key: String,
    /// The rule id it was waived under (`builtin::clearance`, `user::<name>`).
    pub rule: String,
    /// Why the finding is accepted; may be empty.
    #[serde(default)]
    pub note: String,
    /// A waiver saved before #2307 has none; it lapses once and is waived
    /// again.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub fingerprint: String,
    /// Who accepted it. A waiver saved before the author existed has none
    /// and writes no `author` key.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub author: String,
}

impl BoardOptions {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// The colour the user gave `layer`, if any.
    pub fn layer_color(&self, layer: PcbLayer) -> Option<Color> {
        self.layer_colors
            .iter()
            .find(|c| c.layer == layer)
            .map(|c| c.color)
    }

    /// Give `layer` its own colour, or (`None`) the palette's again. Whether
    /// that changed anything.
    pub fn set_layer_color(&mut self, layer: PcbLayer, color: Option<Color>) -> bool {
        if self.layer_color(layer) == color {
            return false;
        }
        self.layer_colors.retain(|c| c.layer != layer);
        if let Some(color) = color {
            self.layer_colors.push(LayerColor { layer, color });
            self.layer_colors
                .sort_by_key(|c| crate::format::pcb_layer_token(c.layer));
        }
        true
    }

    /// The grid a carry or placement of `what` snaps to: the first extra
    /// grid that applies to it, else the board's own grid of `board_step`
    /// counted from the board origin (#2384).
    pub fn grid_for(&self, what: GridUse, board_step: f64) -> GridSpec {
        self.grids
            .iter()
            .find(|grid| grid.applies_to(what))
            .map(|grid| GridSpec {
                step_mm: grid.step_mm,
                origin: grid.origin,
            })
            .unwrap_or(GridSpec {
                step_mm: board_step,
                origin: self.origin.at,
            })
    }
}

/// The way a guide line runs, as the screen shows it at no view turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuideKind {
    Vertical,
    Horizontal,
    /// Rising to the right at 45°.
    Plus45,
    /// Falling to the right at 45°.
    Minus45,
}

impl GuideKind {
    pub const ALL: [GuideKind; 4] = [
        GuideKind::Vertical,
        GuideKind::Horizontal,
        GuideKind::Plus45,
        GuideKind::Minus45,
    ];

    /// A unit vector along the line, in board mm (y grows down the screen).
    pub fn direction(self) -> (f64, f64) {
        let d = std::f64::consts::FRAC_1_SQRT_2;
        match self {
            GuideKind::Vertical => (0.0, 1.0),
            GuideKind::Horizontal => (1.0, 0.0),
            GuideKind::Plus45 => (d, -d),
            GuideKind::Minus45 => (d, d),
        }
    }
}

/// A snap guide: an endless line through `at`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoardGuide {
    pub uuid: Uuid,
    pub kind: GuideKind,
    pub at: Point,
    /// A disabled guide is kept but neither drawn nor snapped to.
    pub enabled: bool,
    pub color: Color,
}

impl BoardGuide {
    /// A new enabled guide of `kind` through `at`, in the first swatch.
    pub fn new(uuid: Uuid, kind: GuideKind, at: Point) -> Self {
        Self {
            uuid,
            kind,
            at,
            enabled: true,
            color: MARK_COLORS[0],
        }
    }

    /// The point of the line nearest `p`.
    pub fn nearest_point(&self, p: Point) -> Point {
        let (dx, dy) = self.kind.direction();
        let t = (p.x - self.at.x) * dx + (p.y - self.at.y) * dy;
        Point::new(self.at.x + t * dx, self.at.y + t * dy)
    }

    /// Where this line crosses `other`; `None` for parallel lines.
    pub fn crossing(&self, other: &BoardGuide) -> Option<Point> {
        let (ax, ay) = self.kind.direction();
        let (bx, by) = other.kind.direction();
        let cross = ax * by - ay * bx;
        if cross.abs() < 1e-12 {
            return None;
        }
        let (wx, wy) = (other.at.x - self.at.x, other.at.y - self.at.y);
        let t = (wx * by - wy * bx) / cross;
        Some(Point::new(self.at.x + t * ax, self.at.y + t * ay))
    }
}

/// What a grid applies to: carried components, or anything else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridUse {
    Components,
    NonComponents,
}

/// An extra snap grid over the board's own.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoardGrid {
    pub uuid: Uuid,
    pub name: String,
    pub step_mm: f64,
    pub origin: Point,
    pub color: Color,
    pub components: bool,
    pub non_components: bool,
}

impl BoardGrid {
    pub fn applies_to(&self, what: GridUse) -> bool {
        match what {
            GridUse::Components => self.components,
            GridUse::NonComponents => self.non_components,
        }
    }
}

/// A grid step and the point it counts from, board mm.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridSpec {
    pub step_mm: f64,
    pub origin: Point,
}

impl GridSpec {
    /// The grid point nearest `p`; `p` itself for an unusable step.
    pub fn snap(&self, p: Point) -> Point {
        if !self.step_mm.is_finite() || self.step_mm <= 0.0 {
            return p;
        }
        let snap = |v: f64, o: f64| ((v - o) / self.step_mm).round() * self.step_mm + o;
        Point::new(snap(p.x, self.origin.x), snap(p.y, self.origin.y))
    }

    /// The move that takes `anchor` to the grid point nearest
    /// `anchor + raw`.
    pub fn snap_delta(&self, anchor: Point, raw: Point) -> Point {
        let to = self.snap(Point::new(anchor.x + raw.x, anchor.y + raw.y));
        Point::new(to.x - anchor.x, to.y - anchor.y)
    }
}

/// The sheet drawn behind the board.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoardSheet {
    /// The sheet is the box around the board shape (the board has no
    /// separate sheet layer); the stored box below is not used.
    pub from_board: bool,
    pub origin: Point,
    pub width: f64,
    pub height: f64,
}

impl Default for BoardSheet {
    fn default() -> Self {
        Self {
            from_board: true,
            origin: Point::ZERO,
            width: 100.0,
            height: 80.0,
        }
    }
}

/// How a polygon pour is named.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolygonNaming {
    /// `NET NAME_LXX_PXXX`
    #[default]
    NetLayerPour,
    /// `LXX_NET NAME_PXXX`
    LayerNetPour,
    /// `Board Layer_NET NAME_PXXX`
    BoardLayerNetPour,
    /// `NET NAME_Board Layer_PXXX`
    NetBoardLayerPour,
}

impl PolygonNaming {
    pub const ALL: [PolygonNaming; 4] = [
        PolygonNaming::NetLayerPour,
        PolygonNaming::LayerNetPour,
        PolygonNaming::BoardLayerNetPour,
        PolygonNaming::NetBoardLayerPour,
    ];

    /// The name of pour number `pour` (from 1) of `net` on copper layer
    /// number `layer` (from 1) called `layer_name`.
    pub fn name(self, net: &str, layer: usize, layer_name: &str, pour: usize) -> String {
        let (l, p) = (format!("L{layer:02}"), format!("P{pour:03}"));
        match self {
            PolygonNaming::NetLayerPour => format!("{net}_{l}_{p}"),
            PolygonNaming::LayerNetPour => format!("{l}_{net}_{p}"),
            PolygonNaming::BoardLayerNetPour => format!("{layer_name}_{net}_{p}"),
            PolygonNaming::NetBoardLayerPour => format!("{net}_{layer_name}_{p}"),
        }
    }
}

/// Whether a designator shows with its channel (physical) or as on the
/// schematic (logical).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DesignatorDisplay {
    #[default]
    Physical,
    Logical,
}

#[cfg(test)]
#[path = "pcb_options_tests.rs"]
mod tests;
