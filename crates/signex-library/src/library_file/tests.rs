//! Unit tests for the .snxlib library-file codec.
use super::*;

fn fixture_manifest() -> SnxlibManifest {
    SnxlibManifest {
        format: FORMAT_TOKEN.into(),
        library_id: Uuid::parse_str("0192a8c0-0000-7000-8000-000000000000").unwrap(),
        library: LibrarySection {
            name: "MyLib".into(),
            description: Some("test library".into()),
        },
        mode: LibraryMode::default(),
        workflow: WorkflowConfig::default(),
        users: UsersConfig::default(),
        classes: Vec::new(),
    }
}

fn make_row(columns: &[&str], values: &[&str]) -> LibraryRow {
    let mut row = LibraryRow::default();
    for (col, val) in columns.iter().zip(values) {
        row.cells.insert((*col).to_string(), (*val).to_string());
    }
    row
}

fn fixture_table_resistors() -> LibraryTable {
    let columns = vec![
        "row_id".to_string(),
        "internal_pn".to_string(),
        "mpn".to_string(),
        "manufacturer".to_string(),
        "value".to_string(),
        "package".to_string(),
    ];
    let cols_ref: Vec<&str> = columns.iter().map(String::as_str).collect();
    let rows = vec![
        make_row(
            &cols_ref,
            &["r1uuid", "R-001", "RC0603FR-0710KL", "Yageo", "10k", "0603"],
        ),
        make_row(
            &cols_ref,
            &["r2uuid", "R-002", "RC0603FR-074K7L", "Yageo", "4k7", "0603"],
        ),
    ];
    LibraryTable {
        columns,
        rows,
        column_types: BTreeMap::new(),
    }
}

/// The foundational round-trip — a library with no tables still
/// preserves the manifest header byte-equal under `parse(write())`.
#[test]
fn round_trip_no_tables() {
    let lib = LibraryFile {
        manifest: fixture_manifest(),
        tables: BTreeMap::new(),
    };
    let s = lib.write().unwrap();
    let back = LibraryFile::parse(&s).unwrap();
    assert_eq!(lib, back);
}

/// Single-table round-trip with the columns + rows from the plan
/// example.
#[test]
fn round_trip_single_table() {
    let mut tables = BTreeMap::new();
    tables.insert("resistors".to_string(), fixture_table_resistors());
    let lib = LibraryFile {
        manifest: fixture_manifest(),
        tables,
    };
    let s = lib.write().unwrap();
    let back = LibraryFile::parse(&s).unwrap();
    assert_eq!(lib, back);
}

/// Two tables with *different* column schemas — proves columns are
/// user-defined per table, not enforced library-wide.
#[test]
fn round_trip_multiple_tables_user_defined_columns() {
    let mut tables = BTreeMap::new();
    tables.insert("resistors".to_string(), fixture_table_resistors());

    let regulator_columns = vec![
        "row_id".to_string(),
        "mpn".to_string(),
        "manufacturer".to_string(),
        "package".to_string(),
        "iout_max_a".to_string(),
    ];
    let cols_ref: Vec<&str> = regulator_columns.iter().map(String::as_str).collect();
    let rows = vec![make_row(
        &cols_ref,
        &["u1uuid", "LM7805", "TI", "TO-220", "1.5"],
    )];
    tables.insert(
        "regulators_5v".to_string(),
        LibraryTable {
            columns: regulator_columns,
            rows,
            column_types: BTreeMap::new(),
        },
    );

    let lib = LibraryFile {
        manifest: fixture_manifest(),
        tables,
    };
    let s = lib.write().unwrap();
    let back = LibraryFile::parse(&s).unwrap();
    assert_eq!(lib, back);
}

/// Idempotence — two consecutive writes produce byte-equal output.
#[test]
fn write_is_byte_idempotent_under_round_trip() {
    let mut tables = BTreeMap::new();
    tables.insert("resistors".to_string(), fixture_table_resistors());
    let lib = LibraryFile {
        manifest: fixture_manifest(),
        tables,
    };
    let first = lib.write().unwrap();
    let parsed = LibraryFile::parse(&first).unwrap();
    let second = parsed.write().unwrap();
    assert_eq!(first, second);
}

/// Format token mismatches are loud (rather than silently parsing as
/// a current-format file with missing fields).
#[test]
fn parse_rejects_unsupported_format_token() {
    let s = r#"format = "snxlib/0"
library_id = "0192a8c0-0000-7000-8000-000000000000"

[library]
name = "X"
"#;
    let err = LibraryFile::parse(s).unwrap_err();
    assert!(matches!(err, LibraryFileError::UnsupportedFormat { .. }));
}

#[test]
fn parse_rejects_duplicate_columns() {
    let s = r#"format = "snxlib/1"
library_id = "0192a8c0-0000-7000-8000-000000000000"

[library]
name = "X"

[tables.dup]
tsv = '''
row_id	mpn	mpn
1	a	b
'''
"#;
    let err = LibraryFile::parse(s).unwrap_err();
    assert!(matches!(err, LibraryFileError::DuplicateColumn { .. }));
}

#[test]
fn parse_rejects_cell_count_mismatch() {
    let s = r#"format = "snxlib/1"
library_id = "0192a8c0-0000-7000-8000-000000000000"

[library]
name = "X"

[tables.bad]
tsv = '''
row_id	mpn	manufacturer
only_two_cells	missing
'''
"#;
    let err = LibraryFile::parse(s).unwrap_err();
    assert!(matches!(err, LibraryFileError::CellCountMismatch { .. }));
}

/// Header-only TSV (no body rows) parses to an empty `rows` vec.
/// This is the shape an empty user-created table has on disk
/// before any rows are added.
#[test]
fn header_only_tsv_is_empty_rows() {
    let s = r#"format = "snxlib/1"
library_id = "0192a8c0-0000-7000-8000-000000000000"

[library]
name = "X"

[tables.empty]
tsv = '''
row_id	mpn
'''
"#;
    let lib = LibraryFile::parse(s).unwrap();
    let t = &lib.tables["empty"];
    assert_eq!(t.columns, vec!["row_id", "mpn"]);
    assert!(t.rows.is_empty());
}

/// Empty TSV (no header at all) is rejected — there's no schema to
/// hang the table off.
#[test]
fn parse_rejects_empty_tsv() {
    let s = r#"format = "snxlib/1"
library_id = "0192a8c0-0000-7000-8000-000000000000"

[library]
name = "X"

[tables.empty]
tsv = '''
'''
"#;
    let err = LibraryFile::parse(s).unwrap_err();
    assert!(matches!(err, LibraryFileError::EmptyTable { .. }));
}

#[test]
fn write_rejects_cell_with_tab() {
    let mut tables = BTreeMap::new();
    tables.insert(
        "t".into(),
        LibraryTable {
            columns: vec!["a".into()],
            rows: vec![make_row(&["a"], &["has\ttab"])],
            column_types: BTreeMap::new(),
        },
    );
    let lib = LibraryFile {
        manifest: fixture_manifest(),
        tables,
    };
    let err = lib.write().unwrap_err();
    assert!(matches!(
        err,
        LibraryFileError::DisallowedControlInCell { .. }
    ));
}

#[test]
fn write_rejects_cell_with_newline() {
    let mut tables = BTreeMap::new();
    tables.insert(
        "t".into(),
        LibraryTable {
            columns: vec!["a".into()],
            rows: vec![make_row(&["a"], &["has\nnewline"])],
            column_types: BTreeMap::new(),
        },
    );
    let lib = LibraryFile {
        manifest: fixture_manifest(),
        tables,
    };
    let err = lib.write().unwrap_err();
    assert!(matches!(
        err,
        LibraryFileError::DisallowedControlInCell { .. }
    ));
}

#[test]
fn write_rejects_cell_with_triple_single_quote() {
    let mut tables = BTreeMap::new();
    tables.insert(
        "t".into(),
        LibraryTable {
            columns: vec!["a".into()],
            rows: vec![make_row(&["a"], &["trip''' quoted"])],
            column_types: BTreeMap::new(),
        },
    );
    let lib = LibraryFile {
        manifest: fixture_manifest(),
        tables,
    };
    let err = lib.write().unwrap_err();
    assert!(matches!(
        err,
        LibraryFileError::DisallowedTripleQuoteInCell { .. }
    ));
}

#[test]
fn write_rejects_column_name_with_tab() {
    let mut tables = BTreeMap::new();
    tables.insert(
        "t".into(),
        LibraryTable {
            columns: vec!["bad\tcol".into()],
            rows: vec![],
            column_types: BTreeMap::new(),
        },
    );
    let lib = LibraryFile {
        manifest: fixture_manifest(),
        tables,
    };
    let err = lib.write().unwrap_err();
    assert!(matches!(
        err,
        LibraryFileError::DisallowedControlInColumn { .. }
    ));
}

/// A direct port of the §1 plan example. Anchors the format against
/// the spec doc so a future schema change has to update both.
#[test]
fn parse_example_from_plan() {
    let s = "format = \"snxlib/1\"\n\
                 library_id = \"0192a8c0-0000-7000-8000-000000000123\"\n\
                 \n\
                 [library]\n\
                 name = \"Loratis-SN-lib\"\n\
                 description = \"\"\n\
                 \n\
                 [tables.resistors]\n\
                 tsv = '''\n\
                 internal_pn\tmpn\tmanufacturer\tvalue\ttolerance\tpackage\n\
                 R-001\tRC0603FR-0710KL\tYageo\t10k\t1%\t0603\n\
                 R-002\tRC0603FR-074K7L\tYageo\t4k7\t1%\t0603\n\
                 '''\n\
                 \n\
                 [tables.regulators_5v]\n\
                 tsv = '''\n\
                 internal_pn\tmpn\tmanufacturer\tpackage\tiout_max_a\n\
                 LIB-005\tLM7805\tTI\tTO-220\t1.5\n\
                 LIB-006\tLM317\tTI\tTO-220\t1.5\n\
                 '''\n";
    let lib = LibraryFile::parse(s).unwrap();
    assert_eq!(lib.manifest.library.name, "Loratis-SN-lib");
    assert_eq!(lib.tables.len(), 2);
    let r = &lib.tables["resistors"];
    assert_eq!(
        r.columns,
        vec![
            "internal_pn",
            "mpn",
            "manufacturer",
            "value",
            "tolerance",
            "package"
        ]
    );
    assert_eq!(r.rows.len(), 2);
    assert_eq!(r.rows[0].cells["mpn"], "RC0603FR-0710KL");
    assert_eq!(r.rows[1].cells["value"], "4k7");

    let u = &lib.tables["regulators_5v"];
    assert_eq!(
        u.columns,
        vec![
            "internal_pn",
            "mpn",
            "manufacturer",
            "package",
            "iout_max_a"
        ]
    );
    assert_eq!(u.rows[0].cells["mpn"], "LM7805");
}

/// `LibraryTable::cell` returns `None` for out-of-schema columns even
/// when the row's `cells` map has an entry — protects against silent
/// schema-drift bugs where a row is written under a column the table
/// doesn't declare.
#[test]
fn cell_accessor_is_schema_scoped() {
    let table = LibraryTable {
        columns: vec!["a".into()],
        rows: vec![],
        column_types: BTreeMap::new(),
    };
    let mut row = LibraryRow::default();
    row.cells.insert("a".into(), "alpha".into());
    row.cells
        .insert("ghost".into(), "should not surface".into());
    assert_eq!(table.cell(&row, "a"), Some("alpha"));
    assert_eq!(table.cell(&row, "ghost"), None);
}

/// Empty cells preserve through round-trip — common in optional
/// columns like `description` or `tags`.
#[test]
fn empty_cells_round_trip() {
    let columns = vec!["mpn".to_string(), "tags".to_string()];
    let rows = vec![
        make_row(&["mpn", "tags"], &["LM317", ""]),
        make_row(&["mpn", "tags"], &["", "automotive"]),
    ];
    let mut tables = BTreeMap::new();
    tables.insert(
        "misc".to_string(),
        LibraryTable {
            columns,
            rows,
            column_types: BTreeMap::new(),
        },
    );
    let lib = LibraryFile {
        manifest: fixture_manifest(),
        tables,
    };
    let s = lib.write().unwrap();
    let back = LibraryFile::parse(&s).unwrap();
    assert_eq!(lib, back);
}

/// Canonical row order on write — rows must emit sorted by `row_id`
/// regardless of in-memory `rows` Vec order, so on-disk file layout
/// is stable across UI sort changes / bulk insertion order. This is
/// the load-bearing fix for the "git blame breaks when rows are
/// reordered" failure mode the architecture critique called out.
#[test]
fn write_emits_canonical_row_order_by_row_id() {
    let columns = vec!["row_id".to_string(), "mpn".to_string()];
    // Insert rows in REVERSE row_id order — write should still emit
    // them sorted ascending.
    let rows = vec![
        make_row(&["row_id", "mpn"], &["zzzz", "Late"]),
        make_row(&["row_id", "mpn"], &["mmmm", "Mid"]),
        make_row(&["row_id", "mpn"], &["aaaa", "Early"]),
    ];
    let mut tables = BTreeMap::new();
    tables.insert(
        "items".to_string(),
        LibraryTable {
            columns,
            rows,
            column_types: BTreeMap::new(),
        },
    );
    let lib = LibraryFile {
        manifest: fixture_manifest(),
        tables,
    };
    let s = lib.write().unwrap();

    // The TSV body should list rows alphabetically by row_id.
    let expected_order = ["aaaa\tEarly", "mmmm\tMid", "zzzz\tLate"];
    let aaaa_pos = s.find("aaaa\tEarly").expect("aaaa row");
    let mmmm_pos = s.find("mmmm\tMid").expect("mmmm row");
    let zzzz_pos = s.find("zzzz\tLate").expect("zzzz row");
    assert!(
        aaaa_pos < mmmm_pos && mmmm_pos < zzzz_pos,
        "expected canonical order {:?} but got: {s}",
        expected_order
    );

    // And the round-trip's rows are in canonical order too.
    let back = LibraryFile::parse(&s).unwrap();
    let row_ids: Vec<&str> = back.tables["items"]
        .rows
        .iter()
        .map(|r| r.cells["row_id"].as_str())
        .collect();
    assert_eq!(row_ids, vec!["aaaa", "mmmm", "zzzz"]);
}

/// Tables without a `row_id` column preserve insertion order — no
/// canonical key to sort by, so we don't reshuffle the user's
/// arbitrary lookup tables.
#[test]
fn write_preserves_insertion_order_when_no_row_id() {
    let columns = vec!["key".to_string(), "value".to_string()];
    let rows = vec![
        make_row(&["key", "value"], &["zebra", "Z"]),
        make_row(&["key", "value"], &["apple", "A"]),
    ];
    let mut tables = BTreeMap::new();
    tables.insert(
        "lookups".to_string(),
        LibraryTable {
            columns,
            rows,
            column_types: BTreeMap::new(),
        },
    );
    let lib = LibraryFile {
        manifest: fixture_manifest(),
        tables,
    };
    let s = lib.write().unwrap();
    let zebra_pos = s.find("zebra\tZ").unwrap();
    let apple_pos = s.find("apple\tA").unwrap();
    assert!(zebra_pos < apple_pos, "expected insertion order kept");
}

/// `column_types` map round-trips through the
/// `[tables.<name>.column_types]` sidecar — the type tokens
/// (`number`, `bool`, `enum:active,preferred,...`) survive parse +
/// write unchanged.
#[test]
fn column_types_round_trip() {
    let columns = vec![
        "row_id".to_string(),
        "mpn".to_string(),
        "rated_power_mw".to_string(),
        "released".to_string(),
        "lifecycle".to_string(),
    ];
    let rows = vec![make_row(
        &["row_id", "mpn", "rated_power_mw", "released", "lifecycle"],
        &["abc-123", "RC0603", "100", "true", "preferred"],
    )];
    let mut column_types = BTreeMap::new();
    column_types.insert("row_id".into(), ColumnType::Uuid);
    column_types.insert("rated_power_mw".into(), ColumnType::Number);
    column_types.insert("released".into(), ColumnType::Bool);
    column_types.insert(
        "lifecycle".into(),
        ColumnType::Enum(vec![
            "active".into(),
            "preferred".into(),
            "deprecated".into(),
            "obsolete".into(),
        ]),
    );

    let mut tables = BTreeMap::new();
    tables.insert(
        "resistors".into(),
        LibraryTable {
            columns,
            rows,
            column_types,
        },
    );
    let lib = LibraryFile {
        manifest: fixture_manifest(),
        tables,
    };
    let s = lib.write().unwrap();
    let back = LibraryFile::parse(&s).unwrap();
    assert_eq!(lib, back);
}

/// Untyped tables round-trip without emitting an empty
/// `[tables.<name>.column_types]` block — keeps the `.snxlib`
/// clean for the common "user just made a quick lookup table" case.
#[test]
fn untyped_tables_skip_column_types_block() {
    let mut tables = BTreeMap::new();
    tables.insert("resistors".to_string(), fixture_table_resistors());
    let lib = LibraryFile {
        manifest: fixture_manifest(),
        tables,
    };
    let s = lib.write().unwrap();
    assert!(
        !s.contains("column_types"),
        "untyped tables must not emit a column_types block; got:\n{s}"
    );
    let back = LibraryFile::parse(&s).unwrap();
    assert_eq!(lib, back);
}

/// `column_types` with a key that doesn't exist in the TSV header
/// is loud — silently rendering dropdowns for ghost columns would
/// be confusing.
#[test]
fn parse_rejects_column_type_for_unknown_column() {
    let s = "format = \"snxlib/1\"\n\
                 library_id = \"0192a8c0-0000-7000-8000-000000000000\"\n\
                 \n\
                 [library]\n\
                 name = \"X\"\n\
                 \n\
                 [tables.t]\n\
                 tsv = '''\n\
                 row_id\tmpn\n\
                 abc\tLM317\n\
                 '''\n\
                 \n\
                 [tables.t.column_types]\n\
                 row_id = \"uuid\"\n\
                 ghost_col = \"number\"\n";
    let err = LibraryFile::parse(s).unwrap_err();
    assert!(matches!(
        err,
        LibraryFileError::ColumnTypeForUnknownColumn { .. }
    ));
}

/// Unknown type tokens fail loudly rather than silently treating
/// the column as `string`.
#[test]
fn parse_rejects_unknown_type_token() {
    let s = "format = \"snxlib/1\"\n\
                 library_id = \"0192a8c0-0000-7000-8000-000000000000\"\n\
                 \n\
                 [library]\n\
                 name = \"X\"\n\
                 \n\
                 [tables.t]\n\
                 tsv = '''\n\
                 row_id\tmpn\n\
                 abc\tLM317\n\
                 '''\n\
                 \n\
                 [tables.t.column_types]\n\
                 row_id = \"banana\"\n";
    let err = LibraryFile::parse(s).unwrap_err();
    // Wrapped through serde, so the inner error is TomlDe.
    assert!(matches!(err, LibraryFileError::TomlDe(_)));
}

/// `ColumnType::Enum` requires at least one non-empty value —
/// `enum:` (empty) or `enum:,` (only blanks) are rejected.
#[test]
fn column_type_parse_rejects_empty_enum() {
    let err = ColumnType::parse_token("enum:").unwrap_err();
    assert!(matches!(err, ColumnTypeParseError::EmptyEnum));
    let err = ColumnType::parse_token("enum:,,").unwrap_err();
    assert!(matches!(err, ColumnTypeParseError::EmptyEnum));
}

/// Token-level round-trip for every variant — paranoia test that
/// every enum variant survives `to_token` → `parse_token`.
#[test]
fn column_type_token_round_trip_all_variants() {
    let cases = [
        ColumnType::String,
        ColumnType::Number,
        ColumnType::Int,
        ColumnType::Bool,
        ColumnType::Uuid,
        ColumnType::Date,
        ColumnType::DateTime,
        ColumnType::Url,
        ColumnType::Version,
        ColumnType::Tags,
        ColumnType::Enum(vec!["a".into(), "b".into(), "c".into()]),
    ];
    for t in cases {
        let token = t.to_token();
        let back = ColumnType::parse_token(&token).unwrap();
        assert_eq!(t, back, "round-trip failed for {:?}", t);
    }
}

/// Column order from the header is preserved through round-trip.
#[test]
fn column_order_preserved() {
    let columns = vec![
        "z_col".to_string(),
        "a_col".to_string(),
        "m_col".to_string(),
    ];
    let rows = vec![make_row(&["z_col", "a_col", "m_col"], &["1", "2", "3"])];
    let mut tables = BTreeMap::new();
    tables.insert(
        "ordered".to_string(),
        LibraryTable {
            columns: columns.clone(),
            rows,
            column_types: BTreeMap::new(),
        },
    );
    let lib = LibraryFile {
        manifest: fixture_manifest(),
        tables,
    };
    let s = lib.write().unwrap();
    let back = LibraryFile::parse(&s).unwrap();
    assert_eq!(back.tables["ordered"].columns, columns);
}
