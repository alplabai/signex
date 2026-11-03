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

fn lib_id(lib: Uuid, sym: Uuid) -> String {
    format!("[{lib}:{sym}]")
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

    let scan = scan_closed_sheets(&[hit, other], SymbolMatch::InLibrary(lib, sym));

    assert_eq!(scan.placed, ["a.snxsch: U1"]);
    assert!(scan.unreadable.is_empty());
}

#[test]
fn a_symbol_outside_any_library_matches_by_uuid_in_any_library() {
    let dir = tempfile::tempdir().unwrap();
    let sym = Uuid::now_v7();
    let path = write_sheet(dir.path(), "a.snxsch", &lib_id(Uuid::now_v7(), sym), "U1");

    let scan = scan_closed_sheets(&[path], SymbolMatch::AnyLibrary(sym));

    assert_eq!(scan.placed, ["a.snxsch: U1"]);
}

#[test]
fn a_sheet_that_cannot_be_read_is_unchecked_not_empty() {
    let dir = tempfile::tempdir().unwrap();
    let garbage = dir.path().join("bad.snxsch");
    std::fs::write(&garbage, "not a sheet").unwrap();
    let missing = dir.path().join("gone.snxsch");

    let scan = scan_closed_sheets(&[garbage, missing], SymbolMatch::AnyLibrary(Uuid::now_v7()));

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
