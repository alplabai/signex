//! The Gerber Viewer's annotations (#2896, mockup row 193). Opening reads
//! only its `path`. A switch without `on` (Flip, Drills, the Open menu)
//! reads its own state; a selection by point reads which layers are shown
//! and the drawing order; Highlight net without `net` reads the selected
//! object: view state the person sets while working, so tool state, and
//! the viewer's selection.

use super::super::forms::Form::{Bridge, HeadlessWithoutOperands, Operand};
use super::super::inputs::Input::{Selection, ToolState};
use super::Annotation;

const SWITCHES: &[&str] = &[
    "flip_gerber_view",
    "show_gerber_drills_on_top",
    "show_gerber_open_menu",
];

pub(super) const GERBER_VIEWER: &[Annotation] = &[
    // The tab opens on nothing implicit.
    (&["open_gerber_viewer"], Bridge, &[]),
    (&["open_gerber_viewer"], Operand, &[]),
    (&["open_gerber_viewer"], HeadlessWithoutOperands, &[]),
    (SWITCHES, Bridge, &[ToolState]),
    (SWITCHES, Operand, &[ToolState]),
    (SWITCHES, HeadlessWithoutOperands, &[ToolState]),
    // The palette form only clears the selection.
    (&["select_gerber_object"], Bridge, &[]),
    (&["select_gerber_object"], Operand, &[ToolState]),
    (&["select_gerber_object"], HeadlessWithoutOperands, &[]),
    (&["highlight_gerber_net"], Bridge, &[Selection, ToolState]),
    (&["highlight_gerber_net"], Operand, &[Selection, ToolState]),
    (
        &["highlight_gerber_net"],
        HeadlessWithoutOperands,
        &[Selection, ToolState],
    ),
];
