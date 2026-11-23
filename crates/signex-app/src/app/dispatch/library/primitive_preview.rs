//! The primitive picker's row preview (mockup row 131): a row click
//! highlights the row and reads its drawing off the UI thread; nothing is
//! bound until Pick.

use std::sync::Arc;

use super::*;
use crate::library::primitive_picker::read_drawing;
use crate::library::state::{PrimitiveChoice, PrimitiveDrawing};

impl Signex {
    /// Highlight `choice` and start reading its drawing. A second click on
    /// the highlighted row reads nothing.
    pub(super) fn handle_picker_row_highlighted(
        &mut self,
        choice: PrimitiveChoice,
    ) -> Task<Message> {
        let libraries = self.library.shared_adapters();
        let Some(picker) = self.library.primitive_picker.as_mut() else {
            return Task::none();
        };
        if picker.selection.highlighted.as_ref() == Some(&choice) {
            return Task::none();
        }
        let kind = picker.kind;
        let generation = picker.selection.highlight(choice.clone());
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || read_drawing(kind, &choice, &libraries))
                    .await
                    .unwrap_or_else(|e| Err(e.to_string()))
            },
            move |result| {
                Message::Library(LibraryMessage::PrimitivePicker(
                    PrimitivePickerMsg::PreviewLoaded {
                        generation,
                        result: result.map(|drawing| drawing.map(Arc::new)),
                    },
                ))
            },
        )
    }

    /// A preview read finished. A failure leaves the column saying the
    /// primitive is unreadable; the reason goes to the log.
    pub(super) fn handle_picker_preview_loaded(
        &mut self,
        generation: u64,
        result: Result<Option<Arc<PrimitiveDrawing>>, String>,
    ) -> Task<Message> {
        let Some(picker) = self.library.primitive_picker.as_mut() else {
            return Task::none();
        };
        let drawing = result.unwrap_or_else(|why| {
            tracing::warn!(target: "signex::library", "picker preview: {why}");
            None
        });
        picker.selection.finish(generation, drawing);
        Task::none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::state::{EditorAddress, PrimitivePickerState, PrimitivePickerTarget};

    fn app_with_picker() -> Signex {
        let (mut app, _boot) = Signex::new();
        app.library.primitive_picker = Some(PrimitivePickerState {
            kind: PrimitiveKind::Footprint,
            target: PrimitivePickerTarget::BrowserRow(EditorAddress::new(
                "l.snxlib".into(),
                "t".into(),
                signex_library::RowId::new(),
            )),
            filter: String::new(),
            error: None,
            scroll_y: 0.0,
            loose: Vec::new(),
            copy_confirm: None,
            copying: false,
            opening_library: None,
            selection: Default::default(),
        });
        app
    }

    /// Decided 2026-11-22: a row click previews and leaves the picker open.
    #[test]
    fn a_row_click_highlights_and_binds_nothing() {
        let mut app = app_with_picker();
        let choice = PrimitiveChoice::Pick(PrimitiveRef::new(
            uuid::Uuid::from_u128(1),
            uuid::Uuid::from_u128(2),
        ));
        let _task = app.update(Message::Library(LibraryMessage::PrimitivePicker(
            PrimitivePickerMsg::RowHighlighted(choice.clone()),
        )));
        let picker = app.library.primitive_picker.as_ref().expect("still open");
        assert_eq!(picker.selection.highlighted, Some(choice));
        assert!(picker.selection.loading);
    }
}
