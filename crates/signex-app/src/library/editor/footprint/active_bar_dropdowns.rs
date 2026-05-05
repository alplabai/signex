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

use signex_types::theme::ThemeId;
use signex_widgets::active_bar_dropdown::{DropdownEntry, DropdownItem};

use crate::icons as ic;
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

/// "Coming soon" stub item — no icon.
fn stub(label: &'static str, path: PathBuf) -> DropdownItem<LibraryMessage> {
    DropdownItem::new(
        label,
        fp(path, PrimitiveEditorMsg::FootprintActiveBarStub(label)),
    )
}

/// "Coming soon" stub item with an icon for visual recognition.
fn stub_with_icon(
    label: &'static str,
    path: PathBuf,
    icon: iced::widget::svg::Handle,
) -> DropdownItem<LibraryMessage> {
    DropdownItem::new(
        label,
        fp(path, PrimitiveEditorMsg::FootprintActiveBarStub(label)),
    )
    .icon(icon)
}

/// Build the entries for the dropdown matching `menu`. `tid` resolves
/// the per-theme accent tint on each SVG icon (icons are reused from
/// the schematic active bar's icon set for visual consistency).
pub fn entries(
    menu: FpActiveBarMenu,
    state: &FootprintEditorState,
    path: PathBuf,
    tid: ThemeId,
) -> Vec<DropdownEntry<LibraryMessage>> {
    match menu {
        FpActiveBarMenu::Filter => filter_entries(state, path),
        FpActiveBarMenu::Snap => snap_entries(state, path),
        FpActiveBarMenu::Place => place_entries(path, tid),
        FpActiveBarMenu::Select => select_entries(path, tid),
        FpActiveBarMenu::Align => align_entries(path, tid),
        FpActiveBarMenu::Body3d => body3d_entries(state, path),
        FpActiveBarMenu::Text => text_entries(state, path, tid),
        FpActiveBarMenu::Shapes => shapes_entries(path, tid),
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

fn place_entries(path: PathBuf, tid: ThemeId) -> Vec<DropdownEntry<LibraryMessage>> {
    vec![
        DropdownEntry::Item(stub_with_icon("Move", path.clone(), ic::icon_dd_move(tid))),
        DropdownEntry::Item(stub_with_icon("Drag", path.clone(), ic::icon_dd_drag(tid))),
        DropdownEntry::Item(stub("Break Track", path.clone())),
        DropdownEntry::Item(stub("Drag Track End", path.clone())),
        DropdownEntry::Separator,
        DropdownEntry::Item(stub_with_icon(
            "Move Selection",
            path.clone(),
            ic::icon_dd_move_sel(tid),
        )),
        DropdownEntry::Item(stub_with_icon(
            "Move Selection by X, Y…",
            path.clone(),
            ic::icon_dd_move_xy(tid),
        )),
        DropdownEntry::Item(stub_with_icon(
            "Rotate Selection",
            path.clone(),
            ic::icon_dd_rotate(tid),
        )),
        DropdownEntry::Item(stub_with_icon(
            "Flip Selection",
            path,
            ic::icon_dd_flip_x(tid),
        )),
    ]
}

fn select_entries(path: PathBuf, tid: ThemeId) -> Vec<DropdownEntry<LibraryMessage>> {
    vec![
        DropdownEntry::Item(stub("Select overlapped", path.clone())),
        DropdownEntry::Item(stub("Select next", path.clone())),
        DropdownEntry::Item(stub_with_icon(
            "Lasso Select",
            path.clone(),
            ic::icon_dd_select_lasso(tid),
        )),
        DropdownEntry::Separator,
        DropdownEntry::Item(stub_with_icon(
            "Inside Area",
            path.clone(),
            ic::icon_dd_select_inside(tid),
        )),
        DropdownEntry::Item(stub_with_icon(
            "Outside Area",
            path.clone(),
            ic::icon_dd_select_outside(tid),
        )),
        DropdownEntry::Item(stub("Touching Rectangle", path.clone())),
        DropdownEntry::Item(stub("Touching Line", path.clone())),
        DropdownEntry::Separator,
        DropdownEntry::Item(stub("All on Layer", path.clone())),
        DropdownEntry::Item(stub_with_icon(
            "All",
            path.clone(),
            ic::icon_dd_select_all(tid),
        )),
        DropdownEntry::Item(stub("Off Grid Pads", path.clone())),
        DropdownEntry::Separator,
        DropdownEntry::Item(stub_with_icon(
            "Toggle Selection",
            path,
            ic::icon_dd_select_toggle(tid),
        )),
    ]
}

fn align_entries(path: PathBuf, tid: ThemeId) -> Vec<DropdownEntry<LibraryMessage>> {
    vec![
        DropdownEntry::Item(stub_with_icon(
            "Align…",
            path.clone(),
            ic::icon_dd_align_menu(tid),
        )),
        DropdownEntry::Separator,
        DropdownEntry::Item(stub_with_icon(
            "Align Left",
            path.clone(),
            ic::icon_dd_align_left(tid),
        )),
        DropdownEntry::Item(stub_with_icon(
            "Align Right",
            path.clone(),
            ic::icon_dd_align_right(tid),
        )),
        DropdownEntry::Item(stub("Align Left (maintain spacing)", path.clone())),
        DropdownEntry::Item(stub("Align Right (maintain spacing)", path.clone())),
        DropdownEntry::Item(stub_with_icon(
            "Align Horizontal Centers",
            path.clone(),
            ic::icon_dd_align_hcenter(tid),
        )),
        DropdownEntry::Item(stub_with_icon(
            "Distribute Horizontally",
            path.clone(),
            ic::icon_dd_dist_horiz(tid),
        )),
        DropdownEntry::Item(stub("Increase Horizontal Spacing", path.clone())),
        DropdownEntry::Item(stub("Decrease Horizontal Spacing", path.clone())),
        DropdownEntry::Separator,
        DropdownEntry::Item(stub_with_icon(
            "Align Top",
            path.clone(),
            ic::icon_dd_align_top(tid),
        )),
        DropdownEntry::Item(stub_with_icon(
            "Align Bottom",
            path.clone(),
            ic::icon_dd_align_bottom(tid),
        )),
        DropdownEntry::Item(stub("Align Top (maintain spacing)", path.clone())),
        DropdownEntry::Item(stub("Align Bottom (maintain spacing)", path.clone())),
        DropdownEntry::Item(stub_with_icon(
            "Align Vertical Centers",
            path.clone(),
            ic::icon_dd_align_vcenter(tid),
        )),
        DropdownEntry::Item(stub_with_icon(
            "Distribute Vertically",
            path.clone(),
            ic::icon_dd_dist_vert(tid),
        )),
        DropdownEntry::Item(stub("Increase Vertical Spacing", path.clone())),
        DropdownEntry::Item(stub("Decrease Vertical Spacing", path.clone())),
        DropdownEntry::Separator,
        DropdownEntry::Item(stub_with_icon(
            "Align To Grid",
            path.clone(),
            ic::icon_dd_align_grid(tid),
        )),
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
    tid: ThemeId,
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
            .icon(ic::icon_dd_text_string(tid))
            .checked(active == PadsTool::PlaceString),
        ),
        DropdownEntry::Item(stub_with_icon(
            "Text Frame",
            path,
            ic::icon_dd_text_frame(tid),
        )),
    ]
}

fn shapes_entries(path: PathBuf, tid: ThemeId) -> Vec<DropdownEntry<LibraryMessage>> {
    // Per user simplification: pure graphics live in Sketch mode
    // only. From Pads mode, Shapes opens the menu but every item is
    // a stub that hints "switch to Sketch mode for graphics".
    vec![
        DropdownEntry::Header("(Sketch mode only — switch via the mode bar)".into()),
        DropdownEntry::Item(stub_with_icon("Line", path.clone(), ic::icon_dd_line(tid))),
        DropdownEntry::Item(stub_with_icon(
            "Arc (Center)",
            path.clone(),
            ic::icon_dd_arc(tid),
        )),
        DropdownEntry::Item(stub_with_icon(
            "Arc (Edge)",
            path.clone(),
            ic::icon_dd_arc(tid),
        )),
        DropdownEntry::Item(stub_with_icon(
            "Arc (Any Angle)",
            path.clone(),
            ic::icon_dd_arc(tid),
        )),
        DropdownEntry::Item(stub_with_icon(
            "Full Circle",
            path.clone(),
            ic::icon_dd_circle(tid),
        )),
        DropdownEntry::Separator,
        DropdownEntry::Item(stub_with_icon(
            "Fill",
            path.clone(),
            ic::icon_dd_polygon(tid),
        )),
        DropdownEntry::Item(stub_with_icon(
            "Solid Region",
            path.clone(),
            ic::icon_dd_polygon(tid),
        )),
        DropdownEntry::Item(stub_with_icon(
            "Rectangle",
            path,
            ic::icon_dd_rect(tid),
        )),
    ]
}
