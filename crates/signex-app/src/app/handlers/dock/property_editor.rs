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
                    true,
                    true,
                );
            }
            crate::panels::PanelMsg::EditSymbolValue(uuid, new_value) => {
                self.apply_engine_command(
                    signex_engine::Command::UpdateText {
                        target: signex_engine::TextTarget::SymbolValue(*uuid),
                        value: new_value.clone(),
                    },
                    true,
                    true,
                );
            }
            crate::panels::PanelMsg::EditSymbolFootprint(uuid, new_value) => {
                self.apply_engine_command(
                    signex_engine::Command::UpdateSymbolFootprint {
                        symbol_id: *uuid,
                        footprint: new_value.clone(),
                    },
                    true,
                    true,
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
            crate::panels::PanelMsg::EditSymbolRotation(uuid, deg) => {
                self.apply_engine_command(
                    signex_engine::Command::SetSymbolRotation {
                        symbol_id: *uuid,
                        rotation_degrees: *deg,
                    },
                    true,
                    true,
                );
            }
            crate::panels::PanelMsg::EditPowerPortStyle {
                symbol_id,
                new_lib_id,
                rotation_degrees,
            } => {
                self.apply_engine_command(
                    signex_engine::Command::UpdateSymbolLibId {
                        symbol_id: *symbol_id,
                        lib_id: new_lib_id.clone(),
                    },
                    true,
                    false,
                );
                self.apply_engine_command(
                    signex_engine::Command::SetSymbolRotation {
                        symbol_id: *symbol_id,
                        rotation_degrees: *rotation_degrees,
                    },
                    true,
                    true,
                );
            }
            crate::panels::PanelMsg::EditSymbolLibId(uuid, new_lib) => {
                self.apply_engine_command(
                    signex_engine::Command::UpdateSymbolLibId {
                        symbol_id: *uuid,
                        lib_id: new_lib.clone(),
                    },
                    true,
                    true,
                );
            }
            crate::panels::PanelMsg::EditSymbolValueFontSizePt(uuid, pt) => {
                self.apply_engine_command(
                    signex_engine::Command::UpdateSymbolTextSize {
                        symbol_id: *uuid,
                        field: signex_engine::SymbolTextField::Value,
                        font_size_mm: (*pt as f64) * 0.18,
                    },
                    true,
                    true,
                );
            }
            crate::panels::PanelMsg::EditLabelText(uuid, new_text) => {
                // Users type `/` in the Properties panel; persist the KiCad
                // escape token so the stored schematic round-trips cleanly.
                let stored =
                    signex_render::schematic::text::escape_for_kicad(new_text);
                self.apply_engine_command(
                    signex_engine::Command::UpdateText {
                        target: signex_engine::TextTarget::Label(*uuid),
                        value: stored,
                    },
                    true,
                    true,
                );
            }
            crate::panels::PanelMsg::EditTextNoteText(uuid, new_text) => {
                let stored =
                    signex_render::schematic::text::escape_for_kicad(new_text);
                self.apply_engine_command(
                    signex_engine::Command::UpdateText {
                        target: signex_engine::TextTarget::TextNote(*uuid),
                        value: stored,
                    },
                    true,
                    true,
                );
            }
            crate::panels::PanelMsg::EditLabelJustifyH(uuid, h) => {
                self.apply_engine_command(
                    signex_engine::Command::UpdateLabelProps {
                        label_id: *uuid,
                        font_size_mm: None,
                        justify: Some(*h),
                        rotation_degrees: None,
                    },
                    true,
                    true,
                );
            }
            crate::panels::PanelMsg::EditLabelRotation(uuid, deg) => {
                self.apply_engine_command(
                    signex_engine::Command::UpdateLabelProps {
                        label_id: *uuid,
                        font_size_mm: None,
                        justify: None,
                        rotation_degrees: Some(*deg),
                    },
                    true,
                    true,
                );
            }
            crate::panels::PanelMsg::EditLabelFontSizePt(uuid, pt) => {
                let mm = (*pt as f64) * 0.18;
                self.apply_engine_command(
                    signex_engine::Command::UpdateLabelProps {
                        label_id: *uuid,
                        font_size_mm: Some(mm),
                        justify: None,
                        rotation_degrees: None,
                    },
                    true,
                    true,
                );
            }
            _ => return false,
        }

        true
    }
}