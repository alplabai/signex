//! #1232 — a placed part's reference prefix comes from its component class
//! under the designator standard of the project that OWNS the target sheet.

use std::collections::HashMap;

use signex_app::app::{LoadedProject, Message, ProjectMsg, ProjectTabMsg, Signex};
use signex_app::library::LibraryMessage;
use signex_app::library::messages::ResolvedPlacement;
use signex_app::library::place::resolve_placement;
use signex_types::designator::DesignatorStandard;
use signex_types::project::{ProjectData, SheetEntry};
use std::path::PathBuf;

use crate::library_place_symbol::{
    Fixture, app_with_library, doc, fixture_of_class, fixture_with_parameters,
};

const SHEET: &str = "place-364.snxsch";

pub(crate) fn project(
    app: &mut Signex,
    name: &str,
    sheets: &[&str],
    standard: Option<DesignatorStandard>,
) -> signex_app::app::ProjectId {
    let id = app.document_state.mint_project_id();
    let mut data = ProjectData {
        name: name.to_string(),
        description: None,
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
        parameters: Default::default(),
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
        case_decided: false,
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
    place_resolved(app, f, diode(f))
}

/// [`place_diode`] where the library row carries `designator_prefix`.
fn place_diode_with(app: &mut Signex, f: &Fixture, prefix_override: &str) -> String {
    let resolved = diode(f);
    assert_eq!(resolved.prefix_override.as_deref(), Some(prefix_override));
    place_resolved(app, f, resolved)
}

fn place_resolved(app: &mut Signex, f: &Fixture, resolved: Box<ResolvedPlacement>) -> String {
    let _ = app.update(Message::Library(LibraryMessage::LibraryComponentResolved {
        target_sheet: PathBuf::from(SHEET),
        library_path: f.snxlib.clone(),
        result: Ok(resolved),
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

/// The General section's picker, as the UI sends it, for project 0.
fn pick_standard(app: &mut Signex, standard: Option<DesignatorStandard>) {
    let project_id = app.document_state.projects[0].id;
    let _ = app.update(Message::Project(ProjectMsg::Tab(
        ProjectTabMsg::DesignatorStandardPicked {
            project_id,
            standard,
        },
    )));
}

#[test]
fn picking_a_standard_in_the_project_tab_sets_it_and_marks_the_project_dirty() {
    let f = fixture_of_class("diode");
    let mut app = app_with_library(&f);
    project(&mut app, "P", &[SHEET], None);
    let pick = |app: &mut Signex, s| {
        pick_standard(app, s);
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
    let f = fixture_of_class("diode");
    let mut app = app_with_library(&f);
    project(&mut app, "P", &[SHEET], None);
    app.document_state.projects[0].data.unknown =
        serde_json::from_str(r#"{"annotation":{"designator_standard":"iso9999"}}"#).unwrap();
    pick_standard(&mut app, None);
    let loaded = &app.document_state.projects[0];
    assert!(loaded.data.has_unreadable_annotation());
    assert!(!app.document_state.dirty_paths.contains(&loaded.path));

    pick_standard(&mut app, Some(DesignatorStandard::Ieee315));
    let loaded = &app.document_state.projects[0];
    assert!(!loaded.data.has_unreadable_annotation());
    assert!(app.document_state.dirty_paths.contains(&loaded.path));
}

#[test]
fn a_row_override_decides_the_placed_prefix() {
    use signex_library::{DESIGNATOR_PREFIX_PARAM, ParamMap, ParamValue};
    let params: ParamMap = [(
        DESIGNATOR_PREFIX_PARAM.to_owned(),
        ParamValue::Text("zd".into()),
    )]
    .into_iter()
    .collect();
    let f = fixture_with_parameters("diode", params);
    // The override wins over the standard's `V` ...
    let mut app = app_with_library(&f);
    project(
        &mut app,
        "P",
        &[SHEET],
        Some(DesignatorStandard::Din40719_2),
    );
    assert_eq!(place_diode_with(&mut app, &f, "zd"), "ZD1");
    // ... and over the symbol's own `D` when the project follows no standard.
    let mut app = app_with_library(&f);
    project(&mut app, "P", &[SHEET], None);
    assert_eq!(place_diode_with(&mut app, &f, "zd"), "ZD1");
}

#[test]
fn legacy_place_resolves_through_resolve_prefix_unchanged() {
    use signex_app::app::Tool;
    use signex_app::canvas::CanvasEvent;
    let f = fixture_of_class("diode");
    let mut app = app_with_library(&f);
    project(
        &mut app,
        "P",
        &[SHEET],
        Some(DesignatorStandard::Din40719_2),
    );
    app.interaction_state.current_tool = Tool::Component;
    app.document_state.panel_ctx.selected_component = Some("Diode:D_1N4148".into());
    let _ = app.update(Message::CanvasEvent(CanvasEvent::Clicked {
        world_x: 10.0,
        world_y: 10.0,
    }));
    // No row, no class: the value's letters decide, whatever the standard.
    assert_eq!(doc(&app).symbols[0].reference, "D1");
}

#[test]
fn legacy_place_never_issues_a_hash_or_blank_prefix() {
    use signex_app::app::Tool;
    use signex_app::canvas::CanvasEvent;
    // `power:#GND` and `Device: R` have no leading letters: `U`, as before,
    // and never a `#` reference on an ordinary (non-power) part.
    for (lib_id, expected) in [
        ("power:#GND", "U1"),
        ("Device: R", "U1"),
        ("Device:R", "R1"),
    ] {
        let f = fixture_of_class("diode");
        let mut app = app_with_library(&f);
        app.interaction_state.current_tool = Tool::Component;
        app.document_state.panel_ctx.selected_component = Some(lib_id.into());
        let _ = app.update(Message::CanvasEvent(CanvasEvent::Clicked {
            world_x: 10.0,
            world_y: 10.0,
        }));
        let placed = &doc(&app).symbols[0];
        assert_eq!(placed.reference, expected, "{lib_id}");
        assert!(!placed.reference.starts_with('#'), "{lib_id}");
    }
}
