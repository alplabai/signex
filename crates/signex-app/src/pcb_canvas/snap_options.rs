//! How carried board items snap (#2353): to the grid, and to the points of
//! board objects near the pointer. Editor state for the session; the board
//! file holds only the grid step.

use std::collections::BTreeSet;

use signex_engine::pcb::snap::SnapObject;
use signex_types::pcb_layer::{CopperLayer, PcbLayer};

/// Which layers' objects the pointer snaps to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SnapScope {
    #[default]
    AllLayers,
    /// Only the active copper layer's objects.
    CurrentLayer,
    /// No object snapping; the grid still applies.
    Off,
}

impl SnapScope {
    pub const ALL: [SnapScope; 3] = [
        SnapScope::AllLayers,
        SnapScope::CurrentLayer,
        SnapScope::Off,
    ];

    /// The scope after this one, as Shift+E steps them.
    pub fn next(self) -> Self {
        match self {
            SnapScope::AllLayers => SnapScope::CurrentLayer,
            SnapScope::CurrentLayer => SnapScope::Off,
            SnapScope::Off => SnapScope::AllLayers,
        }
    }
}

/// How far from the pointer an object point pulls it: a multiple of the
/// board's snap grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SnapDistance {
    Quarter,
    #[default]
    Half,
    One,
    Two,
}

impl SnapDistance {
    pub const ALL: [SnapDistance; 4] = [
        SnapDistance::Quarter,
        SnapDistance::Half,
        SnapDistance::One,
        SnapDistance::Two,
    ];

    /// The multiple of the snap grid.
    pub fn factor(self) -> f64 {
        match self {
            SnapDistance::Quarter => 0.25,
            SnapDistance::Half => 0.5,
            SnapDistance::One => 1.0,
            SnapDistance::Two => 2.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PcbSnapOptions {
    /// Round a carry to the board's snap grid.
    pub to_grids: bool,
    /// Pull a carry onto the board's guides (#2374).
    pub to_guides: bool,
    /// Line a carry up with object points in [`Self::axis_range_mm`].
    pub to_axes: bool,
    pub scope: SnapScope,
    /// The object points snapped to.
    pub objects: BTreeSet<SnapObject>,
    pub distance: SnapDistance,
    /// How far from the pointer an object point lines it up, mm.
    pub axis_range_mm: f64,
}

/// Axis Snap Range of a new session, mm.
pub const DEFAULT_AXIS_RANGE_MM: f64 = 10.0;

/// The least on-screen reach of object and guide snapping, in pixels, so
/// they still catch on a fine grid.
pub const SNAP_REACH_PX: f64 = 8.0;

impl Default for PcbSnapOptions {
    fn default() -> Self {
        Self {
            to_grids: true,
            to_guides: true,
            to_axes: false,
            scope: SnapScope::default(),
            objects: SnapObject::DEFAULT_ON.into_iter().collect(),
            distance: SnapDistance::default(),
            axis_range_mm: DEFAULT_AXIS_RANGE_MM,
        }
    }
}

impl PcbSnapOptions {
    /// Turn `object` on when off, off when on.
    pub fn toggle_object(&mut self, object: SnapObject) {
        if !self.objects.remove(&object) {
            self.objects.insert(object);
        }
    }

    /// The copper layers object snapping looks at while `active` is the
    /// active layer, as `pcb_geom::layer_bit` bits; `None` when it is off.
    pub fn layer_mask(&self, active: CopperLayer) -> Option<u32> {
        match self.scope {
            SnapScope::AllLayers => Some(u32::MAX),
            SnapScope::CurrentLayer => Some(1 << pcb_geom::layer_bit(active)),
            SnapScope::Off => None,
        }
    }

    /// How far from the pointer an object or guide pulls it, in mm: the
    /// Distance setting on the board's grid, or [`SNAP_REACH_PX`] at this
    /// zoom (`px_per_mm`), whichever is larger.
    pub fn catch_radius_mm(&self, grid_mm: f64, px_per_mm: f64) -> f64 {
        let by_grid = self.distance.factor() * grid_mm;
        if px_per_mm.is_finite() && px_per_mm > 0.0 {
            by_grid.max(SNAP_REACH_PX / px_per_mm)
        } else {
            by_grid
        }
    }
}

/// The options a snap reads, the active copper layer, the zoom of the canvas
/// the pointer is on and what that view shows.
#[derive(Debug, Clone)]
pub struct SnapView<'a> {
    pub options: &'a PcbSnapOptions,
    pub active: CopperLayer,
    pub px_per_mm: f64,
    /// The status bar's Snap: off, nothing pulls the pointer.
    pub snap_on: bool,
    /// Object points pull the pointer. Off for what is not on the board
    /// yet and for a bus's centre line: grid and guides only.
    pub objects_on: bool,
    /// The copper layers shown, as `pcb_geom::layer_bit` bits.
    pub shown_copper: u32,
    /// The other layers the view hides.
    pub hidden: Vec<PcbLayer>,
    /// The Multi-Layer row is off.
    pub through_hidden: bool,
    /// The kinds of object the view hides.
    pub hidden_kinds: Vec<pcb_geom::PickKind>,
    /// A running route's net: other nets give no object points.
    pub net: Option<u32>,
    /// A running route's layer; `active` when none runs.
    pub route_layer: Option<CopperLayer>,
}

impl<'a> SnapView<'a> {
    /// A view showing every layer, with Snap on.
    pub fn new(options: &'a PcbSnapOptions, active: CopperLayer, px_per_mm: f64) -> Self {
        Self {
            options,
            active,
            px_per_mm,
            snap_on: true,
            objects_on: true,
            shown_copper: u32::MAX,
            hidden: Vec::new(),
            through_hidden: false,
            hidden_kinds: Vec::new(),
            net: None,
            route_layer: None,
        }
    }

    /// This view with grid, guides and axes only.
    pub fn without_objects(mut self) -> Self {
        self.objects_on = false;
        self
    }
}
