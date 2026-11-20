//! Editor-mode enum + editor bar mode switch + active-bar menu enum +
//! pad-stack tab strip.

use signex_i18n::{MessageId, ids};

/// Footprint editor mode — gate sketch tooling on / off without
/// rewriting the canvas state machine. Phase 5.3 of the v0.13 sketch-
/// mode plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditorMode {
    /// Direct pad-list editing (the existing Phase 0–v0.10 surface).
    #[default]
    Normal,
    /// Parametric sketch mode — Phase 6 UI lives behind this.
    Sketch,
    /// 3D body preview (existing v0.10 viewer).
    View3d,
}

impl EditorMode {
    /// The editor bar label of this mode: Footprint, Sketch, or 3D for the
    /// toggle.
    pub fn switch_label(self) -> MessageId {
        match self {
            EditorMode::Normal => ids::EDITORS_MODE_FOOTPRINT,
            EditorMode::Sketch => ids::EDITORS_MODE_SKETCH,
            EditorMode::View3d => ids::EDITORS_MODE_3D,
        }
    }
}

/// The editor bar's mode switch as the view draws it (#187): a two-state
/// Footprint / Sketch segment and a 3D toggle beside it. In 3D the segment
/// of the mode the toggle returns to stays lit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModeSwitch {
    /// The mode whose segment is lit.
    pub lit: EditorMode,
    pub is_3d_on: bool,
    /// The mode a click on the 3D toggle switches to.
    pub toggle_target: EditorMode,
}

impl ModeSwitch {
    /// The segments, left to right.
    pub const SEGMENTS: [EditorMode; 2] = [EditorMode::Normal, EditorMode::Sketch];

    /// The switch in `mode`; `before_3d` is the mode 3D was entered from.
    pub fn of(mode: EditorMode, before_3d: EditorMode) -> Self {
        // 3D returns to a segment's mode, never to itself.
        let back = match before_3d {
            EditorMode::View3d => EditorMode::Normal,
            other => other,
        };
        match mode {
            EditorMode::View3d => Self {
                lit: back,
                is_3d_on: true,
                toggle_target: back,
            },
            other => Self {
                lit: other,
                is_3d_on: false,
                toggle_target: EditorMode::View3d,
            },
        }
    }

    pub fn is_lit(self, segment: EditorMode) -> bool {
        self.lit == segment
    }
}

/// v0.13 — Altium-style footprint active bar dropdown menus. One per
/// chevron-bearing button in `pads_active_bar`. The dropdown overlay
/// reads this enum to render the matching menu body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FpActiveBarMenu {
    /// Selection Filter — 10 footprint-kind toggle pills.
    Filter,
    /// Snap Options — Grids/Guides/Axes pills + 12 snap-object pills.
    Snap,
    /// Place / Move / Drag — gesture menu.
    Place,
    /// Selection — selection-mode picker.
    Select,
    /// Align — full Altium align/distribute menu.
    Align,
    /// 3D Body — 3D Body, Extruded 3D Body.
    Body3d,
    /// Text — String, Text Frame.
    Text,
    /// Shapes — Sketch-mode only.
    Shapes,
    /// Sketch ▸ Create — Line / Rectangle / Rounded Rectangle /
    /// Circle / Arc / Tangent Arc. Sketch-mode only.
    SketchCreate,
    /// Sketch ▸ Modify — Fillet / Chamfer / Trim / Extend / Mirror /
    /// Offset / Linear, Rectangular + Circular Pattern / Make Pad from
    /// Profile.
    /// Sketch-mode only.
    SketchModify,
}

/// Which sketch tools each Fusion-style group menu owns. The group's
/// trigger button borrows the armed tool's icon when the armed tool
/// belongs to it, so the collapsed bar still shows what's in hand.
impl FpActiveBarMenu {
    pub const SKETCH_CREATE_TOOLS: &'static [super::SketchTool] = &[
        super::SketchTool::Line,
        super::SketchTool::Rectangle,
        super::SketchTool::RoundedRectangle,
        super::SketchTool::Circle,
        super::SketchTool::Arc,
        super::SketchTool::TangentArc,
        super::SketchTool::EdgeArc,
        super::SketchTool::Point,
    ];
    pub const SKETCH_MODIFY_TOOLS: &'static [super::SketchTool] = &[
        super::SketchTool::Fillet,
        super::SketchTool::Chamfer,
        super::SketchTool::Trim,
        super::SketchTool::Extend,
        super::SketchTool::Mirror,
        super::SketchTool::Offset,
        super::SketchTool::LinearPattern,
        super::SketchTool::RectPattern,
        super::SketchTool::CircularPattern,
    ];
}

/// v0.20 — Pad Stack section's tab strip. Matches Altium's three tabs
/// verbatim:
/// - `Simple`: one row per stack family (COPPER / HOLE / PASTE / SOLDER).
/// - `TopMiddleBottom`: COPPER splits into Top / Middle / Bottom rows.
/// - `FullStack`: enumerates the pad's `layers` list verbatim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PadStackTab {
    #[default]
    Simple,
    TopMiddleBottom,
    FullStack,
}

impl PadStackTab {
    pub const ALL: &'static [PadStackTab] = &[
        PadStackTab::Simple,
        PadStackTab::TopMiddleBottom,
        PadStackTab::FullStack,
    ];
    pub fn label(self) -> &'static str {
        match self {
            PadStackTab::Simple => "Simple",
            PadStackTab::TopMiddleBottom => "Top-Middle-Bottom",
            PadStackTab::FullStack => "Full Stack",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{EditorMode, ModeSwitch};

    #[test]
    fn footprint_mode_lights_footprint_and_the_toggle_enters_3d() {
        let s = ModeSwitch::of(EditorMode::Normal, EditorMode::Sketch);
        assert!(s.is_lit(EditorMode::Normal));
        assert!(!s.is_lit(EditorMode::Sketch));
        assert!(!s.is_3d_on);
        assert_eq!(s.toggle_target, EditorMode::View3d);
    }

    #[test]
    fn sketch_mode_lights_sketch_and_the_toggle_enters_3d() {
        let s = ModeSwitch::of(EditorMode::Sketch, EditorMode::Normal);
        assert!(s.is_lit(EditorMode::Sketch));
        assert!(!s.is_lit(EditorMode::Normal));
        assert!(!s.is_3d_on);
        assert_eq!(s.toggle_target, EditorMode::View3d);
    }

    #[test]
    fn in_3d_the_toggle_is_on_and_returns_to_the_mode_it_came_from() {
        for from in ModeSwitch::SEGMENTS {
            let s = ModeSwitch::of(EditorMode::View3d, from);
            assert!(s.is_3d_on, "from {from:?}");
            assert_eq!(s.toggle_target, from);
            assert!(s.is_lit(from), "the segment 3D returns to stays lit");
        }
    }

    #[test]
    fn three_d_never_returns_to_itself() {
        let s = ModeSwitch::of(EditorMode::View3d, EditorMode::View3d);
        assert_eq!(s.toggle_target, EditorMode::Normal);
        assert!(s.is_lit(EditorMode::Normal));
    }

    #[test]
    fn the_segments_read_footprint_then_sketch() {
        assert_eq!(
            ModeSwitch::SEGMENTS,
            [EditorMode::Normal, EditorMode::Sketch]
        );
        assert!(!ModeSwitch::SEGMENTS.contains(&EditorMode::View3d));
    }
}
