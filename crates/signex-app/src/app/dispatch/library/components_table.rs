//! The Components panel table (mockup row 52): applies a table message,
//! rebuilds the cache, and turns what the message owes into tasks: the
//! saved setups and last category written off the UI thread, the header strip moved with the
//! body.

use super::*;

use crate::library::messages::ComponentsTableMsg;
use crate::panels::components_panel::table::{BODY_SCROLL, HEADER_SCROLL};

/// The background-write key of the saved setups.
const VIEWS_PREF: &str = crate::fonts::COMPONENTS_VIEWS_KEY;

impl Signex {
    pub(super) fn handle_components_table(&mut self, msg: ComponentsTableMsg) -> Task<Message> {
        let fx = self.library.components_panel.table.apply(msg);
        self.library.sync_view_caches_in(&self.ui_state.i18n);
        let mut tasks = Vec::new();
        if fx.save_prefs {
            let views = self.library.components_panel.table.prefs();
            tasks.push(crate::fonts::background_pref_write(VIEWS_PREF, move || {
                crate::fonts::write_components_views(&views)
            }));
        }
        if let Some(x) = fx.header_x {
            tasks.push(iced::widget::operation::scrollable::scroll_to(
                HEADER_SCROLL,
                iced::widget::scrollable::AbsoluteOffset {
                    x: Some(x),
                    y: None,
                },
                iced::widget::operation::Animation::Instant,
            ));
        }
        if fx.body_to_top {
            let table = &mut self.library.components_panel.table;
            table.scroll.y = 0.0;
            tasks.push(iced::widget::operation::scrollable::scroll_to(
                BODY_SCROLL,
                iced::widget::scrollable::AbsoluteOffset {
                    x: None,
                    y: Some(0.0),
                },
                iced::widget::operation::Animation::Instant,
            ));
        }
        Task::batch(tasks)
    }
}
