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
                        font_size_mm: (*pt as f64) * signex_types::schematic::SCHEMATIC_PT_TO_MM,
                    },
                    true,
                    true,
                );
            }
            crate::panels::PanelMsg::EditLabelText(uuid, new_text) => {
                // Users type `/` in the Properties panel; persist the Standard
                // escape token so the stored schematic round-trips cleanly.
                let stored = signex_render::schematic::text::escape_for_standard(new_text);
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
                let stored = signex_render::schematic::text::escape_for_standard(new_text);
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
            crate::panels::PanelMsg::EditLabelDirection(uuid, deg, h) => {
                self.apply_engine_command(
                    signex_engine::Command::UpdateLabelProps {
                        label_id: *uuid,
                        font_size_mm: None,
                        justify: Some(*h),
                        rotation_degrees: Some(*deg),
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
                let mm = (*pt as f64) * signex_types::schematic::SCHEMATIC_PT_TO_MM;
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
            crate::panels::PanelMsg::ToggleChildSheetBorderPicker(_) => {
                let was_open = self.document_state.panel_ctx.child_sheet_border_picker_open;
                self.document_state.panel_ctx.child_sheet_border_picker_open = !was_open;
                self.document_state.panel_ctx.child_sheet_fill_picker_open = false;
                self.document_state
                    .panel_ctx
                    .child_sheet_border_advanced_open = false;
                self.document_state.panel_ctx.child_sheet_fill_advanced_open = false;
            }
            crate::panels::PanelMsg::ToggleChildSheetFillPicker(_) => {
                let was_open = self.document_state.panel_ctx.child_sheet_fill_picker_open;
                self.document_state.panel_ctx.child_sheet_fill_picker_open = !was_open;
                self.document_state.panel_ctx.child_sheet_border_picker_open = false;
                self.document_state
                    .panel_ctx
                    .child_sheet_border_advanced_open = false;
                self.document_state.panel_ctx.child_sheet_fill_advanced_open = false;
            }
            crate::panels::PanelMsg::OpenChildSheetAdvancedPicker(_uuid, is_border) => {
                if *is_border {
                    self.document_state
                        .panel_ctx
                        .child_sheet_border_advanced_open = true;
                    self.document_state.panel_ctx.child_sheet_fill_advanced_open = false;
                } else {
                    self.document_state.panel_ctx.child_sheet_fill_advanced_open = true;
                    self.document_state
                        .panel_ctx
                        .child_sheet_border_advanced_open = false;
                }
            }
            crate::panels::PanelMsg::CancelChildSheetColorPicker => {
                self.document_state.panel_ctx.child_sheet_border_picker_open = false;
                self.document_state.panel_ctx.child_sheet_fill_picker_open = false;
                self.document_state
                    .panel_ctx
                    .child_sheet_border_advanced_open = false;
                self.document_state.panel_ctx.child_sheet_fill_advanced_open = false;
            }
            crate::panels::PanelMsg::EditChildSheetBorderColor(uuid, color) => {
                let stroke = iced_color_to_stroke(*color);
                self.document_state.panel_ctx.child_sheet_border_picker_open = false;
                self.document_state
                    .panel_ctx
                    .child_sheet_border_advanced_open = false;
                self.apply_engine_command(
                    signex_engine::Command::UpdateChildSheetStyle {
                        sheet_id: *uuid,
                        stroke_width: None,
                        stroke_color: Some(Some(stroke)),
                        fill_color: None,
                    },
                    true,
                    true,
                );
            }
            crate::panels::PanelMsg::EditChildSheetFillColor(uuid, color) => {
                let stroke = iced_color_to_stroke(*color);
                self.document_state.panel_ctx.child_sheet_fill_picker_open = false;
                self.document_state.panel_ctx.child_sheet_fill_advanced_open = false;
                self.apply_engine_command(
                    signex_engine::Command::UpdateChildSheetStyle {
                        sheet_id: *uuid,
                        stroke_width: None,
                        stroke_color: None,
                        fill_color: Some(Some(stroke)),
                    },
                    true,
                    true,
                );
            }
            crate::panels::PanelMsg::ChildSheetStrokeWidthTyping(_uuid, value) => {
                self.document_state.panel_ctx.child_sheet_stroke_width_buf = Some(value.clone());
            }
            crate::panels::PanelMsg::CommitChildSheetStrokeWidth(uuid) => {
                let parsed = self
                    .document_state
                    .panel_ctx
                    .child_sheet_stroke_width_buf
                    .as_ref()
                    .and_then(|s| s.trim().parse::<f64>().ok());
                self.document_state.panel_ctx.child_sheet_stroke_width_buf = None;
                if let Some(width) = parsed
                    && width >= 0.0
                {
                    self.apply_engine_command(
                        signex_engine::Command::UpdateChildSheetStyle {
                            sheet_id: *uuid,
                            stroke_width: Some(width),
                            stroke_color: None,
                            fill_color: None,
                        },
                        true,
                        true,
                    );
                }
            }
            crate::panels::PanelMsg::ResetChildSheetStyle(uuid) => {
                self.document_state.panel_ctx.child_sheet_border_picker_open = false;
                self.document_state.panel_ctx.child_sheet_fill_picker_open = false;
                self.document_state
                    .panel_ctx
                    .child_sheet_border_advanced_open = false;
                self.document_state.panel_ctx.child_sheet_fill_advanced_open = false;
                self.document_state.panel_ctx.child_sheet_stroke_width_buf = None;
                self.apply_engine_command(
                    signex_engine::Command::UpdateChildSheetStyle {
                        sheet_id: *uuid,
                        stroke_width: Some(0.0),
                        stroke_color: Some(None),
                        fill_color: Some(None),
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

fn iced_color_to_stroke(c: iced::Color) -> signex_types::schematic::StrokeColor {
    signex_types::schematic::StrokeColor {
        r: (c.r.clamp(0.0, 1.0) * 255.0).round() as u8,
        g: (c.g.clamp(0.0, 1.0) * 255.0).round() as u8,
        b: (c.b.clamp(0.0, 1.0) * 255.0).round() as u8,
        a: (c.a.clamp(0.0, 1.0) * 255.0).round() as u8,
    }
}
