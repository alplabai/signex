use super::*;
use signex_types::format::SnxSchematic;
use signex_types::schematic::{SchematicSheet, Symbol};

fn write_sheet(dir: &Path, name: &str, lib_id: &str, reference: &str) -> PathBuf {
    let mut sheet = SchematicSheet::default();
    sheet.symbols.push(Symbol {
        lib_id: lib_id.into(),
        reference: reference.into(),
        ..Default::default()
    });
    let path = dir.join(name);
    std::fs::write(&path, SnxSchematic::new(sheet).write_string().unwrap()).unwrap();
    path
}

fn plan(seeds: &[PathBuf]) -> ScanPlan {
    ScanPlan {
        seeds: seeds
            .iter()
            .map(|p| (p.clone(), CaseFold::host_default()))
            .collect(),
        ..Default::default()
    }
}

/// A sheet that places `lib_id` (if any) and references `children`.
fn write_parent(dir: &Path, name: &str, children: &[&str], lib_id: Option<&str>) -> PathBuf {
    let mut sheet = SchematicSheet::default();
    if let Some(id) = lib_id {
        sheet.symbols.push(Symbol {
            lib_id: id.into(),
            reference: "U1".into(),
            ..Default::default()
        });
    }
    for child in children {
        sheet
            .child_sheets
            .push(signex_types::schematic::ChildSheet {
                uuid: Uuid::now_v7(),
                name: (*child).into(),
                filename: (*child).into(),
                position: signex_types::schematic::Point::new(0.0, 0.0),
                size: (10.0, 10.0),
                stroke_width: 0.0,
                fill: Default::default(),
                stroke_color: None,
                fill_color: None,
                fields_autoplaced: false,
                pins: Vec::new(),
                instances: Vec::new(),
            });
    }
    let path = dir.join(name);
    std::fs::write(&path, SnxSchematic::new(sheet).write_string().unwrap()).unwrap();
    path
}

fn lib_id(lib: Uuid, sym: Uuid) -> String {
    format!("{lib}:{sym}")
}

#[test]
fn sheets_that_place_the_symbol_are_named_by_file_and_reference() {
    let dir = tempfile::tempdir().unwrap();
    let (lib, sym) = (Uuid::now_v7(), Uuid::now_v7());
    let hit = write_sheet(
        dir.path(),
        "a.snxsch",
        &lib_id(lib, sym).to_uppercase(),
        "U1",
    );
    let other = write_sheet(dir.path(), "b.snxsch", &lib_id(lib, Uuid::now_v7()), "U2");

    let scan = scan_closed_sheets(&plan(&[hit, other]), SymbolMatch::InLibrary(lib, sym));

    assert_eq!(scan.placed, ["a.snxsch: U1"]);
    assert!(scan.unreadable.is_empty());
}

#[test]
fn a_symbol_outside_any_library_matches_by_uuid_in_any_library() {
    let dir = tempfile::tempdir().unwrap();
    let sym = Uuid::now_v7();
    let path = write_sheet(dir.path(), "a.snxsch", &lib_id(Uuid::now_v7(), sym), "U1");

    let scan = scan_closed_sheets(&plan(&[path]), SymbolMatch::AnyLibrary(sym));

    assert_eq!(scan.placed, ["a.snxsch: U1"]);
}

#[test]
fn a_sheet_that_cannot_be_read_is_unchecked_not_empty() {
    let dir = tempfile::tempdir().unwrap();
    let garbage = dir.path().join("bad.snxsch");
    std::fs::write(&garbage, "not a sheet").unwrap();
    let missing = dir.path().join("gone.snxsch");

    let scan = scan_closed_sheets(
        &plan(&[garbage, missing]),
        SymbolMatch::AnyLibrary(Uuid::now_v7()),
    );

    assert!(scan.placed.is_empty());
    assert_eq!(scan.unreadable.len(), 2, "{:?}", scan.unreadable);
    assert!(
        scan.unreadable[0].starts_with("bad.snxsch"),
        "{:?}",
        scan.unreadable
    );
    assert!(
        scan.unreadable[1].contains("gone.snxsch (file not found)"),
        "{:?}",
        scan.unreadable
    );
}

#[test]
fn a_child_reached_only_through_child_sheets_is_scanned() {
    let dir = tempfile::tempdir().unwrap();
    let (lib, sym) = (Uuid::now_v7(), Uuid::now_v7());
    // root -> mid -> leaf, and leaf points back at root (a cycle).
    let root = write_parent(dir.path(), "root.snxsch", &["mid.snxsch"], None);
    write_parent(dir.path(), "mid.snxsch", &["leaf.snxsch"], None);
    write_parent(
        dir.path(),
        "leaf.snxsch",
        &["root.snxsch"],
        Some(&lib_id(lib, sym)),
    );

    let scan = scan_closed_sheets(&plan(&[root]), SymbolMatch::InLibrary(lib, sym));

    assert_eq!(scan.placed, ["leaf.snxsch: U1"]);
    assert!(scan.unreadable.is_empty(), "{:?}", scan.unreadable);
}

#[test]
fn an_absent_or_escaping_child_is_unchecked() {
    let dir = tempfile::tempdir().unwrap();
    let root = write_parent(
        dir.path(),
        "root.snxsch",
        &["gone.snxsch", "../outside.snxsch"],
        None,
    );

    let scan = scan_closed_sheets(&plan(&[root]), SymbolMatch::AnyLibrary(Uuid::now_v7()));

    let text = scan.unreadable.join("\n");
    assert!(text.contains("gone.snxsch (file not found)"), "{text}");
    assert!(text.contains("outside its parent sheet's folder"), "{text}");
}

#[test]
fn an_open_sheet_is_left_to_memory() {
    let dir = tempfile::tempdir().unwrap();
    let (lib, sym) = (Uuid::now_v7(), Uuid::now_v7());
    let open = write_parent(dir.path(), "open.snxsch", &[], Some(&lib_id(lib, sym)));
    let mut plan = plan(std::slice::from_ref(&open));
    plan.open = vec![open];

    let scan = scan_closed_sheets(&plan, SymbolMatch::InLibrary(lib, sym));

    assert!(scan.placed.is_empty() && scan.unreadable.is_empty());
}
