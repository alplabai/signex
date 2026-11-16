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
            PcbMsg::BackAnnotate(msg) => self.dispatch_back_annotate_message(msg),
            PcbMsg::Bar(event) => self.handle_pcb_bar(event),
            PcbMsg::PlaceToolPicked(tool) => {
                self.arm_board_tool(tool);
                Task::none()
            }
            PcbMsg::FootprintSwap(event) => self.handle_footprint_swap(event),
            PcbMsg::DxfImport(event) => self.handle_dxf_import(event),
            PcbMsg::LayerTabMenu(event) => {
                self.handle_layer_tab_menu(event);
                Task::none()
            }
        }
    }

    /// The right-click menu of a layer tab (#2379): it opens at the pointer,
    /// one menu at a time, and a picked row changes the view of the board
    /// of the window it was opened in.
    fn handle_layer_tab_menu(&mut self, event: LayerTabMenuMsg) {
        match event {
            LayerTabMenuMsg::Opened { window, layer } => {
                self.close_all_context_menus();
                // The menu takes the cursor: a card the canvas showed goes.
                self.clear_canvas_hover();
                let (x, y) = self.interaction_state.last_mouse_pos;
                self.interaction_state.pcb_layer_tab_menu = Some(LayerTabMenu {
                    x,
                    y,
                    layer,
                    window,
                });
            }
            LayerTabMenuMsg::Dismissed => self.interaction_state.pcb_layer_tab_menu = None,
            LayerTabMenuMsg::Picked(view_event) => {
                if let Some(menu) = self.interaction_state.pcb_layer_tab_menu.take() {
                    self.apply_pcb_view_event(menu.window, view_event);
                }
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
            PcbBarMsg::CommandPicked(command) => {
                *open = None;
                self.handle_menu_message(crate::menu_bar::MenuMessage::Command(command))
            }
        }
    }
}
