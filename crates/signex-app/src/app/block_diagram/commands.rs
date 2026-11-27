//! The `block_*` commands and `new_block_diagram` (mockup row 171, #2796):
//! each reads its operands into the same [`DiagramEdit`] the tab builds and
//! runs it through [`Signex::edit_block_diagram`], so a command and the
//! click it stands for change the diagram alike. Each answers with JSON
//! naming what it made or changed, or is refused with the reason.

use std::path::{Path, PathBuf};

use iced::Task;
use serde_json::{Value, json};
use signex_i18n::{Translatable, ids};
use signex_types::block_diagram::{
    Block, BlockDiagram, BlockFields, BlockPatch, BlockShape, Endpoint, Interface, PortDirection,
    PortSide,
};
use signex_types::schematic::Point;
use signex_types::theme::Color;
use uuid::Uuid;

use super::edit::DiagramEdit;
use super::fields::{parse_mm, parse_rails, parse_specs};
use super::refusal::Refusal;
use super::state::BlockDiagramEditor;
use super::{BLOCK_DIAGRAM_EXTENSION, file_name};
use crate::app::command::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::document_ids::DocumentKind;
use crate::app::{Message, Signex};

/// What running one command did, and any follow-up task.
type Ran = (CommandOutcome, Task<Message>);

/// The spacing of ports placed without an offset, mm.
pub(crate) const PORT_PITCH_MM: f64 = 5.0;

fn answered(result: Result<Value, Refusal>) -> Ran {
    let outcome = match result {
        Ok(value) => CommandOutcome::Answered(value),
        Err(refusal) => CommandOutcome::Disabled {
            reason: refusal.translatable(),
        },
    };
    (outcome, Task::none())
}

/// Run the edit `id` on the diagram `args` names, read first when it is
/// not open (`on_demand`).
fn edited(
    app: &mut Signex,
    id: &str,
    args: &CommandArgs,
    edit: impl FnOnce(&mut Signex, &CommandArgs) -> Result<Value, Refusal>,
) -> Ran {
    match app.open_diagram_first(id, args) {
        Some(waiting) => waiting,
        None => answered(edit(app, args)),
    }
}

fn text<'a>(args: &'a CommandArgs, name: &str) -> Option<&'a str> {
    match args.operand(name) {
        Some(OperandValue::Text(text)) => Some(text.as_str()),
        _ => None,
    }
}

fn required_text<'a>(args: &'a CommandArgs, name: &'static str) -> Result<&'a str, Refusal> {
    text(args, name).ok_or(Refusal::Needs { operand: name })
}

fn parse_id(text: &str, operand: &'static str) -> Result<Uuid, Refusal> {
    Uuid::parse_str(text.trim()).map_err(|_| Refusal::BadId {
        operand,
        text: text.to_string(),
    })
}

fn uuid_of(args: &CommandArgs, name: &'static str) -> Result<Uuid, Refusal> {
    parse_id(required_text(args, name)?, name)
}

fn point(args: &CommandArgs, name: &'static str) -> Result<Option<Point>, Refusal> {
    match args.operand(name) {
        Some(OperandValue::Point(x, y)) => Ok(Some(Point::new(*x, *y))),
        Some(OperandValue::PinRef { .. }) => Err(Refusal::PinPoint { operand: name }),
        _ => Ok(None),
    }
}

fn color(args: &CommandArgs) -> Option<Color> {
    match args.operand("color") {
        Some(OperandValue::Color(r, g, b)) => Some(Color::new(*r, *g, *b, 255)),
        _ => None,
    }
}

fn shape(args: &CommandArgs) -> Result<Option<BlockShape>, Refusal> {
    let Some(t) = text(args, "shape") else {
        return Ok(None);
    };
    BlockShape::from_token(t.trim())
        .map(Some)
        .ok_or_else(|| Refusal::BadShape {
            text: t.to_string(),
        })
}

/// The fields `clear` names, each one of `allowed`.
fn clears(args: &CommandArgs, allowed: &[&str]) -> Result<Vec<String>, Refusal> {
    let Some(list) = text(args, "clear") else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for name in list.split(',').map(str::trim).filter(|n| !n.is_empty()) {
        if !allowed.contains(&name) {
            return Err(Refusal::BadClear {
                text: name.to_string(),
            });
        }
        out.push(name.to_string());
    }
    Ok(out)
}

/// Where the next port on `side` of `block` goes when none is given: one
/// pitch past the ports already on that side.
pub(crate) fn next_port_offset(block: &Block, side: PortSide) -> f64 {
    let count = block.ports.iter().filter(|p| p.side == side).count();
    PORT_PITCH_MM * (count as f64 + 1.0)
}

impl Signex {
    /// The path of the open diagram the `document` operand names.
    pub(crate) fn named_block_diagram(&self, args: &CommandArgs) -> Result<PathBuf, Refusal> {
        let Some(OperandValue::Document(target)) = args.operand("document") else {
            return Err(Refusal::NoDocument);
        };
        let entry = self
            .resolve_document_target(target)
            .map_err(|_| Refusal::NotOpen {
                file: target.to_string(),
            })?;
        if entry.kind != DocumentKind::BlockDiagram {
            return Err(Refusal::NotADiagram {
                name: entry.name,
                kind: entry.kind.as_str(),
            });
        }
        let name = entry.name;
        entry
            .path
            .filter(|path| {
                self.document_state
                    .block_diagrams
                    .editors
                    .contains_key(path)
            })
            .ok_or(Refusal::NotOpen { file: name })
    }

    fn diagram_at(&self, path: &Path) -> Option<&BlockDiagram> {
        self.document_state
            .block_diagrams
            .editors
            .get(path)
            .map(|editor| &editor.diagram)
    }

    /// Make an empty diagram named `name` (the project's name without one)
    /// in the active project's folder and open it, unsaved. Nothing is
    /// written; its first save refuses to replace a file already there.
    pub(crate) fn new_block_diagram(&mut self, name: Option<&str>) -> Result<PathBuf, Refusal> {
        let project = self
            .document_state
            .active_loaded_project()
            .ok_or(Refusal::NoProject)?;
        let stem = name.map_or_else(|| project.data.name.clone(), |n| n.trim().to_string());
        if stem.is_empty() || stem.contains(['/', '\\']) || stem == "." || stem == ".." {
            return Err(Refusal::BadName { name: stem });
        }
        let path = project
            .dir()
            .join(format!("{stem}.{BLOCK_DIAGRAM_EXTENSION}"));
        let diagrams = &mut self.document_state.block_diagrams;
        if diagrams.editors.contains_key(&path) || diagrams.opening.contains(&path) {
            return Err(Refusal::AlreadyOpen {
                file: file_name(&path),
            });
        }
        let editor = BlockDiagramEditor::created(BlockDiagram::new());
        diagrams.editors.insert(path.clone(), editor);
        super::view_of(&mut self.document_view.block_diagrams, &path);
        self.document_state.sync_block_diagram_dirty(&path);
        self.show_block_diagram_tab(&path);
        Ok(path)
    }
}

/// `new_block_diagram [name=]`.
pub(crate) fn new_block_diagram(app: &mut Signex, args: &CommandArgs) -> Ran {
    let made = app.new_block_diagram(text(args, "name")).map(|path| {
        let id = app.diagram_at(&path).map(|diagram| diagram.id.to_string());
        json!({ "id": id, "path": path.display().to_string(), "kind": "block_diagram" })
    });
    answered(made)
}

/// `undo` / `redo` with a `document` that names a diagram: one step of
/// that diagram's own history.
pub(crate) fn step_by_command(app: &mut Signex, path: &Path, undo: bool) -> Ran {
    if app.step_block_diagram(path, undo) {
        return (CommandOutcome::Ran, Task::none());
    }
    let id = if undo {
        ids::BLOCK_DIAGRAM_NOTHING_TO_UNDO
    } else {
        ids::BLOCK_DIAGRAM_NOTHING_TO_REDO
    };
    let outcome = CommandOutcome::Disabled {
        reason: Translatable::new(id),
    };
    (outcome, Task::none())
}

/// `save_document` with a `document` that names a diagram: write it in a
/// task. The command runs once the write is started; the write's own
/// failure is a notice, as for a click on Save.
pub(crate) fn save_by_command(app: &mut Signex, path: &Path) -> Ran {
    match app.begin_block_diagram_save(path) {
        Ok(task) => (CommandOutcome::Ran, task),
        Err(refusal) => answered(Err(refusal)),
    }
}

/// The block properties `block_add` and `block_set` share, as a patch.
fn block_patch(args: &CommandArgs) -> Result<BlockPatch, Refusal> {
    let size = point(args, "size")?;
    let group = text(args, "group")
        .map(|t| parse_id(t, "group"))
        .transpose()?;
    Ok(BlockPatch {
        name: text(args, "name").map(str::to_string),
        function: text(args, "function").map(str::to_string),
        note: text(args, "note").map(str::to_string),
        shape: shape(args)?,
        position: point(args, "at")?,
        width_mm: size.map(|s| s.x),
        height_mm: size.map(|s| s.y),
        color: color(args).map(Some),
        group: group.map(Some),
        rails: text(args, "rails").map(parse_rails),
        specs: text(args, "specs").map(parse_specs).transpose()?,
    })
}

/// `block_add document= name= [at= size= function= note= shape= color=
/// group= rails= specs=]`. Without `at` the block goes right of the others.
pub(crate) fn block_add(app: &mut Signex, args: &CommandArgs) -> Ran {
    edited(app, "block_add", args, add_block)
}

fn add_block(app: &mut Signex, args: &CommandArgs) -> Result<Value, Refusal> {
    let path = app.named_block_diagram(args)?;
    let name = required_text(args, "name")?;
    let patch = block_patch(args)?;
    let at = match patch.position {
        Some(at) => at,
        None => app
            .diagram_at(&path)
            .map_or(Point::ZERO, BlockDiagram::next_free_position),
    };
    let mut fields = BlockFields::named(name, at);
    fields.function = patch.function.unwrap_or_default();
    fields.note = patch.note.unwrap_or_default();
    fields.shape = patch.shape.unwrap_or_default();
    fields.width_mm = patch.width_mm.unwrap_or(fields.width_mm);
    fields.height_mm = patch.height_mm.unwrap_or(fields.height_mm);
    fields.color = patch.color.flatten();
    fields.group = patch.group.flatten();
    fields.rails = patch.rails.unwrap_or_default();
    fields.specs = patch.specs.unwrap_or_default();
    app.edit_block_diagram(&path, DiagramEdit::AddBlock(fields))
}

/// `block_set document= id= [name= at= size= function= note= shape= color=
/// group= rails= specs= clear=]`.
pub(crate) fn block_set(app: &mut Signex, args: &CommandArgs) -> Ran {
    edited(app, "block_set", args, set_block)
}

fn set_block(app: &mut Signex, args: &CommandArgs) -> Result<Value, Refusal> {
    let path = app.named_block_diagram(args)?;
    let id = uuid_of(args, "id")?;
    let mut patch = block_patch(args)?;
    for field in clears(args, &["color", "group"])? {
        if field == "color" {
            patch.color = Some(None);
        } else {
            patch.group = Some(None);
        }
    }
    app.edit_block_diagram(&path, DiagramEdit::SetBlock(id, patch))
}

/// `block_remove document= id=`: the block, its ports and connections.
pub(crate) fn block_remove(app: &mut Signex, args: &CommandArgs) -> Ran {
    edited(app, "block_remove", args, |app, args| {
        one_object(app, args, DiagramEdit::RemoveBlock)
    })
}

/// `block_port_remove document= id=`: the port and its connections.
pub(crate) fn block_port_remove(app: &mut Signex, args: &CommandArgs) -> Ran {
    edited(app, "block_port_remove", args, |app, args| {
        one_object(app, args, DiagramEdit::RemovePort)
    })
}

/// `block_unlink document= id=`: the connection.
pub(crate) fn block_unlink(app: &mut Signex, args: &CommandArgs) -> Ran {
    edited(app, "block_unlink", args, |app, args| {
        one_object(app, args, DiagramEdit::Disconnect)
    })
}

/// `block_ungroup document= id=`: the group; its blocks stay.
pub(crate) fn block_ungroup(app: &mut Signex, args: &CommandArgs) -> Ran {
    edited(app, "block_ungroup", args, |app, args| {
        one_object(app, args, DiagramEdit::RemoveGroup)
    })
}

fn one_object(
    app: &mut Signex,
    args: &CommandArgs,
    edit: fn(Uuid) -> DiagramEdit,
) -> Result<Value, Refusal> {
    let path = app.named_block_diagram(args)?;
    let id = uuid_of(args, "id")?;
    app.edit_block_diagram(&path, edit(id))
}

/// `block_port_add document= block= name= [side= offset= direction=]`.
pub(crate) fn block_port_add(app: &mut Signex, args: &CommandArgs) -> Ran {
    edited(app, "block_port_add", args, add_port)
}

fn add_port(app: &mut Signex, args: &CommandArgs) -> Result<Value, Refusal> {
    let path = app.named_block_diagram(args)?;
    let block = uuid_of(args, "block")?;
    let name = required_text(args, "name")?.to_string();
    let side = match text(args, "side") {
        Some(t) => PortSide::from_token(t.trim()).ok_or_else(|| Refusal::BadSide {
            text: t.to_string(),
        })?,
        None => PortSide::Left,
    };
    let direction = match text(args, "direction") {
        Some(t) => PortDirection::from_token(t.trim()).ok_or_else(|| Refusal::BadDirection {
            text: t.to_string(),
        })?,
        None => PortDirection::Bidirectional,
    };
    let offset_mm = match text(args, "offset") {
        Some(t) => parse_mm(t)?,
        None => app
            .diagram_at(&path)
            .and_then(|d| d.block(block))
            .map_or(0.0, |b| next_port_offset(b, side)),
    };
    let edit = DiagramEdit::AddPort {
        block,
        name,
        side,
        offset_mm,
        direction,
    };
    app.edit_block_diagram(&path, edit)
}

/// The end a `from` or `to` operand names: a block, or a port with its
/// block.
fn endpoint(
    diagram: &BlockDiagram,
    args: &CommandArgs,
    name: &'static str,
) -> Result<Endpoint, Refusal> {
    let id = uuid_of(args, name)?;
    if diagram.block(id).is_some() {
        return Ok(Endpoint {
            block: id,
            port: None,
        });
    }
    diagram
        .blocks
        .iter()
        .find(|b| b.port(id).is_some())
        .map(|b| Endpoint {
            block: b.id,
            port: Some(id),
        })
        .ok_or_else(|| Refusal::NoEnd {
            text: id.to_string(),
        })
}

/// `block_link document= from= to= [interface= label=]`.
pub(crate) fn block_link(app: &mut Signex, args: &CommandArgs) -> Ran {
    edited(app, "block_link", args, link)
}

fn link(app: &mut Signex, args: &CommandArgs) -> Result<Value, Refusal> {
    let path = app.named_block_diagram(args)?;
    let diagram = app.diagram_at(&path).ok_or_else(|| Refusal::NotOpen {
        file: file_name(&path),
    })?;
    let from = endpoint(diagram, args, "from")?;
    let to = endpoint(diagram, args, "to")?;
    let edit = DiagramEdit::Connect {
        from,
        to,
        interface: text(args, "interface").map_or(Interface::Signal, Interface::from_token),
        label: text(args, "label").unwrap_or_default().to_string(),
    };
    app.edit_block_diagram(&path, edit)
}

/// `block_link_set document= id= [interface= label=]`.
pub(crate) fn block_link_set(app: &mut Signex, args: &CommandArgs) -> Ran {
    edited(app, "block_link_set", args, set_link)
}

fn set_link(app: &mut Signex, args: &CommandArgs) -> Result<Value, Refusal> {
    let path = app.named_block_diagram(args)?;
    let edit = DiagramEdit::SetConnection {
        id: uuid_of(args, "id")?,
        interface: text(args, "interface").map(Interface::from_token),
        label: text(args, "label").map(str::to_string),
    };
    app.edit_block_diagram(&path, edit)
}

/// `block_group document= name= [ids= color=]`.
pub(crate) fn block_group(app: &mut Signex, args: &CommandArgs) -> Ran {
    edited(app, "block_group", args, group)
}

fn group(app: &mut Signex, args: &CommandArgs) -> Result<Value, Refusal> {
    let path = app.named_block_diagram(args)?;
    let edit = DiagramEdit::AddGroup {
        name: required_text(args, "name")?.to_string(),
        color: color(args),
        blocks: args
            .targets("ids")
            .map(<[Uuid]>::to_vec)
            .unwrap_or_default(),
    };
    app.edit_block_diagram(&path, edit)
}

/// `block_group_set document= id= [name= color= clear=]`.
pub(crate) fn block_group_set(app: &mut Signex, args: &CommandArgs) -> Ran {
    edited(app, "block_group_set", args, set_group)
}

fn set_group(app: &mut Signex, args: &CommandArgs) -> Result<Value, Refusal> {
    let path = app.named_block_diagram(args)?;
    let cleared = !clears(args, &["color"])?.is_empty();
    let edit = DiagramEdit::SetGroup {
        id: uuid_of(args, "id")?,
        name: text(args, "name").map(str::to_string),
        color: if cleared {
            Some(None)
        } else {
            color(args).map(Some)
        },
    };
    app.edit_block_diagram(&path, edit)
}
