//! What the Board sections of the Properties panel show in board sketch
//! mode (#2234).

/// The Board sections of the Properties panel in board sketch mode (#2234):
/// the board's name and outline facts in place of the footprint fields
/// (#2356), and what the selected entity becomes on the board. Keepouts and
/// cutouts use the Role picker and its sub-forms, as in a footprint.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BoardSketchProps {
    /// The board's name, as the canvas chip shows it.
    pub board_name: String,
    /// Width and height of the board outline's extent in mm; `None`
    /// without an outline.
    pub board_size_mm: Option<(f64, f64)>,
    /// The size and area are read from the sketch's closed outline, which
    /// Bake writes, not from the board's (mockup row 179e).
    pub size_from_shape: bool,
    /// The outline is an axis-aligned rectangle, so its extent is its size;
    /// otherwise the Size row reads as a bounding box, as Board Properties
    /// does.
    pub outline_is_rectangle: bool,
    /// The area the board outline encloses in mm².
    pub board_area_mm2: Option<f64>,
    /// The selected entity, if any.
    pub selected: Option<signex_sketch::id::SketchEntityId>,
    /// It is a line, arc or circle: it can be part of the outline.
    pub is_edge: bool,
    /// It is a circle: it can be a mounting hole.
    pub is_circle: bool,
    /// It is tagged as part of the board outline.
    pub outline: bool,
    /// Its mounting hole attributes, when it is one.
    pub hole: Option<MountingHoleProps>,
    /// The Outline checks lines of the current solve (mockup row 179d): Properties
    /// lists them with nothing selected, the bake preview the failed ones.
    pub outline_checks: Vec<crate::library::editor::footprint::CheckLine>,
    /// The selected entity's Slot, Pour, Mechanical layer or Component
    /// anchor rows (decision 2, 2026-11-20).
    pub target: crate::panels::BoardTargetProps,
    /// The size (length, width) in mm of the slot the selected edge belongs
    /// to, whatever its role, for the Slot header (mockup row 179); `None`
    /// when it is no slot edge.
    pub selected_slot: Option<(f64, f64)>,
    /// The name of the rule profile the project's rules were seeded from,
    /// for the Fab profile group (mockup row 179d); `None` when it names
    /// none.
    pub fab_profile: Option<String>,
    /// The project's router tool diameter in mm (mockup row 179d), for the Fab
    /// profile group; `None` when its fab profile names none.
    pub router_tool_diameter: Option<f64>,
    /// The radius Add fillets applies; `None` when no failing corner can
    /// take a fillet (mockup row 179d).
    pub fillet_radius: Option<f64>,
    /// The open bake preview, shown as the Bake panel (mockup row 179);
    /// `None` while the preview is closed.
    pub bake: Option<crate::panels::BoardBakePreview>,
}

/// A mounting hole's fields as the Properties panel edits them.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MountingHoleProps {
    /// The drill expression; empty means the circle's diameter.
    pub drill: String,
    pub plated: bool,
    /// The net name; empty means no net.
    pub net: String,
}
