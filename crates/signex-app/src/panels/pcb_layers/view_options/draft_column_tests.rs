//! Mockup row 103, decided 2026-11-24 (docs/DECISIONS.md): in View Options,
//! Draft stays its own column between Object and Transparency, not a third
//! eye state.

use iced::Size;

use crate::app::Signex;
use crate::layout_probe::lay_out_texts;
use crate::panels::{LayersPanelUi, LayersTab, PanelKind, PcbLayersPanel};

/// Top copper only, no objects.
fn bare_board() -> signex_types::pcb::PcbBoard {
    serde_json::from_str(
        r#"{
          "uuid": "8d15b2f9-8f86-41d7-9ec4-0f54a4f3a651", "version": 1,
          "generator": "test", "thickness": 1.6, "outline": [],
          "layers": [ { "id": 0, "name": "TopCopper", "layer_type": "signal" } ],
          "setup": null, "nets": [], "footprints": [],
          "segments": [], "vias": [], "zones": [], "graphics": [], "texts": []
        }"#,
    )
    .expect("valid board")
}

#[test]
fn view_options_draws_draft_as_its_own_column_between_object_and_transparency() {
    let (mut app, _boot) = Signex::new();
    let ctx = &mut app.document_view.panel_ctx;
    let view = signex_renderer::pcb::PcbLayerView::default();
    ctx.pcb_layers = PcbLayersPanel::new(&bare_board(), &view, app.ui_state.i18n.clone());
    ctx.pcb_layers_ui = LayersPanelUi {
        tab: LayersTab::ViewOptions,
        ..LayersPanelUi::default()
    };
    let ctx = app.document_view.panel_ctx.clone();
    let texts = lay_out_texts(
        crate::panels::view_panel(PanelKind::Layers, &ctx),
        Size::new(320.0, 2600.0),
    );
    let draft = texts
        .iter()
        .find(|t| t.text == "Draft")
        .expect("a Draft header");
    let y = draft.bounds.center_y();
    let on_row = |label: &str| {
        texts
            .iter()
            .find(|t| t.text == label && (t.bounds.center_y() - y).abs() < 2.0)
            .unwrap_or_else(|| panic!("{label} beside Draft: {texts:#?}"))
    };
    let (object, fade) = (on_row("Object"), on_row("Transparency"));
    assert!(object.bounds.x < draft.bounds.x && draft.bounds.x < fade.bounds.x);
}
