//! v0.13 — Footprint editor active-bar dropdown menu definitions.
//!
//! Each `FpActiveBarMenu` variant maps to a function that returns
//! the list of `DropdownEntry<LibraryMessage>` rows. The actual
//! rendering happens in `signex_widgets::active_bar_dropdown::view`,
//! and the overlay positioning is handled by `pads_active_bar`.
//!
//! Wiring philosophy: items that map to existing primitives (Selection
//! Filter pills, Snap toggles, Place tools) emit the real message;
//! items that need new primitives (Move/Drag/Selection-mode picks /
//! Body3D / TextFrame) emit `FootprintActiveBarStub` so the action
//! logs a "coming soon" warn and dismisses the menu cleanly.

use std::path::PathBuf;

use signex_widgets::active_bar_dropdown::{DropdownEntry, DropdownItem};

use crate::app::Message;
use crate::dock::DockMessage;
use crate::library::editor::footprint::state::{
    FpActiveBarMenu, PadsTool, SelectionFilterKind, SnapSubTab, SnappingMode,
};
use crate::library::messages::{LibraryMessage, PrimitiveEditorMsg};
use crate::panels::{PanelMsg, SnapOptionFlag};

use super::state::FootprintEditorState;

/// Convenience: route a `PrimitiveEditorMsg` to the editor at `path`.
fn fp(path: PathBuf, msg: PrimitiveEditorMsg) -> Message {
    Message::Library(LibraryMessage::PrimitiveEditorEvent { path, msg })
}

/// Convenience: route a `PanelMsg` (typically toggle-state messages
/// shared with the right-dock Properties panel).
fn panel(msg: PanelMsg) -> Message {
    Message::Dock(DockMessage::Panel(msg))
}

/// Convenience: build a "coming soon" stub item.
fn stub(label: &'static str, path: PathBuf) -> DropdownItem<Message> {
    DropdownItem::new(
        label,
        fp(path, PrimitiveEditorMsg::FootprintActiveBarStub(label)),
    )
}

/// Build the entries for the dropdown matching `menu`.
///
/// `state` carries the live FootprintEditorState so item check-state
/// reflects the actual flags. `path` identifies which editor publishes
/// the message (multi-window: each tab has its own state + path).
pub fn entries(
    menu: FpActiveBarMenu,
    state: &FootprintEditorState,
    path: PathBuf,
) -> Vec<DropdownEntry<Message>> {
    match menu {
        FpActiveBarMenu::Filter => filter_entries(state, path),
        FpActiveBarMenu::Snap => snap_entries(state, path),
        FpActiveBarMenu::Place => place_entries(path),
        FpActiveBarMenu::Select => select_entries(path),
        FpActiveBarMenu::Align => align_entries(path),
        FpActiveBarMenu::Body3d => body3d_entries(state, path),
        FpActiveBarMenu::Text => text_entries(state, path),
        FpActiveBarMenu::Shapes => shapes_entries(path),
    }
}

fn filter_entries(
    state: &FootprintEditorState,
    path: PathBuf,
) -> Vec<DropdownEntry<Message>> {
    use SelectionFilterKind as K;
    let f = state.selection_filter;
    let mk_filter_item = |label: &'static str, kind: K| -> DropdownItem<Message> {
        // Filter toggles route through the panel-level message system
        // so the existing FpEditorToggleSelectionFilter dispatcher
        // handles them. The active-bar wraps it in a LibraryMessage
        // via PanelEvent.
        DropdownItem::new(
            label,
            panel(PanelMsg::FpEditorToggleSelectionFilter(kind)),
        )
        .checked(f.get(kind))
    };
    let _ = path;
    vec![
        DropdownEntry::Header("Selection Filter".into()),
        DropdownEntry::Item(mk_filter_item("3D Bodies", K::Bodies3d)),
        DropdownEntry::Item(mk_filter_item("Keepouts", K::Keepouts)),
        DropdownEntry::Item(mk_filter_item("Tracks", K::Tracks)),
        DropdownEntry::Item(mk_filter_item("Arcs", K::Arcs)),
        DropdownEntry::Item(mk_filter_item("Pads", K::Pads)),
        DropdownEntry::Item(mk_filter_item("Vias", K::Vias)),
        DropdownEntry::Item(mk_filter_item("Regions", K::Regions)),
        DropdownEntry::Item(mk_filter_item("Fills", K::Fills)),
        DropdownEntry::Item(mk_filter_item("Texts", K::Texts)),
        DropdownEntry::Item(mk_filter_item("Other", K::Other)),
    ]
}

fn snap_entries(
    state: &FootprintEditorState,
    path: PathBuf,
) -> Vec<DropdownEntry<Message>> {
    let opts = state.snap_options;
    let mk_snap = |label: &'static str, flag: SnapOptionFlag, on: bool| -> DropdownItem<Message> {
        DropdownItem::new(
            label,
            panel(PanelMsg::FpEditorToggleSnapOption(flag)),
        )
        .checked(on)
    };
    let _ = path;
    vec![
        DropdownEntry::Header("Snapping".into()),
        DropdownEntry::Item(
            DropdownItem::new(
                "All Layers",
                panel(PanelMsg::FpEditorSetSnappingMode(SnappingMode::AllLayers)),
            )
            .checked(state.snapping_mode == SnappingMode::AllLayers),
        ),
        DropdownEntry::Item(
            DropdownItem::new(
                "Current Layer",
                panel(PanelMsg::FpEditorSetSnappingMode(SnappingMode::CurrentLayer)),
            )
            .checked(state.snapping_mode == SnappingMode::CurrentLayer),
        ),
        DropdownEntry::Item(
            DropdownItem::new(
                "Off",
                panel(PanelMsg::FpEditorSetSnappingMode(SnappingMode::Off)),
            )
            .checked(state.snapping_mode == SnappingMode::Off),
        ),
        DropdownEntry::Separator,
        DropdownEntry::Header("Sub-tab".into()),
        DropdownEntry::Item(
            DropdownItem::new(
                "Grids",
                panel(PanelMsg::FpEditorSetSnapSubTab(SnapSubTab::Grids)),
            )
            .checked(state.snap_subtab == SnapSubTab::Grids),
        ),
        DropdownEntry::Item(
            DropdownItem::new(
                "Guides",
                panel(PanelMsg::FpEditorSetSnapSubTab(SnapSubTab::Guides)),
            )
            .checked(state.snap_subtab == SnapSubTab::Guides),
        ),
        DropdownEntry::Item(
            DropdownItem::new(
                "Axes",
                panel(PanelMsg::FpEditorSetSnapSubTab(SnapSubTab::Axes)),
            )
            .checked(state.snap_subtab == SnapSubTab::Axes),
        ),
        DropdownEntry::Separator,
        DropdownEntry::Header("Objects for snapping".into()),
        DropdownEntry::Item(mk_snap(
            "Track Vertices",
            SnapOptionFlag::TrackVertices,
            opts.snap_track_vertices,
        )),
        DropdownEntry::Item(mk_snap(
            "Track Lines",
            SnapOptionFlag::TrackLines,
            opts.snap_track_lines,
        )),
        DropdownEntry::Item(mk_snap(
            "Arc Centers",
            SnapOptionFlag::ArcCenters,
            opts.snap_arc_centers,
        )),
        DropdownEntry::Item(mk_snap(
            "Intersections",
            SnapOptionFlag::Intersections,
            opts.snap_intersections,
        )),
        DropdownEntry::Item(mk_snap(
            "Pad Centers",
            SnapOptionFlag::PadCenters,
            opts.snap_pad_centers,
        )),
        DropdownEntry::Item(mk_snap(
            "Pad Vertices",
            SnapOptionFlag::PadVertices,
            opts.snap_pad_vertices,
        )),
        DropdownEntry::Item(mk_snap(
            "Pad Edges",
            SnapOptionFlag::PadEdges,
            opts.snap_pad_edges,
        )),
        DropdownEntry::Item(mk_snap(
            "Via Centers",
            SnapOptionFlag::ViaCenters,
            opts.snap_via_centers,
        )),
        DropdownEntry::Item(mk_snap("Texts", SnapOptionFlag::Texts, opts.snap_texts)),
        DropdownEntry::Item(mk_snap(
            "Regions",
            SnapOptionFlag::Regions,
            opts.snap_regions,
        )),
        DropdownEntry::Item(mk_snap(
            "Footprint Origins",
            SnapOptionFlag::FootprintOrigins,
            opts.snap_footprint_origins,
        )),
        DropdownEntry::Item(mk_snap(
            "3D Body Snap Points",
            SnapOptionFlag::Body3dPoints,
            opts.snap_3d_body_points,
        )),
    ]
}

fn place_entries(path: PathBuf) -> Vec<DropdownEntry<Message>> {
    vec![
        DropdownEntry::Item(stub("Move", path.clone())),
        DropdownEntry::Item(stub("Drag", path.clone())),
        DropdownEntry::Item(stub("Break Track", path.clone())),
        DropdownEntry::Item(stub("Drag Track End", path.clone())),
        DropdownEntry::Separator,
        DropdownEntry::Item(stub("Move Selection", path.clone())),
        DropdownEntry::Item(stub("Move Selection by X, Y…", path.clone())),
        DropdownEntry::Item(stub("Rotate Selection", path.clone())),
        DropdownEntry::Item(stub("Flip Selection", path)),
    ]
}

fn select_entries(path: PathBuf) -> Vec<DropdownEntry<Message>> {
    vec![
        DropdownEntry::Item(stub("Select overlapped", path.clone())),
        DropdownEntry::Item(stub("Select next", path.clone())),
        DropdownEntry::Item(stub("Lasso Select", path.clone())),
        DropdownEntry::Separator,
        DropdownEntry::Item(stub("Inside Area", path.clone())),
        DropdownEntry::Item(stub("Outside Area", path.clone())),
        DropdownEntry::Item(stub("Touching Rectangle", path.clone())),
        DropdownEntry::Item(stub("Touching Line", path.clone())),
        DropdownEntry::Separator,
        DropdownEntry::Item(stub("All on Layer", path.clone())),
        DropdownEntry::Item(stub("All", path.clone())),
        DropdownEntry::Item(stub("Off Grid Pads", path.clone())),
        DropdownEntry::Separator,
        DropdownEntry::Item(stub("Toggle Selection", path)),
    ]
}

fn align_entries(path: PathBuf) -> Vec<DropdownEntry<Message>> {
    vec![
        DropdownEntry::Item(stub("Align…", path.clone())),
        DropdownEntry::Separator,
        DropdownEntry::Item(stub("Align Left", path.clone())),
        DropdownEntry::Item(stub("Align Right", path.clone())),
        DropdownEntry::Item(stub("Align Left (maintain spacing)", path.clone())),
        DropdownEntry::Item(stub("Align Right (maintain spacing)", path.clone())),
        DropdownEntry::Item(stub("Align Horizontal Centers", path.clone())),
        DropdownEntry::Item(stub("Distribute Horizontally", path.clone())),
        DropdownEntry::Item(stub("Increase Horizontal Spacing", path.clone())),
        DropdownEntry::Item(stub("Decrease Horizontal Spacing", path.clone())),
        DropdownEntry::Separator,
        DropdownEntry::Item(stub("Align Top", path.clone())),
        DropdownEntry::Item(stub("Align Bottom", path.clone())),
        DropdownEntry::Item(stub("Align Top (maintain spacing)", path.clone())),
        DropdownEntry::Item(stub("Align Bottom (maintain spacing)", path.clone())),
        DropdownEntry::Item(stub("Align Vertical Centers", path.clone())),
        DropdownEntry::Item(stub("Distribute Vertically", path.clone())),
        DropdownEntry::Item(stub("Increase Vertical Spacing", path.clone())),
        DropdownEntry::Item(stub("Decrease Vertical Spacing", path.clone())),
        DropdownEntry::Separator,
        DropdownEntry::Item(stub("Align To Grid", path.clone())),
        DropdownEntry::Item(stub("Move All Components Origin To Grid", path)),
    ]
}

fn body3d_entries(
    _state: &FootprintEditorState,
    path: PathBuf,
) -> Vec<DropdownEntry<Message>> {
    vec![
        DropdownEntry::Item(stub("3D Body", path.clone())),
        DropdownEntry::Item(stub("Extruded 3D Body", path)),
    ]
}

fn text_entries(
    state: &FootprintEditorState,
    path: PathBuf,
) -> Vec<DropdownEntry<Message>> {
    let active = state.pads_tool;
    vec![
        DropdownEntry::Item(
            DropdownItem::new(
                "String",
                fp(
                    path.clone(),
                    PrimitiveEditorMsg::FootprintSetPadsTool(PadsTool::PlaceString),
                ),
            )
            .checked(active == PadsTool::PlaceString),
        ),
        DropdownEntry::Item(stub("Text Frame", path)),
    ]
}

fn shapes_entries(path: PathBuf) -> Vec<DropdownEntry<Message>> {
    // Per user simplification: pure graphics live in Sketch mode
    // only. From Pads mode, Shapes opens the menu but every item is
    // a stub that hints "switch to Sketch mode for graphics".
    vec![
        DropdownEntry::Header("(Sketch mode only — switch via the mode bar)".into()),
        DropdownEntry::Item(stub("Line", path.clone())),
        DropdownEntry::Item(stub("Arc (Center)", path.clone())),
        DropdownEntry::Item(stub("Arc (Edge)", path.clone())),
        DropdownEntry::Item(stub("Arc (Any Angle)", path.clone())),
        DropdownEntry::Item(stub("Full Circle", path.clone())),
        DropdownEntry::Separator,
        DropdownEntry::Item(stub("Fill", path.clone())),
        DropdownEntry::Item(stub("Solid Region", path.clone())),
        DropdownEntry::Item(stub("Rectangle", path)),
    ]
}
