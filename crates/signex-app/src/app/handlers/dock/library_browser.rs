use super::super::super::*;

impl Signex {
    pub(super) fn handle_dock_library_browser_message(
        &mut self,
        panel_msg: &crate::panels::PanelMsg,
    ) -> bool {
        match panel_msg {
            // F15 — Properties-panel Pick buttons. Same effect as
            // dispatching `LibraryMessage::OpenPrimitivePicker` with a
            // BrowserRow target — but inlined here because the panel
            // dispatcher returns `bool` (no Task<Message>) and we
            // already have `&mut self` in scope to mutate
            // `self.library.primitive_picker` directly.
            crate::panels::PanelMsg::LibraryRowPickSymbol => {
                self.open_library_row_primitive_picker(signex_library::PrimitiveKind::Symbol);
                true
            }
            crate::panels::PanelMsg::LibraryRowPickFootprint => {
                self.open_library_row_primitive_picker(signex_library::PrimitiveKind::Footprint);
                true
            }
            _ => false,
        }
    }

    /// Open the primitive picker for the row described by the active
    /// `panel_ctx.library_row_detail`. Wired by the Properties-panel
    /// Pick Symbol / Pick Footprint buttons.
    fn open_library_row_primitive_picker(&mut self, kind: signex_library::PrimitiveKind) {
        let Some(detail) = self.document_state.panel_ctx.library_row_detail.clone() else {
            return;
        };
        if self.refuse_read_only_write(&detail.library_path) {
            return;
        }
        let address = crate::library::state::EditorAddress::new(
            detail.library_path,
            detail.table,
            signex_library::RowId::from_uuid(detail.row_id),
        );
        self.library.primitive_picker = Some(crate::library::state::PrimitivePickerState {
            kind,
            target: crate::library::state::PrimitivePickerTarget::BrowserRow(address),
            filter: String::new(),
            error: None,
            scroll_y: 0.0,
        });
    }
}
