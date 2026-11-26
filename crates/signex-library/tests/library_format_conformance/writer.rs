//! The bulk `LibraryWriter` (#3110) writes the fixture library byte for byte.

use super::*;

use signex_library::writer::LibraryWriter;

#[test]
fn the_library_writer_reproduces_the_fixture_library() {
    if skip_while_blessing() {
        return;
    }
    let lib = library_file();
    let mut writer = LibraryWriter::new("Conformance", lib.manifest.clone());
    let rows = rows();
    for (table, row) in [
        ("resistors", &rows[1]),
        ("resistors", &rows[0]),
        ("logic", &rows[2]),
    ] {
        writer.add_row(table, row.clone());
    }
    for (name, table) in &lib.tables {
        let draft = writer.table_mut(name);
        draft.column_types = table.column_types.clone();
        draft.pricing = table.pricing.clone();
    }
    let envelope = writer.add_symbol_file(symbol_envelope());
    assert_eq!(format!("symbols/{envelope}"), SYMBOL_ENVELOPE);
    for (fp, file) in footprints() {
        writer
            .add_footprint_file(footprint_envelope(fp, file))
            .unwrap();
    }

    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Conformance");
    let snxlib = writer.write(&root).expect("write the fixture library");
    assert_eq!(snxlib, root.join(SNXLIB_NAME));

    let mut written = BTreeMap::new();
    collect(&root, &root, &mut written);
    let committed: BTreeMap<String, String> = committed()
        .into_iter()
        .filter_map(|(rel, text)| Some((rel.strip_prefix("library/")?.to_string(), text)))
        .collect();
    assert_eq!(
        written.keys().collect::<Vec<_>>(),
        committed.keys().collect::<Vec<_>>(),
        "the writer's file set differs from the fixture library"
    );
    for (rel, text) in &committed {
        assert_same(rel, text, &written[rel]);
    }
}
