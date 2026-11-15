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
            PcbMsg::Sketch(event) => self.handle_board_sketch(event),
            PcbMsg::Reannotate(msg) => self.dispatch_board_reannotate_message(msg),
            PcbMsg::BackAnnotate(msg) => self.dispatch_back_annotate_message(msg),
            PcbMsg::Bar(event) => self.handle_pcb_bar(event),
            PcbMsg::PlaceToolPicked(tool) => {
                self.arm_board_tool(tool);
                Task::none()
            }
            PcbMsg::FootprintSwap(event) => self.handle_footprint_swap(event),
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
