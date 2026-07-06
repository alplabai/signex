use iced::Task;

use super::super::*;

impl Signex {
    pub(crate) fn handle_find_replace_open_requested(
        &mut self,
        replace_mode: bool,
    ) -> Task<Message> {
        self.ui_state.find_replace.open = true;
        self.ui_state.find_replace.replace_mode = replace_mode;
        self.interaction_state.context_menu = None;
        self.ui_state.panel_list_open = false;
        self.refresh_find_matches();
        Task::none()
    }

    pub(crate) fn handle_find_replace_message(
        &mut self,
        msg: crate::find_replace::FindReplaceMsg,
    ) -> Task<Message> {
        use crate::find_replace::FindReplaceMsg;

        match msg {
            FindReplaceMsg::Close => {
                self.ui_state.find_replace.open = false;
            }
            FindReplaceMsg::QueryChanged(query) => {
                self.ui_state.find_replace.query = query;
                self.refresh_find_matches();
            }
            FindReplaceMsg::ReplacementChanged(value) => {
                self.ui_state.find_replace.replacement = value;
            }
            FindReplaceMsg::SelectResult(index) => {
                self.ui_state.find_replace.selected_index = Some(index);
                if let Some(hit) = self.ui_state.find_replace.matches.get(index) {
                    self.interaction_state.active_canvas_mut().selected = vec![hit.item];
                    self.interaction_state
                        .active_canvas_mut()
                        .clear_overlay_cache();
                    self.update_selection_info();
                }
            }
            FindReplaceMsg::ReplaceCurrent => {
                if let Some(index) = self.ui_state.find_replace.selected_index
                    && let Some(hit) = self.ui_state.find_replace.matches.get(index).cloned()
                {
                    // Replace only the matched substring, not the whole
                    // field. Search is a case-insensitive `contains`, so
                    // the replace must be a case-insensitive substring
                    // replace over the hit's original text.
                    let query = self.ui_state.find_replace.query.trim().to_string();
                    let replacement = self.ui_state.find_replace.replacement.clone();
                    let value = replace_all_ci(&hit.text, &query, &replacement);
                    self.apply_engine_command(
                        signex_engine::Command::UpdateText {
                            target: hit.target,
                            value,
                        },
                        true,
                        true,
                    );
                    self.refresh_find_matches();
                }
            }
            FindReplaceMsg::ReplaceAll => {
                if !self.ui_state.find_replace.matches.is_empty() {
                    let query = self.ui_state.find_replace.query.trim().to_string();
                    let replacement = self.ui_state.find_replace.replacement.clone();
                    let commands: Vec<_> = self
                        .ui_state
                        .find_replace
                        .matches
                        .iter()
                        .map(|hit| signex_engine::Command::UpdateText {
                            target: hit.target,
                            value: replace_all_ci(&hit.text, &query, &replacement),
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
        let query = self.ui_state.find_replace.query.trim();
        if query.is_empty() {
            self.ui_state.find_replace.matches.clear();
            self.ui_state.find_replace.selected_index = None;
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

        self.ui_state.find_replace.matches = matches;
        self.ui_state.find_replace.selected_index = if self.ui_state.find_replace.matches.is_empty()
        {
            None
        } else {
            Some(
                self.ui_state
                    .find_replace
                    .selected_index
                    .unwrap_or(0)
                    .min(self.ui_state.find_replace.matches.len() - 1),
            )
        };

        if let Some(index) = self.ui_state.find_replace.selected_index
            && let Some(hit) = self.ui_state.find_replace.matches.get(index)
        {
            self.interaction_state.active_canvas_mut().selected = vec![hit.item];
            self.interaction_state
                .active_canvas_mut()
                .clear_overlay_cache();
            self.update_selection_info();
        }
    }
}

/// Replace every case-insensitive occurrence of `needle` in `haystack`
/// with `replacement`, preserving the rest of the string. Char-based so
/// multi-byte content keeps valid boundaries; ASCII case-folding matches
/// the `contains` used to find the hit. An empty needle is a no-op (the
/// find layer never produces an empty query).
fn replace_all_ci(haystack: &str, needle: &str, replacement: &str) -> String {
    if needle.is_empty() {
        return haystack.to_string();
    }
    let hay: Vec<char> = haystack.chars().collect();
    let ndl: Vec<char> = needle.chars().collect();
    let mut out = String::with_capacity(haystack.len());
    let mut i = 0;
    while i < hay.len() {
        if i + ndl.len() <= hay.len()
            && hay[i..i + ndl.len()]
                .iter()
                .zip(&ndl)
                .all(|(a, b)| a.eq_ignore_ascii_case(b))
        {
            out.push_str(replacement);
            i += ndl.len();
        } else {
            out.push(hay[i]);
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::replace_all_ci;

    #[test]
    fn replaces_only_the_matched_substring_not_the_whole_field() {
        // The regression: replacing "r" with "X" in "GND_RAIL" used to
        // wipe the whole label to "X"; it must yield "GND_XAIL".
        assert_eq!(replace_all_ci("GND_RAIL", "r", "X"), "GND_XAIL");
    }

    #[test]
    fn is_case_insensitive_and_replaces_all_occurrences() {
        assert_eq!(replace_all_ci("R1_r2_R3", "r", "net"), "net1_net2_net3");
    }

    #[test]
    fn leaves_text_unchanged_when_needle_absent_or_empty() {
        assert_eq!(replace_all_ci("VCC", "gnd", "X"), "VCC");
        assert_eq!(replace_all_ci("VCC", "", "X"), "VCC");
    }

    #[test]
    fn replacement_is_literal_not_a_pattern() {
        assert_eq!(replace_all_ci("A_B", "_", "$1"), "A$1B");
    }
}
