//! Open Datasheet (#1301).

use iced::Task;

use super::PlacedPart;
use crate::app::contracts::Message;
use crate::app::state::Signex;

impl Signex {
    pub(super) fn open_part_datasheet(&mut self, _part: PlacedPart) -> Task<Message> {
        Task::none()
    }

    pub(super) fn open_picked_datasheet(&mut self, _index: usize) -> Task<Message> {
        Task::none()
    }
}
