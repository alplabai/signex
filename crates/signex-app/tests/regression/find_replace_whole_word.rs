//! #2525 — Find and Replace with Whole word on (the default) renames the
//! net `VOUT` and leaves the nets that only start with it alone.

use signex_app::app::{EditMsg, Message, Signex};
use signex_app::find_replace::FindReplaceMsg;
use signex_types::schematic::{Label, LabelType, Point, SchematicSheet};
use std::path::PathBuf;

/// A sheet holding the net labels `VOUT`, `VOUT_3V3` and `VOUT_SENSE`.
fn vout_app() -> Signex {
    let label = |x: f64, text: &str| Label::new(text, Point::new(x, 0.0), LabelType::Net);
    let sheet = SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        paper_size: "A4".to_string(),
        labels: vec![
            label(10.0, "VOUT"),
            label(20.0, "VOUT_3V3"),
            label(30.0, "VOUT_SENSE"),
        ],
        ..Default::default()
    };
    let path = PathBuf::from("find-replace-whole-word.snxsch");
    let (mut app, _boot) = Signex::new();
    app.document_state.engines.insert(
        path.clone(),
        signex_engine::Engine::new(sheet).expect("engine"),
    );
    app.document_state.tabs.push(signex_app::app::TabInfo {
        title: "find-replace-whole-word".to_string(),
        path: path.clone(),
        project_id: None,
        kind: signex_app::app::TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path);
    app
}

fn label_texts(app: &Signex) -> Vec<String> {
    app.document_state
        .active_engine()
        .expect("engine")
        .document()
        .labels
        .iter()
        .map(|l| l.text.clone())
        .collect()
}

#[test]
fn whole_word_replace_renames_only_the_exact_net() {
    let mut app = vout_app();
    assert!(
        app.ui_state.find_replace.options.whole_word,
        "Whole word is on by default"
    );
    for msg in [
        FindReplaceMsg::QueryChanged("VOUT".into()),
        FindReplaceMsg::ReplacementChanged("VOUT_3V3".into()),
    ] {
        let _ = app.update(Message::FindReplaceMsg(msg));
    }
    assert_eq!(app.ui_state.find_replace.matches.len(), 1, "VOUT alone");

    let _ = app.update(Message::FindReplaceMsg(FindReplaceMsg::ReplaceAll));

    assert_eq!(label_texts(&app), ["VOUT_3V3", "VOUT_3V3", "VOUT_SENSE"]);
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(
        label_texts(&app),
        ["VOUT", "VOUT_3V3", "VOUT_SENSE"],
        "one undo step"
    );
}
