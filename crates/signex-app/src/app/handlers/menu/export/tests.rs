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
//!    subtree — the netlist input set is the child-sheet graph, not the
//!    printed page set, so a `MissingChild` means *missing*, and when one is
//!    genuine the `.net` export refuses while the PDF degrades loudly;
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

/// A sheet with one component pin sitting on a wire named by a `Global` label.
///
/// Both halves matter: a net only exists where a terminal lands, so the pin is
/// what makes the net real, and the reference is what a dropped subtree costs
/// you on the board — missing components. `net_name` is unqualified in the
/// project netlist because the label is `Global`.
pub(crate) fn sheet_with_net(reference: &str, net_name: &str, children: &[&str]) -> SchematicSheet {
    use signex_types::schematic::{
        HAlign, Label, LabelType, LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle, Symbol,
        VAlign, Wire,
    };
    let mut sheet = schematic(children);
    let origin = Point::new(0.0, 0.0);
    sheet.wires.push(Wire {
        uuid: Uuid::new_v4(),
        start: origin,
        end: Point::new(10.0, 0.0),
        stroke_width: 0.0,
    });
    sheet.labels.push(Label {
        uuid: Uuid::new_v4(),
        text: net_name.to_string(),
        position: origin,
        rotation: 0.0,
        label_type: LabelType::Global,
        shape: String::new(),
        font_size: 1.27,
        justify: HAlign::default(),
        justify_v: VAlign::default(),
    });
    sheet.lib_symbols.insert(
        "Device:R".to_string(),
        LibSymbol {
            id: "Device:R".to_string(),
            reference: "R".to_string(),
            value: String::new(),
            footprint: String::new(),
            datasheet: String::new(),
            description: String::new(),
            keywords: String::new(),
            fp_filters: String::new(),
            in_bom: true,
            on_board: true,
            in_pos_files: true,
            duplicate_pin_numbers_are_jumpers: false,
            graphics: Vec::new(),
            pins: vec![LibPin {
                unit: 0,
                body_style: 1,
                pin: Pin {
                    direction: PinDirection::Passive,
                    shape_style: PinShapeStyle::Plain,
                    position: origin,
                    rotation: 0.0,
                    length: 0.0,
                    name: String::new(),
                    number: "1".to_string(),
                    visible: true,
                    name_visible: true,
                    number_visible: true,
                },
            }],
            show_pin_numbers: true,
            show_pin_names: true,
            pin_name_offset: 0.0,
        },
    );
    sheet.symbols.push(Symbol {
        uuid: Uuid::new_v4(),
        lib_id: "Device:R".to_string(),
        reference: reference.to_string(),
        value: String::new(),
        footprint: String::new(),
        datasheet: String::new(),
        position: origin,
        rotation: 0.0,
        mirror_x: false,
        mirror_y: false,
        unit: 1,
        is_power: false,
        ref_text: None,
        val_text: None,
        fields_autoplaced: false,
        fields_user_placed: false,
        dnp: false,
        in_bom: true,
        on_board: true,
        exclude_from_sim: false,
        locked: false,
        fields: HashMap::new(),
        custom_properties: Vec::new(),
        pin_uuids: HashMap::new(),
        instances: Vec::new(),
        library_id: None,
        row_id: None,
        library_version: String::new(),
    });
    sheet
}

fn net_names(ctx: &signex_output::ExportContext) -> Vec<String> {
    ctx.netlist
        .as_ref()
        .map(|n| n.nets.iter().map(|net| net.name.clone()).collect())
        .unwrap_or_default()
}

/// Every component reference the exported netlist carries a terminal for —
/// what a dropped subtree costs on the board.
fn netlist_references(ctx: &signex_output::ExportContext) -> Vec<String> {
    let mut refs: Vec<String> = ctx
        .netlist
        .iter()
        .flat_map(|n| n.nets.iter())
        .flat_map(|net| net.terminals.iter().map(|t| t.reference.clone()))
        .collect();
    refs.sort();
    refs.dedup();
    refs
}

/// A `Signex` with one loaded, *active* project whose persisted sheet list is
/// `listed`. Sheets are not opened here — callers add the engines they need
/// with [`open`].
pub(crate) fn app_workspace(dir: &str, listed: &[&str]) -> Signex {
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
    app
}

fn workspace(dir: &str, listed: &[&str]) -> DocumentState {
    app_workspace(dir, listed).document_state
}

/// Open `path` as a live engine whose sheet references `children`.
fn open(ds: &mut DocumentState, path: &PathBuf, children: &[&str]) {
    open_with(ds, path, schematic(children));
}

pub(crate) fn open_with(ds: &mut DocumentState, path: &PathBuf, sheet: SchematicSheet) {
    let engine = signex_engine::Engine::new(sheet).expect("engine");
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
    // Round 3 asserted a `MissingChild` here and called that correct. It was
    // not: the child is open as a live engine, so nothing is missing — the
    // export was simply refusing to look at anything outside `data.sheets`.
    // The fix is completeness, not a better apology.
    assert!(
        issues.messages().is_empty(),
        "an open hierarchical child is in memory; no stitch issue is warranted: {:?}",
        issues.messages()
    );
}

// -- Definition of done (a): the netlist input set is complete -------------

#[test]
fn root_active_netlist_contains_a_child_absent_from_data_sheets() {
    // The routine state, and the worst path: the root sheet is active and
    // `child.snxsch` is a hierarchical child that was never added to
    // `data.sheets` (descending into one never appends to that list). Deriving
    // the netlist from the *page* set alone drops the child's whole subtree
    // and exports a plausible-looking netlist missing it.
    let mut ds = workspace("/w/a", &["top.snxsch"]);
    let top = PathBuf::from("/w/a").join("top.snxsch");
    let child = PathBuf::from("/w/a").join("child.snxsch");
    open_with(
        &mut ds,
        &top,
        sheet_with_net("R_TOP", "TOP_NET", &["child.snxsch"]),
    );
    open_with(&mut ds, &child, sheet_with_net("R_CHILD", "CHILD_NET", &[]));
    ds.active_path = Some(top.clone());

    let (ctx, issues) = super::build_export_scope(&ds).expect("context");

    assert_eq!(
        page_paths(&ctx),
        vec![top],
        "the child is not a printed page"
    );
    let names = net_names(&ctx);
    assert!(
        names.iter().any(|n| n == "CHILD_NET"),
        "the child subtree must be in the exported netlist, {names:?}"
    );
    assert_eq!(
        netlist_references(&ctx),
        vec!["R_CHILD".to_string(), "R_TOP".to_string()],
        "a dropped subtree is missing components on the board"
    );
    assert!(
        issues.messages().is_empty(),
        "nothing is missing, so no stitch issue may be manufactured: {:?}",
        issues.messages()
    );
}

#[test]
fn a_child_only_on_disk_is_stitched_without_being_opened() {
    // Same completeness requirement one step further out: the child is neither
    // in `data.sheets` nor open as a tab, but it is sitting next to its parent
    // on disk. `MissingChild` must mean *missing*, not merely *unopened*.
    let dir = std::env::temp_dir().join(format!("signex-export-disk-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("tempdir");
    let child_text =
        signex_types::format::SnxSchematic::new(sheet_with_net("R_DISK", "ON_DISK_NET", &[]))
            .write_string()
            .expect("serialize child");
    std::fs::write(dir.join("child.snxsch"), child_text).expect("write child");

    let dir_str = dir.to_string_lossy().to_string();
    let mut ds = workspace(&dir_str, &["top.snxsch"]);
    let top = dir.join("top.snxsch");
    open_with(
        &mut ds,
        &top,
        sheet_with_net("R_TOP", "TOP_NET", &["child.snxsch"]),
    );
    ds.active_path = Some(top);

    let (ctx, issues) = super::build_export_scope(&ds).expect("context");
    let names = net_names(&ctx);
    std::fs::remove_dir_all(&dir).ok();

    assert!(
        names.iter().any(|n| n == "ON_DISK_NET"),
        "an unopened child on disk must be stitched in, {names:?}"
    );
    assert!(
        issues.messages().is_empty(),
        "nothing is missing: {:?}",
        issues.messages()
    );
}

#[test]
fn a_project_export_roots_at_the_project_root_not_the_active_child() {
    // BEHAVIOURAL CHANGE. Rooting at `active_path` when it happened to be
    // listed made one File > Export Netlist yield either the project netlist
    // or a subtree-only netlist depending on whether the sheet in focus had
    // ever been added to `data.sheets`. An owned project-wide export roots at
    // the project root, always.
    let mut ds = workspace("/w/a", &["top.snxsch", "power.snxsch"]);
    let top = PathBuf::from("/w/a").join("top.snxsch");
    let power = PathBuf::from("/w/a").join("power.snxsch");
    open_with(
        &mut ds,
        &top,
        sheet_with_net("R_TOP", "TOP_NET", &["power.snxsch"]),
    );
    open_with(&mut ds, &power, sheet_with_net("R_POWER", "POWER_NET", &[]));
    ds.active_path = Some(power);

    let names = net_names(&context(&ds));
    assert!(
        names.iter().any(|n| n == "TOP_NET"),
        "a project export is rooted at the project root even with a child \
         sheet focused, so the root's own nets are present: {names:?}"
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

// -- Definition of done (b): per-deliverable severity policy ---------------

/// Count the diagnostics mentioning `marker`. The panel buffer is global, so
/// every test that asserts on it uses a filename unique to itself.
fn diagnostic_count(marker: &str) -> usize {
    let _ = crate::diagnostics::init_logging();
    crate::diagnostics::recent_entries()
        .iter()
        .filter(|e| e.message.contains(marker))
        .count()
}

/// A project whose root references `missing` — a child that is neither open
/// nor on disk. The one case that is a *genuine* `MissingChild`.
fn app_with_missing_child(missing: &str) -> Signex {
    let dir = std::env::temp_dir().join(format!("signex-export-missing-{}", Uuid::new_v4()));
    let mut app = app_workspace(&dir.to_string_lossy(), &["top.snxsch"]);
    let top = dir.join("top.snxsch");
    open_with(
        &mut app.document_state,
        &top,
        sheet_with_net("R_TOP", "TOP_NET", &[missing]),
    );
    app.document_state.active_path = Some(top);
    app
}

#[test]
fn netlist_export_refuses_to_write_an_incomplete_netlist() {
    // The .net file is machine-consumed: a PCB imports it and a missing
    // subtree becomes missing components on the board, plus nets that stayed
    // split where they should have merged through the missing sheet's ports.
    // Nothing downstream reads a warning, so nothing is written.
    let mut app = app_with_missing_child("gone-net.snxsch");
    let out = std::env::temp_dir().join(format!("signex-refuse-{}.net", Uuid::new_v4()));

    let _ = app.handle_export_netlist_finished(Ok(out.clone()));

    assert!(
        !out.exists(),
        "an incomplete netlist must not reach disk: {}",
        out.display()
    );
    let err = app
        .document_state
        .export_error
        .clone()
        .expect("the refusal must be raised through the export_error modal");
    assert!(
        err.contains("gone-net.snxsch"),
        "the modal must name what is missing: {err}"
    );
    std::fs::remove_file(&out).ok();
}

#[test]
fn pdf_export_proceeds_and_warns_once_per_user_action() {
    // The PDF is human-consumed: printing or reviewing mid-refactor with a
    // child genuinely absent from disk is a real workflow, so it degrades
    // rather than refuses — but the warning must reach the Messages panel, and
    // exactly once per user action.
    let marker = format!("gone-pdf-{}.snxsch", Uuid::new_v4());
    let _ = crate::diagnostics::init_logging();
    let before = diagnostic_count(&marker);
    let mut app = app_with_missing_child(&marker);
    let out = std::env::temp_dir().join(format!("signex-partial-{}.pdf", Uuid::new_v4()));

    let _ = app.handle_export_pdf_finished(Ok(out.clone()));

    assert!(
        app.document_state.export_error.is_none(),
        "a partial PDF proceeds: {:?}",
        app.document_state.export_error
    );
    assert!(out.exists(), "the PDF must still be written");
    assert_eq!(
        diagnostic_count(&marker) - before,
        1,
        "the stitch warning must reach the user exactly once per user action"
    );
    std::fs::remove_file(&out).ok();
}

#[test]
fn rerasterizing_the_preview_does_not_flood_the_messages_panel() {
    // Round 3 logged each stitch issue inside the shared context builder,
    // which `rerasterize_print_preview` calls unconditionally — from eleven
    // call sites including a text-input handler that fires per keystroke. The
    // panel keeps 200 entries with no dedupe, so the warnings evicted the
    // user's ERC results and then their own earliest copies.
    let marker = format!("gone-rerender-{}.snxsch", Uuid::new_v4());
    let _ = crate::diagnostics::init_logging();
    let app = app_with_missing_child(&marker);
    let before = diagnostic_count(&marker);

    for _ in 0..25 {
        let _ = super::build_export_context(&app.document_state);
    }

    assert_eq!(
        diagnostic_count(&marker),
        before,
        "the shared context builder must not log — surfacing belongs to the \
         three user-action entry points"
    );
}

// -- Definition of done (c): no listed page vanishes from the .net ---------

/// A flat two-page project: both pages are listed, neither references the
/// other as a child sheet. `project_navigation::add` appends to `data.sheets`
/// with no requirement that anything reference the sheet, so this is routine,
/// not pathological.
fn app_flat_project() -> Signex {
    let mut app = app_workspace("/w/flat", &["a.snxsch", "b.snxsch"]);
    let a = PathBuf::from("/w/flat").join("a.snxsch");
    let b = PathBuf::from("/w/flat").join("b.snxsch");
    open_with(
        &mut app.document_state,
        &a,
        sheet_with_net("R_A", "NET_A", &[]),
    );
    open_with(
        &mut app.document_state,
        &b,
        sheet_with_net("R_B", "NET_B", &[]),
    );
    app.document_state.active_path = Some(b);
    app
}

#[test]
fn a_flat_projects_second_page_cannot_vanish_from_the_netlist_unannounced() {
    // Rooting the netlist at the project root is right, but the stitcher only
    // walks *down* from that root: a listed page nothing references is not in
    // the netlist at all. Nothing is formally `MissingChild`, so before this
    // the issue list was empty, the refusal gate never fired, and a .net
    // holding one page of a two-page board was written to disk in silence —
    // strictly worse than shipping the focused page, because the file looks
    // like the whole project.
    let mut app = app_flat_project();

    let (ctx, issues) = super::build_export_scope(&app.document_state).expect("context");

    // The shortfall is real, not hypothetical.
    assert_eq!(
        netlist_references(&ctx),
        vec!["R_A".to_string()],
        "precondition: the stitcher does not reach an unreferenced page"
    );
    assert!(
        issues.stitch.is_empty(),
        "precondition: nothing is formally MissingChild here — that is the trap"
    );
    assert!(
        issues.netlist_is_incomplete(),
        "a listed page outside the hierarchy is an incomplete netlist"
    );

    // …and the machine-consumed deliverable therefore refuses, by the same
    // per-deliverable policy that already covers MissingChild.
    let out = std::env::temp_dir().join(format!("signex-flat-{}.net", Uuid::new_v4()));
    let _ = app.handle_export_netlist_finished(Ok(out.clone()));

    assert!(
        !out.exists(),
        "a .net missing half the board must not reach disk: {}",
        out.display()
    );
    let err = app
        .document_state
        .export_error
        .clone()
        .expect("the refusal must be raised through the export_error modal");
    assert!(
        err.contains("b.snxsch"),
        "the modal must name the page that is not in the netlist: {err}"
    );
    std::fs::remove_file(&out).ok();
}

#[test]
fn a_page_the_root_does_reach_is_not_reported_as_a_shortfall() {
    // The other side of the same rule — the coverage check must not refuse a
    // perfectly good hierarchical export.
    let mut ds = workspace("/w/a", &["top.snxsch", "power.snxsch"]);
    let top = PathBuf::from("/w/a").join("top.snxsch");
    let power = PathBuf::from("/w/a").join("power.snxsch");
    open_with(
        &mut ds,
        &top,
        sheet_with_net("R_TOP", "TOP_NET", &["power.snxsch"]),
    );
    open_with(&mut ds, &power, sheet_with_net("R_POWER", "POWER_NET", &[]));
    ds.active_path = Some(power);

    let (_ctx, issues) = super::build_export_scope(&ds).expect("context");

    assert!(
        !issues.netlist_is_incomplete(),
        "both pages are in the netlist: {:?}",
        issues.messages()
    );
}

#[test]
fn a_child_that_exists_but_will_not_parse_is_not_called_missing() {
    // "could not be found" sends the user hunting for a file that is sitting
    // right where it should be. The refusal is correct; the diagnosis was not.
    let dir = std::env::temp_dir().join(format!("signex-export-corrupt-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("tempdir");
    std::fs::write(dir.join("child.snxsch"), "(this is not a schematic").expect("write child");

    let dir_str = dir.to_string_lossy().to_string();
    let mut ds = workspace(&dir_str, &["top.snxsch"]);
    let top = dir.join("top.snxsch");
    open_with(
        &mut ds,
        &top,
        sheet_with_net("R_TOP", "TOP_NET", &["child.snxsch"]),
    );
    ds.active_path = Some(top);

    let (_ctx, issues) = super::build_export_scope(&ds).expect("context");
    let messages = issues.messages().join("\n");
    std::fs::remove_dir_all(&dir).ok();

    assert!(
        messages.contains("could not be parsed"),
        "a corrupt child must be diagnosed as unreadable, not absent: {messages}"
    );
    assert!(
        messages.contains("child.snxsch"),
        "the message must name the file: {messages}"
    );
    assert!(
        issues.netlist_is_incomplete(),
        "its subtree is still out of the netlist, so the .net still refuses"
    );
}

mod diagnosis;
