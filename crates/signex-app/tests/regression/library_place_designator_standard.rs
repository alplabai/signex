//! #1232 — a placed part's reference prefix comes from its component class
//! under the designator standard of the project that OWNS the target sheet.

use std::collections::HashMap;

use signex_app::app::{LoadedProject, Message, Signex};
use signex_app::library::LibraryMessage;
use signex_app::library::messages::ResolvedPlacement;
use signex_app::library::place::resolve_placement;
use signex_types::designator::DesignatorStandard;
use signex_types::project::{ProjectData, SheetEntry};
use std::path::PathBuf;

use crate::library_place_symbol::{Fixture, app_with_library, doc, fixture_of_class};

const SHEET: &str = "place-364.snxsch";

fn project(
    app: &mut Signex,
    name: &str,
    sheets: &[&str],
    standard: Option<DesignatorStandard>,
) -> signex_app::app::ProjectId {
    let id = app.document_state.mint_project_id();
    let mut data = ProjectData {
        name: name.to_string(),
        dir: String::new(),
        schematic_root: None,
        pcb_file: None,
        sheets: sheets
            .iter()
            .map(|f| SheetEntry {
                name: (*f).to_string(),
                filename: (*f).to_string(),
                symbols_count: 0,
                wires_count: 0,
                labels_count: 0,
                unknown: Default::default(),
            })
            .collect(),
        variant_definitions: Vec::new(),
        active_variant: None,
        libraries: Vec::new(),
        enable_git: false,
        net_classes: Vec::new(),
        rules: None,
        annotation: None,
        unknown: Default::default(),
    };
    data.set_designator_standard(standard);
    app.document_state.projects.push(LoadedProject {
        id,
        path: PathBuf::from(format!("{name}.snxprj")),
        data,
        pending_libraries: HashMap::new(),
        case_fold: Default::default(),
        case_probed: false,
    });
    id
}

/// The worker half, run for real: the row's own class and symbol letters.
fn diode(f: &Fixture) -> Box<ResolvedPlacement> {
    let open = [(f.library_id, f.snxlib.clone())];
    let resolved = resolve_placement(&f.snxlib, "ICs", signex_library::RowId(f.row_id), &open)
        .expect("resolve");
    assert_eq!(resolved.class, "diode");
    assert_eq!(resolved.prefix, "D");
    Box::new(resolved)
}

fn place_diode(app: &mut Signex, f: &Fixture) -> String {
    let _ = app.update(Message::Library(LibraryMessage::LibraryComponentResolved {
        target_sheet: PathBuf::from(SHEET),
        library_path: f.snxlib.clone(),
        result: Ok(diode(f)),
    }));
    doc(app).symbols[0].reference.clone()
}

#[test]
fn a_diode_under_din_40719_2_is_referenced_v() {
    let f = fixture_of_class("diode");
    let mut app = app_with_library(&f);
    project(
        &mut app,
        "P",
        &[SHEET],
        Some(DesignatorStandard::Din40719_2),
    );
    assert_eq!(place_diode(&mut app, &f), "V1");
}

#[test]
fn the_standard_comes_from_the_project_owning_the_sheet_not_the_active_one() {
    let f = fixture_of_class("diode");
    let mut app = app_with_library(&f);
    project(
        &mut app,
        "A",
        &[SHEET],
        Some(DesignatorStandard::Din40719_2),
    );
    let other = project(&mut app, "B", &[], Some(DesignatorStandard::Ieee315));
    app.document_state.active_project = Some(other);
    assert_eq!(place_diode(&mut app, &f), "V1");
}

#[test]
fn without_a_standard_the_symbol_designator_decides() {
    let f = fixture_of_class("diode");
    let mut app = app_with_library(&f);
    project(&mut app, "P", &[SHEET], None);
    assert_eq!(place_diode(&mut app, &f), "D1");
}

#[test]
fn a_loose_sheet_keeps_the_symbol_designator() {
    let f = fixture_of_class("diode");
    let mut app = app_with_library(&f);
    assert_eq!(place_diode(&mut app, &f), "D1");
}

#[test]
fn picking_a_standard_in_project_options_sets_it_and_marks_the_project_dirty() {
    use signex_app::app::{ProjectMsg, ProjectOptionsState};
    let f = fixture_of_class("diode");
    let mut app = app_with_library(&f);
    project(&mut app, "P", &[SHEET], None);
    app.ui_state.project_options = Some(ProjectOptionsState {
        project_idx: 0,
        name: "P".into(),
        directory: String::new(),
        schematic_root: None,
        pcb_file: None,
        library_count: 0,
    });
    let pick = |app: &mut Signex, s| {
        let _ = app.update(Message::Project(ProjectMsg::DesignatorStandardPicked(s)));
    };

    pick(&mut app, Some(DesignatorStandard::Iec81346_2));
    let loaded = &app.document_state.projects[0];
    assert_eq!(
        loaded.data.designator_standard(),
        Some(DesignatorStandard::Iec81346_2)
    );
    assert!(app.document_state.dirty_paths.contains(&loaded.path));

    pick(&mut app, None);
    assert!(app.document_state.projects[0].data.annotation.is_none());
}

#[test]
fn re_picking_library_designator_keeps_an_unreadable_section_and_stays_clean() {
    use signex_app::app::{ProjectMsg, ProjectOptionsState};
    let f = fixture_of_class("diode");
    let mut app = app_with_library(&f);
    project(&mut app, "P", &[SHEET], None);
    app.document_state.projects[0].data.unknown =
        serde_json::from_str(r#"{"annotation":{"designator_standard":"iso9999"}}"#).unwrap();
    app.ui_state.project_options = Some(ProjectOptionsState {
        project_idx: 0,
        name: "P".into(),
        directory: String::new(),
        schematic_root: None,
        pcb_file: None,
        library_count: 0,
    });
    let _ = app.update(Message::Project(ProjectMsg::DesignatorStandardPicked(None)));
    let loaded = &app.document_state.projects[0];
    assert!(loaded.data.has_unreadable_annotation());
    assert!(!app.document_state.dirty_paths.contains(&loaded.path));

    let _ = app.update(Message::Project(ProjectMsg::DesignatorStandardPicked(
        Some(DesignatorStandard::Ieee315),
    )));
    let loaded = &app.document_state.projects[0];
    assert!(!loaded.data.has_unreadable_annotation());
    assert!(app.document_state.dirty_paths.contains(&loaded.path));
}
