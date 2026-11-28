//! Actions on the selected placed part that lead into the library: Open
//! Datasheet (#1301), Edit Symbol (#1262) and Change Component (#1260).
//!
//! They enter as [`PlacedPartMsg::ActionRequested`] from the canvas context
//! menu, the Properties panel and the command ids, and act on the one
//! selected, non-power symbol. Reading a library file or launching a viewer
//! is a `Task`; `update` only decides, logs and edits the model.

#[cfg(test)]
mod blocker_tests;
mod change_component;
mod change_session;
mod datasheet;
mod edit_symbol;
mod pin_mapping;

pub(crate) use change_component::replacement_of;

use iced::Task;
use signex_types::schematic::{SelectedKind, Symbol};

use super::super::contracts::Message;
use super::super::state::Signex;
use crate::library::messages::{PlacedPartAction, PlacedPartMsg, PlacedPartTarget};
use crate::library::state::ChangeStep;

/// The selected placed part, copied off the active sheet.
pub(crate) struct PlacedPart {
    pub(crate) target: PlacedPartTarget,
    pub(crate) symbol: Symbol,
}

/// How messages name the part: its designator, or "the part" for one that
/// has none yet.
pub(crate) fn part_name(symbol: &Symbol) -> String {
    match symbol.reference.trim() {
        "" => "the part".to_string(),
        reference => reference.to_string(),
    }
}

/// The one symbol `canvas` selects on `engine`'s sheet, unless it is a power
/// port (those carry no library part).
pub(crate) fn selected_placed_symbol<'a>(
    engine: &'a signex_engine::Engine,
    canvas: &crate::canvas::CanvasSlot,
) -> Option<&'a Symbol> {
    let [item] = canvas.selected.as_slice() else {
        return None;
    };
    if item.kind != SelectedKind::Symbol {
        return None;
    }
    engine
        .document()
        .symbols
        .iter()
        .find(|s| s.uuid == item.uuid && !s.is_power)
}

impl Signex {
    /// The one selected symbol of the active sheet, unless it is a power
    /// port (those carry no library part).
    pub(crate) fn selected_placed_part(&self) -> Option<PlacedPart> {
        let sheet = self.document_state.active_path.clone()?;
        let engine = self.document_state.active_engine()?;
        let symbol =
            selected_placed_symbol(engine, self.interaction_state.active_canvas())?.clone();
        Some(PlacedPart {
            target: PlacedPartTarget {
                sheet,
                engine_instance: engine.instance_id(),
                symbol_id: symbol.uuid,
            },
            symbol,
        })
    }

    pub(crate) fn dispatch_placed_part_message(&mut self, msg: PlacedPartMsg) -> Task<Message> {
        match msg {
            PlacedPartMsg::ActionRequested(action) => {
                let Some(part) = self.selected_placed_part() else {
                    crate::diagnostics::log_warning(
                        "Select one placed part first: this action works on a single part, not on a power port.",
                    );
                    return Task::none();
                };
                match action {
                    PlacedPartAction::OpenDatasheet => self.open_part_datasheet(part),
                    PlacedPartAction::EditSymbol => self.edit_part_symbol(part),
                    PlacedPartAction::ChangeComponent => self.start_change_component(part),
                }
            }
            PlacedPartMsg::DatasheetPicked(index) => self.open_picked_datasheet(index),
            PlacedPartMsg::DatasheetOpenFailed(advice) => {
                crate::diagnostics::log_warning(advice);
                Task::none()
            }
            PlacedPartMsg::ChangePartPicked => self.resolve_change_part(),
            PlacedPartMsg::ChangePartResolved {
                target,
                row_id,
                result,
            } => self.show_change_preview(target, row_id, result),
            PlacedPartMsg::ChangePreviewBackedOut => self.back_out_of_change_preview(),
            PlacedPartMsg::ChangeNextPressed => self.step_change(ChangeStep::Compare),
            PlacedPartMsg::ChangeBackPressed => self.step_change(ChangeStep::Pick),
            PlacedPartMsg::ChangeConfirmed => self.confirm_change_component(),
            PlacedPartMsg::ChangeRowClicked(row_id) => self.click_change_row(row_id),
            PlacedPartMsg::ChangeFilterToggled(filter) => self.toggle_change_filter(filter),
            PlacedPartMsg::ChangeKeepToggled(name) => self.toggle_change_keep(name),
            PlacedPartMsg::ChangeScopePicked(scope) => self.pick_change_scope(scope),
            PlacedPartMsg::ChangePinRemapped { old, new } => self.remap_change_pin(old, new),
            PlacedPartMsg::ChangePinCountsRead { target, counts } => {
                self.store_row_pins(target, counts)
            }
        }
    }
}
