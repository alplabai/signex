//! The Exit confirm as drawn (#2262, Graphite mockup row 16).

use std::collections::HashMap;
use std::path::PathBuf;

use signex_i18n::ids;

use super::{QuitEntry, QuitGroupKey, QuitSummary, group_entries};
use crate::app::Signex;
use crate::layout_probe::{find, lay_out_texts};
use crate::library::state::EditorAddress;

fn entry(group: QuitGroupKey, name: &str) -> QuitEntry {
    QuitEntry {
        group,
        kind: None,
        name: name.to_string(),
        meta: String::new(),
        library: None,
    }
}

fn names(groups: &[super::QuitGroup]) -> Vec<(QuitGroupKey, Vec<&str>)> {
    groups
        .iter()
        .map(|g| (g.key, g.entries.iter().map(|e| e.name.as_str()).collect()))
        .collect()
}

/// Projects come first in project order, then the files no project owns,
/// then the libraries; each group keeps its entries' order.
#[test]
fn the_list_is_grouped_by_project_then_other_files_then_libraries() {
    let groups = group_entries(vec![
        entry(QuitGroupKey::Libraries, "TPS62840DLCR"),
        entry(QuitGroupKey::Project(1), "b.snxsch"),
        entry(QuitGroupKey::Other, "loose.snxsch"),
        entry(QuitGroupKey::Project(0), "power.snxsch"),
        entry(QuitGroupKey::Project(0), "usb-c.snxsch"),
        entry(QuitGroupKey::Libraries, "symbols/r.snxsym"),
    ]);
    assert_eq!(
        names(&groups),
        vec![
            (
                QuitGroupKey::Project(0),
                vec!["power.snxsch", "usb-c.snxsch"]
            ),
            (QuitGroupKey::Project(1), vec!["b.snxsch"]),
            (QuitGroupKey::Other, vec!["loose.snxsch"]),
            (
                QuitGroupKey::Libraries,
                vec!["TPS62840DLCR", "symbols/r.snxsym"]
            ),
        ]
    );
    assert!(group_entries(Vec::new()).is_empty());
}

/// The fact line counts every entry, the projects that hold one and each
/// library once.
#[test]
fn the_summary_counts_projects_and_libraries_once() {
    let lib = |name: &str, path: &str| QuitEntry {
        library: Some(PathBuf::from(path)),
        ..entry(QuitGroupKey::Libraries, name)
    };
    let groups = group_entries(vec![
        entry(QuitGroupKey::Project(0), "a.snxsch"),
        entry(QuitGroupKey::Project(0), "b.snxpcb"),
        entry(QuitGroupKey::Other, "loose.snxsch"),
        lib("r.snxsym", "/libs/power.snxlib"),
        lib("TPS62840DLCR", "/libs/power.snxlib"),
        lib("x.snxfpt", "/libs/other.snxlib"),
    ]);
    assert_eq!(
        QuitSummary::of(&groups, 1),
        QuitSummary {
            count: 6,
            has_rows: true,
            projects: 1,
            libraries: 2,
        }
    );
    assert!(!QuitSummary::of(&groups, 0).has_rows);
}

/// Mockup row 16: a folder too long for its column is cut at its start
/// with an ellipsis, so its own name shows. Two folders that differ only
/// before the cut draw the same; two that differ at the end do not.
#[test]
fn a_long_folder_is_cut_at_its_start_so_its_name_shows() {
    let width = super::META_MAX_W.ceil();
    let window = iced::Size::new(width, (super::TS.caption * 4.0).ceil());
    let shared = "c".repeat(width as usize);
    let draw = |head: &str, name: &str| {
        let folder = format!("/{}/{shared}/{name}", head.repeat(width as usize));
        crate::layout_probe::draw_pixels(
            super::meta_cell::<()>(&folder, iced::Color::WHITE),
            window,
        )
    };
    let a = draw("a", "sample");
    let blank =
        crate::layout_probe::draw_pixels(super::meta_cell::<()>("", iced::Color::WHITE), window);
    assert_ne!(a, blank, "the folder draws");
    assert_eq!(a, draw("b", "sample"), "the start of the folder is cut");
    assert_ne!(a, draw("a", "samplf"), "the folder's own name shows");
}

fn dirty_app(paths: &[&str], rows: Vec<EditorAddress>) -> Signex {
    let (mut app, _boot) = Signex::new();
    app.ui_state.window_size = (1400.0, 900.0);
    app.ui_state.app_quit_confirm = Some(crate::app::AppQuitConfirmState {
        dirty_paths: paths.iter().map(PathBuf::from).collect(),
        dirty_rows: rows,
    });
    app
}

fn project(app: &mut Signex, dir: &str, name: &str, sheets: &[&str]) {
    let id = app.document_state.mint_project_id();
    app.document_state
        .projects
        .push(crate::app::state::LoadedProject {
            id,
            path: PathBuf::from(dir).join("p.snxprj"),
            data: signex_types::project::ProjectData {
                document_id: None,
                description: None,
                name: name.to_owned(),
                dir: dir.to_owned(),
                schematic_root: sheets.first().map(|s| (*s).to_owned()),
                pcb_file: None,
                sheets: sheets
                    .iter()
                    .map(|name| signex_types::project::SheetEntry {
                        name: (*name).to_owned(),
                        filename: (*name).to_owned(),
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
                schematic_defaults: Default::default(),
                project_defaults: Default::default(),
                agent_journal: Default::default(),
                schematic_design: Default::default(),
                fab_outputs: None,
                router_tool_diameter: None,
                requirements: Default::default(),
                parameters: Default::default(),
                unknown: Default::default(),
            },
            pending_libraries: HashMap::new(),
            case_fold: Default::default(),
            case_probed: false,
            case_decided: false,
        });
}

/// A project's sheets and its project file are listed under the project,
/// a file no project owns under Other files, and a Component Preview row
/// under the libraries by part number (its editor gone: by row id), each
/// with its kind cap and folder or library.
#[test]
fn entries_are_classified_with_kind_and_folder() {
    let row = EditorAddress::new(
        PathBuf::from("/libs/power.snxlib"),
        "Regulators".to_string(),
        signex_library::RowId::new(),
    );
    let mut app = dirty_app(
        &["/w/a/p.snxprj", "/w/a/top.snxsch", "/x/loose.snxpcb"],
        vec![row.clone()],
    );
    project(&mut app, "/w/a", "carrier", &["top.snxsch"]);
    let st = app.ui_state.app_quit_confirm.clone().expect("open");
    let groups = group_entries(app.quit_entries(&st.dirty_paths, &st.dirty_rows));
    let folder = |p: &str| std::path::Path::new(p).display().to_string();
    let listed: Vec<(QuitGroupKey, Option<signex_i18n::MessageId>, &str, &str)> = groups
        .iter()
        .flat_map(|g| &g.entries)
        .map(|e| (e.group, e.kind, e.name.as_str(), e.meta.as_str()))
        .collect();
    let a = folder("/w/a");
    let x = folder("/x");
    let row_id = row.row_id.to_string();
    assert_eq!(
        listed,
        vec![
            (
                QuitGroupKey::Project(0),
                Some(ids::PANELS_TAB_KIND_PROJECT),
                "p.snxprj",
                a.as_str()
            ),
            (
                QuitGroupKey::Project(0),
                Some(ids::PANELS_TAB_KIND_SCHEMATIC),
                "top.snxsch",
                a.as_str()
            ),
            (
                QuitGroupKey::Other,
                Some(ids::PANELS_TAB_KIND_PCB),
                "loose.snxpcb",
                x.as_str()
            ),
            (
                QuitGroupKey::Libraries,
                Some(ids::PANELS_TAB_KIND_COMPONENT),
                row_id.as_str(),
                "power.snxlib"
            ),
        ]
    );
}

/// An unsaved Export Job is listed with its OUT kind cap and its folder
/// (mockup row 16, OUT fab.snxout).
#[test]
fn an_unsaved_export_job_has_its_out_cap() {
    let app = dirty_app(&["/w/a/fab.snxout"], Vec::new());
    let st = app.ui_state.app_quit_confirm.clone().expect("open");
    let entries = app.quit_entries(&st.dirty_paths, &st.dirty_rows);
    let listed: Vec<(Option<signex_i18n::MessageId>, &str, String)> = entries
        .iter()
        .map(|e| (e.kind, e.name.as_str(), e.meta.clone()))
        .collect();
    let folder = std::path::Path::new("/w/a").display().to_string();
    assert_eq!(
        listed,
        vec![(Some(ids::PANELS_TAB_KIND_OUTPUT_JOB), "fab.snxout", folder)]
    );
    let texts = lay_out_texts(
        app.view_app_quit_confirm_body(),
        iced::Size::new(500.0, 600.0),
    );
    assert!(
        texts.iter().any(|t| t.text == "OUT"),
        "the OUT cap is drawn"
    );
}

/// An unsaved Export Job under a project's folder is listed under that
/// project, as mockup row 16 draws it; one outside every project folder is
/// under Other files (decision 2026-11-24: no `.snxprj` listing needed).
#[test]
fn an_export_job_under_the_project_folder_is_grouped_under_the_project() {
    let mut app = dirty_app(
        &[
            "/w/a/top.snxsch",
            "/w/a/outputs/fab.snxout",
            "/x/loose.snxout",
        ],
        Vec::new(),
    );
    project(&mut app, "/w/a", "carrier", &["top.snxsch"]);
    let st = app.ui_state.app_quit_confirm.clone().expect("open");
    let groups = group_entries(app.quit_entries(&st.dirty_paths, &st.dirty_rows));
    assert_eq!(
        names(&groups),
        vec![
            (QuitGroupKey::Project(0), vec!["top.snxsch", "fab.snxout"]),
            (QuitGroupKey::Other, vec!["loose.snxout"]),
        ]
    );
}

/// The dialog as laid out: the question, the fact line, one heading per
/// group with its count, and kind caps on the rows.
#[test]
fn the_dialog_shows_headings_counts_and_kind_caps() {
    let mut app = dirty_app(
        &["/w/a/top.snxsch", "/w/a/board.snxpcb", "/x/loose.snxsch"],
        Vec::new(),
    );
    project(&mut app, "/w/a", "carrier", &["top.snxsch"]);
    if let Some(p) = app.document_state.projects.last_mut() {
        p.data.pcb_file = Some("board.snxpcb".to_owned());
    }
    let texts = lay_out_texts(
        app.view_app_quit_confirm_body(),
        iced::Size::new(500.0, 600.0),
    );
    let plain: Vec<String> = texts
        .iter()
        .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect();
    let has = |needle: &str| plain.iter().any(|t| t == needle);
    assert!(has("3 unsaved documents in 1 project."), "{plain:#?}");
    assert!(has("CARRIER"), "{plain:#?}");
    assert!(has("OTHER FILES"), "{plain:#?}");
    assert!(has("2") && has("1"), "group counts: {plain:#?}");
    assert!(has("SCH") && has("PCB"), "kind caps: {plain:#?}");
    let carrier = find(&texts, "CARRIER").bounds;
    let other = find(&texts, "OTHER FILES").bounds;
    let board = find(&texts, "board.snxpcb").bounds;
    assert!(carrier.y < board.y && board.y < other.y, "projects first");
}

/// "The unsaved list changed" is an amber callout above the list, not a
/// line under it.
#[test]
fn the_list_changed_note_sits_above_the_list() {
    let mut app = dirty_app(&["/x/loose.snxsch"], Vec::new());
    app.ui_state.quit_confirm_refreshed = true;
    let texts = lay_out_texts(
        app.view_app_quit_confirm_body(),
        iced::Size::new(500.0, 600.0),
    );
    let note = texts
        .iter()
        .find(|t| t.text.contains("The unsaved list changed"))
        .expect("the note")
        .bounds;
    let heading = find(&texts, "OTHER FILES").bounds;
    let row = find(&texts, "loose.snxsch").bounds;
    assert!(
        note.y + note.height <= heading.y,
        "{note:?} above {heading:?}"
    );
    assert!(heading.y < row.y);
}

/// Cancel leads at the left; Discard All, then Save All, end the footer.
#[test]
fn cancel_leads_the_footer() {
    let app = dirty_app(&["/x/loose.snxsch"], Vec::new());
    let texts = lay_out_texts(
        app.view_app_quit_confirm_body(),
        iced::Size::new(500.0, 600.0),
    );
    let cancel = find(&texts, "Cancel").bounds;
    let discard = find(&texts, "Discard All").bounds;
    let save = find(&texts, "Save All").bounds;
    assert!(cancel.x < discard.x && discard.x < save.x);
    assert!(
        discard.x - (cancel.x + cancel.width) > 100.0,
        "Cancel stands apart"
    );
}
