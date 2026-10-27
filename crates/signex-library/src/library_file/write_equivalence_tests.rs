//! #416: the table writer no longer copies every cell, nor looks a row's id
//! up on every sort comparison, and the cell check scans bytes. These pin
//! its output, byte for byte, to the encoder it replaced.
use super::*;

/// The encoder before #416, kept verbatim as the reference.
fn reference_tsv(table: &LibraryTable) -> String {
    let mut out = String::new();
    out.push_str(&table.columns.join("\t"));
    out.push('\n');
    let has_row_id = table.columns.iter().any(|c| c == "row_id");
    let mut order: Vec<usize> = (0..table.rows.len()).collect();
    if has_row_id {
        order.sort_by(|&a, &b| {
            let ra = table.rows[a].cells.get("row_id").map(String::as_str);
            let rb = table.rows[b].cells.get("row_id").map(String::as_str);
            ra.unwrap_or("").cmp(rb.unwrap_or(""))
        });
    }
    for &i in &order {
        let row = &table.rows[i];
        let cells: Vec<String> = table
            .columns
            .iter()
            .map(|c| row.cells.get(c).cloned().unwrap_or_default())
            .collect();
        out.push_str(&cells.join("\t"));
        out.push('\n');
    }
    out
}

fn row(cells: &[(&str, &str)]) -> Arc<LibraryRow> {
    Arc::new(LibraryRow {
        cells: cells
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect(),
    })
}

fn table(columns: &[&str], rows: Vec<Arc<LibraryRow>>) -> LibraryTable {
    LibraryTable {
        columns: columns.iter().map(|c| (*c).to_string()).collect(),
        rows,
        ..LibraryTable::default()
    }
}

fn library(tables: Vec<(&str, LibraryTable)>) -> LibraryFile {
    LibraryFile {
        manifest: SnxlibManifest {
            format: FORMAT_TOKEN.into(),
            library_id: Uuid::parse_str("0192a8c0-0000-7000-8000-000000000000").unwrap(),
            library: LibrarySection {
                name: "Eq".into(),
                description: None,
            },
            mode: LibraryMode::default(),
            workflow: WorkflowConfig::default(),
            users: UsersConfig::default(),
            classes: Vec::new(),
        },
        tables: tables
            .into_iter()
            .map(|(n, t)| (n.to_string(), t))
            .collect(),
    }
}

/// The TSV block of table `name` in a written document.
fn written_block<'a>(doc: &'a str, name: &str) -> &'a str {
    let start = doc.find(&format!("[tables.{name}]\ntsv = '''\n")).unwrap();
    let body = &doc[start..];
    let open = body.find("'''\n").unwrap() + 4;
    let close = body[open..].find("'''").unwrap();
    &body[open..open + close]
}

#[test]
fn written_rows_match_the_reference_encoder_byte_for_byte() {
    // Header not in name order, rows not in id order, a repeated id (stable
    // order), absent and undeclared cells, non-ASCII text.
    let sorted = table(
        &["zeta", "row_id", "alpha", "mpn"],
        vec![
            row(&[("row_id", "c"), ("zeta", "z1"), ("mpn", "Ω-10k")]),
            row(&[("row_id", "a"), ("alpha", "first"), ("extra", "dropped")]),
            row(&[("row_id", "b"), ("alpha", ""), ("mpn", "日本")]),
            row(&[("row_id", "a"), ("alpha", "second"), ("zeta", "µ")]),
            row(&[("aaa", "undeclared"), ("zzz", "undeclared")]),
        ],
    );
    // No `row_id` column: insertion order is kept.
    let unsorted = table(
        &["name", "value"],
        vec![row(&[("name", "y"), ("value", "2")]), row(&[("name", "x")])],
    );
    let expected = [
        ("sorted", reference_tsv(&sorted)),
        ("unsorted", reference_tsv(&unsorted)),
    ];
    let doc = library(vec![("sorted", sorted), ("unsorted", unsorted)])
        .write()
        .unwrap();
    for (name, tsv) in &expected {
        assert_eq!(written_block(&doc, name), tsv.as_str(), "table {name}");
    }
}

#[test]
fn a_control_character_cell_is_still_refused_with_its_row() {
    let lib = library(vec![(
        "t",
        table(
            &["row_id", "mpn"],
            vec![
                row(&[("row_id", "1"), ("mpn", "ok µ")]),
                row(&[("row_id", "2"), ("mpn", "bad\u{7f}")]),
            ],
        ),
    )]);
    match lib.write().unwrap_err() {
        LibraryFileError::DisallowedControlInCell {
            row_index, column, ..
        } => assert_eq!((row_index, column.as_str()), (1, "mpn")),
        other => panic!("unexpected {other}"),
    }
}

/// The row-line cache (#416): after every kind of change (a row edited
/// through `Arc::make_mut`, rows added and removed, a column added, an
/// edited clone, a table dropped) the cached encoding is the full write's.
#[test]
fn cached_row_lines_follow_every_change() {
    fn same(lib: &LibraryFile, lines: &mut RowLines) {
        assert_eq!(
            lib.write_row_edit("t", None, lines).unwrap(),
            lib.write().unwrap()
        );
    }
    let mut lines = RowLines::default();
    let mut lib = library(vec![
        (
            "t",
            table(
                &["row_id", "mpn"],
                vec![
                    row(&[("row_id", "b"), ("mpn", "x")]),
                    row(&[("row_id", "a"), ("mpn", "y")]),
                ],
            ),
        ),
        ("u", table(&["name"], vec![row(&[("name", "n")])])),
    ]);
    same(&lib, &mut lines);

    let t = lib.tables.get_mut("t").unwrap();
    // The cache shares this row, so `make_mut` copies it.
    Arc::make_mut(&mut t.rows[0])
        .cells
        .insert("mpn".into(), "edited".into());
    same(&lib, &mut lines);

    let t = lib.tables.get_mut("t").unwrap();
    t.rows.push(row(&[("row_id", "c"), ("mpn", "z")]));
    same(&lib, &mut lines);
    lib.tables.get_mut("t").unwrap().rows.remove(1);
    same(&lib, &mut lines);
    lib.tables.get_mut("t").unwrap().columns.push("note".into());
    same(&lib, &mut lines);

    let mut clone = lib.clone();
    Arc::make_mut(&mut clone.tables.get_mut("t").unwrap().rows[0])
        .cells
        .insert("note".into(), "clone".into());
    same(&clone, &mut lines);
    same(&lib, &mut lines);

    lib.tables.remove("u");
    same(&lib, &mut lines);
}
