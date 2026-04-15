use iced::Task;

use super::super::*;

impl Signex {
    pub(crate) fn handle_open_find_replace(&mut self, replace_mode: bool) -> Task<Message> {
        self.find_replace.open = true;
        self.find_replace.replace_mode = replace_mode;
        self.context_menu = None;
        self.panel_list_open = false;
        self.refresh_find_matches();
        Task::none()
    }

    pub(crate) fn handle_find_replace_msg(
        &mut self,
        msg: crate::find_replace::FindReplaceMsg,
    ) -> Task<Message> {
        use crate::find_replace::FindReplaceMsg;

        match msg {
            FindReplaceMsg::Close => {
                self.find_replace.open = false;
            }
            FindReplaceMsg::QueryChanged(query) => {
                self.find_replace.query = query;
                self.refresh_find_matches();
            }
            FindReplaceMsg::ReplacementChanged(value) => {
                self.find_replace.replacement = value;
            }
            FindReplaceMsg::SelectResult(index) => {
                self.find_replace.selected_index = Some(index);
                if let Some(hit) = self.find_replace.matches.get(index) {
                    self.canvas.selected = vec![hit.item];
                    self.canvas.clear_overlay_cache();
                    self.update_selection_info();
                }
            }
            FindReplaceMsg::ReplaceCurrent => {
                if let Some(index) = self.find_replace.selected_index
                    && let Some(hit) = self.find_replace.matches.get(index).cloned()
                {
                    self.apply_engine_command(
                        signex_engine::Command::UpdateText {
                            target: hit.target,
                            value: self.find_replace.replacement.clone(),
                        },
                        true,
                        true,
                    );
                    self.refresh_find_matches();
                }
            }
            FindReplaceMsg::ReplaceAll => {
                if !self.find_replace.matches.is_empty() {
                    let commands: Vec<_> = self
                        .find_replace
                        .matches
                        .iter()
                        .map(|hit| signex_engine::Command::UpdateText {
                            target: hit.target,
                            value: self.find_replace.replacement.clone(),
                        })
                        .collect();
                    self.apply_engine_commands(commands, true, true);
                    self.refresh_find_matches();
                }
            }
        }

        Task::none()
    }

    fn refresh_find_matches(&mut self) {
        let query = self.find_replace.query.trim();
        if query.is_empty() {
            self.find_replace.matches.clear();
            self.find_replace.selected_index = None;
            return;
        }

        let needle = query.to_lowercase();
        let mut matches = Vec::new();

        if let Some(snapshot) = self.active_render_snapshot() {
            for label in &snapshot.labels {
                if label.text.to_lowercase().contains(&needle) {
                    matches.push(crate::find_replace::FindMatch {
                        item: signex_types::schematic::SelectedItem::new(
                            label.uuid,
                            signex_types::schematic::SelectedKind::Label,
                        ),
                        target: signex_engine::TextTarget::Label(label.uuid),
                        kind_label: "Net Label".to_string(),
                        text: label.text.clone(),
                    });
                }
            }
            for note in &snapshot.text_notes {
                if note.text.to_lowercase().contains(&needle) {
                    matches.push(crate::find_replace::FindMatch {
                        item: signex_types::schematic::SelectedItem::new(
                            note.uuid,
                            signex_types::schematic::SelectedKind::TextNote,
                        ),
                        target: signex_engine::TextTarget::TextNote(note.uuid),
                        kind_label: "Text Note".to_string(),
                        text: note.text.clone(),
                    });
                }
            }
            for symbol in &snapshot.symbols {
                if symbol.reference.to_lowercase().contains(&needle) {
                    matches.push(crate::find_replace::FindMatch {
                        item: signex_types::schematic::SelectedItem::new(
                            symbol.uuid,
                            signex_types::schematic::SelectedKind::SymbolRefField,
                        ),
                        target: signex_engine::TextTarget::SymbolReference(symbol.uuid),
                        kind_label: "Designator".to_string(),
                        text: symbol.reference.clone(),
                    });
                }
                if symbol.value.to_lowercase().contains(&needle) {
                    matches.push(crate::find_replace::FindMatch {
                        item: signex_types::schematic::SelectedItem::new(
                            symbol.uuid,
                            signex_types::schematic::SelectedKind::SymbolValField,
                        ),
                        target: signex_engine::TextTarget::SymbolValue(symbol.uuid),
                        kind_label: "Value".to_string(),
                        text: symbol.value.clone(),
                    });
                }
            }
        }

        self.find_replace.matches = matches;
        self.find_replace.selected_index = if self.find_replace.matches.is_empty() {
            None
        } else {
            Some(self.find_replace.selected_index.unwrap_or(0).min(self.find_replace.matches.len() - 1))
        };

        if let Some(index) = self.find_replace.selected_index
            && let Some(hit) = self.find_replace.matches.get(index)
        {
            self.canvas.selected = vec![hit.item];
            self.canvas.clear_overlay_cache();
            self.update_selection_info();
        }
    }
}