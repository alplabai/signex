//! #112: the laid-out BOM dialog reads as drawn: one variant control, the
//! one not-fitted option, an issue chip, a footer without the variant, and
//! Cancel beside the format-named Export.

use std::path::PathBuf;

use signex_types::schematic::{SchematicSheet, Symbol};

use crate::app::Signex;
use crate::layout_probe::lay_out_texts;

fn symbol(reference: &str, footprint: &str) -> Symbol {
    Symbol {
        uuid: uuid::Uuid::now_v7(),
        lib_id: "Device:R".to_string(),
        reference: reference.to_string(),
        value: "10k".to_string(),
        footprint: footprint.to_string(),
        ..Default::default()
    }
}

/// The dialog open over one sheet whose R1 has no footprint.
fn app_with_dialog() -> Signex {
    let sheet = SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        paper_size: "A4".to_string(),
        symbols: vec![symbol("R1", ""), symbol("R2", "R_0603")],
        ..Default::default()
    };
    let path = PathBuf::from("bom-view-112.snxsch");
    let engine = signex_engine::Engine::new(sheet).expect("engine");
    let (mut app, _boot) = Signex::new();
    app.document_state.engines.insert(path.clone(), engine);
    app.document_state.tabs.push(crate::app::TabInfo {
        title: "bom-view-112".to_string(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: crate::app::TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path);
    let _ = app.handle_bom_preview_open();
    app
}

/// Every text the dialog lays out, without the bidi isolation marks the
/// catalog puts around arguments.
fn shown(app: &Signex) -> Vec<String> {
    lay_out_texts(app.view_bom_preview_body(), iced::Size::new(1000.0, 700.0))
        .into_iter()
        .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect()
}

#[test]
fn the_dialog_reads_as_drawn() {
    let app = app_with_dialog();
    let texts = shown(&app);
    let has = |label: &str| texts.iter().any(|t| t == label);
    for label in [
        "Variant",
        "Grouped",
        "Ungrouped",
        "Flat",
        "Include not-fitted parts (DNP) for this variant",
        "CSV",
        "XLSX",
        "HTML",
        "Cancel",
        "Export CSV\u{2026}",
    ] {
        assert!(has(label), "{label:?} not in {texts:#?}");
    }
    // Warnings are counted on the chip that opens the Rules tab.
    assert!(
        texts
            .iter()
            .any(|t| t.ends_with(" warnings") || t.ends_with(" warning")),
        "{texts:#?}"
    );
    // The variant is shown once (the toolbar), never in the footer, and the
    // two include options are gone.
    for gone in ["Include DNP", "Include Not Fitted", "Export\u{2026}"] {
        assert!(!has(gone), "{gone:?} in {texts:#?}");
    }
    assert!(
        !texts
            .iter()
            .any(|t| t.starts_with("Current variant") || t.starts_with("Variant: ") || t == "|"),
        "{texts:#?}"
    );
    let status = texts
        .iter()
        .find(|t| t.contains(" lines · "))
        .expect("the footer status");
    assert!(
        status.contains(" parts") || status.contains(" part"),
        "{status}"
    );
}

/// The primary button follows the format picked in the side bar.
#[test]
fn the_export_button_follows_the_format() {
    let mut app = app_with_dialog();
    let _ = app.update(crate::app::Message::BomPreview(
        crate::app::BomPreviewMsg::SetFormat(signex_output::BomFormat::Xlsx),
    ));
    let texts = shown(&app);
    assert!(
        texts.iter().any(|t| t == "Export XLSX\u{2026}"),
        "{texts:#?}"
    );
    assert!(
        !texts.iter().any(|t| t == "Export CSV\u{2026}"),
        "{texts:#?}"
    );
}
