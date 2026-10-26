//! #1138 — pasting a symbol must not carry the original's per-placement
//! instance entries. Since #979 those entries decide the designator at each
//! placement (`Symbol::designator_in`), so a copy that kept them printed and
//! netlisted the original's designators whatever its own `reference` said.

use signex_app::app::{EditMsg, Message, Signex};
use signex_types::schematic::{Point, SchematicSheet, Symbol, SymbolInstance};
use std::path::PathBuf;

fn entry(path: &str, reference: &str) -> SymbolInstance {
    SymbolInstance {
        project: String::new(),
        path: path.to_string(),
        reference: reference.to_string(),
        unit: 1,
    }
}

#[test]
fn a_pasted_symbol_does_not_inherit_the_originals_instance_entries() {
    let path = PathBuf::from("paste-instances.snxsch");
    let engine = signex_engine::Engine::new(SchematicSheet {
        uuid: uuid::Uuid::new_v4(),
        paper_size: "A4".to_string(),
        ..Default::default()
    })
    .expect("engine");
    let (mut app, _initial_task) = Signex::new();
    app.document_state.engines.insert(path.clone(), engine);
    app.document_state.tabs.push(signex_app::app::TabInfo {
        title: "paste-instances".to_string(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: signex_app::app::TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path);

    app.interaction_state.clipboard_symbols = vec![Symbol {
        uuid: uuid::Uuid::new_v4(),
        lib_id: "Device:R".to_string(),
        reference: "R2".to_string(),
        value: "10k".to_string(),
        position: Point::new(0.0, 0.0),
        instances: vec![entry("/A", "R1"), entry("/B", "R101")],
        ..Default::default()
    }];

    let _ = app.update(Message::Edit(EditMsg::Paste));

    let doc = app
        .document_state
        .active_engine()
        .expect("engine")
        .document();
    assert_eq!(doc.symbols.len(), 1, "the paste must place the symbol");
    let pasted = &doc.symbols[0];
    assert!(
        pasted.instances.is_empty(),
        "a pasted copy is a new part and must not keep the original's entries"
    );
    assert_eq!(pasted.reference, "R2");
}
