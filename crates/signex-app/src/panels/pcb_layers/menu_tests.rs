//! m103: the more menu, the set name row and the Active | Set row of the
//! Layers & Colors tab.

use super::*;

/// m103: the more button opens the menu; the menu lists the layer set
/// rows, Import... and Export..., View From Bottom Side and Layer Stack....
#[test]
fn the_more_button_opens_the_menu_in_place() {
    let ctx = context(&PcbLayerView::default());
    assert!(!listed(&ctx, "Save current as set..."), "closed at first");
    let search = row_text(&ctx, "Used On");
    let sent = messages_along(&ctx, "Used On", search.x + search.width, WINDOW.width);
    let toggled = PanelMsg::LayersPanel(LayersPanelMsg::MoreMenuToggled);
    assert!(shown(&sent).contains(&format!("{toggled:?}")), "{sent:?}");

    let ctx = menu_open(&PcbLayerView::default());
    for label in [
        "LAYER SETS",
        "Save current as set...",
        "Delete current set...",
        "Import...",
        "Export...",
        "View From Bottom Side",
        "Layer Stack...",
    ] {
        assert!(listed(&ctx, label), "{label} in the menu");
    }
    let sent = messages_on_click(panel(&ctx), WINDOW, "Save current as set...");
    assert_eq!(
        shown(&sent),
        shown(&[PanelMsg::LayersPanel(LayersPanelMsg::SetNamingStarted)])
    );
}

/// m103: a click outside the menu closes it; a greyed row takes the click
/// and does nothing.
#[test]
fn a_click_outside_the_menu_closes_it_and_greyed_rows_do_nothing() {
    let ctx = menu_open(&PcbLayerView::default());
    // The left edge of a row under the card's column: outside the card.
    let below = Point::new(1.0, row_text(&ctx, "Bottom Layer").center_y());
    let sent = messages_on_click_at(panel(&ctx), WINDOW, None, below);
    assert_eq!(
        shown(&sent),
        shown(&[PanelMsg::LayersPanel(LayersPanelMsg::MoreMenuDismissed)]),
        "the backdrop takes the click, not the row under it"
    );
    for label in [
        "Import...",
        "Export...",
        "Delete current set...",
        "Layer Stack...",
    ] {
        let sent = messages_on_click(panel(&ctx), WINDOW, label);
        assert!(sent.is_empty(), "{label} is greyed: {sent:?}");
    }
}

/// m103: Delete current set... deletes the saved set the view shows; Layer
/// Stack... runs while the Layer Stack command can.
#[test]
fn delete_and_layer_stack_are_live_when_they_can_act() {
    let mut ctx = menu_open(&PcbLayerView::default());
    ctx.pcb_layers.layer_set = Some(LayerSetId::Saved("Assembly".to_string()));
    ctx.pcb_layers.layer_stack_live = true;
    let sent = messages_on_click(panel(&ctx), WINDOW, "Delete current set...");
    assert_eq!(
        shown(&sent),
        shown(&[PanelMsg::LayersPanel(LayersPanelMsg::SetDeleted(
            "Assembly".to_string()
        ))])
    );
    let sent = messages_on_click(panel(&ctx), WINDOW, "Layer Stack...");
    assert_eq!(
        shown(&sent),
        shown(&[PanelMsg::LayersPanel(LayersPanelMsg::LayerStackPicked)])
    );
}

/// m103: the Active | Set row reads the active layer and the set; a pick
/// in either dropdown sends what the layer tabs and the layer set button
/// send.
#[test]
fn the_active_row_picks_the_active_layer_and_the_set() {
    use super::super::layers_tab::active_row::{
        SetChoice, active_picked, set_choices, set_current, set_label, set_picked,
    };
    use signex_renderer::pcb::BuiltinLayerSet;
    let mut active = PcbLayerView::default();
    active.set_active(CopperLayer::Bottom);
    let ctx = context(&active);
    let texts = texts(&ctx);
    let active_y = find(&texts, "ACTIVE").bounds.center_y();
    let named = texts
        .iter()
        .find(|t| t.text == "Bottom Layer")
        .expect("the active layer's name");
    assert!(
        named.bounds.y <= active_y && active_y <= named.bounds.y + named.bounds.height,
        "the Active row names the active layer"
    );
    assert_eq!(
        shown(&[active_picked(TOP)]),
        shown(&[PanelMsg::PcbLayers(PcbViewMsg::ActiveLayerPicked(TOP))])
    );

    let layers = &ctx.pcb_layers;
    assert_eq!(set_current(layers), "Set: All");
    let choices = set_choices(layers);
    assert_eq!(set_label(&layers.i18n, &choices[0]), "All Layers");
    assert_eq!(
        shown(&[set_picked(SetChoice::Builtin(BuiltinLayerSet::All))]),
        shown(&[PanelMsg::PcbLayers(PcbViewMsg::BuiltinLayerSetShown(
            BuiltinLayerSet::All
        ))])
    );
    assert_eq!(
        shown(&[set_picked(SetChoice::Saved("Mine".to_string()))]),
        shown(&[PanelMsg::LayersPanel(LayersPanelMsg::SavedSetShown(
            "Mine".to_string()
        ))])
    );
    let mut saved = context(&PcbLayerView::default());
    saved.pcb_layers.layer_set = Some(LayerSetId::Saved("Mine".to_string()));
    assert_eq!(set_current(&saved.pcb_layers), "Set: Mine");
}

/// m103: the more menu opens and closes; Save current as set... closes it
/// and opens the name row, which closes again without a name.
#[test]
fn the_panel_state_opens_the_more_menu_and_the_name_row() {
    let mut ui = LayersPanelUi::default();
    assert!(ui.apply(&LayersPanelMsg::MoreMenuToggled));
    assert!(ui.more_menu_open);
    ui.apply(&LayersPanelMsg::MoreMenuDismissed);
    assert!(!ui.more_menu_open);
    ui.apply(&LayersPanelMsg::MoreMenuToggled);
    assert!(ui.apply(&LayersPanelMsg::SetNamingStarted));
    assert!(ui.naming && !ui.more_menu_open);
    ui.apply(&LayersPanelMsg::SetNameChanged("Mine".to_string()));
    ui.apply(&LayersPanelMsg::SetNamingCancelled);
    assert!(!ui.naming && ui.set_name.is_empty());
    ui.apply(&LayersPanelMsg::MoreMenuToggled);
    assert!(!ui.apply(&LayersPanelMsg::LayerStackPicked));
    assert!(!ui.more_menu_open, "a picked row closes the menu");
}

/// m103: the name row: Save is live once a name is typed, and Enter saves.
#[test]
fn the_name_row_saves_a_named_set() {
    let ui = LayersPanelUi {
        naming: true,
        ..LayersPanelUi::default()
    };
    let ctx = context_with(&PcbLayerView::default(), ui);
    assert!(messages_on_click(panel(&ctx), WINDOW, "Save").is_empty());
    let ui = LayersPanelUi {
        naming: true,
        set_name: "Mine".to_string(),
        ..LayersPanelUi::default()
    };
    let ctx = context_with(&PcbLayerView::default(), ui);
    assert_eq!(
        shown(&messages_on_click(panel(&ctx), WINDOW, "Save")),
        shown(&[PanelMsg::LayersPanel(LayersPanelMsg::SetSaved)])
    );
}
