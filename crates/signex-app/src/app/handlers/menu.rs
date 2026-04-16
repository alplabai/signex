use iced::Task;

use super::super::*;

impl Signex {
    pub(crate) fn handle_menu_message(&mut self, msg: MenuMessage) -> Task<Message> {
        match msg {
            MenuMessage::OpenProject => Task::perform(
                async {
                    rfd::AsyncFileDialog::new()
                        .set_title("Open Project or Schematic")
                        .add_filter("Signex Project", &["snxprj"])
                        .add_filter("Signex Schematic", &["snxsch"])
                        .add_filter("KiCad Schematic", &["kicad_sch"])
                        .add_filter("KiCad Project", &["kicad_pro"])
                        .add_filter("All Supported", &["snxprj", "snxsch", "kicad_sch", "kicad_pro"])
                        .add_filter("All files", &["*"])
                        .pick_file()
                        .await
                        .map(|f| f.path().to_path_buf())
                },
                Message::FileOpened,
            ),
            MenuMessage::ZoomFit => {
                if self.has_active_schematic() {
                    self.interaction_state.canvas.fit_to_paper();
                    self.interaction_state.canvas.clear_bg_cache();
                    self.interaction_state.canvas.clear_content_cache();
                } else if self.has_active_pcb() {
                    self.interaction_state.pcb_canvas.fit_to_board();
                    self.interaction_state.pcb_canvas.clear_bg_cache();
                    self.interaction_state.pcb_canvas.clear_content_cache();
                }
                Task::none()
            }
            MenuMessage::ToggleGrid => {
                self.ui_state.grid_visible = !self.ui_state.grid_visible;
                self.interaction_state.canvas.grid_visible = self.ui_state.grid_visible;
                self.interaction_state.pcb_canvas.grid_visible = self.ui_state.grid_visible;
                self.interaction_state.canvas.clear_bg_cache();
                self.interaction_state.pcb_canvas.clear_bg_cache();
                Task::none()
            }
            MenuMessage::CycleGrid => {
                self.interaction_state.canvas.clear_bg_cache();
                Task::none()
            }
            MenuMessage::OpenProjectsPanel => {
                self.document_state.dock.add_panel(crate::dock::PanelPosition::Left, crate::panels::PanelKind::Projects);
                Task::none()
            }
            MenuMessage::OpenComponentsPanel => {
                self.document_state.dock.add_panel(crate::dock::PanelPosition::Left, crate::panels::PanelKind::Components);
                Task::none()
            }
            MenuMessage::OpenNavigatorPanel => {
                self.document_state.dock.add_panel(crate::dock::PanelPosition::Right, crate::panels::PanelKind::Navigator);
                Task::none()
            }
            MenuMessage::OpenPropertiesPanel => {
                self.document_state.dock.add_panel(crate::dock::PanelPosition::Right, crate::panels::PanelKind::Properties);
                Task::none()
            }
            MenuMessage::OpenMessagesPanel => {
                self.document_state.dock.add_panel(crate::dock::PanelPosition::Bottom, crate::panels::PanelKind::Messages);
                Task::none()
            }
            MenuMessage::OpenSignalPanel => {
                self.document_state.dock.add_panel(crate::dock::PanelPosition::Bottom, crate::panels::PanelKind::Signal);
                Task::none()
            }
            MenuMessage::PlaceWire => {
                self.interaction_state.current_tool = Tool::Wire;
                self.clear_measurement();
                Task::none()
            }
            MenuMessage::PlaceBus => {
                self.interaction_state.current_tool = Tool::Bus;
                self.clear_measurement();
                Task::none()
            }
            MenuMessage::PlaceLabel => {
                self.interaction_state.current_tool = Tool::Label;
                self.clear_measurement();
                Task::none()
            }
            MenuMessage::PlaceComponent => {
                self.interaction_state.current_tool = Tool::Component;
                self.clear_measurement();
                Task::none()
            }
            MenuMessage::Undo => self.update(Message::Undo),
            MenuMessage::Redo => self.update(Message::Redo),
            MenuMessage::Cut => self.update(Message::Cut),
            MenuMessage::Copy => self.update(Message::Copy),
            MenuMessage::Paste => self.update(Message::Paste),
            MenuMessage::SmartPaste => self.update(Message::SmartPaste),
            MenuMessage::Delete => self.update(Message::DeleteSelected),
            MenuMessage::SelectAll => self.update(Message::Selection(selection_message::SelectionMessage::SelectAll)),
            MenuMessage::Duplicate => self.update(Message::Duplicate),
            MenuMessage::Find => self.update(Message::OpenFind),
            MenuMessage::Replace => self.update(Message::OpenReplace),
            MenuMessage::Save => self.update(Message::SaveFile),
            MenuMessage::SaveAs => Task::perform(
                async {
                    rfd::AsyncFileDialog::new()
                        .set_title("Save Schematic As")
                        .add_filter("Signex Schematic", &["snxsch"])
                        .add_filter("KiCad Schematic", &["kicad_sch"])
                        .save_file()
                        .await
                        .map(|f| f.path().to_path_buf())
                },
                |path| path.map(Message::SaveFileAs).unwrap_or(Message::Noop),
            ),
            MenuMessage::NewProject
            | MenuMessage::ZoomIn
            | MenuMessage::ZoomOut
            | MenuMessage::Annotate
            | MenuMessage::Erc
            | MenuMessage::GenerateBom => Task::none(),
            MenuMessage::OpenPreferences => self.update(Message::OpenPreferences),
        }
    }
}