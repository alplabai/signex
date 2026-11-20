//! The sketch Active Bar's Create ▾ and Modify ▾ menus, the same rows on
//! both hosts (#2518). Each tool row arms its tool through
//! [`FootprintEditorMsg::ActiveBarSetSketchTool`], which also closes the
//! menu; the armed one carries a checkmark. Make Pad from Profile is a
//! footprint action: a board sketch shows it greyed, saying why.

use std::path::PathBuf;

use iced::widget::svg::Handle;
use signex_i18n::{I18n, MessageId, ids, t};
use signex_types::sketch::SketchAction;
use signex_types::theme::ThemeId;
use signex_widgets::active_bar_dropdown::{DropdownEntry, DropdownItem};

use crate::icons as ic;
use crate::library::editor::footprint::state::{FootprintEditorState, SketchTool};
use crate::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};

fn fp(path: PathBuf, msg: FootprintEditorMsg) -> LibraryMessage {
    LibraryMessage::PrimitiveEditorEvent {
        path,
        msg: PrimitiveEdit::Footprint(msg),
    }
}

/// One Create row: label, tool, icon.
type CreateRow = (MessageId, SketchTool, fn(ThemeId) -> Handle);

/// The Create rows, in menu order.
const CREATE_ROWS: [CreateRow; 8] = [
    (
        ids::EDITORS_SKETCH_TOOL_LINE,
        SketchTool::Line,
        ic::icon_shape_line,
    ),
    (
        ids::EDITORS_SKETCH_TOOL_RECTANGLE,
        SketchTool::Rectangle,
        ic::icon_shape_rect,
    ),
    (
        ids::EDITORS_SKETCH_TOOL_ROUNDED_RECTANGLE,
        SketchTool::RoundedRectangle,
        ic::icon_sk_rounded_rect,
    ),
    (
        ids::EDITORS_SKETCH_TOOL_CIRCLE,
        SketchTool::Circle,
        ic::icon_shape_circle,
    ),
    (
        ids::EDITORS_SKETCH_TOOL_ARC,
        SketchTool::Arc,
        ic::icon_shape_arc,
    ),
    (
        ids::EDITORS_SKETCH_TOOL_TANGENT_ARC,
        SketchTool::TangentArc,
        ic::icon_shape_arc,
    ),
    (
        ids::EDITORS_SKETCH_TOOL_EDGE_ARC,
        SketchTool::EdgeArc,
        ic::icon_shape_arc,
    ),
    (
        ids::EDITORS_SKETCH_TOOL_POINT,
        SketchTool::Point,
        ic::icon_sk_c_coincident,
    ),
];

/// Sketch ▸ Create: the drawing tools.
pub fn create_entries(
    state: &FootprintEditorState,
    path: PathBuf,
    tid: ThemeId,
    i18n: &I18n,
) -> Vec<DropdownEntry<LibraryMessage>> {
    let armed = state.active_tool;
    let mut entries = vec![DropdownEntry::Header(
        t!(i18n, ids::EDITORS_SKETCH_MENU_CREATE).into_string(),
    )];
    entries.extend(CREATE_ROWS.iter().map(|&(label, tool, icon)| {
        DropdownEntry::Item(
            DropdownItem::new(
                t!(i18n, label).into_string(),
                fp(
                    path.clone(),
                    FootprintEditorMsg::ActiveBarSetSketchTool(tool),
                ),
            )
            .icon(icon(tid))
            .checked(armed == tool),
        )
    }));
    entries
}

/// Sketch ▸ Modify: the edit tools plus the one-shot Make Pad action, in
/// the order of m187: Fillet, Chamfer, Trim, Extend | Mirror, Offset,
/// Linear, Rectangular and Circular Pattern | Make Pad from Profile.
/// Mirror / Offset / the Pattern tools consume a selection, so they grey
/// out when nothing is selected rather than arming a tool that would only
/// warn.
pub fn modify_entries(
    state: &FootprintEditorState,
    path: PathBuf,
    tid: ThemeId,
    i18n: &I18n,
) -> Vec<DropdownEntry<LibraryMessage>> {
    let armed = state.active_tool;
    let has_selection = state.selected_sketch.is_some()
        || state.selected_sketch_secondary.is_some()
        || !state.selected_sketch_extra.is_empty();
    let row = |label: MessageId, tool: SketchTool, icon: Handle, needs_selection: bool| {
        DropdownEntry::Item(
            DropdownItem::new(
                t!(i18n, label).into_string(),
                fp(
                    path.clone(),
                    FootprintEditorMsg::ActiveBarSetSketchTool(tool),
                ),
            )
            .icon(icon)
            .checked(armed == tool)
            .disabled(needs_selection && !has_selection),
        )
    };
    let make_pad = DropdownItem::new(
        t!(i18n, ids::EDITORS_SKETCH_MAKE_PAD).into_string(),
        fp(path.clone(), FootprintEditorMsg::SketchMakePadFromProfile),
    )
    .icon(ic::icon_sk_make_pad(tid));
    let make_pad = if state.sketch_host().offers(SketchAction::MakePadFromProfile) {
        make_pad
    } else {
        make_pad
            .disabled(true)
            .shortcut(t!(i18n, ids::EDITORS_SKETCH_FOOTPRINT_ONLY).into_string())
            .tooltip(t!(i18n, ids::EDITORS_SKETCH_MAKE_PAD_BOARD).into_string())
    };
    vec![
        DropdownEntry::Header(t!(i18n, ids::EDITORS_SKETCH_MENU_MODIFY).into_string()),
        row(
            ids::EDITORS_SKETCH_TOOL_FILLET,
            SketchTool::Fillet,
            ic::icon_sk_fillet(tid),
            false,
        ),
        row(
            ids::SKETCH_TOOLS_CHAMFER,
            SketchTool::Chamfer,
            ic::icon_sk_chamfer(tid),
            false,
        ),
        row(
            ids::EDITORS_SKETCH_TOOL_TRIM,
            SketchTool::Trim,
            ic::icon_sk_trim(tid),
            false,
        ),
        row(
            ids::SKETCH_TOOLS_EXTEND,
            SketchTool::Extend,
            ic::icon_sk_extend(tid),
            false,
        ),
        DropdownEntry::Separator,
        DropdownEntry::Header(t!(i18n, ids::EDITORS_SKETCH_MENU_NEEDS_SELECTION).into_string()),
        row(
            ids::EDITORS_SKETCH_TOOL_MIRROR,
            SketchTool::Mirror,
            ic::icon_sk_mirror(tid),
            true,
        ),
        row(
            ids::EDITORS_SKETCH_TOOL_OFFSET,
            SketchTool::Offset,
            ic::icon_sk_offset(tid),
            true,
        ),
        row(
            ids::SKETCH_TOOLS_LINEAR_PATTERN,
            SketchTool::LinearPattern,
            ic::icon_sk_linear_pattern(tid),
            true,
        ),
        row(
            ids::EDITORS_SKETCH_TOOL_RECT_PATTERN,
            SketchTool::RectPattern,
            ic::icon_sk_rect_pattern(tid),
            true,
        ),
        row(
            ids::EDITORS_SKETCH_TOOL_CIRCULAR_PATTERN,
            SketchTool::CircularPattern,
            ic::icon_sk_circular_pattern(tid),
            true,
        ),
        DropdownEntry::Separator,
        DropdownEntry::Item(make_pad),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_library::Footprint;

    fn en_us() -> I18n {
        I18n::new(&signex_i18n::LanguagePref::Locale("en-US".into()))
    }

    fn make_pad_row(state: &FootprintEditorState) -> DropdownItem<LibraryMessage> {
        let entries = modify_entries(state, PathBuf::from("t"), ThemeId::DEFAULT, &en_us());
        entries
            .into_iter()
            .find_map(|entry| match entry {
                DropdownEntry::Item(item) if item.label == "Make Pad from Profile" => Some(item),
                _ => None,
            })
            .expect("the Make Pad row")
    }

    #[test]
    fn make_pad_from_profile_is_greyed_on_a_board_and_says_why() {
        let mut state = FootprintEditorState::from_footprint(&Footprint::empty("t"));
        assert!(!make_pad_row(&state).disabled, "a footprint can make pads");

        let board = signex_types::project::SetupProfile::Standard
            .pcb(2)
            .new_board();
        state.board_backdrop = Some(std::sync::Arc::new(
            crate::library::editor::footprint::BoardBackdrop::from_board(&board),
        ));
        let row = make_pad_row(&state);
        assert!(row.disabled);
        assert_eq!(row.shortcut.as_deref(), Some("footprint only"));
        assert!(row.tooltip.is_some());
    }

    #[test]
    fn create_offers_point_on_both_hosts() {
        let state = FootprintEditorState::from_footprint(&Footprint::empty("t"));
        let labels: Vec<String> =
            create_entries(&state, PathBuf::from("t"), ThemeId::DEFAULT, &en_us())
                .into_iter()
                .filter_map(|entry| match entry {
                    DropdownEntry::Item(item) => Some(item.label),
                    _ => None,
                })
                .collect();
        assert_eq!(
            labels,
            [
                "Line",
                "Rectangle",
                "Rounded Rectangle",
                "Circle",
                "Arc",
                "Tangent Arc",
                "Edge Arc",
                "Point"
            ]
        );
    }
}

#[cfg(test)]
#[path = "menus_tests.rs"]
mod menus_tests;
