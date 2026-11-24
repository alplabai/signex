//! The panel commands from their operands (#2410): an agent or the CLI names
//! the board (`document`) and gets the panel as JSON after each edit, the
//! same JSON `board_panel` reads. The edits run the same path as the menu
//! rows and the Properties panel, one undo step each.

use std::path::Path;

use iced::Task;
use serde_json::{Map, Value, json};
use signex_types::pcb_panel::{BoardPanel, PanelField, PanelFieldKind};

use crate::app::command::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::{Message, Signex};

type Ran = (CommandOutcome, Task<Message>);

fn refused(reason: impl Into<String>) -> Ran {
    (CommandOutcome::disabled(reason), Task::none())
}

fn text<'a>(args: &'a CommandArgs, name: &str) -> Option<&'a str> {
    match args.operand(name) {
        Some(OperandValue::Text(text)) => Some(text.as_str()),
        _ => None,
    }
}

/// Every setting of `panel` by its operand name.
fn settings(panel: &BoardPanel) -> Value {
    let mut out = Map::new();
    for field in PanelField::ALL {
        let value = match field.kind() {
            PanelFieldKind::Count => json!(field.count(panel)),
            PanelFieldKind::Length => json!(field.length_mm(panel)),
            PanelFieldKind::Separation => json!(panel.separation.token()),
        };
        out.insert(field.id().to_string(), value);
    }
    Value::Object(out)
}

impl Signex {
    /// The board at `path`, its revision, its panel's settings (`null`
    /// without one) and what the panel comes to, or why it does not lay out.
    pub(crate) fn board_panel_answer(&self, path: &Path) -> Result<Value, String> {
        let engine = self
            .document_state
            .board_engine(path)
            .ok_or_else(|| format!("no PCB is open at {}", path.display()))?;
        let board = engine.board();
        let mut answer = json!({
            "document": path.display().to_string(),
            "revision": engine.revision(),
            "panel": Value::Null,
        });
        let Some(panel) = board.options.panel.as_ref() else {
            return Ok(answer);
        };
        answer["panel"] = settings(panel);
        answer["layout"] = match pcb_geom::panel::layout(&board.outline, panel) {
            Ok(laid) => json!({
                "width_mm": laid.width_mm(),
                "height_mm": laid.height_mm(),
                "copies": laid.copies.len(),
                "tabs": laid.tabs,
                "bite_holes": laid.bite_holes.len(),
                "v_scores": laid.v_scores.len(),
                "fiducials": laid.fiducials.len(),
                "tooling_holes": laid.tooling_holes.len(),
                "rectangular": laid.rectangular,
            }),
            Err(error) => json!({ "error": error.to_string() }),
        };
        Ok(answer)
    }

    /// `board_panel document=<board>`: the board's panel as JSON.
    pub(crate) fn board_panel_query(&self, args: &CommandArgs) -> Result<Value, String> {
        let path = self.named_output_board(args)?;
        self.board_panel_answer(&path)
    }
}

/// The answer after an edit: the panel as it now is and whether it changed.
fn edited(app: &Signex, path: &Path, changed: Result<bool, String>) -> Ran {
    let changed = match changed {
        Ok(changed) => changed,
        Err(reason) => return refused(reason),
    };
    match app.board_panel_answer(path) {
        Ok(mut answer) => {
            answer["changed"] = json!(changed);
            (CommandOutcome::Answered(answer), Task::none())
        }
        Err(reason) => refused(reason),
    }
}

/// `panelize_board document=<board>`: the default panel; a board that has
/// one keeps it (`changed: false`).
pub(crate) fn panelize_board(app: &mut Signex, args: &CommandArgs) -> Ran {
    let path = match app.named_output_board(args) {
        Ok(path) => path,
        Err(reason) => return refused(reason),
    };
    let changed = app.panelize(&path);
    edited(app, &path, changed)
}

/// `remove_board_panel document=<board>`: the board on its own again;
/// refused for a board without a panel.
pub(crate) fn remove_board_panel(app: &mut Signex, args: &CommandArgs) -> Ran {
    let path = match app.named_output_board(args) {
        Ok(path) => path,
        Err(reason) => return refused(reason),
    };
    let changed = app.remove_panel(&path);
    edited(app, &path, changed)
}

/// `set_board_panel document=<board> field=<setting> value=<text>`: one
/// setting, a whole number, a length in mm, or `mouse_bites` / `v_score`.
pub(crate) fn set_board_panel(app: &mut Signex, args: &CommandArgs) -> Ran {
    let path = match app.named_output_board(args) {
        Ok(path) => path,
        Err(reason) => return refused(reason),
    };
    let Some(field) = text(args, "field").and_then(PanelField::from_id) else {
        let known: Vec<&str> = PanelField::ALL.iter().map(|f| f.id()).collect();
        return refused(format!("`field` is one of {}", known.join(", ")));
    };
    let Some(value) = text(args, "value") else {
        return refused("needs the operand `value`");
    };
    let changed = app.edit_board_panel(&path, |panel| field.set_text(panel, value));
    edited(app, &path, changed)
}
