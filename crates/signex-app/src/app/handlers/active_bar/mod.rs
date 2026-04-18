use iced::Task;

use super::super::*;

mod action_groups;
mod filter_controls;
mod placement_presets;

impl Signex {
    pub(crate) fn handle_active_bar_message(
        &mut self,
        msg: crate::active_bar::ActiveBarMsg,
    ) -> Task<Message> {
        use crate::active_bar::ActiveBarMsg;

        match msg {
            ActiveBarMsg::ToggleMenu(menu) => {
                self.interaction_state.active_bar_menu = Some(menu);
                self.interaction_state.context_menu = None;
                Task::none()
            }
            ActiveBarMsg::CloseMenus => {
                self.interaction_state.active_bar_menu = None;
                Task::none()
            }
            ActiveBarMsg::ToggleFilter(filter) => self.handle_active_bar_filter_toggle(filter),
            ActiveBarMsg::ToggleAllFilters => self.handle_active_bar_all_filters_toggle(),
            ActiveBarMsg::Action(action) => self.handle_active_bar_action(action),
        }
    }

    fn remember_active_bar_group(&mut self, action: &crate::active_bar::ActiveBarAction) {
        use crate::active_bar::ActiveBarAction;

        let group = match action {
            ActiveBarAction::DrawWire
            | ActiveBarAction::DrawBus
            | ActiveBarAction::PlaceBusEntry
            | ActiveBarAction::PlaceNetLabel => Some("wiring"),
            ActiveBarAction::PlacePowerGND
            | ActiveBarAction::PlacePowerVCC
            | ActiveBarAction::PlacePowerPlus12
            | ActiveBarAction::PlacePowerPlus5
            | ActiveBarAction::PlacePowerMinus5
            | ActiveBarAction::PlacePowerArrow
            | ActiveBarAction::PlacePowerWave
            | ActiveBarAction::PlacePowerBar
            | ActiveBarAction::PlacePowerCircle
            | ActiveBarAction::PlacePowerSignalGND
            | ActiveBarAction::PlacePowerEarth => Some("power"),
            ActiveBarAction::PlaceTextString
            | ActiveBarAction::PlaceTextFrame
            | ActiveBarAction::PlaceNote => Some("text"),
            ActiveBarAction::DrawArc
            | ActiveBarAction::DrawFullCircle
            | ActiveBarAction::DrawEllipticalArc
            | ActiveBarAction::DrawEllipse
            | ActiveBarAction::DrawLine
            | ActiveBarAction::DrawRectangle
            | ActiveBarAction::DrawRoundRectangle
            | ActiveBarAction::DrawPolygon
            | ActiveBarAction::DrawBezier
            | ActiveBarAction::PlaceGraphic => Some("shapes"),
            ActiveBarAction::PlaceSignalHarness
            | ActiveBarAction::PlaceHarnessConnector
            | ActiveBarAction::PlaceHarnessEntry => Some("harness"),
            ActiveBarAction::PlacePort | ActiveBarAction::PlaceOffSheetConnector => Some("port"),
            ActiveBarAction::PlaceSheetSymbol
            | ActiveBarAction::PlaceSheetEntry
            | ActiveBarAction::PlaceDeviceSheetSymbol
            | ActiveBarAction::PlaceReuseBlock => Some("sheet"),
            ActiveBarAction::PlaceParameterSet
            | ActiveBarAction::PlaceNoERC
            | ActiveBarAction::PlaceDiffPair
            | ActiveBarAction::PlaceBlanket
            | ActiveBarAction::PlaceCompileMask => Some("directives"),
            _ => None,
        };

        if let Some(group_name) = group {
            self.interaction_state
                .last_tool
                .insert(group_name.to_string(), action.clone());
        }
    }
}
