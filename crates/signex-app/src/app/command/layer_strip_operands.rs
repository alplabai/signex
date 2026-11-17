//! The layer strip's commands from their operands (#2394): what the layer
//! tab menu and the layer set button do, for an agent or the CLI. A command
//! on a board's view names that board with `document` and acts on its view
//! alone, wherever the board is shown: it opens, focuses or switches nothing.
//! The per-user ones (`delete_layer_set`, `set_layer_tab_names`) name no
//! board. Each changes what its menu row changes.

use std::path::PathBuf;

use iced::Task;
use signex_renderer::pcb::{BuiltinLayerSet, LayerKind};
use signex_types::format::parse_pcb_layer;
use signex_types::pcb_layer::PcbLayer;

use super::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::document::DocumentKey;
use crate::app::handlers::pcb_layer_strip::BoardViewTarget;
use crate::app::{LayerSetMsg, LayerStripMsg, Message, PcbMsg, PcbViewMsg, Signex};
use crate::layer_strip_prefs::LayerNameStyle;

type Ran = (CommandOutcome, Task<Message>);

/// The text of operand `name`, trimmed; empty when it is not given.
fn text_operand<'a>(args: &'a CommandArgs, name: &str) -> &'a str {
    match args.operand(name) {
        Some(OperandValue::Text(text)) => text.trim(),
        _ => "",
    }
}

fn refused(reason: impl Into<String>) -> Ran {
    (CommandOutcome::disabled(reason), Task::none())
}

fn ran(task: Task<Message>) -> Ran {
    (CommandOutcome::Ran, task)
}

/// The path of the open board `document` names.
fn named_board(app: &Signex, args: &CommandArgs) -> Result<PathBuf, String> {
    let Some(OperandValue::Document(target)) = args.operand("document") else {
        return Err("the operand `document` is needed".to_string());
    };
    let entry = app.resolve_document_target(target)?;
    match entry.key {
        Some(DocumentKey::Pcb(path)) => Ok(path),
        Some(_) => Err(format!(
            "the layer strip belongs to a board, and {} is a {}",
            entry.name,
            entry.kind.as_str()
        )),
        None => Err(format!(
            "{} is referenced by a project but not loaded; open it first",
            entry.name
        )),
    }
}

/// `layer`: a board layer by its name in the board file.
fn layer_operand(args: &CommandArgs) -> Result<PcbLayer, String> {
    let token = text_operand(args, "layer");
    parse_pcb_layer(token).ok_or_else(|| {
        format!(
            "`{token}` is not a board layer: name it as the board file does, e.g. `BottomCopper`"
        )
    })
}

/// `group`: a kind of layer.
fn kind_operand(args: &CommandArgs) -> Result<LayerKind, String> {
    let token = text_operand(args, "group");
    LayerKind::from_token(token).ok_or_else(|| {
        format!(
            "`{token}` is not a kind of layer: use signal, plane, overlay, mask, paste, mechanical or all"
        )
    })
}

/// Apply `event` to the view of the board `document` names.
fn on_named_board(app: &mut Signex, args: &CommandArgs, event: Result<PcbViewMsg, String>) -> Ran {
    let path = match named_board(app, args) {
        Ok(path) => path,
        Err(reason) => return refused(reason),
    };
    match event {
        Ok(event) if app.apply_pcb_view_event_at(&path, event) => ran(Task::none()),
        Ok(_) => refused("the board is not open"),
        Err(reason) => refused(reason),
    }
}

/// `hide_layer layer=.. document=..`: the tab menu's Hide row.
pub(super) fn hide_layer(app: &mut Signex, args: &CommandArgs) -> Ran {
    let event = layer_operand(args).map(PcbViewMsg::LayerHidden);
    on_named_board(app, args, event)
}

/// `highlight_layer layer=.. document=..`: the tab menu's Highlight row;
/// highlighting the same layer again ends it.
pub(super) fn highlight_layer(app: &mut Signex, args: &CommandArgs) -> Ran {
    let event = layer_operand(args).map(PcbViewMsg::LayerHighlightToggled);
    on_named_board(app, args, event)
}

/// `hide_layer_group group=.. document=..`: a row of Hide layers ▸.
pub(super) fn hide_layer_group(app: &mut Signex, args: &CommandArgs) -> Ran {
    let event = kind_operand(args).map(|kind| PcbViewMsg::LayerKindVisibilitySet {
        kind,
        visible: false,
    });
    on_named_board(app, args, event)
}

/// `show_layer_group group=.. document=..`: a row of Show layers ▸.
pub(super) fn show_layer_group(app: &mut Signex, args: &CommandArgs) -> Ran {
    let event = kind_operand(args).map(|kind| PcbViewMsg::LayerKindVisibilitySet {
        kind,
        visible: true,
    });
    on_named_board(app, args, event)
}

/// `show_layer_set name=.. document=..`: a built-in set by its token, else
/// a saved set by its name.
pub(super) fn show_layer_set(app: &mut Signex, args: &CommandArgs) -> Ran {
    let name = text_operand(args, "name");
    if let Some(set) = BuiltinLayerSet::from_token(name) {
        return on_named_board(app, args, Ok(PcbViewMsg::BuiltinLayerSetShown(set)));
    }
    if !app.has_saved_layer_set(name) {
        return refused(format!("no layer set is called `{name}`"));
    }
    let path = match named_board(app, args) {
        Ok(path) => path,
        Err(reason) => return refused(reason),
    };
    if app.show_saved_layer_set(&BoardViewTarget::Board(path), name) {
        ran(Task::none())
    } else {
        refused("the board is not open")
    }
}

/// `save_layer_set name=.. document=..`: Save current as set... with that
/// name. A built-in set's name would hide the saved set, so it is refused.
pub(super) fn save_layer_set(app: &mut Signex, args: &CommandArgs) -> Ran {
    let name = text_operand(args, "name");
    if name.is_empty() {
        return refused("a layer set needs a name");
    }
    if BuiltinLayerSet::from_token(name).is_some() {
        return refused(format!("`{name}` is the name of a built-in layer set"));
    }
    let path = match named_board(app, args) {
        Ok(path) => path,
        Err(reason) => return refused(reason),
    };
    let task = app.save_layer_set(&BoardViewTarget::Board(path), name);
    ran(task)
}

/// `delete_layer_set name=..`: Delete on Manage sets....
pub(super) fn delete_layer_set(app: &mut Signex, args: &CommandArgs) -> Ran {
    let name = text_operand(args, "name");
    if !app.has_saved_layer_set(name) {
        return refused(format!("no saved layer set is called `{name}`"));
    }
    let task = app.update(Message::Pcb(PcbMsg::LayerSet(LayerSetMsg::Deleted(
        name.to_string(),
    ))));
    ran(task)
}

/// `set_layer_tab_names style=..`: the tab menu's Short, Medium and Long
/// tab names rows.
pub(super) fn set_layer_tab_names(app: &mut Signex, args: &CommandArgs) -> Ran {
    let token = text_operand(args, "style");
    match LayerNameStyle::from_token(token) {
        Some(style) => {
            let setting = LayerStripMsg::NameStyleSet(style);
            ran(app.update(Message::Pcb(PcbMsg::LayerStrip(setting))))
        }
        None => refused(format!(
            "`{token}` is not a tab name style: use short, medium or long"
        )),
    }
}
