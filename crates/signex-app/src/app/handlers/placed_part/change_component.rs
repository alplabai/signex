//! Change Component (#1260).

use iced::Task;

use super::PlacedPart;
use crate::app::contracts::Message;
use crate::app::state::Signex;
use crate::library::messages::{PlacedPartTarget, ResolvedPlacement};

impl Signex {
    pub(super) fn start_change_component(&mut self, _part: PlacedPart) -> Task<Message> {
        Task::none()
    }

    pub(super) fn resolve_change_part(&mut self) -> Task<Message> {
        Task::none()
    }

    pub(super) fn show_change_preview(
        &mut self,
        _target: PlacedPartTarget,
        _result: Result<Box<ResolvedPlacement>, String>,
    ) -> Task<Message> {
        Task::none()
    }

    pub(super) fn back_out_of_change_preview(&mut self) -> Task<Message> {
        Task::none()
    }

    pub(super) fn confirm_change_component(&mut self) -> Task<Message> {
        Task::none()
    }
}
