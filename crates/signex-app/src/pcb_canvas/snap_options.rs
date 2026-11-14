//! How carried board items snap (#2353): to the grid, and to the points of
//! board objects near the pointer. Editor state for the session; the board
//! file holds only the grid step.

use std::collections::BTreeSet;

use signex_engine::pcb::snap::SnapObject;
use signex_types::pcb_layer::CopperLayer;

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
    pub scope: SnapScope,
    /// The object points snapped to.
    pub objects: BTreeSet<SnapObject>,
    pub distance: SnapDistance,
}

impl Default for PcbSnapOptions {
    fn default() -> Self {
        Self {
            to_grids: true,
            scope: SnapScope::default(),
            objects: SnapObject::DEFAULT_ON.into_iter().collect(),
            distance: SnapDistance::default(),
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
}
