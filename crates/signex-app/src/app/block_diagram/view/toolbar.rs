//! The toolbar (mockup row 171): the view switch and Re-sync, then the
//! Select and Connect tools, Add Block, Add Port, Group and Delete, Undo,
//! Redo, Save and Fit, and the SVG and PDF… exports, with the Variant field
//! and Show GND. The Power tree view keeps the switch, Re-sync and Variant. A tool that has nothing to act on is
//! greyed with the reason as its hint; on a read-only diagram every edit is
//! greyed and the end of the bar says so.

use iced::widget::{pick_list, row};
use iced::{Element, Widget as _};
use signex_i18n::{Localized, MessageId, ids};
use signex_widgets::doc_tab_shell::Toolbar;
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::{CheckboxLabel as _, button, checkbox, segmented, text};

use super::{Look, SP, TS, VariantField};
use crate::app::block_diagram::state::{
    BlockDiagramEditor, DiagramItem, DiagramTool, DiagramView, ExportKind,
};
use crate::app::block_diagram::{ExportEvent, TabEvent};
use crate::choice::{Choice, chosen};
use crate::panels::graphite::hint_tooltip;
use crate::styles::ti;
use crate::styles::tokens::resolve;

pub(super) fn view<'a>(
    look: Look<'a>,
    editor: &'a BlockDiagramEditor,
    variant: VariantField,
) -> Toolbar<'a, TabEvent> {
    let editable = !editor.is_read_only();
    let one_block = matches!(editor.single_selection(), Some(DiagramItem::Block(_)));
    let some_block = !editor.selected_blocks().is_empty();
    let some = !editor.selection.is_empty();
    let edit = |label, event: TabEvent, ready: bool, tip, why_not| {
        let on_press = (editable && ready).then_some(event);
        let hint = if !editable {
            look.t(ids::BLOCK_DIAGRAM_READ_ONLY_TIP)
        } else if ready {
            look.t(tip)
        } else {
            look.t(why_not)
        };
        tool(look, label, false, on_press, hint)
    };
    let head = Toolbar::new()
        .push(views(look, editor))
        .separator()
        .push(edit(
            ids::BLOCK_DIAGRAM_RESYNC,
            TabEvent::ResyncPressed,
            true,
            ids::BLOCK_DIAGRAM_RESYNC_TIP,
            ids::BLOCK_DIAGRAM_RESYNC_TIP,
        ))
        .separator()
        .push(variant_field(look, variant));
    if editor.view == DiagramView::PowerTree {
        return head;
    }
    let show_ground = checkbox(editor.show_ground)
        .style(crate::panels::form_look::checkbox_style)
        .size(TS.body)
        .labelled(look.t(ids::BLOCK_DIAGRAM_SHOW_GND))
        .text_size(TS.dense)
        .on_toggle(TabEvent::ShowGroundToggled)
        .boxed();
    let mut bar = head
        .push(show_ground)
        .separator()
        .push(tools(look, editor))
        .separator()
        .push(edit(
            ids::BLOCK_DIAGRAM_ADD_BLOCK,
            TabEvent::AddBlockPressed,
            true,
            ids::BLOCK_DIAGRAM_ADD_BLOCK_TIP,
            ids::BLOCK_DIAGRAM_ADD_BLOCK_TIP,
        ))
        .push(edit(
            ids::BLOCK_DIAGRAM_ADD_PORT,
            TabEvent::AddPortPressed,
            one_block,
            ids::BLOCK_DIAGRAM_ADD_PORT_TIP,
            ids::BLOCK_DIAGRAM_NEEDS_ONE_BLOCK,
        ))
        .push(edit(
            ids::BLOCK_DIAGRAM_GROUP,
            TabEvent::GroupPressed,
            some_block,
            ids::BLOCK_DIAGRAM_GROUP_TIP,
            ids::BLOCK_DIAGRAM_NEEDS_BLOCKS,
        ))
        .push(edit(
            ids::BLOCK_DIAGRAM_DELETE,
            TabEvent::DeletePressed,
            some,
            ids::BLOCK_DIAGRAM_DELETE_TIP,
            ids::BLOCK_DIAGRAM_NEEDS_SELECTION,
        ))
        .separator()
        .push(edit(
            ids::BLOCK_DIAGRAM_UNDO,
            TabEvent::UndoPressed,
            !editor.undo.is_empty(),
            ids::BLOCK_DIAGRAM_UNDO_TIP,
            ids::BLOCK_DIAGRAM_NOTHING_TO_UNDO,
        ))
        .push(edit(
            ids::BLOCK_DIAGRAM_REDO,
            TabEvent::RedoPressed,
            !editor.redo.is_empty(),
            ids::BLOCK_DIAGRAM_REDO_TIP,
            ids::BLOCK_DIAGRAM_NOTHING_TO_REDO,
        ))
        .push(edit(
            ids::BLOCK_DIAGRAM_SAVE,
            TabEvent::SavePressed,
            editor.is_dirty(),
            ids::BLOCK_DIAGRAM_SAVE_TIP,
            ids::BLOCK_DIAGRAM_NOTHING_TO_SAVE,
        ))
        .push(tool(
            look,
            ids::BLOCK_DIAGRAM_FIT,
            false,
            Some(TabEvent::FitPressed),
            look.t(ids::BLOCK_DIAGRAM_FIT_TIP),
        ))
        .separator()
        .push(tool(
            look,
            ids::BLOCK_DIAGRAM_EXPORT_SVG,
            editor
                .export
                .as_ref()
                .is_some_and(|d| d.kind == ExportKind::Svg),
            Some(TabEvent::Export(ExportEvent::Opened(ExportKind::Svg))),
            look.t(ids::BLOCK_DIAGRAM_EXPORT_SVG_TIP),
        ))
        .push(tool(
            look,
            ids::BLOCK_DIAGRAM_EXPORT_PDF,
            editor
                .export
                .as_ref()
                .is_some_and(|d| d.kind == ExportKind::Pdf),
            Some(TabEvent::Export(ExportEvent::Opened(ExportKind::Pdf))),
            look.t(ids::BLOCK_DIAGRAM_EXPORT_PDF_TIP),
        ));
    if !editable {
        bar = bar.push_end(
            text(look.t(ids::BLOCK_DIAGRAM_READ_ONLY))
                .size(TS.dense)
                .color(ti(look.tokens.warning))
                .boxed(),
        );
    } else if editor.connect_from.is_some() {
        bar = bar.push_end(
            text(look.t(ids::BLOCK_DIAGRAM_CONNECT_PICK_END))
                .size(TS.dense)
                .color(resolve(look.tokens).ink3)
                .boxed(),
        );
    }
    bar
}

/// The Variant field: one variant at a time (mockup row 171).
fn variant_field<'a>(look: Look<'a>, field: VariantField) -> Element<'a, TabEvent> {
    let choices: Vec<Choice<String>> = field
        .names
        .into_iter()
        .map(|name| Choice {
            label: name.clone(),
            value: name,
        })
        .collect();
    let selected = chosen(&choices, &field.shown);
    let list = pick_list(selected, choices, |c: &Choice<String>| c.label.clone())
        .graphite()
        .on_select(|c: Choice<String>| TabEvent::VariantPicked(c.value))
        .text_size(TS.dense)
        .boxed();
    row![
        text(look.t(ids::BLOCK_DIAGRAM_VARIANT))
            .size(TS.dense)
            .color(resolve(look.tokens).ink3)
            .boxed(),
        list,
    ]
    .spacing(SP.xs)
    .align_y(iced::Alignment::Center)
    .boxed()
}

/// The view switch: Block diagram | Power tree.
fn views<'a>(look: Look<'a>, editor: &BlockDiagramEditor) -> Element<'a, TabEvent> {
    let cells: Vec<(DiagramView, signex_widgets::Label<'a>)> = vec![
        (
            DiagramView::Blocks,
            look.t(ids::BLOCK_DIAGRAM_VIEW_BLOCKS).into(),
        ),
        (
            DiagramView::PowerTree,
            look.t(ids::BLOCK_DIAGRAM_VIEW_POWER).into(),
        ),
    ];
    segmented(
        cells,
        editor.view,
        resolve(look.tokens).segmented(TS.dense),
        TabEvent::ViewPicked,
    )
}

/// Select | Connect.
fn tools<'a>(look: Look<'a>, editor: &BlockDiagramEditor) -> Element<'a, TabEvent> {
    let cells: Vec<(DiagramTool, signex_widgets::Label<'a>)> = vec![
        (
            DiagramTool::Select,
            look.t(ids::BLOCK_DIAGRAM_TOOL_SELECT).into(),
        ),
        (
            DiagramTool::Connect,
            look.t(ids::BLOCK_DIAGRAM_TOOL_CONNECT).into(),
        ),
    ];
    segmented(
        cells,
        editor.tool,
        resolve(look.tokens).segmented(TS.dense),
        TabEvent::ToolPicked,
    )
}

/// A toolbar button with its hint: greyed and deaf without a message.
fn tool<'a>(
    look: Look<'a>,
    label: MessageId,
    on: bool,
    on_press: Option<TabEvent>,
    tip: Localized,
) -> Element<'a, TabEvent> {
    let r = resolve(look.tokens);
    let ink = match (on_press.is_some(), on) {
        (false, _) => r.ink4,
        (true, true) => ti(look.tokens.accent),
        (true, false) => ti(look.tokens.text),
    };
    let face = button(text(look.t(label)).size(TS.dense).color(ink))
        .padding([SP.xs, SP.s])
        .style(crate::styles::ghost_button(look.tokens))
        .on_press_maybe(on_press);
    hint_tooltip(
        face.boxed(),
        tip,
        look.tokens,
        iced::widget::tooltip::Position::Bottom,
    )
}
