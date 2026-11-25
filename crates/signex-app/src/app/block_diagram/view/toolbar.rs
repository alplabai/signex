//! The toolbar (mockup row 171): the Select and Connect tools, Add Block,
//! Add Port, Group and Delete, then Undo, Redo, Save and Fit. A tool that
//! has nothing to act on is greyed with the reason as its hint; on a
//! read-only diagram every edit is greyed and the end of the bar says so.

use iced::{Element, Widget as _};
use signex_i18n::{Localized, MessageId, ids};
use signex_widgets::doc_tab_shell::Toolbar;
use signex_widgets::{button, segmented, text};

use super::{Look, SP, TS};
use crate::app::block_diagram::TabEvent;
use crate::app::block_diagram::state::{BlockDiagramEditor, DiagramItem, DiagramTool};
use crate::panels::graphite::hint_tooltip;
use crate::styles::ti;
use crate::styles::tokens::resolve;

pub(super) fn view<'a>(look: Look<'a>, editor: &'a BlockDiagramEditor) -> Toolbar<'a, TabEvent> {
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
    let mut bar = Toolbar::new()
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
