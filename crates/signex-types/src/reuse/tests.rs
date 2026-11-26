use std::path::{Path, PathBuf};

use serde_json::json;
use uuid::Uuid;

use super::template::{apply_project_template, apply_sheet_template};
use super::*;
use crate::object_id::SequentialIds;
use crate::project::ProjectData;
use crate::schematic::{DrawingPlacement, Point, SchDrawing, SchematicSheet, TextNote, Wire};

fn id(n: u128) -> Uuid {
    Uuid::from_u128(1000 + n)
}

fn note(n: u128) -> TextNote {
    serde_json::from_value(json!({
        "uuid": id(n),
        "text": "Rev ${REV}",
        "position": { "x": 1.0, "y": 2.0 },
    }))
    .unwrap()
}

fn wire(n: u128) -> Wire {
    serde_json::from_value(json!({
        "uuid": id(n),
        "start": { "x": 0.0, "y": 0.0 },
        "end": { "x": 5.0, "y": 0.0 },
    }))
    .unwrap()
}

fn template_sheet() -> SchematicSheet {
    let mut sheet = SchematicSheet {
        uuid: id(0),
        paper_size: "A4".into(),
        ..Default::default()
    };
    sheet.frame.show_title_block = false;
    sheet.title_block.insert("company".into(), "Alp Lab".into());
    sheet.variables.insert("REV".into(), "B".into());
    sheet.drawings = vec![SchDrawing::Line {
        uuid: id(1),
        start: Point::new(0.0, 0.0),
        end: Point::new(10.0, 0.0),
        width: 0.0,
        stroke_color: None,
        locked: false,
    }];
    sheet
        .drawing_placements
        .insert(id(1), DrawingPlacement::Above);
    sheet.text_notes = vec![note(2)];
    sheet.wires = vec![wire(3)];
    sheet
}

#[test]
fn a_sheet_takes_the_templates_frame_with_new_ids_and_not_its_circuit() {
    let template = template_sheet();
    let mut sheet = SchematicSheet {
        uuid: id(50),
        paper_size: "A3".into(),
        ..Default::default()
    };
    apply_sheet_template(&template, &mut sheet, &mut SequentialIds::default());
    assert_eq!(sheet.uuid, id(50), "the sheet keeps its own id");
    assert_eq!(sheet.paper_size, "A4");
    assert!(!sheet.frame.show_title_block);
    assert_eq!(sheet.title_block["company"], "Alp Lab");
    assert_eq!(sheet.variables["REV"], "B");
    assert_eq!(sheet.drawings.len(), 1);
    assert_eq!(sheet.text_notes.len(), 1);
    assert_ne!(sheet.drawings[0].uuid(), id(1), "a drawing gets a new id");
    assert_ne!(sheet.text_notes[0].uuid, id(2), "a note gets a new id");
    assert_eq!(
        sheet.drawing_placement(sheet.drawings[0].uuid()),
        DrawingPlacement::Above,
        "the placement follows the new id"
    );
    assert!(sheet.wires.is_empty(), "a circuit fragment is a snippet");
}

fn project(json: serde_json::Value) -> ProjectData {
    let mut base = json!({"name": "P", "dir": "", "schematic_root": null, "pcb_file": null});
    if let (Some(base), Some(extra)) = (base.as_object_mut(), json.as_object()) {
        base.extend(extra.clone());
    }
    serde_json::from_value(base).unwrap()
}

#[test]
fn a_project_takes_what_its_template_sets_and_keeps_the_rest() {
    let template = project(json!({
        "parameters": {"company": "Alp Lab", "rev": "A"},
        "project_defaults": {"sheet": {"paper_size": "A4", "show_title_block": true}},
    }));
    let mut data = project(json!({
        "parameters": {"rev": "1", "drawn_by": "me"},
        "project_defaults": {"profile": "standard"},
    }));
    apply_project_template(&template, &mut data);
    assert_eq!(data.parameters["company"], "Alp Lab");
    assert_eq!(data.parameters["rev"], "A", "the template's value wins");
    assert_eq!(data.parameters["drawn_by"], "me");
    let sheet = data
        .project_defaults
        .sheet
        .as_ref()
        .expect("sheet defaults");
    assert_eq!(sheet.paper_size, "A4");
    assert_eq!(
        data.project_defaults.profile.as_deref(),
        Some("standard"),
        "a default the template leaves unset stays"
    );
    assert!(data.net_classes.is_empty());
    assert!(data.sheets.is_empty(), "no sheet is listed by a template");
}

#[test]
fn the_scope_tokens_round_trip() {
    for scope in [ReuseScope::Project, ReuseScope::Library] {
        assert_eq!(ReuseScope::from_token(scope.token()), Some(scope));
    }
    assert_eq!(ReuseScope::from_token("user"), None);
}

#[test]
fn an_entry_name_is_checked_as_a_file_name() {
    assert_eq!(checked_name("  amp 1 "), Ok("amp 1".to_string()));
    assert_eq!(checked_name(""), Err(NameRefusal::Empty));
    assert_eq!(checked_name("../amp"), Err(NameRefusal::Characters));
    assert_eq!(checked_name("amp."), Err(NameRefusal::Characters));
    assert_eq!(checked_name("com1.amp"), Err(NameRefusal::Reserved));
    assert_eq!(
        checked_name(&"x".repeat(MAX_NAME_CHARS + 1)),
        Err(NameRefusal::TooLong)
    );
}

#[test]
fn each_kind_has_its_folder_and_a_project_template_is_kept_in_the_library_only() {
    let root = Path::new("root");
    assert_eq!(
        ReuseKind::Snippet.entry_path(root, "amp"),
        PathBuf::from("root/snippets/amp.snxsch")
    );
    assert_eq!(
        ReuseKind::SheetTemplate.entry_path(root, "A4"),
        PathBuf::from("root/templates/sheets/A4.snxsch")
    );
    assert_eq!(
        ReuseKind::ProjectTemplate.entry_path(root, "power"),
        PathBuf::from("root/templates/projects/power")
    );
    assert!(!ReuseKind::ProjectTemplate.kept_in(ReuseScope::Project));
    assert!(ReuseKind::ProjectTemplate.kept_in(ReuseScope::Library));
    assert!(ReuseKind::Snippet.kept_in(ReuseScope::Project));
}

#[test]
fn a_listing_reads_each_scope_skips_strays_and_sorts() {
    let project = tempfile::tempdir().unwrap();
    let library = tempfile::tempdir().unwrap();
    let snippets = ReuseKind::Snippet.folder(project.path());
    std::fs::create_dir_all(&snippets).unwrap();
    std::fs::write(snippets.join("filter.snxsch"), "").unwrap();
    std::fs::write(snippets.join("amp.snxsch"), "").unwrap();
    std::fs::write(snippets.join("notes.txt"), "").unwrap();
    std::fs::write(snippets.join(".hidden.snxsch"), "").unwrap();
    let shared = ReuseKind::Snippet.folder(library.path());
    std::fs::create_dir_all(&shared).unwrap();
    std::fs::write(shared.join("ldo.snxsch"), "").unwrap();
    let templates = ReuseKind::ProjectTemplate.folder(library.path());
    std::fs::create_dir_all(templates.join("power")).unwrap();
    // A project folder never holds a project template.
    std::fs::create_dir_all(ReuseKind::ProjectTemplate.folder(project.path()).join("x")).unwrap();

    let roots = [
        (ReuseScope::Project, project.path().to_path_buf()),
        (ReuseScope::Library, library.path().to_path_buf()),
    ];
    let listing = list(&roots, &[ReuseKind::Snippet, ReuseKind::ProjectTemplate]);
    let names: Vec<(ReuseKind, ReuseScope, &str)> = listing
        .entries
        .iter()
        .map(|e| (e.kind, e.scope, e.name.as_str()))
        .collect();
    assert_eq!(
        names,
        [
            (ReuseKind::Snippet, ReuseScope::Project, "amp"),
            (ReuseKind::Snippet, ReuseScope::Project, "filter"),
            (ReuseKind::Snippet, ReuseScope::Library, "ldo"),
            (ReuseKind::ProjectTemplate, ReuseScope::Library, "power"),
        ]
    );
    assert!(listing.unreadable.is_empty(), "{:?}", listing.unreadable);
    assert_eq!(listing.of_kind(ReuseKind::Snippet).count(), 3);
}

#[test]
fn a_missing_folder_lists_nothing() {
    let root = tempfile::tempdir().unwrap();
    let listing = list(
        &[(ReuseScope::Library, root.path().join("none"))],
        &[ReuseKind::Snippet, ReuseKind::SheetTemplate],
    );
    assert_eq!(listing, ReuseListing::default());
}

#[test]
fn a_project_template_names_its_one_project_file() {
    let dir = tempfile::tempdir().unwrap();
    assert!(project_template_file(dir.path()).is_err_and(|why| why.contains("no .snxprj")));
    std::fs::write(dir.path().join("power.snxprj"), "").unwrap();
    assert_eq!(
        project_template_file(dir.path()),
        Ok(dir.path().join("power.snxprj"))
    );
    std::fs::write(dir.path().join("other.snxprj"), "").unwrap();
    assert!(project_template_file(dir.path()).is_err_and(|why| why.contains("holds 2")));
}
