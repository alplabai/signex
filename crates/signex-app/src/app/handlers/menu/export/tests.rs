//! Export scope regressions (#406) — asserted on the *emitted page set*.
//!
//! These drive [`super::build_export_scope`] itself rather than the scope
//! helper it calls, because the shipped artifact is the page list: the earlier
//! round of this fix tested only that a new helper returned what it was written
//! to return, which cannot go red on a revert.
//!
//! The wrong-deliverable failures covered:
//!
//! 1. a loose schematic focused while a project is loaded must export *itself*,
//!    not the sticky `active_project`'s sheet list;
//! 2. a hierarchical child sheet — opened by descending into a sheet symbol,
//!    which never adds it to the project's persisted `sheets` list — must
//!    still resolve to its owning project and export the project's pages, not
//!    ship a one-page PDF of the child alone;
//! 3. …and the netlist that ships with it must not quietly omit that child's
//!    subtree: `build_project_netlist`'s stitch issues reach the user;
//! 4. a project directory recorded in `data.dir` that no longer matches the
//!    `.snxprj` on disk must not resolve ownership and sheet paths differently;
//! 5. the loose page set is sorted, not `HashMap`-ordered.

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

/// The context alone, for the cases that do not care about stitch issues.
fn context(ds: &DocumentState) -> signex_output::ExportContext {
    super::build_export_scope(ds).expect("context").0
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

    let ctx = context(&ds);

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

    let (ctx, issues) = super::build_export_scope(&ds).expect("context");

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
    // The point of this assertion. `child.snxsch` is referenced by `top` but
    // is not in `data.sheets`, so it is not an exported page and not in the
    // stitch input either: the netlist that ships is missing that whole
    // subtree. It is `Some`, so the "no netlist" warning does not fire, and
    // without surfacing the stitch issues an incomplete netlist would go to
    // fab looking perfectly plausible. Before the fallback existed this
    // yielded `None` — a loud failure; the user must be told either way.
    assert!(
        issues.iter().any(|i| matches!(
            i,
            signex_net::StitchIssue::MissingChild { filename, .. } if filename == "child.snxsch"
        )),
        "the omitted subtree must be reported to the user, not dropped: {issues:?}"
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

    let ctx = context(&ds);
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

    let ctx = context(&ds);
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
    let ctx = context(&ds);
    assert_eq!(page_paths(&ctx), vec![stray]);
}

#[test]
fn loose_export_page_order_is_stable_across_rebuilds() {
    // `engines` is a `HashMap`. The loose page set is fed straight into
    // `PreviewState.sheet_files`, which the print preview re-seeds on every
    // rerasterize — hash-iteration order makes the Settings-tab file-picker
    // rows visibly reshuffle between rerasterizes. Only the active page's
    // position is pinned by the export; the rest must be deterministic.
    let mut ds = workspace("/w/a", &["top.snxsch"]);
    let active = PathBuf::from("/w/loose").join("m.snxsch");
    // Eight loose sheets, not two: an unsorted `HashMap` walk has a 1-in-5040
    // chance of coming out sorted by luck, so a thinner fixture would let the
    // regression through green on most runs.
    for name in [
        "z.snxsch", "a.snxsch", "k.snxsch", "b.snxsch", "q.snxsch", "c.snxsch", "t.snxsch",
        "d.snxsch",
    ] {
        open(&mut ds, &PathBuf::from("/w/loose").join(name), &[]);
    }
    open(&mut ds, &active, &[]);
    ds.active_path = Some(active.clone());

    let order: Vec<PathBuf> = context(&ds).sheets.iter().map(|s| s.path.clone()).collect();
    assert_eq!(order.first(), Some(&active), "active page stays first");
    let mut sorted_tail = order[1..].to_vec();
    sorted_tail.sort();
    assert_eq!(
        order[1..],
        sorted_tail[..],
        "remaining pages must be sorted"
    );
    for _ in 0..8 {
        let again: Vec<PathBuf> = context(&ds).sheets.iter().map(|s| s.path.clone()).collect();
        assert_eq!(again, order, "page order must not depend on hash iteration");
    }
}

#[test]
fn a_stale_persisted_dir_does_not_desync_ownership_from_the_sheet_paths() {
    // `data.dir` is a persisted string; `path` is where the `.snxprj` actually
    // is. A project file moved on disk (or one recording an absolute `dir`
    // that no longer matches) makes the two disagree. If ownership resolves
    // off one and the sheet paths off the other, ownership succeeds while
    // every page resolves to a non-existent path and the export silently
    // emits nothing.
    let mut ds = workspace("/w/a", &["top.snxsch"]);
    ds.projects[0].data.dir = "/w/somewhere-else".to_string();
    let top = PathBuf::from("/w/a").join("top.snxsch");
    open(&mut ds, &top, &[]);
    ds.active_path = Some(top.clone());

    let ctx = context(&ds);
    assert_eq!(
        page_paths(&ctx),
        vec![top],
        "one convention only: the sheet path must be the one ownership matched"
    );
}
