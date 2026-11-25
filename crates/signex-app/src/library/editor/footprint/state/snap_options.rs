//! Snap-related configuration types: `SnapOptions`, `SnapSubTab`,
//! `SnappingMode`, `GridDisplay`, `GridDef`, `Guide`, `GuideAxis`.

/// v0.17.0 — per-priority snap toggles surfaced on the empty-canvas
/// Properties panel. Mirrors Altium's "Snap Options" checklist.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SnapOptions {
    /// Snap onto an existing sketch Point within `POINT_SNAP_RADIUS_PX`.
    pub point_hit: bool,
    /// Horizontal / Vertical inference within `AXIS_THRESHOLD_DEG`.
    pub horizontal_vertical: bool,
    /// Multi-of-`ANGLE_STEP_DEG` snap within `ANGLE_THRESHOLD_DEG`.
    pub angle: bool,
    /// Round to the nearest `grid_step_mm`.
    pub grid: bool,
    /// v0.18.9 — author-controlled grid step in mm.
    pub grid_step_mm: f64,
    /// v0.18.19 — fine grid display style.
    pub fine_grid_display: GridDisplay,
    /// v0.18.19 — coarse grid display style.
    pub coarse_grid_display: GridDisplay,
    /// v0.18.19 — coarse-grid multiplier (typically 5 or 10).
    pub coarse_multiplier: u32,
    pub snap_track_vertices: bool,
    pub snap_track_lines: bool,
    pub snap_arc_centers: bool,
    pub snap_intersections: bool,
    pub snap_pad_centers: bool,
    pub snap_pad_vertices: bool,
    pub snap_pad_edges: bool,
    pub snap_via_centers: bool,
    pub snap_texts: bool,
    pub snap_regions: bool,
    pub snap_footprint_origins: bool,
    pub snap_3d_body_points: bool,
    /// v0.13 — Altium "Snap Distance" (mm).
    pub snap_distance_mm: f64,
    /// v0.13 — Altium "Axis Snap Range" (mm).
    pub axis_snap_range_mm: f64,
    pub snap_to_grids: bool,
    pub snap_to_guides: bool,
    pub snap_to_axes: bool,
    /// What a board sketch snaps to on the board under it (#2518). A
    /// footprint sketch has no board, so it never reads these.
    pub board_features: BoardFeatureSnaps,
    /// A sketch drawing tool snaps to a Line's midpoint and lands a
    /// Midpoint constraint (mockup row 188).
    pub snap_midpoints: bool,
    /// A sketch drawing tool snaps to a Circle's or an Arc's centre and
    /// lands a Coincident on it (mockup row 188).
    pub snap_centres: bool,
}

/// The board-feature snaps of a board sketch: one switch for all of them
/// and one per kind, all on by default. A board-feature snap places the
/// point only; it lands no constraint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoardFeatureSnaps {
    pub enabled: bool,
    pub outline: bool,
    pub pads: bool,
    pub holes: bool,
    pub vias: bool,
    pub track_ends: bool,
    pub courtyards: bool,
}

impl Default for BoardFeatureSnaps {
    fn default() -> Self {
        Self {
            enabled: true,
            outline: true,
            pads: true,
            holes: true,
            vias: true,
            track_ends: true,
            courtyards: true,
        }
    }
}

impl Default for SnapOptions {
    fn default() -> Self {
        Self {
            point_hit: true,
            horizontal_vertical: true,
            angle: true,
            grid: true,
            grid_step_mm: 1.0,
            fine_grid_display: GridDisplay::Lines,
            coarse_grid_display: GridDisplay::Lines,
            coarse_multiplier: 5,
            snap_track_vertices: true,
            snap_track_lines: false,
            snap_arc_centers: true,
            snap_intersections: false,
            snap_pad_centers: true,
            snap_pad_vertices: false,
            snap_pad_edges: false,
            snap_via_centers: true,
            snap_texts: false,
            snap_regions: false,
            snap_footprint_origins: true,
            snap_3d_body_points: false,
            snap_distance_mm: 0.203,
            axis_snap_range_mm: 5.08,
            snap_to_grids: true,
            snap_to_guides: false,
            snap_to_axes: false,
            board_features: BoardFeatureSnaps::default(),
            snap_midpoints: true,
            snap_centres: true,
        }
    }
}

impl SnapOptions {
    /// What Snap Distance and Axis Snap Range take, in mm.
    pub const REACH_RANGE_MM: std::ops::RangeInclusive<f64> = 0.001..=100.0;
    /// The `numeric_buffers` key of Snap Distance's refused text.
    pub const DISTANCE_BUFFER: &'static str = "snap.distance";
    /// The `numeric_buffers` key of Axis Snap Range's refused text.
    pub const AXIS_RANGE_BUFFER: &'static str = "snap.axis_range";

    /// Whether the cursor rounds to the grid: the Grids switch of the Snap
    /// menu and Properties is on, and the step is usable.
    pub fn snaps_to_grid(&self) -> bool {
        self.grid && self.snap_to_grids && self.grid_step_mm > 1e-9
    }
}

/// v0.18.19 — Altium grid display style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GridDisplay {
    #[default]
    Lines,
    Dots,
    Hidden,
}

/// v0.18.20 — Altium-style guide line. One of `x` / `y` is set,
/// representing a vertical (X = const) or horizontal (Y = const)
/// guide.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Guide {
    pub axis: GuideAxis,
    pub position_mm: f64,
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GuideAxis {
    #[default]
    Vertical,
    Horizontal,
}

/// v0.18.21 — One row in the Cartesian Grid Manager. Each grid is a
/// named (step / fine_display / coarse_display / multiplier) bundle.
#[derive(Debug, Clone, PartialEq)]
pub struct GridDef {
    pub name: String,
    pub step_mm: f64,
    pub fine_display: GridDisplay,
    pub coarse_display: GridDisplay,
    pub coarse_multiplier: u32,
}

impl Default for GridDef {
    fn default() -> Self {
        Self {
            name: "Grid".into(),
            step_mm: 1.0,
            fine_display: GridDisplay::Lines,
            coarse_display: GridDisplay::Lines,
            coarse_multiplier: 5,
        }
    }
}

impl GridDef {
    /// Seed the implicit "Global Snap Grid" row from a `SnapOptions`
    /// snapshot. Used when the FootprintEditorState first materialises
    /// to keep the legacy single-grid behaviour intact.
    pub fn from_snap_options(opts: &SnapOptions) -> Self {
        Self {
            name: "Global Snap Grid".into(),
            step_mm: opts.grid_step_mm,
            fine_display: opts.fine_grid_display,
            coarse_display: opts.coarse_grid_display,
            coarse_multiplier: opts.coarse_multiplier,
        }
    }
}

/// v0.18.13 — Altium Snap Options sub-tabs (Grids / Guides / Axes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SnapSubTab {
    #[default]
    Grids,
    Guides,
    Axes,
}

/// v0.18.13 — Altium Snapping mode (3-state segment).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SnappingMode {
    #[default]
    AllLayers,
    CurrentLayer,
    Off,
}
