//! The pattern Numbering Mask (mockup row 126): the pad numbers a Grid or
//! Polar pattern skips. The listed numbers become the pattern's suppressed
//! instances in one undo step; text naming a number the pattern does not
//! have stays in the field and changes nothing.

use signex_bake::array::{ArrayPreview, instances_numbered, preview_array};
use signex_sketch::array::{Array, ArrayId, ArrayKind};
use signex_sketch::entity::EntityKind;
use signex_sketch::sketch::SketchData;

use super::super::*;
use super::shape::rebuild_sketch;
use crate::panels::{PATTERN_PREVIEW_MAX_INSTANCES, number_mask_buffer_key};

/// The instances of `array` with their numbers, as the pattern form shows
/// them.
fn pattern_preview(sketch: &SketchData, array: &Array) -> ArrayPreview {
    let point = |id| {
        sketch
            .entities
            .iter()
            .find(|e| e.id == id)
            .and_then(|e| match e.kind {
                EntityKind::Point { x, y } => Some([x, y]),
                _ => None,
            })
    };
    let center = match &array.kind {
        ArrayKind::Polar { center, .. } => point(*center),
        _ => None,
    };
    preview_array(
        array,
        sketch,
        point(array.kind.source()),
        center,
        PATTERN_PREVIEW_MAX_INSTANCES,
    )
}

impl Signex {
    pub(crate) fn fp_editor_set_array_number_mask(
        &mut self,
        array_id: ArrayId,
        value: String,
    ) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor
                .state
                .numeric_buffers
                .insert(number_mask_buffer_key(array_id), value.clone());
            let cells = editor.primitive().sketch.as_ref().and_then(|sketch| {
                let array = sketch.arrays.iter().find(|a| a.id == array_id)?;
                instances_numbered(&pattern_preview(sketch, array), &value).ok()
            });
            if let Some(cells) = cells {
                // Keyed, so typing a mask out is one undo step.
                let key = Some(("array_number_mask", array_id.0.as_u128()));
                editor.record_edit(key, |editor| {
                    let Some(array) = super::shape::sketch_array_mut(editor, array_id) else {
                        return false;
                    };
                    if !array.kind.set_suppressed_instances(cells) {
                        return false;
                    }
                    rebuild_sketch(editor);
                    true
                });
            }
        }
        self.refresh_panel_ctx();
        true
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use signex_library::{Footprint, FootprintFile};
    use signex_sketch::array::{Array, ArrayId, ArrayKind, NumberingScheme};
    use signex_sketch::attr::PadAttr;
    use signex_sketch::entity::{Entity, EntityKind};
    use signex_sketch::id::SketchEntityId;
    use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
    use signex_sketch::sketch::SketchData;

    use crate::app::{FootprintDocument, Signex, TabInfo, TabKind};
    use crate::panels::number_mask_buffer_key;

    /// An app on a footprint whose sketch has a 2 × 4 grid of pad 1,
    /// numbered 1…8 row by row.
    fn app_on_a_grid(path: &Path) -> (Signex, ArrayId) {
        let path = path.to_path_buf();
        let plane = PlaneId::new();
        let mut sketch = SketchData {
            planes: vec![Plane {
                id: plane,
                kind: PlaneKind::BoardTop,
            }],
            ..SketchData::default()
        };
        let source = SketchEntityId::new();
        let mut point = Entity::new(source, plane, EntityKind::Point { x: 0.0, y: 0.0 });
        point.pad = Some(PadAttr {
            number: "1".into(),
            size_x_expr: "0.5mm".into(),
            size_y_expr: "0.5mm".into(),
            ..PadAttr::default()
        });
        sketch.entities.push(point);
        let array = Array {
            id: ArrayId::new(),
            kind: ArrayKind::Grid {
                source,
                nx_expr: "2".into(),
                ny_expr: "4".into(),
                dx_expr: "1mm".into(),
                dy_expr: "1mm".into(),
                depopulation: None,
            },
            numbering: NumberingScheme::default(),
        };
        let array_id = array.id;
        sketch.arrays.push(array);
        let mut footprint = Footprint::empty("grid");
        footprint.sketch = Some(sketch);
        let file = FootprintFile::from_footprint(footprint);
        let (mut app, _task) = Signex::new();
        app.document_state
            .footprint_editors
            .insert(path.clone(), FootprintDocument::new(path.clone(), file));
        app.document_state.tabs.push(TabInfo {
            title: "grid".into(),
            path: path.clone(),
            project_id: None,
            kind: TabKind::FootprintEditor(path),
        });
        app.document_state.active_tab = app.document_state.tabs.len() - 1;
        (app, array_id)
    }

    fn suppressed(app: &Signex, path: &Path, array_id: ArrayId) -> Vec<(u32, u32)> {
        let editor = &app.document_state.footprint_editors[path];
        let sketch = editor.primitive().sketch.as_ref().expect("a sketch");
        let array = sketch.arrays.iter().find(|a| a.id == array_id).unwrap();
        match &array.kind {
            ArrayKind::Grid { depopulation, .. } => depopulation
                .as_ref()
                .map(|d| d.suppressed_instances.clone())
                .unwrap_or_default(),
            _ => unreachable!("a grid"),
        }
    }

    /// Mockup row 126: "4,5" skips pads 4 and 5 (row by row on a 2-wide grid:
    /// cells (1, 1) and (0, 2)) in one undo step.
    #[test]
    fn a_number_mask_skips_the_listed_pads_in_one_undo_step() {
        let path = PathBuf::from("number-mask.snxfpt");
        let (mut app, array_id) = app_on_a_grid(&path);
        app.fp_editor_set_array_number_mask(array_id, "4,5".into());
        assert_eq!(suppressed(&app, &path, array_id), vec![(1, 1), (0, 2)]);
        let editor = app.document_state.footprint_editors.get_mut(&path).unwrap();
        assert_eq!(editor.history.undo_len(), 1);
        assert!(editor.undo());
        assert!(suppressed(&app, &path, array_id).is_empty());
    }

    #[test]
    fn a_number_the_pattern_lacks_stays_in_the_field_and_changes_nothing() {
        let path = PathBuf::from("number-mask-refused.snxfpt");
        let (mut app, array_id) = app_on_a_grid(&path);
        app.fp_editor_set_array_number_mask(array_id, "4,B4".into());
        assert!(suppressed(&app, &path, array_id).is_empty());
        let editor = &app.document_state.footprint_editors[&path];
        assert_eq!(editor.history.undo_len(), 0);
        assert_eq!(
            editor
                .state
                .numeric_buffers
                .get(&number_mask_buffer_key(array_id))
                .map(String::as_str),
            Some("4,B4")
        );
    }

    /// The source pad's number ("1") is refused: slot (0, 0) is never
    /// written, so the source pad stays baked and drawn.
    #[test]
    fn the_source_pads_number_changes_nothing() {
        let path = PathBuf::from("number-mask-source.snxfpt");
        let (mut app, array_id) = app_on_a_grid(&path);
        app.fp_editor_set_array_number_mask(array_id, "1".into());
        assert!(suppressed(&app, &path, array_id).is_empty());
        app.fp_editor_set_array_number_mask(array_id, "4,1".into());
        assert!(suppressed(&app, &path, array_id).is_empty());
        let editor = &app.document_state.footprint_editors[&path];
        assert_eq!(editor.history.undo_len(), 0);
    }

    /// The field's hint never suggests a number the field refuses: not "1",
    /// a linear pattern's source pad, nor "A1", a BGA's.
    #[test]
    fn the_hint_never_names_a_source_pad() {
        let (app, _) = app_on_a_grid(Path::new("number-mask-hint.snxfpt"));
        let hint = signex_i18n::t!(
            app.ui_state.i18n,
            signex_i18n::ids::EDITORS_ROLE_PATTERN_NUMBER_MASK_HINT
        )
        .into_string();
        let examples = hint.split_once("e.g.").map_or(hint.as_str(), |(_, e)| e);
        let numbers: Vec<&str> = examples
            .split([',', ' '])
            .map(str::trim)
            .filter(|n| !n.is_empty() && *n != "or")
            .collect();
        assert!(!numbers.is_empty(), "{hint}");
        for source in ["1", "A1"] {
            assert!(!numbers.contains(&source), "{source} in {hint}");
        }
    }

    #[test]
    fn clearing_the_mask_brings_the_pads_back() {
        let path = PathBuf::from("number-mask-clear.snxfpt");
        let (mut app, array_id) = app_on_a_grid(&path);
        app.fp_editor_set_array_number_mask(array_id, "8".into());
        assert_eq!(suppressed(&app, &path, array_id), vec![(1, 3)]);
        app.fp_editor_set_array_number_mask(array_id, String::new());
        assert!(suppressed(&app, &path, array_id).is_empty());
    }
}
