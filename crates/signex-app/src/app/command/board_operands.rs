//! The board commands of the DRC results panel (#2399), the DRC fixes
//! (#2940) and the board Net Classes panel (#2400), run from their
//! operands. Each names its board with
//! `document` and runs the same app path as the panel's button, so a person,
//! the CLI and an agent get one behaviour; each answers with what it did as
//! JSON (`CommandOutcome::Answered`).

use std::path::PathBuf;

use iced::Task;
use serde_json::Value;
use signex_engine::NetClassCommand;
use signex_engine::pcb::drc_fix::{FixRule, FixScope};
use signex_types::format::parse_pcb_layer;
use signex_types::net_class::NetClassStyle;
use signex_types::pcb_layer::PcbLayer;
use signex_types::theme::Color;
use signex_types::violation::Severity;

use super::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::document::DocumentKey;
use crate::app::handlers::board_net_classes::{typed_mm, with_rule};
use crate::app::{Message, Signex};
use crate::panels::{BoardNetClassField, DrcFilter};

/// What running one command from its operands did, and any follow-up task.
type Ran = (CommandOutcome, Task<Message>);

fn answered(result: Result<Value, String>) -> Ran {
    let outcome = match result {
        Ok(answer) => CommandOutcome::Answered(answer),
        Err(reason) => CommandOutcome::disabled(reason),
    };
    (outcome, Task::none())
}

/// The open board `args` names.
fn named_board(app: &Signex, args: &CommandArgs, what: &str) -> Result<PathBuf, String> {
    let Some(OperandValue::Document(target)) = args.operand("document") else {
        return Err(format!("{what} needs the operand `document`"));
    };
    let entry = app.resolve_document_target(target)?;
    match entry.key {
        Some(DocumentKey::Pcb(path)) => Ok(path),
        Some(_) => Err(format!(
            "{what} acts on a board, and {} is a {}",
            entry.name,
            entry.kind.as_str()
        )),
        None => Err(format!(
            "{} is referenced by a project but not open; open it first",
            entry.name
        )),
    }
}

fn text<'a>(args: &'a CommandArgs, name: &str) -> Option<&'a str> {
    match args.operand(name) {
        Some(OperandValue::Text(text)) => Some(text.as_str()),
        _ => None,
    }
}

fn required_text<'a>(args: &'a CommandArgs, name: &str, what: &str) -> Result<&'a str, String> {
    text(args, name).ok_or_else(|| format!("{what} needs the operand `{name}`"))
}

/// `show_drc_violation document=.. id=K`: select the violation's objects
/// and centre the view on it, as a click on its row does.
fn show_drc_violation_answer(app: &mut Signex, args: &CommandArgs) -> Result<Value, String> {
    let what = "`show_drc_violation`";
    let board = named_board(app, args, what)?;
    let key = required_text(args, "id", what)?.to_string();
    app.show_drc_violation(&board, &key)
}

/// `waive_drc_violation document=.. id=K [text=note] [author=name]`: waive
/// it with the note and author, as the panel's Waive does; one board undo
/// step.
fn waive_drc_violation_answer(app: &mut Signex, args: &CommandArgs) -> Result<Value, String> {
    let what = "`waive_drc_violation`";
    let board = named_board(app, args, what)?;
    let key = required_text(args, "id", what)?.to_string();
    let waiver = crate::app::handlers::DrcWaiverText {
        note: text(args, "text").unwrap_or_default().to_string(),
        author: text(args, "author").unwrap_or_default().to_string(),
    };
    app.set_drc_waiver(&board, &key, Some(waiver))
}

/// `unwaive_drc_violation document=.. id=K`: take its waiver back.
fn unwaive_drc_violation_answer(app: &mut Signex, args: &CommandArgs) -> Result<Value, String> {
    let what = "`unwaive_drc_violation`";
    let board = named_board(app, args, what)?;
    let key = required_text(args, "id", what)?.to_string();
    app.set_drc_waiver(&board, &key, None)
}

/// `dry_run=true`: answer with the planned change and change nothing.
fn dry_run(args: &CommandArgs) -> bool {
    matches!(args.operand("dry_run"), Some(OperandValue::Bool(true)))
}

/// `fix_drc_violation document=.. violation=K [dry_run=]` (#2940): fix the
/// finding with waiver key `K`, never by waiving it; one board undo step.
fn fix_drc_violation_answer(app: &mut Signex, args: &CommandArgs) -> Result<Value, String> {
    let what = "`fix_drc_violation`";
    let board = named_board(app, args, what)?;
    let key = required_text(args, "violation", what)?.trim().to_string();
    app.fix_drc(&board, &FixScope::One(key), dry_run(args))
}

/// `fix_drc document=.. [rule=<kind>] [dry_run=]` (#2940): fix every
/// finding of one rule kind, or of every kind with a fix (`all` or left out).
fn fix_drc_answer(app: &mut Signex, args: &CommandArgs) -> Result<Value, String> {
    let board = named_board(app, args, "`fix_drc`")?;
    let rule = match filter_text(args, "rule") {
        None => None,
        Some(token) => Some(FixRule::parse(token).ok_or_else(|| {
            let kinds: Vec<&str> = FixRule::ALL.iter().map(|r| r.token()).collect();
            format!(
                "`{token}` is not a rule with a fix; one of {}",
                kinds.join(", ")
            )
        })?),
    };
    app.fix_drc(&board, &FixScope::Every(rule), dry_run(args))
}

/// A filter operand: left out or `all` is every value.
fn filter_text<'a>(args: &'a CommandArgs, name: &str) -> Option<&'a str> {
    text(args, name)
        .map(str::trim)
        .filter(|t| !t.is_empty() && !t.eq_ignore_ascii_case("all"))
}

/// The filter `args` asks for.
fn drc_filter(args: &CommandArgs) -> Result<DrcFilter, String> {
    let layer = match filter_text(args, "layer") {
        None => None,
        Some(token) => match parse_pcb_layer(token) {
            Some(PcbLayer::Copper(layer)) => Some(layer),
            _ => return Err(format!("`{token}` is not a copper layer (TopCopper, ...)")),
        },
    };
    let severity = match filter_text(args, "severity").map(str::to_ascii_lowercase) {
        None => None,
        Some(word) => Some(match word.as_str() {
            "error" => Severity::Error,
            "warning" => Severity::Warning,
            "info" => Severity::Info,
            _ => return Err(format!("`{word}` is not a severity (error, warning, info)")),
        }),
    };
    Ok(DrcFilter {
        rule: filter_text(args, "rule").map(str::to_string),
        layer,
        severity,
        show_waived: matches!(args.operand("waived"), Some(OperandValue::Bool(true))),
    })
}

/// `filter_drc_results document=.. [rule=] [layer=] [severity=] [waived=]`:
/// set what the DRC results show and answer with the rows shown.
fn filter_drc_results_answer(app: &mut Signex, args: &CommandArgs) -> Result<Value, String> {
    let board = named_board(app, args, "`filter_drc_results`")?;
    let filter = drc_filter(args)?;
    app.filter_drc_results(&board, filter)
}

/// `add_net_class document=.. [name=N]`: a class in the board's project.
fn add_net_class_answer(app: &mut Signex, args: &CommandArgs) -> Result<Value, String> {
    let board = named_board(app, args, "`add_net_class`")?;
    let name = text(args, "name").map(str::to_string);
    app.add_board_net_class(&board, name)
}

/// `rename_net_class document=.. class=C name=N`.
fn rename_net_class_answer(app: &mut Signex, args: &CommandArgs) -> Result<Value, String> {
    let what = "`rename_net_class`";
    let board = named_board(app, args, what)?;
    let command = NetClassCommand::Rename {
        from: required_text(args, "class", what)?.to_string(),
        to: required_text(args, "name", what)?.to_string(),
    };
    app.edit_board_net_classes(&board, command)
}

/// `delete_net_class document=.. class=C`.
fn delete_net_class_answer(app: &mut Signex, args: &CommandArgs) -> Result<Value, String> {
    let what = "`delete_net_class`";
    let board = named_board(app, args, what)?;
    let name = required_text(args, "class", what)?.to_string();
    app.edit_board_net_classes(&board, NetClassCommand::Remove { name })
}

/// `set_net_class_rules document=.. class=C [color=#rrggbb] [width=..] ..`:
/// the colour and the board rules given, as one step; an empty length
/// unsets that rule, one left out keeps its value.
fn set_net_class_rules_answer(app: &mut Signex, args: &CommandArgs) -> Result<Value, String> {
    let what = "`set_net_class_rules`";
    let board = named_board(app, args, what)?;
    let name = required_text(args, "class", what)?;
    let class = app
        .board_net_classes(&board)
        .iter()
        .find(|c| c.name == name)
        .cloned()
        .ok_or_else(|| format!("no net class named \"{name}\""))?;
    let mut rules = class.board_rules();
    let mut any_rule = false;
    for field in BoardNetClassField::RULES {
        if let Some(typed) = text(args, field.operand()) {
            let mm = typed_mm(typed)
                .ok_or_else(|| format!("`{}` must be a number of millimetres", field.operand()))?;
            rules = with_rule(rules, field, mm);
            any_rule = true;
        }
    }
    let mut commands = Vec::new();
    if let Some(OperandValue::Color(r, g, b)) = args.operand("color") {
        commands.push(NetClassCommand::SetStyle {
            name: class.name.clone(),
            style: NetClassStyle {
                color: Some(Color::new(*r, *g, *b, 255)),
                ..class.style
            },
        });
    }
    if any_rule {
        commands.push(NetClassCommand::SetBoardRules {
            name: class.name.clone(),
            rules,
        });
    }
    if commands.is_empty() {
        return Err(format!("{what} needs a colour or a rule to set"));
    }
    app.edit_board_net_classes(&board, NetClassCommand::Batch { commands })
}

/// `assign_board_net_class document=.. net_name=N [class=C]`: the net into
/// that class (out of every other class's list), or out of its listed one.
fn assign_board_net_class_answer(app: &mut Signex, args: &CommandArgs) -> Result<Value, String> {
    let what = "`assign_board_net_class`";
    let board = named_board(app, args, what)?;
    let net = required_text(args, "net_name", what)?.to_string();
    let class = text(args, "class")
        .filter(|c| !c.trim().is_empty())
        .map(str::to_string);
    app.edit_board_net_classes(&board, NetClassCommand::AssignNet { net, class })
}

/// The operand handlers: each runs its `_answer` and reports what it
/// answered, or why it was refused.
macro_rules! answering {
    ($($name:ident => $answer:ident;)+) => {$(
        pub(super) fn $name(app: &mut Signex, args: &CommandArgs) -> Ran {
            answered($answer(app, args))
        }
    )+};
}

answering! {
    show_drc_violation => show_drc_violation_answer;
    waive_drc_violation => waive_drc_violation_answer;
    unwaive_drc_violation => unwaive_drc_violation_answer;
    fix_drc_violation => fix_drc_violation_answer;
    fix_drc => fix_drc_answer;
    filter_drc_results => filter_drc_results_answer;
    add_net_class => add_net_class_answer;
    rename_net_class => rename_net_class_answer;
    delete_net_class => delete_net_class_answer;
    set_net_class_rules => set_net_class_rules_answer;
    assign_board_net_class => assign_board_net_class_answer;
}
