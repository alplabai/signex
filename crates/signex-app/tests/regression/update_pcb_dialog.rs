//! #2371 — the Update PCB dialog's table: net rows of a new board, the
//! summary chips, Show problems only, the group tick, Choose footprint,
//! Space and Enter. Fixture and helpers: `update_pcb.rs`.

use iced::keyboard;
use signex_app::app::{InputMsg, RowAction, RowFilter, TableGroup};
use signex_app::library::messages::{LibraryMessage, PrimitivePickerMsg};
use signex_app::library::state::PrimitivePickerTarget;
use signex_engine::pcb::eco::EcoGroup;
use signex_types::schematic::power_port_value_field;

use super::*;

const R4: Uuid = Uuid::from_u128(0x1312_4004);

fn port(net: &str, x: f64, n: u128) -> Symbol {
    let at = Point::new(x, 0.0);
    let lib_id = format!("power:{net}");
    Symbol {
        uuid: Uuid::from_u128(0x2371_0000 + n),
        lib_id: lib_id.clone(),
        reference: format!("#PWR0{n}"),
        value: net.to_string(),
        position: at,
        unit: 1,
        is_power: true,
        val_text: Some(power_port_value_field(&lib_id, at)),
        ..Default::default()
    }
}

/// R1.1 and R3.1 on VCC, R2.2 and R4.2 on GND; R3 and R4 have no footprint.
fn powered(lib: &Library) -> Vec<Symbol> {
    vec![
        part(lib, R3, "R3", 40.0, false),
        part(lib, R4, "R4", 60.0, false),
        port("VCC", 0.0, 1),
        port("GND", 30.0, 2),
        port("VCC", 40.0, 3),
        port("GND", 70.0, 4),
    ]
}

fn powered_list() -> Fixture {
    let mut f = fixture_with(powered, None, ProjectDefaults::default());
    new_board_and_list(&mut f);
    f
}

fn shown(f: &Fixture) -> Vec<EcoRow> {
    state(f).shown_rows().into_iter().cloned().collect()
}

fn key_press(f: &mut Fixture, named: keyboard::key::Named) {
    let key = keyboard::Key::Named(named);
    let event = keyboard::Event::KeyPressed {
        key: key.clone(),
        modified_key: key,
        physical_key: keyboard::key::Physical::Unidentified(
            keyboard::key::NativeCode::Unidentified,
        ),
        location: keyboard::Location::Standard,
        modifiers: keyboard::Modifiers::default(),
        text: None,
        repeat: false,
    };
    let window = f.app.ui_state.main_window_id.expect("main window");
    send(
        &mut f.app,
        Message::Input(InputMsg::KeyInput { window, event }),
    );
}

/// A new board has no nets, so its net rows only add them, and its pads
/// get their first nets: every row of the list is an Add.
#[test]
fn a_new_board_lists_only_add_rows_for_nets() {
    let f = powered_list();
    let list = list(&f);
    let nets: Vec<&EcoChange> = list
        .rows
        .iter()
        .filter(|r| r.group == EcoGroup::Nets)
        .map(|r| &r.change)
        .collect();
    assert!(
        nets.iter().all(|c| matches!(c, EcoChange::AddNet { .. })),
        "{nets:#?}"
    );
    let names: Vec<&str> = nets
        .iter()
        .filter_map(|c| match c {
            EcoChange::AddNet { name, .. } => Some(name.as_str()),
            _ => None,
        })
        .collect();
    assert!(
        names.contains(&"VCC") && names.contains(&"GND"),
        "{names:?}"
    );
    assert!(
        list.rows
            .iter()
            .all(|r| RowAction::of(&r.change) == RowAction::Add),
        "a new board only gains"
    );
}

/// A summary chip lists only its kind; a second click lists every row.
#[test]
fn a_chip_filters_the_rows() {
    let mut f = powered_list();
    let all = list(&f).rows.len();
    up(
        &mut f.app,
        UpdatePcbMsg::FilterChipClicked(RowFilter::Problems),
    );
    let problems = shown(&f);
    assert_eq!(problems.len(), 2, "R3 and R4 have no footprint");
    assert!(problems.iter().all(|r| r.blocked.is_some()));

    up(
        &mut f.app,
        UpdatePcbMsg::FilterChipClicked(RowFilter::Action(RowAction::Remove)),
    );
    assert!(shown(&f).is_empty(), "a new board removes nothing");

    up(
        &mut f.app,
        UpdatePcbMsg::FilterChipClicked(RowFilter::Action(RowAction::Remove)),
    );
    assert_eq!(shown(&f).len(), all, "the second click clears the filter");
}

/// Show problems only lists only the rows with a problem, under any chip.
#[test]
fn show_problems_only_lists_only_the_problem_rows() {
    let mut f = powered_list();
    up(&mut f.app, UpdatePcbMsg::ProblemsOnlyToggled(true));
    let rows = shown(&f);
    assert!(!rows.is_empty() && rows.iter().all(|r| r.blocked.is_some()));
    assert!(
        !state(&f).is_folded(TableGroup::Components),
        "a group with problems opens itself"
    );
    up(&mut f.app, UpdatePcbMsg::ProblemsOnlyToggled(false));
    assert_eq!(shown(&f).len(), list(&f).rows.len());
}

/// Choose footprint... opens the footprint picker for the part; the pick
/// sets the part's footprint on the schematic as one undo step, and the row
/// re-checks: no longer blocked, ticked.
#[test]
fn choose_footprint_sets_the_parts_footprint_and_the_row_can_be_ticked() {
    let mut f = powered_list();
    let key = add_row(&f, R3).key.clone();
    assert!(matches!(
        add_row(&f, R3).blocked,
        Some(EcoBlock::MissingFootprint(_))
    ));
    up(
        &mut f.app,
        UpdatePcbMsg::ChooseFootprintRequested(key.clone()),
    );
    let picker = f
        .app
        .library
        .primitive_picker
        .as_ref()
        .expect("the footprint picker opened");
    assert!(matches!(
        &picker.target,
        PrimitivePickerTarget::DesignPart { part, .. } if *part == R3
    ));

    let adapter = LocalGitAdapter::open(&f._lib.snxlib).expect("open");
    let footprint = adapter
        .read_row("ICs", signex_library::RowId(f._lib.row_id))
        .expect("row")
        .footprint_ref
        .expect("the row binds a footprint");
    send(
        &mut f.app,
        Message::Library(LibraryMessage::PrimitivePicker(PrimitivePickerMsg::Pick(
            footprint,
        ))),
    );
    assert!(
        f.app.library.primitive_picker.is_none(),
        "the picker closed"
    );
    let r3 = add_row(&f, R3);
    assert!(r3.blocked.is_none(), "{:?}", r3.blocked);
    assert!(r3.on, "the fixed row is ticked");
    up(&mut f.app, UpdatePcbMsg::RowToggled { key, on: false });
    assert!(!add_row(&f, R3).on, "and can be unticked");

    let symbol_footprint = |f: &Fixture| {
        f.app.document_state.engines[&f.sheet]
            .document()
            .symbols
            .iter()
            .find(|s| s.uuid == R3)
            .map(|s| s.footprint.clone())
    };
    let set = symbol_footprint(&f).expect("R3");
    assert!(set.contains(&footprint.uuid.to_string()), "{set}");
    send(&mut f.app, menu_row("undo"));
    assert_eq!(
        symbol_footprint(&f).as_deref(),
        Some(""),
        "one undo takes the choice back"
    );
}

/// The group box shows a mix when some rows are unticked and ticks them all
/// again; Execute applies only the ticked rows, in one undo step.
#[test]
fn a_group_tick_is_tri_state_and_execute_applies_only_ticked_rows() {
    let mut f = fixture();
    new_board_and_list(&mut f);
    let r2 = add_row(&f, R2).key.clone();
    up(
        &mut f.app,
        UpdatePcbMsg::RowToggled {
            key: r2.clone(),
            on: false,
        },
    );
    let ticks = state(&f)
        .group_ticks(TableGroup::Components)
        .expect("a list");
    assert!(ticks.mixed(), "{ticks:?}");
    up(
        &mut f.app,
        UpdatePcbMsg::GroupToggled {
            group: TableGroup::Components,
            on: true,
        },
    );
    assert!(add_row(&f, R2).on, "the group box ticked R2 again");
    up(&mut f.app, UpdatePcbMsg::RowToggled { key: r2, on: false });

    let count = state(&f).execute_count();
    up(&mut f.app, UpdatePcbMsg::ExecuteRequested);
    assert!(f.app.ui_state.update_pcb.is_none(), "{count} rows ran");
    assert_eq!(references(&f), ["R1"], "only the ticked R1 was placed");
    assert_eq!(engine(&f).undo_len(), 1, "one undo step");
}

/// Space ticks the clicked row, Enter executes, Esc closes; Execute is off
/// with nothing ticked.
#[test]
fn space_ticks_the_focused_row_and_enter_executes() {
    let mut f = fixture();
    new_board_and_list(&mut f);
    let r2 = add_row(&f, R2).key.clone();
    up(&mut f.app, UpdatePcbMsg::RowFocused(r2));
    key_press(&mut f, keyboard::key::Named::Space);
    assert!(!add_row(&f, R2).on, "Space unticked R2");

    for group in TableGroup::ALL {
        up(&mut f.app, UpdatePcbMsg::GroupToggled { group, on: false });
    }
    assert_eq!(state(&f).execute_count(), 0);
    key_press(&mut f, keyboard::key::Named::Enter);
    assert!(
        f.app.ui_state.update_pcb.is_some(),
        "Enter does nothing with nothing ticked"
    );

    let r1 = add_row(&f, R1).key.clone();
    up(&mut f.app, UpdatePcbMsg::RowToggled { key: r1, on: true });
    key_press(&mut f, keyboard::key::Named::Enter);
    assert!(f.app.ui_state.update_pcb.is_none(), "Enter executed");
    assert_eq!(references(&f), ["R1"]);
}
