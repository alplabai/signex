//! Edits of a footprint's generated features (mockup row 124): each is one
//! undo step that re-bakes the strokes. Text that is refused stays in its
//! field and changes nothing.

use signex_library::primitive::footprint::GeneratedFeatures;

use super::super::*;
use crate::app::FootprintDocument;
use crate::library::editor::footprint::{PAD_CLEARANCE_BUFFER, SILK_LINE_WIDTH_BUFFER};
use crate::panels::FootprintFeatureEdit;

/// The history key of a typed field, as `FootprintDocument::record_edit` takes it.
type FieldKey = (&'static str, u128);

impl Signex {
    pub(in crate::app::handlers::dock::sch_library) fn handle_fp_editor_generated_feature(
        &mut self,
        edit: &FootprintFeatureEdit,
    ) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            apply(editor, edit);
        }
        self.refresh_panel_ctx();
        true
    }
}

fn apply(editor: &mut FootprintDocument, edit: &FootprintFeatureEdit) {
    match edit {
        FootprintFeatureEdit::SilkOutlineToggled => {
            change(editor, None, |g| g.silk_outline = !g.silk_outline);
        }
        FootprintFeatureEdit::PinOneMarkerToggled => {
            change(editor, None, |g| g.pin_one_marker = !g.pin_one_marker);
        }
        FootprintFeatureEdit::AssemblyOutlineToggled => {
            change(editor, None, |g| g.assembly_outline = !g.assembly_outline);
        }
        FootprintFeatureEdit::SilkLineWidthTyped(text) => {
            let key = ("generated_silk_line_width", 0);
            typed(
                editor,
                SILK_LINE_WIDTH_BUFFER,
                key,
                text,
                |v| v > 0.0,
                |g, v| {
                    g.silk_line_width_mm = v;
                },
            );
        }
        FootprintFeatureEdit::PadClearanceTyped(text) => {
            let key = ("generated_pad_clearance", 0);
            typed(
                editor,
                PAD_CLEARANCE_BUFFER,
                key,
                text,
                |v| v >= 0.0,
                |g, v| {
                    g.pad_clearance_mm = v;
                },
            );
        }
    }
}

/// Keep `text` in its field, and store its value when `fits` takes it.
fn typed(
    editor: &mut FootprintDocument,
    buffer: &str,
    key: FieldKey,
    text: &str,
    fits: fn(f64) -> bool,
    set: fn(&mut GeneratedFeatures, f64),
) {
    editor
        .state
        .numeric_buffers
        .insert(buffer.to_owned(), text.to_owned());
    if let Some(value) = crate::numeric_field::parse_field_f64(text).filter(|v| fits(*v)) {
        change(editor, Some(key), |g| set(g, value));
    }
}

/// One undo step: the new settings and the strokes baked from them. A
/// change that leaves the settings as they were records nothing.
fn change(
    editor: &mut FootprintDocument,
    key: Option<FieldKey>,
    set: impl FnOnce(&mut GeneratedFeatures),
) {
    let mut next = editor.primitive().generated.clone();
    set(&mut next);
    if next == editor.primitive().generated {
        return;
    }
    editor.record_edit(key, |editor| {
        let footprint = editor.primitive_mut();
        footprint.generated = next;
        signex_bake::bake_generated(footprint);
        editor.mark_dirty();
        editor.canvas_cache.clear();
        true
    });
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use signex_library::primitive::footprint::Polygon;
    use signex_library::{Footprint, FootprintFile, Pad};

    use crate::app::{FootprintDocument, Signex, TabInfo, TabKind};
    use crate::library::editor::footprint::SILK_LINE_WIDTH_BUFFER;
    use crate::panels::FootprintFeatureEdit as Edit;

    fn app_on_a_part(path: &Path) -> Signex {
        let mut footprint = Footprint::empty("part");
        footprint.pads = vec![Pad {
            number: "1".into(),
            size: [1.0, 0.6],
            position: [-2.0, 0.0],
            ..Pad::default()
        }];
        footprint.body_3d.outline = Some(Polygon::new(vec![[-2.0, -1.0], [2.0, 1.0]]));
        let file = FootprintFile::from_footprint(footprint);
        let (mut app, _task) = Signex::new();
        app.document_state.footprint_editors.insert(
            path.to_path_buf(),
            FootprintDocument::new(path.to_path_buf(), file),
        );
        app.document_state.tabs.push(TabInfo {
            title: "part".into(),
            path: path.to_path_buf(),
            cached_document: None,
            project_id: None,
            kind: TabKind::FootprintEditor(path.to_path_buf()),
        });
        app.document_state.active_tab = app.document_state.tabs.len() - 1;
        app
    }

    #[test]
    fn a_switch_is_one_undo_step_that_brings_the_strokes_and_takes_them_back() {
        let path = PathBuf::from("generated.snxfpt");
        let mut app = app_on_a_part(&path);
        app.handle_fp_editor_generated_feature(&Edit::AssemblyOutlineToggled);
        let editor = app.document_state.footprint_editors.get_mut(&path).unwrap();
        assert_eq!(editor.primitive().generated.assembly.len(), 4);
        assert_eq!(editor.history.undo_len(), 1);
        assert!(editor.undo());
        assert!(editor.primitive().generated.is_default());
        assert!(editor.redo());
        assert_eq!(editor.primitive().generated.assembly.len(), 4);
    }

    #[test]
    fn refused_text_stays_in_the_field_and_records_nothing() {
        let path = PathBuf::from("generated-refused.snxfpt");
        let mut app = app_on_a_part(&path);
        for text in ["abc", "0", "-1"] {
            app.handle_fp_editor_generated_feature(&Edit::SilkLineWidthTyped(text.into()));
            let editor = &app.document_state.footprint_editors[&path];
            assert!(editor.primitive().generated.is_default(), "{text}");
            assert_eq!(editor.history.undo_len(), 0, "{text}");
            assert_eq!(
                editor
                    .state
                    .numeric_buffers
                    .get(SILK_LINE_WIDTH_BUFFER)
                    .map(String::as_str),
                Some(text)
            );
        }
    }

    #[test]
    fn a_zero_clearance_is_taken_and_a_negative_one_is_not() {
        let path = PathBuf::from("generated-clearance.snxfpt");
        let mut app = app_on_a_part(&path);
        app.handle_fp_editor_generated_feature(&Edit::PadClearanceTyped("0".into()));
        app.handle_fp_editor_generated_feature(&Edit::PadClearanceTyped("-0.1".into()));
        let editor = &app.document_state.footprint_editors[&path];
        assert_eq!(editor.primitive().generated.pad_clearance_mm, 0.0);
    }

    #[test]
    fn a_pad_write_back_rebakes_the_strokes() {
        use crate::library::editor::footprint::FootprintEditorState;
        let path = PathBuf::from("generated-pad.snxfpt");
        let mut app = app_on_a_part(&path);
        app.handle_fp_editor_generated_feature(&Edit::SilkOutlineToggled);
        let editor = app.document_state.footprint_editors.get_mut(&path).unwrap();
        let before = editor.primitive().generated.silk.clone();
        editor.state.move_pad(0, 2.0, 0.0);
        let (state, footprint) = editor.parts_mut();
        FootprintEditorState::sync_pads_to_primitive(state, footprint);
        assert_ne!(footprint.generated.silk, before);
    }
}
