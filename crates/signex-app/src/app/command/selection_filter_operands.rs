//! The schematic Selection Filter from operands (#2417). `set_selection_filters`
//! and `apply_selection_filter_preset` run the handlers of the Filter
//! panel and the Active Bar's filter dropdown, so a CLI or an agent sets the
//! one filter a click sets. The filter is window state: no document, no undo.

use std::collections::HashSet;

use iced::Task;

use super::{CommandArgs, CommandOutcome, OperandValue};
use crate::active_bar::{SelectionFilter, preset_label};
use crate::app::{Message, Signex};
use crate::dock::DockMessage;
use crate::panels::{PanelMsg, SchFilterMsg};

type Ran = (CommandOutcome, Task<Message>);

fn refused(reason: impl Into<String>) -> Ran {
    (CommandOutcome::disabled(reason), Task::none())
}

/// The categories `kinds` names: `all`, `none`, or comma-separated
/// [`SelectionFilter::token`]s.
pub(super) fn parse_kinds(text: &str) -> Result<HashSet<SelectionFilter>, String> {
    match text.trim() {
        "all" => return Ok(SelectionFilter::ALL.iter().copied().collect()),
        "none" => return Ok(HashSet::new()),
        _ => {}
    }
    text.split(',')
        .map(str::trim)
        .map(|token| {
            SelectionFilter::from_token(token).ok_or_else(|| {
                let known: Vec<&str> = SelectionFilter::ALL.iter().map(|f| f.token()).collect();
                format!(
                    "`{token}` is not a selection filter kind; use all, none or {}",
                    known.join(", ")
                )
            })
        })
        .collect()
}

/// `set_selection_filters kinds=..`: the named categories on, every other off.
pub(super) fn set_selection_filters(app: &mut Signex, args: &CommandArgs) -> Ran {
    let Some(OperandValue::Text(kinds)) = args.operand("kinds") else {
        return refused("`set_selection_filters` needs the operand `kinds`");
    };
    match parse_kinds(kinds) {
        Ok(filters) => (CommandOutcome::Ran, app.set_selection_filters(filters)),
        Err(reason) => refused(reason),
    }
}

/// `apply_selection_filter_preset name=..`: the preset's categories replace
/// the filter, as a click on its chip does. An unnamed preset answers to the
/// label its chip shows ("Filter 2").
pub(super) fn apply_selection_filter_preset(app: &mut Signex, args: &CommandArgs) -> Ran {
    let Some(OperandValue::Text(name)) = args.operand("name") else {
        return refused("`apply_selection_filter_preset` needs the operand `name`");
    };
    let presets = &app.interaction_state.custom_filter_presets;
    let found = presets
        .iter()
        .position(|preset| preset.name == *name)
        .or_else(|| {
            presets
                .iter()
                .enumerate()
                .position(|(idx, preset)| preset_label(idx, preset) == *name)
        });
    let Some(idx) = found else {
        return refused(format!("no selection filter preset is named `{name}`"));
    };
    let click = PanelMsg::SchFilter(SchFilterMsg::PresetApplied(idx));
    let task = app.update(Message::Dock(DockMessage::Panel(click)));
    (CommandOutcome::Ran, task)
}
