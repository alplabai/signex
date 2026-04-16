use super::super::super::*;

impl Signex {
    pub(super) fn handle_dock_property_editor_message(
        &mut self,
        panel_msg: &crate::panels::PanelMsg,
    ) -> bool {
        match panel_msg {
            crate::panels::PanelMsg::EditSymbolDesignator(uuid, new_value) => {
                self.apply_engine_command(
                    signex_engine::Command::UpdateText {
                        target: signex_engine::TextTarget::SymbolReference(*uuid),
                        value: new_value.clone(),
                    },
                    false,
                    false,
                );
            }
            crate::panels::PanelMsg::EditSymbolValue(uuid, new_value) => {
                self.apply_engine_command(
                    signex_engine::Command::UpdateText {
                        target: signex_engine::TextTarget::SymbolValue(*uuid),
                        value: new_value.clone(),
                    },
                    false,
                    false,
                );
            }
            crate::panels::PanelMsg::EditSymbolFootprint(uuid, new_value) => {
                self.apply_engine_command(
                    signex_engine::Command::UpdateSymbolFootprint {
                        symbol_id: *uuid,
                        footprint: new_value.clone(),
                    },
                    false,
                    false,
                );
            }
            crate::panels::PanelMsg::ToggleSymbolMirrorX(uuid) => {
                self.apply_engine_command(
                    signex_engine::Command::MirrorSelection {
                        items: vec![signex_types::schematic::SelectedItem::new(
                            *uuid,
                            signex_types::schematic::SelectedKind::Symbol,
                        )],
                        axis: signex_engine::MirrorAxis::Vertical,
                    },
                    true,
                    true,
                );
            }
            crate::panels::PanelMsg::ToggleSymbolMirrorY(uuid) => {
                self.apply_engine_command(
                    signex_engine::Command::MirrorSelection {
                        items: vec![signex_types::schematic::SelectedItem::new(
                            *uuid,
                            signex_types::schematic::SelectedKind::Symbol,
                        )],
                        axis: signex_engine::MirrorAxis::Horizontal,
                    },
                    true,
                    true,
                );
            }
            crate::panels::PanelMsg::ToggleSymbolLocked(uuid)
            | crate::panels::PanelMsg::ToggleSymbolDnp(uuid) => {
                let _ = uuid;
            }
            crate::panels::PanelMsg::EditLabelText(uuid, new_text) => {
                self.apply_engine_command(
                    signex_engine::Command::UpdateText {
                        target: signex_engine::TextTarget::Label(*uuid),
                        value: new_text.clone(),
                    },
                    false,
                    false,
                );
            }
            crate::panels::PanelMsg::EditTextNoteText(uuid, new_text) => {
                self.apply_engine_command(
                    signex_engine::Command::UpdateText {
                        target: signex_engine::TextTarget::TextNote(*uuid),
                        value: new_text.clone(),
                    },
                    false,
                    false,
                );
            }
            _ => return false,
        }

        true
    }
}