//! v0.13 — Footprint editor active-bar dropdown menu definitions.
//!
//! Each `FpActiveBarMenu` variant maps to a function that returns the
//! list of `DropdownEntry<LibraryMessage>` rows. Rendering happens in
//! `signex_widgets::active_bar_dropdown::view`; overlay positioning is
//! handled by the caller (`unified_active_bar`).
//!
//! Wiring philosophy: items that map to existing primitives (Selection
//! Filter pills, Snap toggles, snap-mode picks, Place tools, Drag Track
//! End, Body3D, Extruded 3D Body, Move Selection by X,Y, Text Frame)
//! emit the real `FootprintEditorMsg`; the few items that still need
//! new primitives (Break Track, the generic Align… dialog launcher)
//! emit `FootprintActiveBarStub` so the action logs a "coming soon"
//! warn and dismisses the menu cleanly.

use std::path::PathBuf;

use signex_types::theme::ThemeId;
use signex_widgets::active_bar_dropdown::{DropdownEntry, DropdownItem};

use crate::icons as ic;
use crate::library::editor::footprint::state::{
    FpActiveBarMenu, PadsTool, SelectionFilterKind, SketchTool, SnapSubTab, SnappingMode,
};
use crate::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
use crate::panels::SnapOptionFlag;

use super::state::FootprintEditorState;

/// Convenience: route a `FootprintEditorMsg` to the editor at `path`.
fn fp(path: PathBuf, msg: FootprintEditorMsg) -> LibraryMessage {
    LibraryMessage::PrimitiveEditorEvent {
        path,
        msg: PrimitiveEdit::Footprint(msg),
    }
}

/// "Coming soon" stub item — no icon.
fn stub(label: &'static str, path: PathBuf) -> DropdownItem<LibraryMessage> {
    DropdownItem::new(label, fp(path, FootprintEditorMsg::ActiveBarStub(label)))
}

/// "Coming soon" stub item with an icon for visual recognition.
fn stub_with_icon(
    label: &'static str,
    path: PathBuf,
    icon: iced::widget::svg::Handle,
) -> DropdownItem<LibraryMessage> {
    DropdownItem::new(label, fp(path, FootprintEditorMsg::ActiveBarStub(label))).icon(icon)
}

/// v0.14 — real Align/Distribute/Spacing item, no icon. Emits
/// [`FootprintEditorMsg::AlignPads`] so the dispatcher
/// transforms the current pad selection.
fn align_item(
    label: &'static str,
    path: PathBuf,
    op: crate::library::editor::footprint::state::AlignOp,
) -> DropdownItem<LibraryMessage> {
    DropdownItem::new(label, fp(path, FootprintEditorMsg::AlignPads(op)))
}

/// v0.14 — real Align/Distribute item with an icon.
fn align_item_with_icon(
    label: &'static str,
    path: PathBuf,
    op: crate::library::editor::footprint::state::AlignOp,
    icon: iced::widget::svg::Handle,
) -> DropdownItem<LibraryMessage> {
    DropdownItem::new(label, fp(path, FootprintEditorMsg::AlignPads(op))).icon(icon)
}

/// Build the entries for the dropdown matching `menu`. `tid` resolves
/// the per-theme accent tint on each SVG icon (icons are reused from
/// the schematic active bar's icon set for visual consistency).
/// `footprint_presets` are the named multi-preset shortcuts shown on
/// row 1 of the Filter dropdown — footprint-native presets keyed on
/// `SelectionFilterKind` (Task 6), not the schematic
/// `CustomFilterPreset`.
pub fn entries(
    menu: FpActiveBarMenu,
    state: &FootprintEditorState,
    path: PathBuf,
    tid: ThemeId,
    footprint_presets: &[crate::active_bar::FootprintFilterPreset],
) -> Vec<DropdownEntry<LibraryMessage>> {
    match menu {
        FpActiveBarMenu::Filter => filter_entries(state, path, footprint_presets),
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
    footprint_presets: &[crate::active_bar::FootprintFilterPreset],
) -> Vec<DropdownEntry<LibraryMessage>> {
    use SelectionFilterKind as K;
    use iced::widget::{column, container, row};
    use iced::{Color, Length};
    use signex_widgets::active_bar_dropdown::chip_btn;

    let f = state.selection_filter;
    // Theme accent — matches the schematic Filter dropdown chips.
    let chip_border = Color::from_rgba8(0xE7, 0x8B, 0x2A, 1.0);

    // Chip factory delegates to the shared `chip_btn` helper so PCB +
    // schematic + footprint render identical chip chrome.
    let make_chip = |label: &'static str, kind: K| -> iced::Element<'static, LibraryMessage> {
        chip_btn(
            label,
            LibraryMessage::PrimitiveEditorEvent {
                path: path.clone(),
                msg: PrimitiveEdit::Footprint(FootprintEditorMsg::ToggleSelectionFilter(kind)),
            },
            f.get(kind),
            chip_border,
        )
    };

    // All-On / All-Off toggle: click flips every kind. Domain must
    // match `FootprintToggleAllFilters`'s handler (SelectionFilterKind::ALL,
    // 12 kinds — set_all's full domain), not the 10-kind ALTIUM_PILLS
    // chip-row subset, or the label can lie (e.g. "All - On" while
    // Pours/Cutouts are off) and a click flips the wrong direction.
    let all_on = K::ALL.iter().all(|k| f.get(*k));
    let all_btn = chip_btn(
        if all_on { "All - On" } else { "All - Off" },
        LibraryMessage::PrimitiveEditorEvent {
            path: path.clone(),
            msg: PrimitiveEdit::Footprint(FootprintEditorMsg::ToggleAllFilters),
        },
        all_on,
        chip_border,
    );

    // Top row: All toggle + footprint-preset shortcut chips + a
    // minimal capture affordance. Task 6 — no rename UI yet; captured
    // presets get a default `Filter {n}` name (see filter_presets.rs).
    let mut top_row = iced::widget::Row::new()
        .spacing(4)
        .align_y(iced::Alignment::Center)
        .push(all_btn);
    for (idx, preset) in footprint_presets.iter().enumerate() {
        let label = if preset.name.trim().is_empty() {
            format!("Filter {}", idx + 1)
        } else {
            preset.name.clone()
        };
        top_row = top_row.push(chip_btn(
            label,
            LibraryMessage::PrimitiveEditorEvent {
                path: path.clone(),
                msg: PrimitiveEdit::Footprint(FootprintEditorMsg::ApplyFilterPreset(idx)),
            },
            false,
            chip_border,
        ));
    }
    if footprint_presets.len() < crate::active_bar::CUSTOM_FILTER_PRESET_LIMIT {
        top_row = top_row.push(chip_btn(
            "+ Save Preset",
            LibraryMessage::PrimitiveEditorEvent {
                path: path.clone(),
                msg: PrimitiveEdit::Footprint(FootprintEditorMsg::CaptureFilterPreset),
            },
            false,
            chip_border,
        ));
    }

    // 3-row layout matching the schematic Filter dropdown:
    //   row 1: All toggle + preset shortcut chips
    //   row 2: 5 chips (3D Bodies / Keepouts / Tracks / Arcs / Pads)
    //   row 3: 5 chips (Vias / Regions / Fills / Texts / Other)
    let layout = column![
        container(top_row).padding([4, 8]),
        container(
            column![
                row![
                    make_chip("3D Bodies", K::Bodies3d),
                    make_chip("Keepouts", K::Keepouts),
                    make_chip("Tracks", K::Tracks),
                    make_chip("Arcs", K::Arcs),
                    make_chip("Pads", K::Pads),
                ]
                .spacing(4)
                .align_y(iced::Alignment::Center),
                row![
                    make_chip("Vias", K::Vias),
                    make_chip("Regions", K::Regions),
                    make_chip("Fills", K::Fills),
                    make_chip("Texts", K::Texts),
                    make_chip("Other", K::Other),
                ]
                .spacing(4)
                .align_y(iced::Alignment::Center),
            ]
            .spacing(4),
        )
        .padding([4, 8]),
    ]
    .spacing(0)
    .width(Length::Shrink);

    vec![DropdownEntry::Custom(layout.into())]
}

fn snap_entries(state: &FootprintEditorState, path: PathBuf) -> Vec<DropdownEntry<LibraryMessage>> {
    let _ = SnapSubTab::Grids; // silence unused-import lint when nothing references it
    let opts = state.snap_options;
    let mk_mode = |label: &'static str, mode: SnappingMode| -> DropdownItem<LibraryMessage> {
        DropdownItem::new(
            label,
            fp(
                path.clone(),
                FootprintEditorMsg::ActiveBarSetSnappingMode(mode),
            ),
        )
        .checked(state.snapping_mode == mode)
    };
    let mk_snap =
        |label: &'static str, flag: SnapOptionFlag, on: bool| -> DropdownItem<LibraryMessage> {
            DropdownItem::new(
                label,
                fp(path.clone(), FootprintEditorMsg::ActiveBarToggleSnap(flag)),
            )
            .checked(on)
        };
    vec![
        DropdownEntry::Header("Snap layers".into()),
        DropdownEntry::Item(mk_mode("All Layers", SnappingMode::AllLayers)),
        DropdownEntry::Item(mk_mode("Current Layer", SnappingMode::CurrentLayer)),
        DropdownEntry::Item(mk_mode("Off", SnappingMode::Off)),
        DropdownEntry::Separator,
        DropdownEntry::Header("Snap targets".into()),
        DropdownEntry::Item(mk_snap(
            "Grids",
            SnapOptionFlag::SnapToGrids,
            opts.snap_to_grids,
        )),
        DropdownEntry::Item(mk_snap(
            "Guides",
            SnapOptionFlag::SnapToGuides,
            opts.snap_to_guides,
        )),
        DropdownEntry::Item(mk_snap(
            "Axes",
            SnapOptionFlag::SnapToAxes,
            opts.snap_to_axes,
        )),
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
    // v0.14 — Move / Drag / Move Selection all activate the Select
    // tool. In a footprint there is no separate "move tool": pad
    // movement IS drag-under-Select (the canvas hit-tests a pad under
    // the Select tool and emits `FootprintMovePad` while dragging — see
    // `canvas/mod.rs`). Altium's Move and Drag differ only by whether
    // ratlines are preserved; a footprint has no ratlines, so both map
    // to the same "grab and drag the selection" behaviour. Picking any
    // of these arms `PadsTool::Select` (and closes the menu in the
    // dispatcher) so the user can immediately grab a pad.
    let activate_select =
        |p: PathBuf| -> LibraryMessage { fp(p, FootprintEditorMsg::SetPadsTool(PadsTool::Select)) };
    vec![
        DropdownEntry::Item(
            DropdownItem::new("Move", activate_select(path.clone())).icon(ic::icon_dd_move(tid)),
        ),
        DropdownEntry::Item(
            DropdownItem::new("Drag", activate_select(path.clone())).icon(ic::icon_dd_drag(tid)),
        ),
        // Break Track still needs the track-segment split primitive
        // (out of scope for #361) so it stays a stub. Drag Track End no
        // longer does: the endpoint-drag path already exists, so the row
        // arms the real Sketch-mode DragTrackEnd tool below.
        // v0.15: needs track-segment split infra
        DropdownEntry::Item(stub("Break Track", path.clone())),
        // #361 — arm the endpoint-biased segment grab. Switches to
        // Sketch mode + the DragTrackEnd tool (see
        // canvas/input/tools.rs::try_drag_track_end_grab); a left-press
        // on any sketch Line then drags its nearer endpoint live.
        DropdownEntry::Item(DropdownItem::new(
            "Drag Track End",
            fp(
                path.clone(),
                FootprintEditorMsg::ActiveBarSetSketchTool(SketchTool::DragTrackEnd),
            ),
        )),
        DropdownEntry::Separator,
        DropdownEntry::Item(
            DropdownItem::new("Move Selection", activate_select(path.clone()))
                .icon(ic::icon_dd_move_sel(tid)),
        ),
        // v0.14 — "by X, Y…" opens the typed-delta Move-By modal so the
        // user can enter an exact mm offset instead of nudging by one
        // grid step (the one-step nudge stays reachable via
        // `FootprintActiveBarNudgeSelection`, which the modal's Confirm
        // shares geometry with through `footprint_nudge_selection`).
        DropdownEntry::Item(
            DropdownItem::new(
                "Move Selection by X, Y…",
                fp(path.clone(), FootprintEditorMsg::MoveByOpen),
            )
            .icon(ic::icon_dd_move_xy(tid)),
        ),
        DropdownEntry::Item(
            DropdownItem::new(
                "Rotate Selection",
                fp(path.clone(), FootprintEditorMsg::ActiveBarRotateSelection),
            )
            .icon(ic::icon_dd_rotate(tid)),
        ),
        DropdownEntry::Item(
            DropdownItem::new(
                "Flip Selection",
                fp(path, FootprintEditorMsg::ActiveBarFlipSelection),
            )
            .icon(ic::icon_dd_flip_x(tid)),
        ),
    ]
}

fn select_entries(path: PathBuf, tid: ThemeId) -> Vec<DropdownEntry<LibraryMessage>> {
    use crate::library::editor::footprint::state::FpSelectionMode;
    vec![
        DropdownEntry::Item(DropdownItem::new(
            "Select overlapped",
            fp(path.clone(), FootprintEditorMsg::SelectOverlapped),
        )),
        DropdownEntry::Item(DropdownItem::new(
            "Select next",
            fp(path.clone(), FootprintEditorMsg::SelectNextOverlapped),
        )),
        DropdownEntry::Item(
            DropdownItem::new(
                "Lasso Select",
                fp(path.clone(), FootprintEditorMsg::LassoArm),
            )
            .icon(ic::icon_dd_select_lasso(tid)),
        ),
        DropdownEntry::Separator,
        DropdownEntry::Item(
            DropdownItem::new(
                "Inside Area",
                fp(
                    path.clone(),
                    FootprintEditorMsg::SetSelectionMode2d(FpSelectionMode::Inside),
                ),
            )
            .icon(ic::icon_dd_select_inside(tid)),
        ),
        DropdownEntry::Item(
            DropdownItem::new(
                "Outside Area",
                fp(
                    path.clone(),
                    FootprintEditorMsg::SetSelectionMode2d(FpSelectionMode::Outside),
                ),
            )
            .icon(ic::icon_dd_select_outside(tid)),
        ),
        DropdownEntry::Item(DropdownItem::new(
            "Touching Rectangle",
            fp(
                path.clone(),
                FootprintEditorMsg::SetSelectionMode2d(FpSelectionMode::Touching),
            ),
        )),
        DropdownEntry::Item(DropdownItem::new(
            "Touching Line",
            fp(path.clone(), FootprintEditorMsg::TouchingLineArm),
        )),
        DropdownEntry::Separator,
        DropdownEntry::Item(DropdownItem::new(
            "All on Layer",
            fp(path.clone(), FootprintEditorMsg::SelectAllOnLayer),
        )),
        DropdownEntry::Item(
            DropdownItem::new(
                "All",
                fp(path.clone(), FootprintEditorMsg::ActiveBarSelectAll),
            )
            .icon(ic::icon_dd_select_all(tid)),
        ),
        DropdownEntry::Item(DropdownItem::new(
            "Off Grid Pads",
            fp(path.clone(), FootprintEditorMsg::SelectOffGridPads),
        )),
        DropdownEntry::Separator,
        DropdownEntry::Item(
            DropdownItem::new(
                "Toggle Selection",
                fp(path, FootprintEditorMsg::ActiveBarClearSelection),
            )
            .icon(ic::icon_dd_select_toggle(tid)),
        ),
    ]
}

fn align_entries(path: PathBuf, tid: ThemeId) -> Vec<DropdownEntry<LibraryMessage>> {
    use crate::library::editor::footprint::state::AlignOp;
    vec![
        // The generic "Align…" dialog launcher stays a stub for v0.14 —
        // the concrete operations below cover the day-to-day flow; the
        // dialog (per-axis radio + reference picker) is a later task.
        DropdownEntry::Item(stub_with_icon(
            "Align…",
            path.clone(),
            ic::icon_dd_align_menu(tid),
        )),
        DropdownEntry::Separator,
        DropdownEntry::Item(align_item_with_icon(
            "Align Left",
            path.clone(),
            AlignOp::Left,
            ic::icon_dd_align_left(tid),
        )),
        DropdownEntry::Item(align_item_with_icon(
            "Align Right",
            path.clone(),
            AlignOp::Right,
            ic::icon_dd_align_right(tid),
        )),
        // "maintain spacing" variants align the selection's edge while
        // preserving each pad's offset on the OTHER axis — which for a
        // pure left/right align is exactly the centre-X move (the
        // cross-axis Y is untouched). Same target as the plain align.
        DropdownEntry::Item(align_item(
            "Align Left (maintain spacing)",
            path.clone(),
            AlignOp::Left,
        )),
        DropdownEntry::Item(align_item(
            "Align Right (maintain spacing)",
            path.clone(),
            AlignOp::Right,
        )),
        DropdownEntry::Item(align_item_with_icon(
            "Align Horizontal Centers",
            path.clone(),
            AlignOp::CenterH,
            ic::icon_dd_align_hcenter(tid),
        )),
        DropdownEntry::Item(align_item_with_icon(
            "Distribute Horizontally",
            path.clone(),
            AlignOp::DistributeH,
            ic::icon_dd_dist_horiz(tid),
        )),
        DropdownEntry::Item(align_item(
            "Increase Horizontal Spacing",
            path.clone(),
            AlignOp::IncreaseHSpacing,
        )),
        DropdownEntry::Item(align_item(
            "Decrease Horizontal Spacing",
            path.clone(),
            AlignOp::DecreaseHSpacing,
        )),
        DropdownEntry::Separator,
        DropdownEntry::Item(align_item_with_icon(
            "Align Top",
            path.clone(),
            AlignOp::Top,
            ic::icon_dd_align_top(tid),
        )),
        DropdownEntry::Item(align_item_with_icon(
            "Align Bottom",
            path.clone(),
            AlignOp::Bottom,
            ic::icon_dd_align_bottom(tid),
        )),
        DropdownEntry::Item(align_item(
            "Align Top (maintain spacing)",
            path.clone(),
            AlignOp::Top,
        )),
        DropdownEntry::Item(align_item(
            "Align Bottom (maintain spacing)",
            path.clone(),
            AlignOp::Bottom,
        )),
        DropdownEntry::Item(align_item_with_icon(
            "Align Vertical Centers",
            path.clone(),
            AlignOp::CenterV,
            ic::icon_dd_align_vcenter(tid),
        )),
        DropdownEntry::Item(align_item_with_icon(
            "Distribute Vertically",
            path.clone(),
            AlignOp::DistributeV,
            ic::icon_dd_dist_vert(tid),
        )),
        DropdownEntry::Item(align_item(
            "Increase Vertical Spacing",
            path.clone(),
            AlignOp::IncreaseVSpacing,
        )),
        DropdownEntry::Item(align_item(
            "Decrease Vertical Spacing",
            path.clone(),
            AlignOp::DecreaseVSpacing,
        )),
        DropdownEntry::Separator,
        DropdownEntry::Item(
            DropdownItem::new(
                "Align To Grid",
                fp(
                    path.clone(),
                    FootprintEditorMsg::ActiveBarAlignSelectionToGrid,
                ),
            )
            .icon(ic::icon_dd_align_grid(tid)),
        ),
        DropdownEntry::Item(DropdownItem::new(
            "Move All Components Origin To Grid",
            fp(path, FootprintEditorMsg::ActiveBarMoveOriginToGrid),
        )),
    ]
}

fn body3d_entries(
    _state: &FootprintEditorState,
    path: PathBuf,
) -> Vec<DropdownEntry<LibraryMessage>> {
    vec![
        DropdownEntry::Item(DropdownItem::new(
            "3D Body",
            fp(path.clone(), FootprintEditorMsg::MintBody3d),
        )),
        DropdownEntry::Item(DropdownItem::new(
            "Extruded 3D Body",
            fp(path, FootprintEditorMsg::MintExtrudedBody3d),
        )),
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
                    FootprintEditorMsg::SetPadsTool(PadsTool::PlaceString),
                ),
            )
            .icon(ic::icon_dd_text_string(tid))
            .checked(active == PadsTool::PlaceString),
        ),
        // v0.14 — "Text Frame" drags a bounding-box rectangle and
        // appends a framed silk `Text` (item ③). Unlike "String"
        // above, this is a press-drag-release gesture, not a
        // 1-click drop.
        DropdownEntry::Item(
            DropdownItem::new(
                "Text Frame",
                fp(
                    path,
                    FootprintEditorMsg::SetPadsTool(PadsTool::PlaceTextFrame),
                ),
            )
            .icon(ic::icon_dd_text_frame(tid))
            .checked(active == PadsTool::PlaceTextFrame),
        ),
    ]
}

fn shapes_entries(path: PathBuf, tid: ThemeId) -> Vec<DropdownEntry<LibraryMessage>> {
    // Per user simplification: pure graphics live in Sketch mode
    // only. Picking an item here switches the editor to Sketch mode
    // and arms the matching SketchTool — single-click parity with
    // Altium's Place ▸ Line / Arc / Rectangle flow.
    let arm = |tool: SketchTool| -> LibraryMessage {
        fp(
            path.clone(),
            FootprintEditorMsg::ActiveBarSetSketchTool(tool),
        )
    };
    vec![
        DropdownEntry::Header("Sketch mode tools".into()),
        DropdownEntry::Item(
            DropdownItem::new("Line", arm(SketchTool::Line)).icon(ic::icon_dd_line(tid)),
        ),
        DropdownEntry::Item(
            DropdownItem::new("Arc (Center)", arm(SketchTool::Arc)).icon(ic::icon_dd_arc(tid)),
        ),
        DropdownEntry::Item(
            DropdownItem::new("Arc (Edge)", arm(SketchTool::Arc)).icon(ic::icon_dd_arc(tid)),
        ),
        DropdownEntry::Item(
            DropdownItem::new("Arc (Any Angle)", arm(SketchTool::Arc)).icon(ic::icon_dd_arc(tid)),
        ),
        DropdownEntry::Item(
            DropdownItem::new("Full Circle", arm(SketchTool::Circle)).icon(ic::icon_dd_circle(tid)),
        ),
        DropdownEntry::Separator,
        // v0.14 — "Fill" and "Solid Region" are synonyms for the same
        // primitive: a closed-loop *filled* polygon (`FpGraphic { filled:
        // true }`). Both arm the existing Pads-mode `PlaceRegion` tool;
        // the dispatcher flips the editor back to Normal mode and the
        // canvas multi-click gesture (`region_or_polygon_click`) records
        // `filled = true`. Mirrors the working "String" place button.
        DropdownEntry::Item(
            DropdownItem::new(
                "Fill",
                fp(
                    path.clone(),
                    FootprintEditorMsg::SetPadsTool(PadsTool::PlaceRegion),
                ),
            )
            .icon(ic::icon_dd_polygon(tid)),
        ),
        DropdownEntry::Item(
            DropdownItem::new(
                "Solid Region",
                fp(
                    path.clone(),
                    FootprintEditorMsg::SetPadsTool(PadsTool::PlaceRegion),
                ),
            )
            .icon(ic::icon_dd_polygon(tid)),
        ),
        DropdownEntry::Item(
            DropdownItem::new("Rectangle", arm(SketchTool::Rectangle)).icon(ic::icon_dd_rect(tid)),
        ),
    ]
}
