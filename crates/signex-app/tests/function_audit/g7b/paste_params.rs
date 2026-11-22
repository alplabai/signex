//! Rows m69 (Paste Array) and m70 (Parameter Manager).

use super::*;

// ── m69 Paste Array ─────────────────────────────────────────────────────

fn array(app: &mut Signex, message: PasteArrayMsg) {
    send(app, Message::Edit(EditMsg::PasteArray(message)));
}

/// R1 and a label `D0` copied, the dialog open.
fn app_with_array_open() -> Signex {
    let mut sheet = sheet();
    sheet.labels.push(net_label("D0", (200.0, 200.0)));
    let mut app = app_with(sheet);
    let r1 = symbol(&app, "R1").uuid;
    let d0 = doc(&app).labels[3].uuid;
    app.interaction_state.active_canvas_mut().selected = vec![
        SelectedItem::new(r1, SelectedKind::Symbol),
        SelectedItem::new(d0, SelectedKind::Label),
    ];
    send(&mut app, Message::Edit(EditMsg::Copy));
    send(&mut app, Message::Edit(EditMsg::PasteArrayRequested));
    let request = PasteRequest {
        intent: PasteIntent::Array,
        epoch: app.interaction_state.placement_epoch,
        path: app.document_state.active_path.clone(),
        dialog: 0,
        from_agent: false,
    };
    send(
        &mut app,
        Message::Edit(EditMsg::PasteTextRead {
            request,
            text: None,
        }),
    );
    assert!(app.ui_state.paste_array.is_some(), "the dialog is open");
    app
}

fn type_array(app: &mut Signex, count: &str, dx: &str, dy: &str, step: &str) {
    array(app, PasteArrayMsg::CountChanged(count.into()));
    array(app, PasteArrayMsg::XSpacingChanged(dx.into()));
    array(app, PasteArrayMsg::YSpacingChanged(dy.into()));
    array(app, PasteArrayMsg::StepChanged(step.into()));
}

fn d_labels(app: &Signex) -> Vec<String> {
    label_texts(app)
        .into_iter()
        .filter(|t| t.starts_with('D'))
        .collect()
}

#[test]
fn m69_paste_places_spaced_counted_copies_as_one_undo_step() {
    let mut app = app_with_array_open();
    let original = symbol(&app, "R1").position;
    type_array(&mut app, "3", "10.16", "5.08", "1");
    let dialog = app.ui_state.paste_array.as_ref().unwrap();
    assert_eq!(dialog.placed_items(), Some(6), "the footer's item count");

    array(&mut app, PasteArrayMsg::ApplyPressed);

    assert!(app.ui_state.paste_array.is_none(), "Paste closes the dialog");
    let symbols = &doc(&app).symbols;
    assert_eq!(symbols.len(), 4 + 3);
    for (k, copy) in symbols[4..].iter().enumerate() {
        let k = (k + 1) as f64;
        assert!(near(copy.position.x, original.x + 10.16 * k));
        assert!(near(copy.position.y, original.y + 5.08 * k));
    }
    assert_eq!(d_labels(&app), ["D0", "D1", "D2", "D3"]);
    assert!(is_dirty(&app));

    undo(&mut app);
    assert_eq!(doc(&app).symbols.len(), 4, "one undo takes the array back");
    assert_eq!(d_labels(&app), ["D0"]);
}

#[test]
fn m69_count_up_off_keeps_the_label_on_every_copy() {
    let mut app = app_with_array_open();
    type_array(&mut app, "2", "10.16", "0", "1");
    array(&mut app, PasteArrayMsg::CountUpToggled(false));
    assert_eq!(app.ui_state.paste_array.as_ref().unwrap().step, "0");

    array(&mut app, PasteArrayMsg::ApplyPressed);
    assert_eq!(d_labels(&app), ["D0", "D0", "D0"]);
}

#[test]
fn m69_count_up_on_again_restores_a_step() {
    let mut app = app_with_array_open();
    app.ui_state.increment_step = 2;
    array(&mut app, PasteArrayMsg::CountUpToggled(false));
    array(&mut app, PasteArrayMsg::CountUpToggled(true));
    assert_eq!(app.ui_state.paste_array.as_ref().unwrap().step, "2");
}

#[test]
fn m69_the_designators_choice_reset_renumbers_the_copies() {
    let mut app = app_with_array_open();
    type_array(&mut app, "1", "10.16", "0", "1");
    array(
        &mut app,
        PasteArrayMsg::DesignatorsChosen(PasteDesignators::Reset),
    );
    array(&mut app, PasteArrayMsg::ApplyPressed);
    assert_eq!(doc(&app).symbols[4].reference, "R?");
}

#[test]
fn m69_cancel_places_nothing() {
    let mut app = app_with_array_open();
    type_array(&mut app, "3", "10.16", "0", "1");
    array(&mut app, PasteArrayMsg::Closed);
    assert!(app.ui_state.paste_array.is_none());
    assert_eq!(doc(&app).symbols.len(), 4);
    assert!(!is_dirty(&app));
}

#[test]
fn m69_copies_outside_1_to_1000_are_refused_and_the_dialog_stays() {
    for bad in ["0", "1001", "x"] {
        let mut app = app_with_array_open();
        type_array(&mut app, bad, "10.16", "0", "1");
        array(&mut app, PasteArrayMsg::ApplyPressed);
        assert!(app.ui_state.paste_array.is_some(), "{bad}: stays open");
        assert_eq!(doc(&app).symbols.len(), 4, "{bad}: nothing placed");
        assert!(!is_dirty(&app), "{bad}");
    }
}

#[test]
fn m69_a_non_finite_spacing_is_refused() {
    let mut app = app_with_array_open();
    type_array(&mut app, "2", "inf", "0", "1");
    array(&mut app, PasteArrayMsg::ApplyPressed);
    assert!(app.ui_state.paste_array.is_some());
    assert_eq!(doc(&app).symbols.len(), 4);
}

// ── m70 Parameter Manager ───────────────────────────────────────────────

#[test]
fn m70_a_cell_writes_through_and_its_keystrokes_are_one_undo_step() {
    let mut app = app();
    pm(&mut app, Pm::Open);
    edit_cell(&mut app, "R1", "Tolerance", "2");
    edit_cell(&mut app, "R1", "Tolerance", "2%");
    assert_eq!(field(&app, "R1", "Tolerance"), Some("2%"));
    assert!(is_dirty(&app));

    undo(&mut app);
    assert_eq!(field(&app, "R1", "Tolerance"), Some("1%"));
}

#[test]
fn m70_enter_ends_the_cells_undo_step() {
    let mut app = app();
    pm(&mut app, Pm::Open);
    edit_cell(&mut app, "R1", "Tolerance", "2%");
    pm(&mut app, Pm::CellSubmitted);
    edit_cell(&mut app, "R1", "Tolerance", "3%");
    undo(&mut app);
    assert_eq!(field(&app, "R1", "Tolerance"), Some("2%"));
}

#[test]
fn m70_revert_restores_every_cell_as_one_step_and_undo_brings_them_back() {
    let mut app = app();
    pm(&mut app, Pm::Open);
    edit_cell(&mut app, "R1", "Tolerance", "2%");
    pm(&mut app, Pm::CellSubmitted);
    edit_cell(&mut app, "R2", "Tolerance", "10%");
    pm(&mut app, Pm::CellSubmitted);
    edit_cell(&mut app, "R3", "MPN", "GRM31CR71E106KA12");
    pm(&mut app, Pm::CellSubmitted);

    pm(&mut app, Pm::SessionReverted);
    assert_eq!(field(&app, "R1", "Tolerance"), Some("1%"));
    assert_eq!(field(&app, "R2", "Tolerance"), Some("5%"));
    assert_eq!(field(&app, "R3", "MPN"), None, "a created cell goes away");

    undo(&mut app);
    assert_eq!(field(&app, "R1", "Tolerance"), Some("2%"));
    assert_eq!(field(&app, "R2", "Tolerance"), Some("10%"));
    assert_eq!(field(&app, "R3", "MPN"), Some("GRM31CR71E106KA12"));
}

#[test]
fn m70_add_column_gives_every_part_but_a_power_port_an_empty_value() {
    let mut sheet = sheet();
    let mut gnd = part("#PWR1", "GND", "lib:r", 180.0);
    gnd.is_power = true;
    sheet.symbols.push(gnd);
    let mut app = app_with(sheet);
    pm(&mut app, Pm::Open);
    pm(&mut app, Pm::NewKeyChanged(" Voltage ".into()));
    pm(&mut app, Pm::ColumnAdded);

    for reference in ["R1", "R2", "R3", "C1"] {
        assert_eq!(field(&app, reference, "Voltage"), Some(""), "{reference}");
    }
    assert_eq!(field(&app, "#PWR1", "Voltage"), None);
    assert!(
        app.ui_state.parameter_table.new_key.is_empty(),
        "the name box clears"
    );

    undo(&mut app);
    assert_eq!(field(&app, "R1", "Voltage"), None, "one undo step");
    assert_eq!(field(&app, "C1", "Voltage"), None);
}

#[test]
fn m70_add_column_keeps_a_value_a_part_already_has() {
    let mut app = app();
    pm(&mut app, Pm::Open);
    pm(&mut app, Pm::NewKeyChanged("Tolerance".into()));
    pm(&mut app, Pm::ColumnAdded);
    assert_eq!(field(&app, "R1", "Tolerance"), Some("1%"));
    assert_eq!(field(&app, "R3", "Tolerance"), Some(""));
}

#[test]
fn m70_a_blank_or_reserved_column_name_adds_nothing() {
    for bad in ["   ", "Value", "reference", "FOOTPRINT"] {
        let mut app = app();
        pm(&mut app, Pm::Open);
        pm(&mut app, Pm::NewKeyChanged(bad.into()));
        pm(&mut app, Pm::ColumnAdded);
        assert!(!is_dirty(&app), "{bad:?}");
        assert_eq!(symbol(&app, "R3").fields.len(), 0, "{bad:?}");
    }
}

#[test]
fn m70_remove_column_clears_it_from_every_part_in_one_undo_step() {
    let mut app = app();
    pm(&mut app, Pm::Open);
    pm(
        &mut app,
        Pm::ParameterRemoved {
            key: "Tolerance".into(),
        },
    );
    assert_eq!(field(&app, "R1", "Tolerance"), None);
    assert_eq!(field(&app, "R2", "Tolerance"), None);
    undo(&mut app);
    assert_eq!(field(&app, "R1", "Tolerance"), Some("1%"));
    assert_eq!(field(&app, "R2", "Tolerance"), Some("5%"));
}

#[test]
fn m70_the_filter_and_clear_filter_leave_the_document_alone() {
    let mut app = app();
    pm(&mut app, Pm::Open);
    pm(&mut app, Pm::FilterChanged("QFN-32".into()));
    assert_eq!(app.ui_state.parameter_table.filter, "QFN-32");
    pm(&mut app, Pm::FilterCleared);
    assert!(app.ui_state.parameter_table.filter.is_empty());
    assert!(!is_dirty(&app));
}

#[test]
fn m70_done_closes_and_keeps_the_edits() {
    let mut app = app();
    pm(&mut app, Pm::Open);
    edit_cell(&mut app, "R1", "Tolerance", "2%");
    pm(&mut app, Pm::Close);
    assert!(!app.ui_state.parameter_manager_open);
    assert_eq!(field(&app, "R1", "Tolerance"), Some("2%"));
    assert!(is_dirty(&app));
}
