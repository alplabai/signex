//! The symbol editor's curve tools as an agent or the CLI runs them (#2838):
//! `symbol_draw_elliptical_arc`, `symbol_draw_bezier`,
//! `symbol_draw_round_rectangle` and `symbol_draw_pie`, on the open symbol
//! `document` names. Points are the symbol's own mm, as the editor's status
//! bar reads them; angles are degrees, counter-clockwise from the start to
//! the end. Each shape commits through the tools' own commit
//! ([`place_shape`]), one undo step on the part and body the editor shows,
//! and answers with the graphic as JSON.

use std::path::PathBuf;

use iced::Task;
use serde_json::{Value, json};
use signex_library::SymbolGraphicKind;

use super::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::document_ids::DocumentKind;
use crate::app::{Message, Signex};
use crate::library::editor::symbol::place_shape;

type Ran = (CommandOutcome, Task<Message>);

fn refused(reason: impl Into<String>) -> Ran {
    (CommandOutcome::disabled(reason), Task::none())
}

/// A point operand in the symbol's mm. A pin reference names a pin of a
/// placed part on a sheet, which a symbol body has none of.
fn point(args: &CommandArgs, name: &str) -> Result<[f64; 2], String> {
    match args.operand(name) {
        Some(OperandValue::Point(x, y)) => Ok([*x, *y]),
        Some(OperandValue::PinRef { .. }) => Err(format!(
            "`{name}` is a point `x,y` in the symbol's mm, not a pin reference"
        )),
        _ => Err(format!("needs the operand `{name}`")),
    }
}

fn length(args: &CommandArgs, name: &str) -> Result<f64, String> {
    match args.operand(name) {
        Some(OperandValue::Length(mm)) => Ok(*mm),
        _ => Err(format!("needs the operand `{name}`")),
    }
}

fn angle(args: &CommandArgs, name: &str) -> Result<f64, String> {
    match args.operand(name) {
        Some(OperandValue::Angle(deg)) => Ok(*deg),
        _ => Err(format!("needs the operand `{name}`")),
    }
}

/// The id a shape's kind answers with.
fn kind_id(kind: &SymbolGraphicKind) -> &'static str {
    match kind {
        SymbolGraphicKind::Bezier { .. } => "bezier",
        SymbolGraphicKind::EllipticalArc { .. } => "elliptical_arc",
        SymbolGraphicKind::RoundRectangle { .. } => "round_rectangle",
        SymbolGraphicKind::Pie { .. } => "pie",
        _ => "other",
    }
}

/// `symbol_draw_elliptical_arc document=.. center=x,y radius_x=..
/// radius_y=.. start_angle=.. end_angle=..`.
pub(super) fn elliptical_arc(app: &mut Signex, args: &CommandArgs) -> Ran {
    app.run_symbol_shape(args, |args| {
        Ok(SymbolGraphicKind::EllipticalArc {
            center: point(args, "center")?,
            radius_x: length(args, "radius_x")?,
            radius_y: length(args, "radius_y")?,
            start_deg: angle(args, "start_angle")?,
            end_deg: angle(args, "end_angle")?,
        })
    })
}

/// `symbol_draw_bezier document=.. start=x,y control_1=x,y control_2=x,y
/// end=x,y`.
pub(super) fn bezier(app: &mut Signex, args: &CommandArgs) -> Ran {
    app.run_symbol_shape(args, |args| {
        Ok(SymbolGraphicKind::Bezier {
            points: [
                point(args, "start")?,
                point(args, "control_1")?,
                point(args, "control_2")?,
                point(args, "end")?,
            ],
        })
    })
}

/// `symbol_draw_round_rectangle document=.. start=x,y end=x,y
/// [corner_radius=..]`: without a radius, the tool's default.
pub(super) fn round_rectangle(app: &mut Signex, args: &CommandArgs) -> Ran {
    app.run_symbol_shape(args, |args| {
        let mut kind =
            SymbolGraphicKind::round_rectangle(point(args, "start")?, point(args, "end")?);
        if let (
            Some(OperandValue::Length(asked)),
            SymbolGraphicKind::RoundRectangle { corner_radius, .. },
        ) = (args.operand("corner_radius"), &mut kind)
        {
            *corner_radius = *asked;
        }
        Ok(kind)
    })
}

/// `symbol_draw_pie document=.. center=x,y radius=.. start_angle=..
/// end_angle=..`.
pub(super) fn pie(app: &mut Signex, args: &CommandArgs) -> Ran {
    app.run_symbol_shape(args, |args| {
        Ok(SymbolGraphicKind::Pie {
            center: point(args, "center")?,
            radius: length(args, "radius")?,
            start_deg: angle(args, "start_angle")?,
            end_deg: angle(args, "end_angle")?,
        })
    })
}

impl Signex {
    /// The open symbol `document` names. Refused when it names none, names
    /// another kind, or the symbol is not open in an editor.
    fn named_symbol(&self, args: &CommandArgs) -> Result<PathBuf, String> {
        let Some(OperandValue::Document(named)) = args.operand("document") else {
            return Err("needs the operand `document`".to_string());
        };
        let entry = self.resolve_document_target(named)?;
        if entry.kind != DocumentKind::Symbol {
            return Err(format!(
                "{} is a {}, not a symbol",
                entry.name,
                entry.kind.as_str()
            ));
        }
        let path = entry
            .path
            .ok_or_else(|| format!("{} has no file", entry.name))?;
        if !self.document_state.symbol_editors.contains_key(&path) {
            return Err(format!("{} is not open: open the symbol first", entry.name));
        }
        Ok(path)
    }

    /// Build the shape from `args` and place it on the named symbol as one
    /// undo step; the answer is the graphic as it landed.
    fn run_symbol_shape(
        &mut self,
        args: &CommandArgs,
        shape: impl FnOnce(&CommandArgs) -> Result<SymbolGraphicKind, String>,
    ) -> Ran {
        let path = match self.named_symbol(args) {
            Ok(path) => path,
            Err(reason) => return refused(reason),
        };
        let kind = match shape(args) {
            Ok(kind) => kind,
            Err(reason) => return refused(reason),
        };
        let Some(editor) = self.document_state.symbol_editors.get_mut(&path) else {
            return refused("the symbol is not open");
        };
        let index = match place_shape(editor, kind) {
            Ok(index) => index,
            Err(refusal) => return refused(refusal.reason()),
        };
        let answer = shape_answer(editor, index, &path);
        // The Properties panel reads the editor's graphics, as after a click.
        self.refresh_panel_ctx();
        (CommandOutcome::Answered(answer), Task::none())
    }
}

/// The graphic at `index` as the answer carries it.
fn shape_answer(
    editor: &crate::app::SymbolDocument,
    index: usize,
    path: &std::path::Path,
) -> Value {
    let Some(graphic) = editor.primitive().graphics.get(index) else {
        return Value::Null;
    };
    json!({
        "document": path.display().to_string(),
        "index": index,
        "kind": kind_id(&graphic.kind),
        "part": graphic.part_number,
        "body_style": graphic.body_style,
        "stroke_width": graphic.stroke_width,
        "shape": serde_json::to_value(&graphic.kind).unwrap_or(Value::Null),
    })
}
