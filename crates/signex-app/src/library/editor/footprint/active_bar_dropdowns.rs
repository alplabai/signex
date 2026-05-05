//! v0.13 — Footprint editor active-bar dropdown menu definitions.
//!
//! Each `FpActiveBarMenu` variant maps to a function that returns the
//! list of `DropdownEntry<LibraryMessage>` rows. Rendering happens in
//! `signex_widgets::active_bar_dropdown::view`; overlay positioning is
//! handled by the caller (`unified_active_bar`).
//!
//! Wiring philosophy: items that map to existing primitives (Selection
//! Filter pills, Snap toggles, snap-mode picks, Place tools) emit the
//! real `PrimitiveEditorMsg`; items that need new primitives
//! (Move/Drag/Selection-mode picks / Body3D / TextFrame) emit
//! `FootprintActiveBarStub` so the action logs a "coming soon" warn
//! and dismisses the menu cleanly.

use std::path::PathBuf;

use signex_widgets::active_bar_dropdown::{DropdownEntry, DropdownItem};

use crate::library::editor::footprint::state::{
    FpActiveBarMenu, PadsTool, SelectionFilterKind, SnapSubTab, SnappingMode,
};
use crate::library::messages::{LibraryMessage, PrimitiveEditorMsg};
use crate::panels::SnapOptionFlag;

use super::state::FootprintEditorState;

/// Convenience: route a `PrimitiveEditorMsg` to the editor at `path`.
fn fp(path: PathBuf, msg: PrimitiveEditorMsg) -> LibraryMessage {
    LibraryMessage::PrimitiveEditorEvent { path, msg }
}

/// "Coming soon" stub item.
fn stub(label: &'static str, path: PathBuf) -> DropdownItem<LibraryMessage> {
    DropdownItem::new(
        label,
        fp(path, PrimitiveEditorMsg::FootprintActiveBarStub(label)),
    )
}

/// Build the entries for the dropdown matching `menu`.
pub fn entries(
    menu: FpActiveBarMenu,
    state: &FootprintEditorState,
    path: PathBuf,
) -> Vec<DropdownEntry<LibraryMessage>> {
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
) -> Vec<DropdownEntry<LibraryMessage>> {
    use SelectionFilterKind as K;
    let f = state.selection_filter;
    let mk = |label: &'static str, kind: K| -> DropdownItem<LibraryMessage> {
        DropdownItem::new(
            label,
            fp(
                path.clone(),
                PrimitiveEditorMsg::FootprintToggleSelectionFilter(kind),
            ),
        )
        .checked(f.get(kind))
    };
    vec![
        DropdownEntry::Header("Selection Filter".into()),
        DropdownEntry::Item(mk("3D Bodies", K::Bodies3d)),
        DropdownEntry::Item(mk("Keepouts", K::Keepouts)),
        DropdownEntry::Item(mk("Tracks", K::Tracks)),
        DropdownEntry::Item(mk("Arcs", K::Arcs)),
        DropdownEntry::Item(mk("Pads", K::Pads)),
        DropdownEntry::Item(mk("Vias", K::Vias)),
        DropdownEntry::Item(mk("Regions", K::Regions)),
        DropdownEntry::Item(mk("Fills", K::Fills)),
        DropdownEntry::Item(mk("Texts", K::Texts)),
        DropdownEntry::Item(mk("Other", K::Other)),
    ]
}

fn snap_entries(
    state: &FootprintEditorState,
    path: PathBuf,
) -> Vec<DropdownEntry<LibraryMessage>> {
    let opts = state.snap_options;
    let mk_mode = |label: &'static str, mode: SnappingMode| -> DropdownItem<LibraryMessage> {
        DropdownItem::new(
            label,
            fp(
                path.clone(),
                PrimitiveEditorMsg::FootprintActiveBarSetSnappingMode(mode),
            ),
        )
        .checked(state.snapping_mode == mode)
    };
    let mk_sub = |label: &'static str, sub: SnapSubTab| -> DropdownItem<LibraryMessage> {
        DropdownItem::new(
            label,
            fp(
                path.clone(),
                PrimitiveEditorMsg::FootprintActiveBarSetSnapSubTab(sub),
            ),
        )
        .checked(state.snap_subtab == sub)
    };
    let mk_snap =
        |label: &'static str, flag: SnapOptionFlag, on: bool| -> DropdownItem<LibraryMessage> {
            DropdownItem::new(
                label,
                fp(
                    path.clone(),
                    PrimitiveEditorMsg::FootprintActiveBarToggleSnap(flag),
                ),
            )
            .checked(on)
        };
    vec![
        DropdownEntry::Header("Snapping".into()),
        DropdownEntry::Item(mk_mode("All Layers", SnappingMode::AllLayers)),
        DropdownEntry::Item(mk_mode("Current Layer", SnappingMode::CurrentLayer)),
        DropdownEntry::Item(mk_mode("Off", SnappingMode::Off)),
        DropdownEntry::Separator,
        DropdownEntry::Header("Sub-tab".into()),
        DropdownEntry::Item(mk_sub("Grids", SnapSubTab::Grids)),
        DropdownEntry::Item(mk_sub("Guides", SnapSubTab::Guides)),
        DropdownEntry::Item(mk_sub("Axes", SnapSubTab::Axes)),
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

fn place_entries(path: PathBuf) -> Vec<DropdownEntry<LibraryMessage>> {
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

fn select_entries(path: PathBuf) -> Vec<DropdownEntry<LibraryMessage>> {
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

fn align_entries(path: PathBuf) -> Vec<DropdownEntry<LibraryMessage>> {
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
) -> Vec<DropdownEntry<LibraryMessage>> {
    vec![
        DropdownEntry::Item(stub("3D Body", path.clone())),
        DropdownEntry::Item(stub("Extruded 3D Body", path)),
    ]
}

fn text_entries(
    state: &FootprintEditorState,
    path: PathBuf,
) -> Vec<DropdownEntry<LibraryMessage>> {
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

fn shapes_entries(path: PathBuf) -> Vec<DropdownEntry<LibraryMessage>> {
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
