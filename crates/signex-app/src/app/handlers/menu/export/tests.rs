//! Export scope regressions (#406) — asserted on the *emitted page set*.
//!
//! These drive [`build_export_context`] itself rather than the scope helper it
//! calls, because the shipped artifact is the page list: the earlier round of
//! this fix tested only that a new helper returned what it was written to
//! return, which cannot go red on a revert.
//!
//! Two distinct wrong-deliverable failures are covered:
//!
//! 1. a loose schematic focused while a project is loaded must export *itself*,
//!    not the sticky `active_project`'s sheet list;
//! 2. a hierarchical child sheet — opened by descending into a sheet symbol,
//!    which never adds it to the project's persisted `sheets` list — must
//!    still resolve to its owning project and export the project's pages, not
//!    ship a one-page PDF of the child alone.

use std::collections::HashMap;
use std::path::PathBuf;

use signex_types::schematic::{ChildSheet, FillType, Point, SchematicSheet};
use uuid::Uuid;

use crate::app::Signex;
use crate::app::state::{DocumentState, LoadedProject};
use signex_types::project::{ProjectData, SheetEntry};

fn child_ref(filename: &str) -> ChildSheet {
    ChildSheet {
        uuid: Uuid::nil(),
        name: String::new(),
        filename: filename.to_string(),
        position: Point::ZERO,
        size: (0.0, 0.0),
        stroke_width: 0.0,
        fill: FillType::None,
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: Vec::new(),
        instances: Vec::new(),
    }
}

fn schematic(children: &[&str]) -> SchematicSheet {
    SchematicSheet {
        uuid: Uuid::new_v4(),
        version: 0,
        generator: String::new(),
        generator_version: String::new(),
        paper_size: String::new(),
        root_sheet_page: String::new(),
        symbols: Vec::new(),
        wires: Vec::new(),
        junctions: Vec::new(),
        labels: Vec::new(),
        child_sheets: children.iter().map(|f| child_ref(f)).collect(),
        no_connects: Vec::new(),
        text_notes: Vec::new(),
        buses: Vec::new(),
        bus_entries: Vec::new(),
        drawings: Vec::new(),
        no_erc_directives: Vec::new(),
        title_block: HashMap::new(),
        lib_symbols: HashMap::new(),
    }
}

/// A `DocumentState` with one loaded, *active* project whose persisted sheet
/// list is `listed`. Sheets are not opened here — callers add the engines they
/// need with [`open`].
fn workspace(dir: &str, listed: &[&str]) -> DocumentState {
    let (mut app, _task) = Signex::new();
    let id = app.document_state.mint_project_id();
    app.document_state.projects.push(LoadedProject {
        id,
        path: PathBuf::from(dir).join("ProjectA.snxprj"),
        data: ProjectData {
            name: "ProjectA".to_string(),
            dir: dir.to_string(),
            schematic_root: listed.first().map(|f| (*f).to_string()),
            pcb_file: None,
            sheets: listed
                .iter()
                .map(|f| SheetEntry {
                    name: (*f).to_string(),
                    filename: (*f).to_string(),
                    symbols_count: 0,
                    wires_count: 0,
                    labels_count: 0,
                })
                .collect(),
            variant_definitions: Vec::new(),
            active_variant: None,
            libraries: Vec::new(),
            enable_git: false,
        },
        pending_libraries: HashMap::new(),
    });
    // The sticky pointer every one of these regressions is about.
    app.document_state.active_project = Some(id);
    app.document_state
}

/// Open `path` as a live engine whose sheet references `children`.
fn open(ds: &mut DocumentState, path: &PathBuf, children: &[&str]) {
    let engine = signex_engine::Engine::new(schematic(children)).expect("engine");
    ds.engines.insert(path.clone(), engine);
}

fn page_paths(ctx: &signex_output::ExportContext) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = ctx.sheets.iter().map(|s| s.path.clone()).collect();
    paths.sort();
    paths
}

#[test]
fn loose_schematic_exports_itself_not_the_sticky_projects_sheets() {
    // #406 verbatim: ProjectA is loaded and still the active project; the user
    // has File > Open'd an unrelated schematic and is looking at it. Exporting
    // must emit that one document.
    let mut ds = workspace("/w/a", &["top.snxsch", "power.snxsch"]);
    let top = PathBuf::from("/w/a").join("top.snxsch");
    let power = PathBuf::from("/w/a").join("power.snxsch");
    let loose = PathBuf::from("/w/loose").join("scratch.snxsch");
    open(&mut ds, &top, &[]);
    open(&mut ds, &power, &[]);
    open(&mut ds, &loose, &[]);
    ds.active_path = Some(loose.clone());

    let ctx = super::build_export_context(&ds).expect("context");

    assert_eq!(
        page_paths(&ctx),
        vec![loose],
        "a loose schematic must export as a standalone sheet — ProjectA's \
         open tabs must not ride along as extra pages"
    );
    assert!(
        ctx.netlist.is_some(),
        "netlist roots at the loose document itself"
    );
}

#[test]
fn hierarchical_child_sheet_exports_its_owning_project() {
    // The critical one. `top.snxsch` is the only entry in ProjectA's persisted
    // sheet list; `child.snxsch` was opened by double-clicking a sheet symbol,
    // which opens the tab without registering it. Treating "absent from
    // data.sheets" as "loose" ships a one-page PDF of the child alone, with a
    // netlist derived from the child only — silently, to fab.
    let mut ds = workspace("/w/a", &["top.snxsch"]);
    let top = PathBuf::from("/w/a").join("top.snxsch");
    let child = PathBuf::from("/w/a").join("child.snxsch");
    open(&mut ds, &top, &["child.snxsch"]);
    open(&mut ds, &child, &[]);
    ds.active_path = Some(child.clone());

    let ctx = super::build_export_context(&ds).expect("context");

    assert_eq!(
        page_paths(&ctx),
        vec![top],
        "descending into a hierarchical child must still export the owning \
         project's page set, not the child on its own"
    );
    assert!(
        ctx.netlist.is_some(),
        "the active path is below the exported set, so the netlist falls back \
         to the project's own root sheet rather than resolving to None"
    );
}

#[test]
fn grandchild_resolves_through_two_hops_to_the_project() {
    let mut ds = workspace("/w/a", &["top.snxsch"]);
    let top = PathBuf::from("/w/a").join("top.snxsch");
    let mid = PathBuf::from("/w/a").join("mid.snxsch");
    let leaf = PathBuf::from("/w/a").join("leaf.snxsch");
    open(&mut ds, &top, &["mid.snxsch"]);
    open(&mut ds, &mid, &["leaf.snxsch"]);
    open(&mut ds, &leaf, &[]);
    ds.active_path = Some(leaf);

    let ctx = super::build_export_context(&ds).expect("context");
    assert_eq!(page_paths(&ctx), vec![top]);
}

#[test]
fn listed_project_sheet_exports_the_whole_project() {
    // The path that already worked — kept so a scope change that over-corrects
    // into "everything is loose" is caught too.
    let mut ds = workspace("/w/a", &["top.snxsch", "power.snxsch"]);
    let top = PathBuf::from("/w/a").join("top.snxsch");
    let power = PathBuf::from("/w/a").join("power.snxsch");
    open(&mut ds, &top, &[]);
    open(&mut ds, &power, &[]);
    ds.active_path = Some(power);

    let ctx = super::build_export_context(&ds).expect("context");
    let mut expected = vec![top, PathBuf::from("/w/a").join("power.snxsch")];
    expected.sort();
    assert_eq!(page_paths(&ctx), expected);
}

#[test]
fn schematic_inside_the_project_directory_but_unlisted_stays_loose() {
    // Weaker desync: parent-directory matching would claim this file for
    // ProjectA and drag its sheet list into the export. Nothing references it
    // as a child either, so the hierarchy walk finds no parent.
    let mut ds = workspace("/w/a", &["top.snxsch"]);
    let top = PathBuf::from("/w/a").join("top.snxsch");
    let stray = PathBuf::from("/w/a").join("stray.snxsch");
    open(&mut ds, &top, &[]);
    open(&mut ds, &stray, &[]);
    ds.active_path = Some(stray.clone());

    assert!(
        ds.project_for_path(&stray).is_some(),
        "precondition: parent-dir matching does claim this file"
    );
    let ctx = super::build_export_context(&ds).expect("context");
    assert_eq!(page_paths(&ctx), vec![stray]);
}
