use iced::Task;

use super::super::*;

impl Signex {
    pub(super) fn dispatch_pcb_message(&mut self, message: PcbMsg) -> Task<Message> {
        match message {
            PcbMsg::RatsnestNetVisibilityToggled(net) => {
                if let Some(path) = self.active_tab_path() {
                    self.interaction_state.pcb_ratsnest.toggle_net(&path, net);
                    // The cached lines stay; only the snapshot is rebuilt.
                    self.sync_pcb_canvas_from_visible_board();
                }
                Task::none()
            }
            PcbMsg::DrcRequested => self.handle_pcb_drc_requested(),
            PcbMsg::DrcFinished {
                generation,
                loose,
                result,
            } => {
                self.handle_pcb_drc_finished(generation, loose, &result);
                Task::none()
            }
            PcbMsg::DrcViolationPicked(index) => {
                self.handle_pcb_drc_violation_picked(index);
                Task::none()
            }
            PcbMsg::CanvasInWindow { window, msg } => {
                self.run_in_window(window, false, |app| app.dispatch_pcb_canvas_message(msg))
            }
            PcbMsg::View(event) => {
                self.apply_pcb_view_event(None, event);
                Task::none()
            }
            PcbMsg::ViewInWindow { window, event } => {
                self.apply_pcb_view_event(Some(window), event);
                Task::none()
            }
            PcbMsg::ViewRotation(event) => self.apply_view_rotation(None, event),
            PcbMsg::ViewRotationInWindow { window, event } => {
                self.run_in_window(window, false, |app| {
                    app.apply_view_rotation(Some(window), event)
                })
            }
            PcbMsg::Board3d(event) => self.handle_board_3d(event),
            PcbMsg::Sketch(event) => self.handle_board_sketch(event),
            PcbMsg::Reannotate(msg) => self.dispatch_board_reannotate_message(msg),
            PcbMsg::FabOutputs(msg) => self.dispatch_fab_outputs_message(msg),
            PcbMsg::Panel(msg) => self.dispatch_board_panel_message(msg),
            PcbMsg::BackAnnotate(msg) => self.dispatch_back_annotate_message(msg),
            PcbMsg::CompareBoard(msg) => self.dispatch_compare_board_message(msg),
            PcbMsg::Output(msg) => self.dispatch_board_output_message(msg),
            PcbMsg::Bar(event) => self.handle_pcb_bar(event),
            PcbMsg::PlaceToolPicked(tool) => {
                self.arm_board_tool(tool);
                // Properties shows what the next click places.
                let form = self.raise_board_tool_form();
                // From the bar, Place, a key or the palette alike, the
                // tool's bar group shows it next (#2392).
                let group = crate::app::pcb_bar_groups::PcbBarGroup::of_tool(tool);
                let remembered = match (group, tool.command()) {
                    (Some(group), Some(command)) => self.remember_pcb_bar_row(group, command),
                    _ => Task::none(),
                };
                Task::batch([form, remembered])
            }
            PcbMsg::RouteModeStepped => {
                self.step_route_mode();
                Task::none()
            }
            PcbMsg::ArcModeStepped => {
                self.step_arc_mode();
                Task::none()
            }
            PcbMsg::RepourRequested { selected } => {
                self.repour_board_in_view(selected);
                Task::none()
            }
            PcbMsg::FootprintSwap(event) => self.handle_footprint_swap(event),
            PcbMsg::DxfImport(event) => self.handle_dxf_import(event),
            PcbMsg::PartSwap(event) => self.handle_part_swap(event),
            PcbMsg::Room(event) => self.handle_room(event),
            PcbMsg::CreateRooms(event) => self.handle_create_rooms(event),
            PcbMsg::ReuseBlock(event) => self.handle_reuse_block(event),
            PcbMsg::LayerTabMenu(event) => self.handle_layer_tab_menu(event),
            PcbMsg::LayerSetMenu(event) => self.handle_layer_set_menu(event),
            PcbMsg::LayerStrip(setting) => self.apply_layer_strip_setting(setting),
            PcbMsg::LayerSet(event) => self.handle_layer_set(event),
            PcbMsg::LockRequested { locked } => {
                self.lock_pcb_selection(locked);
                Task::none()
            }
            PcbMsg::Origin(event) => {
                self.handle_board_origin(event);
                Task::none()
            }
            PcbMsg::Net(event) => {
                self.handle_board_net(event);
                Task::none()
            }
            PcbMsg::Tools(event) => {
                self.handle_board_tools(event);
                Task::none()
            }
            PcbMsg::Tune(event) => {
                self.handle_tune(event);
                Task::none()
            }
        }
    }

    /// The PCB Active Bar's dropdowns (#2352): one is open at a time, and a
    /// picked row runs its command as its menu row does.
    fn handle_pcb_bar(&mut self, event: PcbBarMsg) -> Task<Message> {
        let open = &mut self.interaction_state.pcb_bar_menu;
        match event {
            PcbBarMsg::MenuToggled(menu) => {
                *open = (*open != Some(menu)).then_some(menu);
                Task::none()
            }
            PcbBarMsg::MenuDismissed => {
                *open = None;
                Task::none()
            }
            PcbBarMsg::MenuRightPressed(menu) => {
                *open = (*open != Some(menu)).then_some(menu);
                self.set_swallow_right_release(true);
                Task::none()
            }
            PcbBarMsg::FlyoutRightPressed => {
                *open = None;
                self.set_swallow_right_release(true);
                Task::none()
            }
            PcbBarMsg::CommandPicked(command) => {
                let picked_from = open.take();
                let remembered = match picked_from {
                    Some(PcbBarMenu::Group(group)) => {
                        self.remember_pcb_bar_row(group, command.as_str())
                    }
                    _ => Task::none(),
                };
                let ran = self.handle_menu_message(crate::menu_bar::MenuMessage::Command(command));
                Task::batch([remembered, ran])
            }
        }
    }

    /// Note that `group` ran `command`, a live row of it, and write the
    /// change to `prefs.json` off the update thread (#2392). Nothing when
    /// the group already shows that row.
    pub(crate) fn remember_pcb_bar_row(
        &mut self,
        group: crate::app::pcb_bar_groups::PcbBarGroup,
        command: &str,
    ) -> Task<Message> {
        if !self.ui_state.pcb_bar_last_tools.remember(group, command) {
            return Task::none();
        }
        let last = self.ui_state.pcb_bar_last_tools.clone();
        crate::fonts::background_pref_write("pcb_bar_last_tools", move || {
            crate::fonts::write_pcb_bar_last_tools(&last)
        })
    }
}
