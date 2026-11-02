//! #1867 — a placed part follows its library row: the MPN and manufacturer
//! are stamped at placement, and a row whose footprint (or MPN) moved on
//! shows as drift that "Update from library" applies as one undo step.

use signex_app::app::{EditMsg, Message, Signex};
use signex_erc::RuleKind;
use signex_library::adapter::LibraryAdapter;
use signex_library::adapters::local_git::LocalGitAdapter;
use signex_library::primitive::{Footprint, PrimitiveRef};

use crate::library_place_symbol::{Fixture, app_with_library, doc, fixture, place};
use crate::library_symbol_drift::{check_drift, click_update, is_drifted, select_first};

/// Bind a saved footprint named `name` on the fixture's row.
fn bind_footprint(f: &Fixture, name: &str) {
    let adapter = LocalGitAdapter::open(&f.snxlib).expect("open");
    let footprint = Footprint::empty(name);
    let uuid = footprint.uuid;
    adapter
        .save_footprint(footprint, "seed footprint")
        .expect("save footprint");
    let mut row = adapter
        .read_row("ICs", signex_library::RowId(f.row_id))
        .expect("row");
    row.footprint_ref = Some(PrimitiveRef::new(f.library_id, uuid));
    adapter.update_row("ICs", row, "bind").expect("bind");
}

fn reports(app: &Signex, rule: RuleKind) -> usize {
    signex_erc::run(doc(app))
        .iter()
        .filter(|v| v.rule == rule)
        .count()
}

#[test]
fn placement_stamps_the_row_mpn_and_manufacturer() {
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);

    let fields = &doc(&app).symbols[0].fields;
    assert_eq!(fields.get("MPN").map(String::as_str), Some("TWO-1"));
    assert_eq!(fields.get("Manufacturer").map(String::as_str), Some("Acme"));
    assert_eq!(reports(&app, RuleKind::PartWithoutMpn), 0);
}

#[test]
fn a_part_placed_before_the_footprint_was_bound_updates_in_one_undo_step() {
    let _ = signex_app::diagnostics::init_logging();
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    assert!(doc(&app).symbols[0].footprint.is_empty());
    assert_eq!(reports(&app, RuleKind::PartWithoutFootprint), 1);

    bind_footprint(&f, "SOIC-8");
    let id = select_first(&mut app);
    check_drift(&mut app);
    assert!(is_drifted(&app), "the bound footprint is drift");
    assert!(
        doc(&app).symbols[0].footprint.is_empty(),
        "nothing is rewritten before the user asks"
    );

    click_update(&mut app, id);
    assert_eq!(doc(&app).symbols[0].footprint, "SOIC-8");
    assert_eq!(reports(&app, RuleKind::PartWithoutFootprint), 0);
    assert!(!is_drifted(&app));
    let logged = signex_app::diagnostics::recent_entries()
        .iter()
        .any(|e| e.message.contains("updated from library") && e.message.contains("SOIC-8"));
    assert!(logged, "the change is shown in Messages");

    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert!(
        doc(&app).symbols[0].footprint.is_empty(),
        "one undo restores the part"
    );
    assert_eq!(doc(&app).symbols.len(), 1, "and keeps it placed");
}

#[test]
fn a_row_footprint_change_is_reported_as_drift() {
    let f = fixture();
    bind_footprint(&f, "SOIC-8");
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    select_first(&mut app);
    check_drift(&mut app);
    assert!(!is_drifted(&app), "an unchanged row is not drift");

    bind_footprint(&f, "TSSOP-8");
    check_drift(&mut app);
    assert!(is_drifted(&app));

    let id = doc(&app).symbols[0].uuid;
    click_update(&mut app, id);
    assert_eq!(doc(&app).symbols[0].footprint, "TSSOP-8");
}
