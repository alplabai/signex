//! The four layouts Signex ships. Contents: docs/WORKSPACE_LAYOUTS.md,
//! "Built-in layouts".

use super::model::{BuiltinLayout, LayoutId, WorkspaceLayout};
use crate::dock::{
    DEFAULT_LEFT_DOCK_WIDTH, DEFAULT_RIGHT_DOCK_WIDTH, DockLayoutSnapshot, RegionLayout,
};
use crate::panels::PanelKind;

fn side(panels: &[PanelKind], width: f32) -> RegionLayout {
    RegionLayout {
        panels: panels.to_vec(),
        collapsed: false,
        width: Some(width.round() as u32),
        kept: Vec::new(),
    }
}

fn bottom(panels: &[PanelKind]) -> RegionLayout {
    RegionLayout {
        panels: panels.to_vec(),
        collapsed: false,
        width: None,
        kept: Vec::new(),
    }
}

/// The docks of `b` as shipped: left, right and bottom panels in tab order.
pub fn shipped_dock(b: BuiltinLayout) -> DockLayoutSnapshot {
    use PanelKind::*;
    let (left, right, below): (&[PanelKind], &[PanelKind], &[PanelKind]) = match b {
        BuiltinLayout::Schematic => (&[Projects, Components], &[Properties], &[Messages]),
        // The mock's "Layers strip" is the Layers panel in the bottom dock.
        BuiltinLayout::Layout => (&[Projects], &[Properties, PcbNets], &[Layers]),
        BuiltinLayout::Review => (&[], &[History], &[Messages, CommandLog]),
        BuiltinLayout::Notebook => (&[Projects], &[Annotations, Properties], &[]),
    };
    DockLayoutSnapshot {
        left: side(left, DEFAULT_LEFT_DOCK_WIDTH),
        right: side(right, DEFAULT_RIGHT_DOCK_WIDTH),
        bottom: bottom(below),
    }
}

/// `b` as shipped: its docks, no split, never saved.
pub fn shipped(b: BuiltinLayout) -> WorkspaceLayout {
    WorkspaceLayout {
        id: LayoutId::Builtin(b),
        name: String::new(),
        dock: Some(shipped_dock(b)),
        split: None,
        saved_at: None,
        from_current: false,
    }
}
