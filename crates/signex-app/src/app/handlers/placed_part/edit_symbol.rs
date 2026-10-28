//! Edit Symbol (#1262).

use iced::Task;

use super::PlacedPart;
use crate::app::contracts::Message;
use crate::app::state::Signex;

impl Signex {
    pub(super) fn edit_part_symbol(&mut self, _part: PlacedPart) -> Task<Message> {
        Task::none()
    }
}
