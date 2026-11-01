//! #1842 — the footprint bound on a library row reaches the placed symbol,
//! so ERC, the netlist and the BOM (which read the symbol's own footprint
//! field) see it.

use signex_app::library::place::resolve_placement;
use signex_erc::RuleKind;
use signex_library::adapter::LibraryAdapter;
use signex_library::adapters::local_git::LocalGitAdapter;
use signex_library::primitive::{Footprint, PrimitiveRef};

use crate::library_place_symbol::{
    Fixture, PLACE_AT, app_with_library, click, complete, doc, fixture,
};

const FOOTPRINT: &str = "SOIC-8";

/// Bind a saved footprint named `FOOTPRINT` on the fixture's row.
fn bind_footprint(f: &Fixture) {
    let adapter = LocalGitAdapter::open(&f.snxlib).expect("open");
    let footprint = Footprint::empty(FOOTPRINT);
    let uuid = footprint.uuid;
    adapter
        .save_footprint(footprint, "seed footprint")
        .expect("save footprint");
    let id = signex_library::RowId(f.row_id);
    let mut row = adapter.read_row("ICs", id).expect("row");
    row.footprint_ref = Some(PrimitiveRef::new(f.library_id, uuid));
    adapter
        .update_row("ICs", row, "bind footprint")
        .expect("bind");
}

fn place(f: &Fixture) -> signex_types::schematic::SchematicSheet {
    let mut app = app_with_library(f);
    let open = [(f.library_id, f.snxlib.clone())];
    let resolved = resolve_placement(&f.snxlib, "ICs", signex_library::RowId(f.row_id), &open);
    complete(&mut app, f, "place-364.snxsch", resolved.map(Box::new));
    click(&mut app, PLACE_AT);
    doc(&app).clone()
}

fn missing_footprint_reports(sheet: &signex_types::schematic::SchematicSheet) -> usize {
    signex_erc::run(sheet)
        .iter()
        .filter(|v| v.rule == RuleKind::PartWithoutFootprint)
        .count()
}

#[test]
fn a_part_placed_from_a_row_with_a_footprint_carries_it() {
    let f = fixture();
    bind_footprint(&f);

    let sheet = place(&f);

    assert_eq!(sheet.symbols[0].footprint, FOOTPRINT);
    assert_eq!(
        missing_footprint_reports(&sheet),
        0,
        "ERC sees the footprint"
    );
}

#[test]
fn a_part_placed_from_a_row_without_one_is_still_reported() {
    let f = fixture();

    let sheet = place(&f);

    assert!(sheet.symbols[0].footprint.is_empty());
    assert_eq!(missing_footprint_reports(&sheet), 1);
}
