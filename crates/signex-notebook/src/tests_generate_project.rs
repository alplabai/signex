//! The report's facts from the project file and the notebook files
//! (mockup row 201): sheets, revision, variants and blocks.

use std::path::Path;

use signex_types::project::{ProjectData, parse_project_bytes};

use crate::generate::{notebook_blocks, revision_of, sheets_of, variant_of, variants_of};
use crate::{AnnotationKind, NotebookSources};

fn project(json: &str) -> ProjectData {
    parse_project_bytes(Path::new("buck-3v3.snxprj"), json.as_bytes(), |_| false)
        .expect("a project")
}

const PROJECT: &str = r#"{
    "name": "buck-3v3",
    "dir": "",
    "schematic_root": "power.snxsch",
    "pcb_file": null,
    "sheets": [
        {"name": "mcu", "filename": "mcu.snxsch", "symbols_count": 18},
        {"name": "power", "filename": "power.snxsch", "symbols_count": 21},
        {"name": "io", "filename": "io.snxsch", "symbols_count": 9}
    ],
    "parameters": {"REVISION": " B "},
    "variant_definitions": ["Lite", "Full"],
    "active_variant": "full"
}"#;

#[test]
fn the_root_sheet_comes_first_and_the_others_keep_list_order() {
    let sheets = sheets_of(&project(PROJECT));
    let listed: Vec<(&str, usize, usize)> = sheets
        .iter()
        .map(|s| (s.name.as_str(), s.depth, s.parts))
        .collect();
    assert_eq!(listed, [("power", 0, 21), ("mcu", 1, 18), ("io", 1, 9)]);
}

#[test]
fn the_revision_is_the_revision_parameter_in_any_case() {
    assert_eq!(revision_of(&project(PROJECT)).as_deref(), Some("B"));
    let none = project(r#"{"name": "p", "dir": "", "schematic_root": null, "pcb_file": null}"#);
    assert_eq!(revision_of(&none), None);
}

#[test]
fn the_variants_are_base_then_the_projects_own() {
    let data = project(PROJECT);
    assert_eq!(variants_of(&data), ["Base", "Lite", "Full"]);
    assert_eq!(variant_of(&data, None).as_deref(), Ok("Full"));
    assert_eq!(variant_of(&data, Some("lite")).as_deref(), Ok("Lite"));
    assert_eq!(variant_of(&data, Some("base")).as_deref(), Ok("Base"));
    assert!(
        variant_of(&data, Some("Max"))
            .unwrap_err()
            .contains("`Max`")
    );
}

#[test]
fn the_blocks_of_the_notebook_and_its_chapters_are_read_main_first() {
    let mut sources = NotebookSources::new(
        "design.snxnbk",
        r#"#include "chapters/power.snxnbk"
#annotation(kind: "decision", status: "accepted", title: "Main decision")[Body.]"#,
    );
    sources.insert(
        "chapters/power.snxnbk",
        r#"#annotation(kind: "issue", status: "open", title: "Chapter issue")[Body.]"#,
    );
    let blocks = notebook_blocks(&sources);
    let read: Vec<(AnnotationKind, &str)> =
        blocks.iter().map(|b| (b.kind, b.title.as_str())).collect();
    assert_eq!(
        read,
        [
            (AnnotationKind::Decision, "Main decision"),
            (AnnotationKind::Issue, "Chapter issue"),
        ]
    );
}
