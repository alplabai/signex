//! Open Datasheet (#1301): resolve the part's library row, then open the
//! one document it offers or ask which of several.

use iced::Task;

use super::{PlacedPart, part_name};
use crate::app::contracts::Message;
use crate::app::state::Signex;
use crate::library::PickerState;
use crate::library::placed_part::opener::open_task;
use crate::library::placed_part::{DatasheetTarget, PartSource, datasheet_targets};
use crate::library::state::{DatasheetChoice, PickerMode};

impl Signex {
    pub(super) fn open_part_datasheet(&mut self, part: PlacedPart) -> Task<Message> {
        let symbol = &part.symbol;
        let reference = part_name(symbol);
        let row = self.row_of_placed_symbol(symbol);
        let source = match (&row, symbol.library_id.zip(symbol.row_id)) {
            (Some((root, _, row)), _) => PartSource::Row {
                row,
                library_file: root,
            },
            (None, Some(_)) => PartSource::RowUnavailable,
            (None, None) => PartSource::NoRow,
        };
        let targets = match datasheet_targets(&reference, source, &symbol.datasheet) {
            Ok(targets) => targets,
            Err(advice) => {
                crate::diagnostics::log_warning(advice);
                return Task::none();
            }
        };
        match <[DatasheetTarget; 1]>::try_from(targets) {
            Ok([only]) => open_task(only.location, reference, self.library.datasheet_opener),
            Err(targets) => {
                self.library.picker = Some(PickerState {
                    mode: PickerMode::Datasheets(DatasheetChoice { reference, targets }),
                    ..PickerState::default()
                });
                Task::none()
            }
        }
    }

    /// The user picked document `index` in the datasheet list.
    pub(super) fn open_picked_datasheet(&mut self, index: usize) -> Task<Message> {
        let Some(PickerState {
            mode: PickerMode::Datasheets(choice),
            ..
        }) = self.library.picker.take()
        else {
            return Task::none();
        };
        let reference = choice.reference.clone();
        match choice.targets.into_iter().nth(index) {
            Some(target) => open_task(target.location, reference, self.library.datasheet_opener),
            None => Task::none(),
        }
    }
}
