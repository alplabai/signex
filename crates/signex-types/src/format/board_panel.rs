//! The `[board_options.panel]` table of a `.snxpcb` (#2410): the board's
//! panel. Lengths are integer nm like every other on-disk geometry; a panel
//! that breaks a rule of [`BoardPanel::check`] is refused on load, so the
//! board engine never holds one. See `docs/PANELISATION.md`.

use serde::{Deserialize, Serialize};

use super::FormatError;
use super::units::{mm_to_nm, nm_to_mm};
use crate::pcb_panel::{BoardPanel, MouseBites, PanelRails, PanelSeparation};

const PANEL: &str = "board_options.panel";

/// `[board_options.panel]` as stored.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(in crate::format) struct PanelRaw {
    columns: u32,
    rows: u32,
    spacing_x: i64,
    spacing_y: i64,
    rail_top: i64,
    rail_bottom: i64,
    rail_left: i64,
    rail_right: i64,
    fiducial_diameter: i64,
    tooling_hole_diameter: i64,
    separation: PanelSeparation,
    bite_hole_diameter: i64,
    bite_pitch: i64,
    tab_width: i64,
    tabs_per_edge: u32,
}

/// The panel the table holds, checked.
pub(in crate::format) fn read(raw: PanelRaw) -> Result<BoardPanel, FormatError> {
    let panel = BoardPanel {
        columns: raw.columns,
        rows: raw.rows,
        spacing_x: nm_to_mm(raw.spacing_x),
        spacing_y: nm_to_mm(raw.spacing_y),
        rails: PanelRails {
            top: nm_to_mm(raw.rail_top),
            bottom: nm_to_mm(raw.rail_bottom),
            left: nm_to_mm(raw.rail_left),
            right: nm_to_mm(raw.rail_right),
        },
        fiducial_diameter: nm_to_mm(raw.fiducial_diameter),
        tooling_hole_diameter: nm_to_mm(raw.tooling_hole_diameter),
        separation: raw.separation,
        bites: MouseBites {
            hole_diameter: nm_to_mm(raw.bite_hole_diameter),
            pitch: nm_to_mm(raw.bite_pitch),
            tab_width: nm_to_mm(raw.tab_width),
            tabs_per_edge: raw.tabs_per_edge,
        },
    };
    panel.check().map_err(|error| FormatError::TsvFieldParse {
        block: PANEL.to_string(),
        row: 0,
        field: "panel".to_string(),
        message: error.to_string(),
    })?;
    Ok(panel)
}

/// The table for `panel`.
pub(in crate::format) fn write(panel: &BoardPanel) -> Result<PanelRaw, FormatError> {
    let nm = |mm: f64, field: &'static str| {
        mm_to_nm(mm, field).map_err(|e| FormatError::NonFiniteField {
            block: PANEL.to_string(),
            row: 0,
            field: e.0.to_string(),
        })
    };
    let (rails, bites) = (&panel.rails, &panel.bites);
    Ok(PanelRaw {
        columns: panel.columns,
        rows: panel.rows,
        spacing_x: nm(panel.spacing_x, "spacing_x")?,
        spacing_y: nm(panel.spacing_y, "spacing_y")?,
        rail_top: nm(rails.top, "rail_top")?,
        rail_bottom: nm(rails.bottom, "rail_bottom")?,
        rail_left: nm(rails.left, "rail_left")?,
        rail_right: nm(rails.right, "rail_right")?,
        fiducial_diameter: nm(panel.fiducial_diameter, "fiducial_diameter")?,
        tooling_hole_diameter: nm(panel.tooling_hole_diameter, "tooling_hole_diameter")?,
        separation: panel.separation,
        bite_hole_diameter: nm(bites.hole_diameter, "bite_hole_diameter")?,
        bite_pitch: nm(bites.pitch, "bite_pitch")?,
        tab_width: nm(bites.tab_width, "tab_width")?,
        tabs_per_edge: bites.tabs_per_edge,
    })
}
