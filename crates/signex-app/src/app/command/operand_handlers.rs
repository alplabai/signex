//! The commands that run their operands (#1934, #2155): a command acts on the
//! operands a caller parsed ([`CommandArgs::parse_operands`]) only if it has
//! a handler here. One that declares operands and has none is refused
//! ([`Signex::run_operands`]), so no caller is ever told `Ran` for operands
//! nobody read. There is no flag to switch: a handler is code that reads them.
//!
//! A handler runs the same app commit path as the click or the button of the
//! same id, so one command id has one behaviour on every surface.

use iced::Task;
use signex_types::theme::Color;

use super::{CommandArgs, CommandOutcome, OperandValue, enablement};
use crate::app::mutation_gateway::CommandOutcome as Mutation;
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;

/// What running one command from its operands did, and any follow-up task.
type Ran = (CommandOutcome, Task<Message>);

/// Runs one command from its parsed operands.
pub(crate) type OperandHandler = fn(&mut Signex, &CommandArgs) -> Ran;

/// Every command that runs its operands, by id. Each needs a click-equivalence
/// test (`operand_handler_tests`); `every_operand_command_has_a_handler_or_is_refused`
/// keeps this table and the catalog in step.
const HANDLERS: &[(&str, OperandHandler)] = &[
    ("color_net", color_net),
    ("clear_all_net_colors", clear_all_net_colors),
    ("assign_net_class", assign_net_class),
];

/// The handler of `command`, if it runs its operands.
pub(crate) fn operand_handler(command: &AppCommandId) -> Option<OperandHandler> {
    HANDLERS
        .iter()
        .find(|(id, _)| *id == command.as_str())
        .map(|(_, handler)| *handler)
}

/// Why `command` does nothing without operands on a surface that gives it
/// none (palette, key binding, menu): its GUI surface is another widget, and
/// a command never opens, closes or focuses one as a side effect.
pub(crate) fn needs_operands_reason(command: &AppCommandId) -> Option<&'static str> {
    match command.as_str() {
        "assign_net_class" => Some(
            "`assign_net_class` needs the operand `net_name` — \
             use the Net Class row in Properties",
        ),
        _ => None,
    }
}

/// Whether a headless caller must name the document (`path`) the command
/// acts on: it never falls back to the active one.
pub(crate) fn headless_needs_path(command: &AppCommandId) -> bool {
    command.as_str() == "assign_net_class"
}

/// The ids that run their operands.
#[cfg(test)]
pub(super) fn handled_ids() -> impl Iterator<Item = &'static str> {
    HANDLERS.iter().map(|(id, _)| *id)
}

impl Signex {
    /// Run `command` from the operands in `args`: its handler, behind the same
    /// enablement gate as a dispatch. A command with no handler is refused.
    pub(crate) fn run_operands(&mut self, command: &AppCommandId, args: &CommandArgs) -> Ran {
        let Some(handler) = operand_handler(command) else {
            return refused("operands are declared but not run yet (#1934)");
        };
        if let Some(reason) = enablement::disabled_reason(command, &self.command_context(None)) {
            return refused(reason);
        }
        handler(self, args)
    }
}

fn refused(reason: impl Into<String>) -> Ran {
    (CommandOutcome::disabled(reason), Task::none())
}

fn ran() -> Ran {
    (CommandOutcome::Ran, Task::none())
}

/// `Ok` when no `path` was given, or it names the active document.
fn acts_on_active_document(app: &Signex, path: Option<&OperandValue>) -> Result<(), String> {
    let Some(OperandValue::Path(given)) = path else {
        return Ok(());
    };
    match app.document_state.active_path.as_deref() {
        Some(active) if active == std::path::Path::new(given) => Ok(()),
        _ => Err(format!("{given} is not the active document")),
    }
}

/// `color_net at=x,y color=#rrggbb [path=..]`: colour the net whose wire is
/// under the point, as a pen click does.
fn color_net(app: &mut Signex, args: &CommandArgs) -> Ran {
    let (Some(OperandValue::Point(x, y)), Some(OperandValue::Color(r, g, b))) =
        (args.operand("at"), args.operand("color"))
    else {
        return refused("`color_net` needs the operands `at` and `color`");
    };
    if let Err(reason) = acts_on_active_document(app, args.operand("path")) {
        return refused(reason);
    }
    let net = app.net_elements_at(*x, *y);
    if net.is_empty() {
        return refused(format!("there is no wire at {x},{y}"));
    }
    match app.commit_net_colors(&net, Color::new(*r, *g, *b, 255)) {
        (Mutation::Changed, _) => ran(),
        (Mutation::Unchanged, _) => refused("the net already has that colour"),
        (Mutation::Failed, why) => refused(why.unwrap_or_else(|| "no colour was applied".into())),
    }
}

/// `clear_all_net_colors [path=..]`: clear the colours of every open sheet, or
/// of the one at `path`, as the Active Bar button does.
fn clear_all_net_colors(app: &mut Signex, args: &CommandArgs) -> Ran {
    let only = match args.operand("path") {
        Some(OperandValue::Path(path)) => Some(std::path::PathBuf::from(path)),
        _ => None,
    };
    if let Some(path) = &only
        && !app.document_state.engines.contains_key(path)
    {
        return refused(format!("{} is not an open document", path.display()));
    }
    let cleared = app.run_clear_all_net_colors(only.as_deref());
    if cleared.sheets > 0 {
        ran()
    } else if cleared.kept_read_only > 0 {
        refused("the sheet is read-only: its net colours were kept")
    } else {
        refused("there are no net colours to clear")
    }
}

/// `assign_net_class net_name=N [class=C] [path=..]`: put the net in the
/// class (or take it out of its listed one), as the label picker does.
fn assign_net_class(app: &mut Signex, args: &CommandArgs) -> Ran {
    let Some(OperandValue::Text(net)) = args.operand("net_name") else {
        return refused("`assign_net_class` needs the operand `net_name`");
    };
    let sheet = match args.operand("path") {
        Some(OperandValue::Path(path)) => Some(std::path::PathBuf::from(path)),
        _ => None,
    };
    let class = match args.operand("class") {
        Some(OperandValue::Text(class)) => Some(class.as_str()),
        _ => None,
    };
    match app.assign_net_to_class(sheet.as_deref(), net, class) {
        Ok(()) => ran(),
        Err(reason) => refused(reason),
    }
}
