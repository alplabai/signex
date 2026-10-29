//! The hover insight: note what the cursor rests on, and on the dwell timer
//! answer from the cached index with a card of the net's name, class and pins.

use iced::Task;
use signex_types::schematic::Point;

use crate::app::state::net_nav::{HoverCard, PendingNetAction};

use super::super::super::*;
use super::index_job::IndexLookup;
use super::target::{is_net_kind, net_of_target};

impl Signex {
    /// Note the object under the cursor after a move. Cheap: it reads the
    /// canvas hit test only; the index is consulted on the dwell, never here.
    pub(crate) fn observe_net_hover(
        &mut self,
        hit: Option<signex_types::schematic::SelectedItem>,
        world: (f64, f64),
    ) {
        let target = hit.filter(|item| is_net_kind(item.kind)).and_then(|item| {
            let sheet = self.document_state.active_path.clone()?;
            Some((sheet, item, Point::new(world.0, world.1)))
        });
        let enabled = self.net_hover_allowed();
        let screen = self.interaction_state.last_mouse_pos;
        self.document_state.net_nav.hover.observe(
            target,
            screen,
            enabled,
            std::time::Instant::now(),
        );
    }

    /// The hover card shows only with the preference on and nothing being
    /// placed or dragged.
    fn net_hover_allowed(&self) -> bool {
        self.ui_state.net_hover_enabled
            && self.interaction_state.current_tool == crate::app::Tool::Select
            && self.interaction_state.placement.is_none()
    }

    /// The dwell timer fired: answer for the candidate once its dwell is up.
    pub(crate) fn hover_dwell_ticked(&mut self) -> Task<Message> {
        let now = std::time::Instant::now();
        let Some(candidate) = self
            .document_state
            .net_nav
            .hover
            .candidate
            .clone()
            .filter(|c| c.is_due(now))
        else {
            return Task::none();
        };
        if !self.net_hover_allowed() {
            self.settle_hover(None);
            return Task::none();
        }
        let Some(project) = self.project_of_sheet(&candidate.sheet) else {
            self.settle_hover(None);
            return Task::none();
        };
        match self.net_index_lookup(project) {
            IndexLookup::Ready(data) => {
                let card = data.key_of(&candidate.sheet).and_then(|key| {
                    let net = net_of_target(&data, &key, &candidate.item, candidate.world)?;
                    let hover = data.index.hover(net, &data.classes)?;
                    Some(HoverCard::from_hover(
                        &hover,
                        |k| data.sheet_label(k),
                        candidate.screen,
                    ))
                });
                self.settle_hover(card);
                Task::none()
            }
            IndexLookup::Building(build) => {
                self.wait_for_net_index(PendingNetAction::HoverDwell, build)
            }
            IndexLookup::Unavailable => {
                self.settle_hover(None);
                Task::none()
            }
        }
    }

    /// The dwell is answered: show `card`, or nothing, and stop the timer.
    fn settle_hover(&mut self, card: Option<HoverCard>) {
        let hover = &mut self.document_state.net_nav.hover;
        if let Some(candidate) = hover.candidate.as_mut() {
            candidate.settled = true;
        }
        hover.card = card;
    }
}
