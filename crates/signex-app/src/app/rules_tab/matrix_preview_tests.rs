//! #2530 - the matrix preview: the hovered or pinned pair is drawn, and a
//! value typed on the drawing writes the pinned cell as one undo step.

use signex_drc_dsl::table::matrix::CellKey;
use signex_widgets::dimension_label::DimensionLabelMessage as L;

use super::RulesTabMsg;
use super::drawing::{PreviewMsg, PreviewTarget};
use super::matrix::Shown;
use super::matrix_tests::{app_with_board, send, show_matrix, shown_at, undo_steps};
use crate::app::Signex;
use crate::app::state::ProjectId;

fn target(app: &Signex, id: ProjectId) -> Option<PreviewTarget> {
    app.document_state.project_tabs[&id]
        .rules
        .drawing_target
        .clone()
}

#[test]
fn a_hovered_class_cell_draws_its_class_pair() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = app_with_board(&dir);
    show_matrix(&mut app, id);
    assert_eq!(target(&app, id), None, "nothing hovered or pinned");
    send(
        &mut app,
        RulesTabMsg::MatrixHovered {
            project_id: id,
            key: Some(CellKey::new("LV", "HV")),
        },
    );
    // The empty cell draws at its fallback, the board-wide 0.2 mm.
    assert_eq!(
        target(&app, id),
        Some(PreviewTarget::ClassPair {
            a: "LV".into(),
            b: "HV".into(),
            gap_mm: 0.2,
        })
    );
    let _ = app.view_rules_tab(id);
}

#[test]
fn a_pinned_object_pair_row_draws_its_two_objects() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = app_with_board(&dir);
    show_matrix(&mut app, id);
    let pair = CellKey::new("LV", "HV").with_pair("track-via").unwrap();
    send(
        &mut app,
        RulesTabMsg::MatrixPairPinned {
            project_id: id,
            key: pair,
        },
    );
    assert_eq!(
        target(&app, id),
        Some(PreviewTarget::ObjectPair {
            objects: ["track".into(), "via".into()],
            gap_mm: 0.2,
        })
    );
    let _ = app.view_rules_tab(id);
}

#[test]
fn a_value_typed_on_the_drawing_writes_the_pinned_cell_as_one_undo_step() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = app_with_board(&dir);
    show_matrix(&mut app, id);
    let key = CellKey::new("LV", "HV");
    send(
        &mut app,
        RulesTabMsg::MatrixPairPinned {
            project_id: id,
            key: key.clone(),
        },
    );
    let _ = app.view_rules_tab(id);
    let before = undo_steps(&app, id);
    // The gap is the drawing's one label.
    for step in [
        L::LabelPressed(0),
        L::DraftEdited("0.65".into()),
        L::DraftSubmitted,
    ] {
        send(
            &mut app,
            RulesTabMsg::PreviewActed {
                project_id: id,
                msg: PreviewMsg::Labels(step),
            },
        );
    }
    assert_eq!(shown_at(&app, id, &key), Some(Shown::Written(0.65)));
    assert_eq!(undo_steps(&app, id), before + 1);
    assert_eq!(
        target(&app, id),
        Some(PreviewTarget::ClassPair {
            a: "LV".into(),
            b: "HV".into(),
            gap_mm: 0.65,
        }),
        "the drawing redraws at the written value"
    );
    let _ = app.view_rules_tab(id);
}

/// Mockup row 90: the hovered cell's drawing names its two tracks over its
/// classes (column class first), reads its gap as "≥ 0.2 mm" and sets a
/// scale bar beside it.
#[test]
fn a_hovered_cell_is_drawn_with_its_title_gap_and_scale() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = app_with_board(&dir);
    show_matrix(&mut app, id);
    send(
        &mut app,
        RulesTabMsg::MatrixHovered {
            project_id: id,
            key: Some(CellKey::new("LV", "HV")),
        },
    );
    let drawn = target(&app, id).expect("the hovered cell is drawn");
    let texts = super::drawing::drawn_texts(&drawn, signex_i18n::I18n::en_us_ref());
    for want in [
        "Track ↔ Track",
        "HV ↔ LV · different nets",
        "≥ 0.2 mm",
        "0.5 mm",
    ] {
        assert!(texts.iter().any(|t| t == want), "{want}: {texts:?}");
    }
    let _ = app.view_rules_tab(id);
}
