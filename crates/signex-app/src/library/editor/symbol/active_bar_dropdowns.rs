//! v0.13 — SchLib (.snxsym) editor active-bar dropdown menu
//! definitions.
//!
//! Each `SymActiveBarMenu` variant maps to a function that returns a
//! list of `DropdownEntry<LibraryMessage>` rows. Rendering lives in
//! `signex_widgets::active_bar_dropdown::view`; the chevron-trigger
//! buttons + overlay positioning are owned by `symbol/active_bar.rs`.
//!
//! Wiring philosophy mirrors the footprint editor: items that map to
//! existing primitives (Selection Filter pills, Shape tools) emit the
//! real `PrimitiveEditorMsg`; items that need new primitives emit
//! `SymbolActiveBarStub` so the action logs a "coming soon" warn and
//! dismisses the menu cleanly.

use std::path::PathBuf;

use signex_types::theme::ThemeId;
use signex_widgets::active_bar_dropdown::{DropdownEntry, DropdownItem};

use crate::icons as ic;
use crate::library::editor::symbol::canvas::SymbolTool;
use crate::library::editor::symbol::state::{
    SymActiveBarMenu, SymbolFilterKind, SymbolSelectionFilter,
};
use crate::library::messages::{LibraryMessage, PrimitiveEditorMsg, SymbolToolMsg};

/// Convenience: route a `PrimitiveEditorMsg` to the editor at `path`.
fn sym(path: PathBuf, msg: PrimitiveEditorMsg) -> LibraryMessage {
    LibraryMessage::PrimitiveEditorEvent { path, msg }
}

/// "Coming soon" stub item with no icon.
fn stub(label: &'static str, path: PathBuf) -> DropdownItem<LibraryMessage> {
    DropdownItem::new(
        label,
        sym(path, PrimitiveEditorMsg::SymbolActiveBarStub(label)),
    )
}

/// Stub item with an icon for visual recognition.
fn stub_with_icon(
    label: &'static str,
    path: PathBuf,
    icon: iced::widget::svg::Handle,
) -> DropdownItem<LibraryMessage> {
    DropdownItem::new(
        label,
        sym(path, PrimitiveEditorMsg::SymbolActiveBarStub(label)),
    )
    .icon(icon)
}

/// Build the entries for the dropdown matching `menu`. `tid` resolves
/// the per-theme accent tint on each SVG icon (icons reuse the
/// schematic active bar's icon set for visual consistency across
/// editors).
pub fn entries(
    menu: SymActiveBarMenu,
    selection_filter: SymbolSelectionFilter,
    active_tool: SymbolTool,
    path: PathBuf,
    tid: ThemeId,
) -> Vec<DropdownEntry<LibraryMessage>> {
    match menu {
        SymActiveBarMenu::Filter => filter_entries(selection_filter, path),
        SymActiveBarMenu::Snap => snap_entries(path),
        SymActiveBarMenu::Place => place_entries(path, tid),
        SymActiveBarMenu::Select => select_entries(path, tid),
        SymActiveBarMenu::Align => align_entries(path, tid),
        SymActiveBarMenu::Pin => pin_entries(active_tool, path),
        SymActiveBarMenu::Text => text_entries(active_tool, path, tid),
        SymActiveBarMenu::Shapes => shapes_entries(active_tool, path, tid),
    }
}

fn filter_entries(
    f: SymbolSelectionFilter,
    path: PathBuf,
) -> Vec<DropdownEntry<LibraryMessage>> {
    use iced::widget::{column, container, row};
    use iced::{Color, Length};
    use signex_widgets::active_bar_dropdown::chip_btn;
    use SymbolFilterKind as K;

    let chip_border = Color::from_rgba8(0xE7, 0x8B, 0x2A, 1.0);

    let make_chip = |label: &'static str, kind: K| -> iced::Element<'static, LibraryMessage> {
        chip_btn(
            label,
            LibraryMessage::PrimitiveEditorEvent {
                path: path.clone(),
                msg: PrimitiveEditorMsg::SymbolToggleSelectionFilter(kind),
            },
            f.get(kind),
            chip_border,
        )
    };

    // All-On / All-Off toggle.
    let all_on = K::ALTIUM_PILLS.iter().all(|k| f.get(*k));
    let all_btn = chip_btn(
        if all_on { "All - On" } else { "All - Off" },
        LibraryMessage::PrimitiveEditorEvent {
            path: path.clone(),
            msg: PrimitiveEditorMsg::SymbolActiveBarStub("All filters"),
        },
        all_on,
        chip_border,
    );

    let layout = column![
        container(row![all_btn].spacing(4).align_y(iced::Alignment::Center)).padding([4, 8]),
        container(
            row![
                make_chip("Pins", K::Pins),
                make_chip("Drawings", K::Drawings),
                make_chip("Texts", K::Texts),
                make_chip("Designators", K::Designators),
            ]
            .spacing(4)
            .align_y(iced::Alignment::Center),
        )
        .padding([4, 8]),
        container(
            row![
                make_chip("Values", K::Values),
                make_chip("Parameters", K::Parameters),
                make_chip("Other", K::Other),
            ]
            .spacing(4)
            .align_y(iced::Alignment::Center),
        )
        .padding([4, 8]),
    ]
    .spacing(0)
    .width(Length::Shrink);

    vec![DropdownEntry::Custom(layout.into())]
}

fn snap_entries(path: PathBuf) -> Vec<DropdownEntry<LibraryMessage>> {
    let _ = path;
    // SchLib snap surface is simpler than footprint — pin tips, line
    // endpoints, arc centres, text origins. All stubs until the
    // SchLib snap subsystem ships.
    vec![
        DropdownEntry::Header("Snap targets".into()),
        DropdownEntry::Item(DropdownItem::new(
            "Pin Tips",
            LibraryMessage::PrimitiveEditorEvent {
                path: PathBuf::new(),
                msg: PrimitiveEditorMsg::SymbolActiveBarStub("Snap → Pin Tips"),
            },
        )),
        DropdownEntry::Item(DropdownItem::new(
            "Line Endpoints",
            LibraryMessage::PrimitiveEditorEvent {
                path: PathBuf::new(),
                msg: PrimitiveEditorMsg::SymbolActiveBarStub("Snap → Line Endpoints"),
            },
        )),
        DropdownEntry::Item(DropdownItem::new(
            "Arc Centres",
            LibraryMessage::PrimitiveEditorEvent {
                path: PathBuf::new(),
                msg: PrimitiveEditorMsg::SymbolActiveBarStub("Snap → Arc Centres"),
            },
        )),
        DropdownEntry::Item(DropdownItem::new(
            "Text Origins",
            LibraryMessage::PrimitiveEditorEvent {
                path: PathBuf::new(),
                msg: PrimitiveEditorMsg::SymbolActiveBarStub("Snap → Text Origins"),
            },
        )),
    ]
}

fn place_entries(path: PathBuf, tid: ThemeId) -> Vec<DropdownEntry<LibraryMessage>> {
    vec![
        DropdownEntry::Item(stub_with_icon("Move", path.clone(), ic::icon_dd_move(tid))),
        DropdownEntry::Item(stub_with_icon("Drag", path.clone(), ic::icon_dd_drag(tid))),
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
        DropdownEntry::Item(stub_with_icon(
            "All",
            path.clone(),
            ic::icon_dd_select_all(tid),
        )),
        DropdownEntry::Separator,
        DropdownEntry::Item(stub_with_icon(
            "Toggle Selection",
            path,
            ic::icon_dd_select_toggle(tid),
        )),
    ]
}

fn align_entries(path: PathBuf, tid: ThemeId) -> Vec<DropdownEntry<LibraryMessage>> {
    // Same Altium align/distribute set as the footprint editor —
    // align/distribute is a graphics-agnostic operation.
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
        DropdownEntry::Separator,
        DropdownEntry::Item(stub_with_icon(
            "Align To Grid",
            path,
            ic::icon_dd_align_grid(tid),
        )),
    ]
}

fn pin_entries(
    active_tool: SymbolTool,
    path: PathBuf,
) -> Vec<DropdownEntry<LibraryMessage>> {
    // SchLib has one Place Pin tool today; the variants (input /
    // output / passive / etc.) mutate a pin AFTER placement via the
    // Properties panel. Surface "Place Pin" as the only wired item;
    // the variant rows are stubs that hint future flow.
    vec![
        DropdownEntry::Item(
            DropdownItem::new(
                "Place Pin",
                sym(
                    path.clone(),
                    PrimitiveEditorMsg::SymbolSetTool(SymbolToolMsg::AddPin),
                ),
            )
            .checked(active_tool == SymbolTool::AddPin),
        ),
        DropdownEntry::Separator,
        DropdownEntry::Header("Variants (set via Properties)".into()),
        DropdownEntry::Item(stub("Input", path.clone())),
        DropdownEntry::Item(stub("Output", path.clone())),
        DropdownEntry::Item(stub("Passive", path.clone())),
        DropdownEntry::Item(stub("Bidirectional", path.clone())),
        DropdownEntry::Item(stub("Power", path)),
    ]
}

fn text_entries(
    active_tool: SymbolTool,
    path: PathBuf,
    tid: ThemeId,
) -> Vec<DropdownEntry<LibraryMessage>> {
    vec![
        DropdownEntry::Item(
            DropdownItem::new(
                "String",
                sym(
                    path.clone(),
                    PrimitiveEditorMsg::SymbolSetTool(SymbolToolMsg::PlaceText),
                ),
            )
            .icon(ic::icon_dd_text_string(tid))
            .checked(active_tool == SymbolTool::PlaceText),
        ),
        DropdownEntry::Item(stub_with_icon(
            "Text Frame",
            path,
            ic::icon_dd_text_frame(tid),
        )),
    ]
}

fn shapes_entries(
    active_tool: SymbolTool,
    path: PathBuf,
    tid: ThemeId,
) -> Vec<DropdownEntry<LibraryMessage>> {
    let arm = |tool: SymbolToolMsg| -> LibraryMessage {
        sym(path.clone(), PrimitiveEditorMsg::SymbolSetTool(tool))
    };
    vec![
        DropdownEntry::Item(
            DropdownItem::new("Line", arm(SymbolToolMsg::PlaceLine))
                .icon(ic::icon_dd_line(tid))
                .checked(active_tool == SymbolTool::PlaceLine),
        ),
        DropdownEntry::Item(
            DropdownItem::new("Arc", arm(SymbolToolMsg::PlaceArc))
                .icon(ic::icon_dd_arc(tid))
                .checked(active_tool == SymbolTool::PlaceArc),
        ),
        DropdownEntry::Item(
            DropdownItem::new("Ellipse / Circle", arm(SymbolToolMsg::PlaceCircle))
                .icon(ic::icon_dd_circle(tid))
                .checked(active_tool == SymbolTool::PlaceCircle),
        ),
        DropdownEntry::Item(
            DropdownItem::new("Rectangle", arm(SymbolToolMsg::PlaceRectangle))
                .icon(ic::icon_dd_rect(tid))
                .checked(active_tool == SymbolTool::PlaceRectangle),
        ),
        DropdownEntry::Separator,
        DropdownEntry::Item(stub("Round Rectangle", path.clone())),
        DropdownEntry::Item(stub("Polygon", path.clone())),
        DropdownEntry::Item(stub("Bezier", path.clone())),
        DropdownEntry::Item(stub("Pie Chart", path.clone())),
        DropdownEntry::Item(stub("Elliptical Arc", path)),
    ]
}
