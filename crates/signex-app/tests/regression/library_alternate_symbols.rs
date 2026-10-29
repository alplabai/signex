//! #1230 — a component row's alternate symbols. A placed symbol bound to an
//! alternate counts as bound to its row: the Library Updates apply path
//! adopts the alternate's changes, and nothing reverts it to the default.

use crate::library_place_symbol::{
    Fixture, app_with_library, doc, fixture, place, two_unit_symbol,
};
use crate::library_symbol_drift::{add_pin, pin_count, run_task, select_first};
use signex_app::app::{EditMsg, Message, Signex};
use signex_app::dock::DockMessage;
use signex_app::library::messages::{EditorMsg, LibraryMessage, PrimitivePickerMsg};
use signex_app::library::state::{ComponentPreviewState, EditorAddress, PrimitivePickerTarget};
use signex_app::panels::PanelMsg;
use signex_library::adapter::LibraryAdapter;
use signex_library::adapters::local_git::LocalGitAdapter;
use signex_library::primitive::{PrimitiveKind, PrimitiveRef, SymbolFile};
use signex_library::{AlternateSymbol, RowId};
use uuid::Uuid;

pub(crate) const TABLE: &str = "ICs";

/// An alternate symbol of the fixture's row: same pins, its own uuid.
pub(crate) struct Alternate {
    pub(crate) symbol_id: Uuid,
    pub(crate) lib_id: String,
}

/// Write a second symbol file into `f`'s library and list it as the row's
/// alternate "IEC". Run before the library is mounted.
pub(crate) fn add_alternate(f: &Fixture) -> Alternate {
    add_alternate_shaped(f, "IEC", |_| {})
}

/// [`add_alternate`] labelled `label`, with `shape` applied to its symbol.
pub(crate) fn add_alternate_shaped(
    f: &Fixture,
    label: &str,
    shape: impl FnOnce(&mut signex_library::primitive::Symbol),
) -> Alternate {
    let mut symbol = two_unit_symbol();
    symbol.uuid = Uuid::now_v7();
    symbol.name = format!("TWO-UNIT-{label}");
    shape(&mut symbol);
    let symbol_id = symbol.uuid;
    write_alternate_symbol(f, &symbol);

    let adapter = LocalGitAdapter::open(&f.snxlib).expect("open");
    let mut row = adapter
        .read_row(TABLE, RowId::from_uuid(f.row_id))
        .expect("row");
    row.alternate_symbols.push(AlternateSymbol::new(
        PrimitiveRef::new(f.library_id, symbol_id),
        "0.0.1",
        label,
    ));
    adapter
        .update_row(TABLE, row, "add alternate")
        .expect("update");
    Alternate {
        symbol_id,
        lib_id: format!("{}:{symbol_id}", f.library_id),
    }
}

pub(crate) fn write_alternate_symbol(f: &Fixture, symbol: &signex_library::primitive::Symbol) {
    let path = f
        .snxlib
        .parent()
        .unwrap()
        .join("symbols")
        .join(format!("{}.snxsym", symbol.uuid));
    std::fs::write(
        path,
        SymbolFile::from_symbol(symbol.clone())
            .to_toml_string()
            .expect("emit"),
    )
    .expect("write alternate symbol");
}

/// Re-key the placed symbol onto the alternate, with a stale embedded copy
/// of it, as after switching to the alternate on an older sheet.
fn bind_placed_symbol_to(app: &mut Signex, lib_id: &str) {
    let engine = app.document_state.active_engine_mut().unwrap();
    let mut d = engine.document().clone();
    let old = d.symbols[0].lib_id.clone();
    let mut copy = d.lib_symbols[&old].clone();
    copy.id = lib_id.to_string();
    d.lib_symbols.insert(lib_id.to_string(), copy);
    d.symbols[0].lib_id = lib_id.to_string();
    d.symbols[0].library_version = "0.0.1".into();
    engine.set_document(d);
    let path = app.document_state.active_path.clone().unwrap();
    let document = doc(app).clone();
    app.document_state.engines.insert(
        path.clone(),
        signex_engine::Engine::new_with_path(document, Some(path)).unwrap(),
    );
}

/// A symbol placed from an alternate is bound to its row: the apply path
/// adopts the alternate's changed pins and stamps the row's version instead
/// of refusing with "its row binds a different symbol".
#[test]
fn apply_adopts_changes_of_an_alternate_the_row_lists() {
    let _ = signex_app::diagnostics::init_logging();
    let f = fixture();
    let alt = add_alternate(&f);
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    bind_placed_symbol_to(&mut app, &alt.lib_id);

    let mut changed = two_unit_symbol();
    changed.uuid = alt.symbol_id;
    changed.name = "TWO-UNIT-IEC".into();
    add_pin(&mut changed);
    write_alternate_symbol(&f, &changed);

    let before = pin_count(&app);
    let sheet = app.document_state.active_path.clone().unwrap();
    app.scan_library_updates_for_open_schematic(sheet);

    assert_eq!(
        pin_count(&app),
        before + 1,
        "the alternate's pin was adopted"
    );
    assert_eq!(doc(&app).symbols[0].lib_id, alt.lib_id, "not reverted");
    assert_eq!(doc(&app).symbols[0].library_version, "1.2.3");
}

/// Click the Properties "Symbol" pick list: send the switch and run its
/// off-thread read to completion.
fn switch_to(app: &mut Signex, symbol: Uuid, lib_id: &str) {
    for message in start_switch(app, symbol, lib_id) {
        let _ = app.update(message);
    }
}

/// Click the pick list and run the off-thread read, but hand back its
/// completion messages instead of delivering them.
fn start_switch(app: &mut Signex, symbol: Uuid, lib_id: &str) -> Vec<Message> {
    let task = app.update(Message::Dock(DockMessage::Panel(
        PanelMsg::SwitchSymbolChoice(symbol, lib_id.to_string()),
    )));
    run_task(task)
}

fn deliver_all(app: &mut Signex, messages: Vec<Message>) {
    for message in messages {
        let _ = app.update(message);
    }
}

fn logged(needle: &str) -> bool {
    signex_app::diagnostics::recent_entries()
        .iter()
        .any(|e| e.message.contains(needle))
}

/// Wire the end of pin `number` of the first placed symbol.
fn wire_pin(app: &mut Signex, number: &str) {
    use signex_types::schematic::{Point, SymbolTransform, Wire};
    let tip = {
        let sheet = doc(app);
        let sym = &sheet.symbols[0];
        let pin = sheet.lib_symbols[&sym.lib_id]
            .pins
            .iter()
            .find(|p| p.pin.number == number)
            .unwrap();
        SymbolTransform::from_symbol(sym).apply(pin.pin.position)
    };
    app.document_state
        .active_engine_mut()
        .unwrap()
        .execute(signex_engine::Command::PlaceWireSegment {
            wire: Wire {
                start: tip,
                end: Point::new(tip.x - 10.0, tip.y),
                ..Default::default()
            },
        })
        .unwrap();
}

/// Move pin `number` of a library symbol 2.54 mm along Y.
fn move_pin(symbol: &mut signex_library::primitive::Symbol, number: &str) {
    symbol
        .pins
        .iter_mut()
        .find(|p| p.number == number)
        .unwrap()
        .position[1] += 2.54;
}

fn choices(app: &Signex) -> Vec<(String, bool)> {
    app.document_state
        .panel_ctx
        .symbol_choices
        .iter()
        .map(|c| (c.label.clone(), c.selected))
        .collect()
}

/// The switch rewrites the placed symbol's `lib_id`, embeds the chosen
/// drawing, and is one undo step that restores the default.
#[test]
fn switching_to_an_alternate_is_one_undoable_step() {
    let f = fixture();
    let alt = add_alternate(&f);
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    let id = select_first(&mut app);
    let default_lib_id = doc(&app).symbols[0].lib_id.clone();

    switch_to(&mut app, id, &alt.lib_id);
    assert_eq!(doc(&app).symbols[0].lib_id, alt.lib_id);
    assert!(doc(&app).lib_symbols.contains_key(&alt.lib_id));
    assert_eq!(
        choices(&app),
        vec![("Default".to_string(), false), ("IEC".to_string(), true)]
    );

    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(doc(&app).symbols[0].lib_id, default_lib_id);
    assert!(!doc(&app).lib_symbols.contains_key(&alt.lib_id));
    assert_eq!(
        choices(&app),
        vec![("Default".to_string(), true), ("IEC".to_string(), false)]
    );

    let _ = app.update(Message::Edit(EditMsg::Redo));
    assert_eq!(doc(&app).symbols[0].lib_id, alt.lib_id);
}

/// A drawing the row does not offer is refused and logged; the sheet is
/// left as it was.
#[test]
fn switching_to_a_symbol_the_row_does_not_offer_is_refused() {
    let _ = signex_app::diagnostics::init_logging();
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    let id = select_first(&mut app);
    let before = doc(&app).symbols[0].lib_id.clone();

    switch_to(
        &mut app,
        id,
        &format!("{}:{}", f.library_id, Uuid::now_v7()),
    );
    assert_eq!(doc(&app).symbols[0].lib_id, before);
    let logged = signex_app::diagnostics::recent_entries()
        .iter()
        .any(|e| e.message.contains("does not offer it"));
    assert!(logged, "the refusal must reach the Messages panel");
}

// ---- Component Preview: add / relabel / reorder / remove ----

/// Open a Component Preview over the fixture's row.
fn open_preview(app: &mut Signex, f: &Fixture) -> EditorAddress {
    let adapter = LocalGitAdapter::open(&f.snxlib).expect("open");
    let row = adapter
        .read_row(TABLE, RowId::from_uuid(f.row_id))
        .expect("row");
    let address = EditorAddress::new(f.snxlib.clone(), TABLE.into(), RowId::from_uuid(f.row_id));
    let mut preview = ComponentPreviewState::from_row(f.snxlib.clone(), TABLE.into(), row);
    preview.symbol = app
        .library
        .set
        .resolve_symbol(&PrimitiveRef::new(f.library_id, f.symbol_id))
        .unwrap();
    app.library.editors.insert(address.clone(), preview);
    address
}

/// Pick `symbol` through the real picker flow as the alternate to add.
fn pick_alternate(app: &mut Signex, address: &EditorAddress, library_id: Uuid, symbol: Uuid) {
    let _ = app.update(Message::Library(LibraryMessage::OpenPrimitivePicker {
        kind: PrimitiveKind::Symbol,
        target: PrimitivePickerTarget::PreviewAlternate(address.clone()),
    }));
    let _ = app.update(Message::Library(LibraryMessage::PrimitivePicker(
        PrimitivePickerMsg::Pick(PrimitiveRef::new(library_id, symbol)),
    )));
}

fn edit(app: &mut Signex, address: &EditorAddress, msg: EditorMsg) {
    let _ = app.update(Message::Library(LibraryMessage::EditorEvent {
        library_path: address.library_path.clone(),
        table: address.table.clone(),
        row_id: address.row_id,
        msg,
    }));
}

fn compatible_symbol(name: &str) -> signex_library::primitive::Symbol {
    let mut s = two_unit_symbol();
    s.uuid = Uuid::now_v7();
    s.name = name.into();
    s
}

/// An alternate with the same pin numbers is added, labelled after the
/// symbol; it can then be relabelled, reordered and removed, each marking
/// the tab dirty.
#[test]
fn the_editor_adds_relabels_reorders_and_removes_alternates() {
    let f = fixture();
    let mut app = app_with_library(&f);
    let first = compatible_symbol("IEC");
    write_alternate_symbol(&f, &first);
    let address = open_preview(&mut app, &f);

    pick_alternate(&mut app, &address, f.library_id, first.uuid);
    let state = &app.library.editors[&address];
    assert_eq!(state.row.alternate_symbols.len(), 1);
    assert_eq!(state.row.alternate_symbols[0].label, "IEC");
    assert!(state.dirty);
    assert!(state.alternate_error.is_none());

    let second = compatible_symbol("ANSI");
    let path = f
        .snxlib
        .parent()
        .unwrap()
        .join("symbols")
        .join("second.snxsym");
    std::fs::write(
        path,
        SymbolFile::from_symbol(second.clone())
            .to_toml_string()
            .unwrap(),
    )
    .unwrap();
    pick_alternate(&mut app, &address, f.library_id, second.uuid);
    assert_eq!(app.library.editors[&address].row.alternate_symbols.len(), 2);

    edit(
        &mut app,
        &address,
        EditorMsg::AlternateSymbolRelabeled {
            idx: 1,
            label: "US".into(),
        },
    );
    edit(
        &mut app,
        &address,
        EditorMsg::AlternateSymbolMoved { from: 1, to: 0 },
    );
    let labels: Vec<String> = app.library.editors[&address]
        .row
        .alternate_symbols
        .iter()
        .map(|a| a.label.clone())
        .collect();
    assert_eq!(labels, vec!["US".to_string(), "IEC".to_string()]);

    edit(
        &mut app,
        &address,
        EditorMsg::AlternateSymbolRemoved { idx: 0 },
    );
    let state = &app.library.editors[&address];
    assert_eq!(state.row.alternate_symbols.len(), 1);
    assert_eq!(state.row.alternate_symbols[0].label, "IEC");
}

/// A symbol whose pin numbers differ from the default's is refused: the row
/// is untouched, the editor shows why, and the Messages panel logs it.
#[test]
fn an_alternate_with_different_pins_is_refused_loudly() {
    let _ = signex_app::diagnostics::init_logging();
    let f = fixture();
    let mut app = app_with_library(&f);
    let mut mismatched = compatible_symbol("PINOUT-B");
    add_pin(&mut mismatched);
    write_alternate_symbol(&f, &mismatched);
    let address = open_preview(&mut app, &f);

    pick_alternate(&mut app, &address, f.library_id, mismatched.uuid);
    let state = &app.library.editors[&address];
    assert!(state.row.alternate_symbols.is_empty(), "not accepted");
    assert!(!state.dirty);
    let shown = state.alternate_error.as_deref().unwrap();
    assert!(shown.contains("extra 4"), "{shown}");
    let logged = signex_app::diagnostics::recent_entries()
        .iter()
        .any(|e| e.message.contains("Alternate symbol not added") && e.message.contains("extra 4"));
    assert!(logged, "the refusal must reach the Messages panel");
}

// ---- Switch safety: connected pins, stale results, stranded drawings ----

/// A placed symbol with pin 1 wired, and an alternate shaped by `shape`.
fn wired_with_alternate(
    shape: impl FnOnce(&mut signex_library::primitive::Symbol),
) -> (Signex, Fixture, Alternate, Uuid) {
    let _ = signex_app::diagnostics::init_logging();
    let f = fixture();
    let alt = add_alternate_shaped(&f, "IEC", shape);
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    wire_pin(&mut app, "1");
    let id = select_first(&mut app);
    (app, f, alt, id)
}

/// Moving a pin that has a wire on it would leave the wire dangling: the
/// switch is refused and logged, and the sheet is untouched.
#[test]
fn a_switch_that_moves_a_connected_pin_is_refused() {
    let (mut app, _f, alt, id) = wired_with_alternate(|s| move_pin(s, "1"));
    let before = doc(&app).symbols[0].lib_id.clone();

    switch_to(&mut app, id, &alt.lib_id);

    assert_eq!(doc(&app).symbols[0].lib_id, before);
    assert!(!doc(&app).lib_symbols.contains_key(&alt.lib_id));
    assert!(
        logged("U1: switching to IEC would disconnect pin 1; not switched"),
        "the refusal must name the reference and the pin"
    );
}

/// Only an unconnected pin moves: nothing dangles, so the switch applies.
#[test]
fn a_switch_that_moves_only_an_unconnected_pin_is_applied() {
    let (mut app, _f, alt, id) = wired_with_alternate(|s| move_pin(s, "3"));
    switch_to(&mut app, id, &alt.lib_id);
    assert_eq!(doc(&app).symbols[0].lib_id, alt.lib_id);
}

/// Same geometry: the wired pin stays where it is, so the switch applies.
#[test]
fn a_switch_with_the_same_geometry_is_applied() {
    let (mut app, _f, alt, id) = wired_with_alternate(|_| {});
    switch_to(&mut app, id, &alt.lib_id);
    assert_eq!(doc(&app).symbols[0].lib_id, alt.lib_id);
}

/// Two switches whose results arrive in reverse order: the older one is
/// dropped and logged, the newer choice stays.
#[test]
fn a_late_result_does_not_overwrite_a_newer_choice() {
    let _ = signex_app::diagnostics::init_logging();
    let f = fixture();
    let first = add_alternate_shaped(&f, "IEC", |_| {});
    let second = add_alternate_shaped(&f, "ANSI", |_| {});
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    let id = select_first(&mut app);

    let older = start_switch(&mut app, id, &first.lib_id);
    let newer = start_switch(&mut app, id, &second.lib_id);
    deliver_all(&mut app, newer);
    deliver_all(&mut app, older);

    assert_eq!(doc(&app).symbols[0].lib_id, second.lib_id);
    assert!(logged("a newer switch of it was started"));
}

/// A result for a sheet that was closed and re-opened meanwhile belongs to
/// another engine instance and is dropped.
#[test]
fn a_result_for_a_reopened_sheet_is_dropped() {
    let (mut app, _f, alt, id) = wired_with_alternate(|_| {});
    let messages = start_switch(&mut app, id, &alt.lib_id);
    let before = doc(&app).symbols[0].lib_id.clone();
    let path = app.document_state.active_path.clone().unwrap();
    let document = doc(&app).clone();
    app.document_state.engines.insert(
        path.clone(),
        signex_engine::Engine::new_with_path(document, Some(path)).unwrap(),
    );

    deliver_all(&mut app, messages);

    assert_eq!(doc(&app).symbols[0].lib_id, before);
    assert!(logged(
        "the sheet is no longer active or was reopened; pick again"
    ));
}

/// A symbol left on a drawing its row dropped keeps the Symbol list, with
/// that drawing marked, so the default can still be picked.
#[test]
fn a_stranded_drawing_keeps_the_list_and_the_default_can_be_picked() {
    let f = fixture();
    let alt = add_alternate(&f);
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    let id = select_first(&mut app);
    let default_lib_id = doc(&app).symbols[0].lib_id.clone();
    switch_to(&mut app, id, &alt.lib_id);

    for library in &mut app.library.open_libraries {
        for rows in library.tables.values_mut() {
            for row in rows {
                row.alternate_symbols.clear();
            }
        }
    }
    // Undo drops the selection; reselect, then redo refreshes Properties.
    let _ = app.update(Message::Edit(EditMsg::Undo));
    select_first(&mut app);
    let _ = app.update(Message::Edit(EditMsg::Redo));
    assert_eq!(doc(&app).symbols[0].lib_id, alt.lib_id);
    assert_eq!(
        choices(&app),
        vec![
            ("Default".to_string(), false),
            ("(no longer offered)".to_string(), true)
        ]
    );

    switch_to(&mut app, id, &default_lib_id);
    assert_eq!(doc(&app).symbols[0].lib_id, default_lib_id);
}

/// An alternate whose unconnected pin 3 sits 2.54 mm to the right of the
/// default's.
fn move_pin_x(symbol: &mut signex_library::primitive::Symbol, number: &str) {
    symbol
        .pins
        .iter_mut()
        .find(|p| p.number == number)
        .unwrap()
        .position[0] += 2.54;
}

/// A moved pin that lands on a wire end belonging to something else would
/// merge nets silently: the switch is refused and names the pin.
#[test]
fn a_switch_that_lands_a_pin_on_a_foreign_wire_end_is_refused() {
    use signex_types::schematic::{Point, SymbolTransform, Wire};
    let (mut app, _f, alt, id) = wired_with_alternate(|s| move_pin_x(s, "3"));
    let landing = {
        let sheet = doc(&app);
        let sym = &sheet.symbols[0];
        let pin = sheet.lib_symbols[&sym.lib_id]
            .pins
            .iter()
            .find(|p| p.pin.number == "3")
            .unwrap();
        let tip = SymbolTransform::from_symbol(sym).apply(pin.pin.position);
        Point::new(tip.x + 2.54, tip.y)
    };
    app.document_state
        .active_engine_mut()
        .unwrap()
        .execute(signex_engine::Command::PlaceWireSegment {
            wire: Wire {
                start: landing,
                end: Point::new(landing.x + 10.0, landing.y),
                ..Default::default()
            },
        })
        .unwrap();
    let before = doc(&app).symbols[0].lib_id.clone();

    switch_to(&mut app, id, &alt.lib_id);

    assert_eq!(doc(&app).symbols[0].lib_id, before);
    assert!(
        logged("U1: switching to IEC would connect pin 3; not switched"),
        "the refusal must name the pin that would join the wire"
    );
}

/// Switching unit 2 to a drawing that has nothing for unit 2 would leave the
/// instance drawing nothing: refused.
#[test]
fn a_switch_to_a_drawing_without_the_placed_unit_is_refused() {
    let _ = signex_app::diagnostics::init_logging();
    let f = fixture();
    let alt = add_alternate_shaped(&f, "IEC", |s| {
        for pin in &mut s.pins {
            pin.part_number = 1;
        }
        s.graphics.retain(|g| g.part_number == 1);
    });
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    {
        let engine = app.document_state.active_engine_mut().unwrap();
        let mut d = engine.document().clone();
        d.symbols[0].unit = 2;
        engine.set_document(d);
    }
    let id = select_first(&mut app);
    let before = doc(&app).symbols[0].lib_id.clone();

    switch_to(&mut app, id, &alt.lib_id);

    assert_eq!(doc(&app).symbols[0].lib_id, before);
    assert!(logged(
        "U1: switching to IEC would draw nothing for unit 2; not switched"
    ));
}
