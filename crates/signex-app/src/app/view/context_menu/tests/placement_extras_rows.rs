//! Part Actions ▸ Autoplace Fields and Copy Attributes (#1667): each row
//! runs its command through `Signex::update` on the right-clicked part.

use super::row_effects::{
    app_with, assert_menu_closed, canvas_rows, doc, press, row_is_live, select_symbols, sheet_of,
    symbol,
};

use signex_types::schematic::{HAlign, LibSymbol, Point, TextProp, VAlign};

/// One part `R1` with a body, its reference dragged far off by the user.
fn part_with_moved_reference() -> crate::app::Signex {
    let mut part = symbol(1, "R1", 10.0, 10.0);
    part.ref_text = Some(TextProp {
        position: Point::new(90.0, 90.0),
        rotation: 0.0,
        font_size: 1.27,
        font_family: None,
        justify_h: HAlign::Left,
        justify_v: VAlign::Bottom,
        hidden: false,
    });
    part.fields_user_placed = true;
    let mut sheet = sheet_of(vec![part]);
    let body: LibSymbol = serde_json::from_str(r#"{"id":"Device:R"}"#).expect("body");
    sheet.lib_symbols.insert(body.id.clone(), body);
    let mut app = app_with(sheet);
    select_symbols(&mut app);
    app
}

#[test]
fn the_part_rows_are_live_for_a_selected_part_only() {
    let mut app = part_with_moved_reference();
    let rows = canvas_rows(&app);
    assert!(row_is_live(&rows, "autoplace_fields"));
    assert!(row_is_live(&rows, "copy_attributes_or_add_vertex"));

    app.interaction_state.active_canvas_mut().selected.clear();
    let rows = canvas_rows(&app);
    assert!(!row_is_live(&rows, "autoplace_fields"));
    assert!(!row_is_live(&rows, "copy_attributes_or_add_vertex"));
}

#[test]
fn autoplace_fields_row_brings_the_reference_back_to_its_part() {
    let mut app = part_with_moved_reference();

    let _ = press(&mut app, "autoplace_fields");

    let part = &doc(&app).symbols[0];
    assert!(!part.fields_user_placed, "the fields count as autoplaced");
    let at = part.ref_text.as_ref().expect("reference").position;
    assert!(
        (at.x - 10.0).abs() < 20.0 && (at.y - 10.0).abs() < 20.0,
        "the reference sits by its part: {at:?}"
    );
    assert_menu_closed(&app, "autoplace_fields");
}

#[test]
fn copy_attributes_row_puts_another_of_the_part_on_the_cursor() {
    let mut app = part_with_moved_reference();

    let _ = press(&mut app, "copy_attributes_or_add_vertex");

    let session = app
        .interaction_state
        .placement
        .as_ref()
        .expect("a part is on the cursor");
    let copy = session.payload.lone_symbol().expect("one part");
    assert_eq!(copy.lib_id, "Device:R");
    assert_eq!(copy.value, "10k");
    assert_eq!(doc(&app).symbols.len(), 1, "nothing placed before a click");
    assert_menu_closed(&app, "copy_attributes_or_add_vertex");
}
