//! Rows m71 (net colour palette), m72 (custom net colour picker), m73
//! (Custom Selection Filter) and m74 (Placement Paused, read-only sheet).

use super::*;

fn wire_color(app: &Signex, index: usize) -> Option<Color> {
    let doc = doc(app);
    doc.net_colors.get(&doc.wires[index].uuid).copied()
}

// ── m71 Net colour palette ──────────────────────────────────────────────

#[test]
fn m71_a_swatch_colours_every_wire_of_that_net_in_one_undo_step() {
    let mut app = app();
    net(&mut app, NetColorMsg::Open);
    assert!(app.ui_state.net_color_palette_open);
    net(&mut app, NetColorMsg::RowOpened("VOUT".into()));
    set_net(&mut app, "VOUT", Some(RED));

    assert_eq!(wire_color(&app, 0), Some(RED));
    assert_eq!(wire_color(&app, 1), Some(RED));
    assert_eq!(wire_color(&app, 2), None, "another net keeps its colour");
    assert_eq!(app.ui_state.net_color_palette.chooser, None, "chooser closes");
    assert!(is_dirty(&app));

    undo(&mut app);
    assert!(doc(&app).net_colors.is_empty());
}

#[test]
fn m71_clear_in_the_chooser_removes_only_that_nets_colour() {
    let mut app = app();
    set_net(&mut app, "VOUT", Some(RED));
    set_net(&mut app, "VOUT_3V3", Some(RED));
    set_net(&mut app, "VOUT", None);
    assert_eq!(wire_color(&app, 0), None);
    assert_eq!(wire_color(&app, 1), None);
    assert_eq!(wire_color(&app, 2), Some(RED));

    undo(&mut app);
    assert_eq!(wire_color(&app, 0), Some(RED), "Clear is one undo step");
}

#[test]
fn m71_clear_all_colours_is_one_undo_step() {
    let mut app = app();
    set_net(&mut app, "VOUT", Some(RED));
    set_net(&mut app, "VOUT_3V3", Some(RED));
    net(&mut app, NetColorMsg::ClearedAll);
    assert!(doc(&app).net_colors.is_empty());

    undo(&mut app);
    assert_eq!(doc(&app).net_colors.len(), 3, "all three wires come back");
}

#[test]
fn m71_clicking_a_row_again_closes_its_chooser() {
    let mut app = app();
    net(&mut app, NetColorMsg::Open);
    net(&mut app, NetColorMsg::RowOpened("VOUT".into()));
    assert_eq!(
        app.ui_state.net_color_palette.chooser.as_deref(),
        Some("VOUT")
    );
    net(&mut app, NetColorMsg::RowOpened("VOUT".into()));
    assert_eq!(app.ui_state.net_color_palette.chooser, None);
}

#[test]
fn m71_reopening_starts_with_no_filter_and_no_chooser() {
    let mut app = app();
    net(&mut app, NetColorMsg::Open);
    net(&mut app, NetColorMsg::FilterChanged("VO".into()));
    net(&mut app, NetColorMsg::RowOpened("VOUT".into()));
    net(&mut app, NetColorMsg::Close);
    assert!(!app.ui_state.net_color_palette_open);
    net(&mut app, NetColorMsg::Open);
    assert!(app.ui_state.net_color_palette.filter.is_empty());
    assert_eq!(app.ui_state.net_color_palette.chooser, None);
    assert!(!is_dirty(&app), "opening and closing edits nothing");
}

#[test]
fn m71_a_net_with_no_label_on_the_sheet_changes_nothing() {
    let mut app = app();
    set_net(&mut app, "NOT_ON_THIS_SHEET", Some(RED));
    assert!(doc(&app).net_colors.is_empty());
    assert!(!app.ui_state.net_colors.contains_key("NOT_ON_THIS_SHEET"));
    assert!(!is_dirty(&app));
}

// ── m72 Custom net colour picker ────────────────────────────────────────

fn draft_rgb(app: &Signex) -> (u8, u8, u8) {
    let c = app.ui_state.net_color_custom.draft;
    let channel = |v: f32| (v * 255.0).round() as u8;
    (channel(c.r), channel(c.g), channel(c.b))
}

#[test]
fn m72_use_color_colours_the_net_the_picker_opened_for() {
    let mut app = app();
    net(&mut app, NetColorMsg::Open);
    net(&mut app, NetColorMsg::RowOpened("VOUT".into()));
    net(&mut app, NetColorMsg::CustomOpenedFor("VOUT".into()));
    assert!(app.ui_state.net_color_custom.show);
    assert_eq!(app.ui_state.net_color_palette.chooser, None);

    net(&mut app, NetColorMsg::CustomHexChanged("#14B8A6".into()));
    assert_eq!(draft_rgb(&app), TEAL);
    let draft = app.ui_state.net_color_custom.draft;
    net(&mut app, NetColorMsg::CustomSubmit(draft));

    let teal = Color::new(TEAL.0, TEAL.1, TEAL.2, 255);
    assert!(!app.ui_state.net_color_custom.show, "Use Color closes it");
    assert_eq!(wire_color(&app, 0), Some(teal));
    assert_eq!(wire_color(&app, 1), Some(teal));
    assert_eq!(wire_color(&app, 2), None);
    assert!(is_dirty(&app));

    undo(&mut app);
    assert!(doc(&app).net_colors.is_empty());
}

#[test]
fn m72_the_picker_starts_from_the_nets_current_colour() {
    let mut app = app();
    set_net(&mut app, "VOUT", Some(RED));
    net(&mut app, NetColorMsg::CustomOpenedFor("VOUT".into()));
    assert_eq!(draft_rgb(&app), (RED.r, RED.g, RED.b));
}

#[test]
fn m72_the_rgb_fields_edit_the_draft_and_clamp_at_255() {
    let mut app = app();
    net(&mut app, NetColorMsg::CustomOpenedFor("VOUT".into()));
    net(&mut app, NetColorMsg::CustomHexChanged("#000000".into()));
    net(&mut app, NetColorMsg::CustomChannel(Channel::R, "20".into()));
    net(&mut app, NetColorMsg::CustomChannel(Channel::G, "184".into()));
    net(&mut app, NetColorMsg::CustomChannel(Channel::B, "999".into()));
    assert_eq!(draft_rgb(&app), (20, 184, 255));
}

#[test]
fn m72_text_that_is_no_number_keeps_the_channel() {
    let mut app = app();
    net(&mut app, NetColorMsg::CustomOpenedFor("VOUT".into()));
    net(&mut app, NetColorMsg::CustomHexChanged("#14B8A6".into()));
    net(&mut app, NetColorMsg::CustomChannel(Channel::R, "abc".into()));
    net(&mut app, NetColorMsg::CustomChannel(Channel::G, "".into()));
    assert_eq!(draft_rgb(&app), TEAL);
}

#[test]
fn m72_a_bad_hex_keeps_the_draft_and_the_typed_text() {
    let mut app = app();
    net(&mut app, NetColorMsg::CustomOpenedFor("VOUT".into()));
    net(&mut app, NetColorMsg::CustomHexChanged("#14B8A6".into()));
    for bad in ["#14B", "#GGGGGG", "14B8A6FF"] {
        net(&mut app, NetColorMsg::CustomHexChanged(bad.into()));
        assert_eq!(draft_rgb(&app), TEAL, "{bad}");
        assert_eq!(app.ui_state.net_color_custom.hex.as_deref(), Some(bad));
    }
    assert!(doc(&app).net_colors.is_empty());
}

#[test]
fn m72_cancel_leaves_the_net_uncoloured() {
    let mut app = app();
    net(&mut app, NetColorMsg::CustomOpenedFor("VOUT".into()));
    net(&mut app, NetColorMsg::CustomHexChanged("#14B8A6".into()));
    net(&mut app, NetColorMsg::CustomShow(false));
    let custom = &app.ui_state.net_color_custom;
    assert!(!custom.show);
    assert_eq!(custom.net, None);
    assert!(doc(&app).net_colors.is_empty());
    assert!(!is_dirty(&app));
}

#[test]
fn m72_a_cancelled_pick_is_not_offered_again_on_the_next_open() {
    let mut app = app();
    let untouched = draft_rgb(&app);
    net(&mut app, NetColorMsg::CustomOpenedFor("VOUT".into()));
    net(&mut app, NetColorMsg::CustomHexChanged("#112233".into()));
    net(&mut app, NetColorMsg::CustomShow(false));
    net(&mut app, NetColorMsg::CustomOpenedFor("VOUT_3V3".into()));
    assert_eq!(draft_rgb(&app), untouched, "Cancel discards the pick");
}

#[test]
fn m72_without_a_row_use_color_arms_the_net_colour_pen() {
    let mut app = app();
    net(&mut app, NetColorMsg::CustomShow(true));
    let c = iced::Color::from_rgb8(TEAL.0, TEAL.1, TEAL.2);
    net(&mut app, NetColorMsg::CustomSubmit(c));
    let teal = Color::new(TEAL.0, TEAL.1, TEAL.2, 255);
    assert_eq!(app.ui_state.pending_net_color, Some(teal));
    assert!(doc(&app).net_colors.is_empty(), "the pen colours on a click");
}

// ── m73 Custom Selection Filter ─────────────────────────────────────────

fn filter(app: &mut Signex, message: SelectionFilterMsg) {
    send(app, Message::SelectionFilter(message));
}

fn footprint_app() -> (Signex, PathBuf) {
    let (mut app, _task) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    let path = PathBuf::from("g7b-part.snxfpt");
    let file =
        signex_library::FootprintFile::from_footprint(signex_library::Footprint::empty("fp"));
    app.document_state
        .footprint_editors
        .insert(path.clone(), FootprintDocument::new(path.clone(), file));
    app.document_state.tabs.push(TabInfo {
        title: "part".to_owned(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::FootprintEditor(path.clone()),
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.interaction_state.footprint_filter_presets = Vec::new();
    (app, path)
}

fn pads_on(app: &Signex, path: &Path) -> bool {
    app.document_state.footprint_editors[path]
        .state
        .selection_filter
        .pads
}

#[test]
fn m73_apply_writes_the_kinds_to_the_editor_and_saves_the_named_preset() {
    let (mut app, path) = footprint_app();
    assert!(pads_on(&app, &path));
    filter(&mut app, SelectionFilterMsg::OpenCustom);
    filter(
        &mut app,
        SelectionFilterMsg::ToggleCustomKind(SelectionFilterKind::Pads),
    );
    filter(
        &mut app,
        SelectionFilterMsg::CustomPresetNameEdited("Pads + text".into()),
    );
    assert!(pads_on(&app, &path), "the live filter waits for Apply");

    filter(&mut app, SelectionFilterMsg::ApplyCustom);

    assert!(app.ui_state.selection_filter_custom.is_none());
    assert!(!pads_on(&app, &path));
    let presets = &app.interaction_state.footprint_filter_presets;
    assert_eq!(presets.len(), 1);
    assert_eq!(presets[0].name, "Pads + text");
    assert!(!presets[0].kinds.contains(&SelectionFilterKind::Pads));
}

#[test]
fn m73_all_and_none_set_every_shown_kind() {
    let (mut app, _path) = footprint_app();
    filter(&mut app, SelectionFilterMsg::OpenCustom);
    filter(&mut app, SelectionFilterMsg::CustomKindsAllSet(false));
    let count = |app: &Signex| {
        let draft = app.ui_state.selection_filter_custom.as_ref();
        draft.expect("dialog").shown_on_count()
    };
    assert_eq!(count(&app), 0);
    filter(&mut app, SelectionFilterMsg::CustomKindsAllSet(true));
    assert_eq!(count(&app), 8);
}

#[test]
fn m73_a_blank_preset_name_becomes_the_next_filter_number() {
    let (mut app, _path) = footprint_app();
    filter(&mut app, SelectionFilterMsg::OpenCustom);
    filter(
        &mut app,
        SelectionFilterMsg::ToggleCustomKind(SelectionFilterKind::Texts),
    );
    filter(
        &mut app,
        SelectionFilterMsg::CustomPresetNameEdited("   ".into()),
    );
    filter(&mut app, SelectionFilterMsg::ApplyCustom);
    assert_eq!(
        app.interaction_state.footprint_filter_presets[0].name,
        "Filter 1"
    );
}

#[test]
fn m73_cancel_leaves_the_filter_and_the_presets() {
    let (mut app, path) = footprint_app();
    filter(&mut app, SelectionFilterMsg::OpenCustom);
    filter(&mut app, SelectionFilterMsg::CustomKindsAllSet(false));
    filter(&mut app, SelectionFilterMsg::CloseCustom);
    assert!(app.ui_state.selection_filter_custom.is_none());
    assert!(pads_on(&app, &path));
    assert!(app.interaction_state.footprint_filter_presets.is_empty());
}

#[test]
fn m73_no_dialog_opens_on_a_schematic_sheet() {
    let mut app = app();
    filter(&mut app, SelectionFilterMsg::OpenCustom);
    assert!(app.ui_state.selection_filter_custom.is_none());
}

// ── m74 Placement paused / read-only ────────────────────────────────────

fn tool(app: &mut Signex, message: ToolMessage) {
    send(app, Message::Tool(message));
}

fn pause_wire(app: &mut Signex) {
    tool(app, ToolMessage::SelectTool(Tool::Wire));
    tool(app, ToolMessage::PrePlacementTab);
}

fn set_width(app: &mut Signex, width: WireWidthPreset) {
    send(
        app,
        Message::Dock(DockMessage::Panel(PanelMsg::SetPrePlacementWireWidth(width))),
    );
}

fn form_width(app: &Signex) -> Option<WireWidthPreset> {
    let form = app.document_state.panel_ctx.pre_placement.as_ref();
    form.map(|f| f.wire_width)
}

fn is_paused(app: &Signex) -> bool {
    app.interaction_state.active_canvas().placement_paused
}

fn click(app: &mut Signex, x: f64, y: f64) {
    send(
        app,
        Message::CanvasEvent(CanvasEvent::Clicked {
            world_x: x,
            world_y: y,
        }),
    );
}

#[test]
fn m74_tab_pauses_and_a_canvas_click_places_nothing() {
    let mut app = app();
    let wires = doc(&app).wires.len();
    pause_wire(&mut app);
    assert!(is_paused(&app));
    click(&mut app, 200.0, 200.0);
    assert!(!app.interaction_state.wire_drawing, "the click is swallowed");
    assert_eq!(doc(&app).wires.len(), wires);
    assert!(!is_dirty(&app));
}

#[test]
fn m74_resume_keeps_the_edits_and_the_next_click_places_again() {
    let mut app = app();
    pause_wire(&mut app);
    set_width(&mut app, WireWidthPreset::Large);
    tool(&mut app, ToolMessage::ResumePlacement);
    assert!(!is_paused(&app));
    assert_eq!(app.interaction_state.current_tool, Tool::Wire);
    assert_eq!(form_width(&app), Some(WireWidthPreset::Large));
    click(&mut app, 200.0, 200.0);
    assert!(app.interaction_state.wire_drawing, "placing goes on");
}

#[test]
fn m74_escape_drops_the_edits_and_keeps_placing_then_leaves_the_tool() {
    let mut app = app();
    pause_wire(&mut app);
    set_width(&mut app, WireWidthPreset::Large);
    tool(&mut app, ToolMessage::ResumePlacement);

    tool(&mut app, ToolMessage::PrePlacementTab);
    set_width(&mut app, WireWidthPreset::Medium);
    escape(&mut app);
    assert!(!is_paused(&app));
    assert_eq!(app.interaction_state.current_tool, Tool::Wire);
    assert_eq!(form_width(&app), Some(WireWidthPreset::Large));

    escape(&mut app);
    assert_eq!(app.interaction_state.current_tool, Tool::Select);
}

#[test]
fn m74_tab_again_goes_on_placing_with_the_edits() {
    let mut app = app();
    pause_wire(&mut app);
    set_width(&mut app, WireWidthPreset::Large);
    tool(&mut app, ToolMessage::PrePlacementTab);
    assert!(!is_paused(&app), "the second Tab ends the pause");
    assert_eq!(form_width(&app), Some(WireWidthPreset::Large));
}

#[test]
fn m74_tab_with_no_tool_armed_does_not_pause() {
    let mut app = app();
    tool(&mut app, ToolMessage::PrePlacementTab);
    assert!(!is_paused(&app));
}

#[test]
fn m74_a_read_only_sheet_refuses_every_edit_and_stays_clean() {
    let mut app = read_only_app();
    assert!(doc(&app).is_read_only());
    let was = at(&app, "R1");

    open_move(&mut app, "5.08", "2.54");
    mv(&mut app, MoveSelectionMsg::Apply);
    assert_eq!(at(&app, "R1"), was, "Move");

    edit_cell(&mut app, "R1", "Tolerance", "2%");
    assert_eq!(field(&app, "R1", "Tolerance"), Some("1%"), "a parameter");

    set_net(&mut app, "VOUT", Some(RED));
    assert!(doc(&app).net_colors.is_empty(), "a net colour");
    assert!(!app.ui_state.net_colors.contains_key("VOUT"));

    open_replace(&mut app, "VOUT", "VOUT_A");
    find(&mut app, FindReplaceMsg::ReplaceAll);
    assert_eq!(label_texts(&app), ["VOUT", "VOUT_3V3", "VOUT"], "Replace");

    assert!(!is_dirty(&app));
}
