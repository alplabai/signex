//! Mockup row 171's Power tree page and export: the Variant field, Show
//! GND, the scenario line, a click on a part, the Pages and Paper of an
//! export, and a closed diagram listed in the Projects tree. Driven through
//! `Signex::update`, the layout probe and the headless command route.

use std::path::{Path, PathBuf};

use iced::Size;
use signex_types::block_diagram::{Connection, Endpoint, Interface};
use uuid::Uuid;

use super::state::{DiagramView, ExportPage, ExportPaper};
use super::{BlockDiagramMsg, ExportEvent, TabEvent};
use crate::app::command::project_name_tests::project_app;
use crate::app::command::queries::power_tree_tests::write_child_with_rails;
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;
use crate::layout_probe::{lay_out_texts, messages_on_click};
use crate::task_driver::TaskDriver;

const WINDOW: Size = Size::new(1600.0, 900.0);
const DCDC: &str =
    include_str!("../../../../signex-types/tests/corpus/block_diagram/dcdc-buck.snxblk");

fn send(app: &mut Signex, path: &Path, event: TabEvent) {
    let task = app.update(Message::from(BlockDiagramMsg::Tab {
        path: path.to_path_buf(),
        event,
    }));
    TaskDriver::new().settle(app, task).expect("settles");
}

/// `project_app`'s project, its child sheet with rails (U2 not fitted in
/// Lite), and a new diagram open in its tab.
fn with_rails(dir: &tempfile::TempDir) -> (Signex, PathBuf) {
    let mut app = project_app(dir);
    write_child_with_rails(dir);
    app.document_state.projects[0]
        .data
        .variant_definitions
        .push("Lite".into());
    app.document_state.active_project = Some(app.document_state.projects[0].id);
    let _ = app.update(Message::from(BlockDiagramMsg::NewRequested));
    (app, dir.path().join("ProjectA.snxblk"))
}

fn texts(app: &Signex, path: &Path) -> Vec<String> {
    lay_out_texts(app.view_block_diagram_tab(path.to_path_buf()), WINDOW)
        .into_iter()
        .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect()
}

fn rail_loads(app: &Signex, path: &Path, net: &str) -> Vec<String> {
    let editor = app.open_diagram(path).expect("open");
    let read = editor
        .tab
        .power
        .read
        .as_ref()
        .expect("read")
        .as_ref()
        .expect("ok");
    let rail = read.tree.rail(net).expect("rail");
    rail.loads
        .iter()
        .map(|load| read.topology.parts[load.part].designator.clone())
        .collect()
}

#[test]
fn the_variant_field_reads_the_tree_for_the_variant_picked() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = with_rails(&dir);
    send(
        &mut app,
        &path,
        TabEvent::ViewPicked(DiagramView::PowerTree),
    );
    assert_eq!(rail_loads(&app, &path, "+3V3"), ["U2"]);
    let shown = texts(&app, &path);
    assert!(shown.iter().any(|t| t == "Variant"), "{shown:?}");
    assert!(
        shown
            .iter()
            .any(|t| t.contains("Scenario Max load · sources 1 of 1 active")
                && t.contains("worst rail headroom 79 % (+3V3)")),
        "{shown:?}"
    );

    send(&mut app, &path, TabEvent::VariantPicked("Lite".into()));
    let editor = app.open_diagram(&path).expect("open");
    let read = editor
        .tab
        .power
        .read
        .as_ref()
        .expect("read")
        .as_ref()
        .expect("ok");
    assert_eq!(read.variant.as_deref(), Some("Lite"));
    assert!(
        rail_loads(&app, &path, "+3V3").is_empty(),
        "U2 is not fitted"
    );

    send(&mut app, &path, TabEvent::VariantPicked("Base".into()));
    assert_eq!(rail_loads(&app, &path, "+3V3"), ["U2"]);
}

#[test]
fn a_click_on_a_part_opens_its_sheet() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = with_rails(&dir);
    send(
        &mut app,
        &path,
        TabEvent::ViewPicked(DiagramView::PowerTree),
    );
    let editor = app.open_diagram(&path).expect("open");
    let read = editor
        .tab
        .power
        .read
        .as_ref()
        .expect("read")
        .as_ref()
        .expect("ok");
    let u2 = read
        .topology
        .parts
        .iter()
        .position(|part| part.designator == "U2")
        .expect("U2");
    let clicked: Vec<TabEvent> =
        messages_on_click(app.view_block_diagram_tab(path.clone()), WINDOW, "U2 U2")
            .into_iter()
            .filter_map(|message| match message {
                Message::BlockDiagram(BlockDiagramMsg::Tab { event, .. }) => Some(event),
                _ => None,
            })
            .collect();
    assert_eq!(clicked, [TabEvent::PartOpened(u2)]);
    send(&mut app, &path, TabEvent::PartOpened(u2));
    let active = app.active_tab_path().expect("a tab");
    assert_eq!(active, dir.path().join("child.snxsch"));
}

fn ground_link(label: &str) -> Connection {
    let end = |block| Endpoint { block, port: None };
    Connection {
        id: Uuid::now_v7(),
        from: end(Uuid::from_u128(1)),
        to: end(Uuid::from_u128(2)),
        interface: Interface::Power,
        label: label.into(),
        derived_from: None,
    }
}

#[test]
fn show_gnd_hides_ground_connections_until_ticked() {
    let gnd = ground_link("GND");
    let rail = ground_link("VOUT_3V3");
    assert!(super::is_ground_link(&gnd));
    assert!(!super::is_ground_link(&rail));

    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = with_rails(&dir);
    assert!(!app.open_diagram_view(&path).expect("open").show_ground);
    let clicked: Vec<TabEvent> =
        messages_on_click(app.view_block_diagram_tab(path.clone()), WINDOW, "Show GND")
            .into_iter()
            .filter_map(|message| match message {
                Message::BlockDiagram(BlockDiagramMsg::Tab { event, .. }) => Some(event),
                _ => None,
            })
            .collect();
    assert_eq!(clicked, [TabEvent::ShowGroundToggled(true)]);
    let look = |app: &Signex| {
        let editor = app.open_diagram(&path).expect("open");
        let tokens = signex_types::theme::theme_tokens(signex_types::theme::ThemeId::GraphiteLight);
        let look = super::view::Look {
            tokens: &tokens,
            i18n: &app.ui_state.i18n,
        };
        let canvas = super::view::canvas_look(look, editor.tab.show_ground);
        (canvas.shows(&gnd), canvas.shows(&rail))
    };
    assert_eq!(look(&app), (false, true), "hidden as drawn");
    send(&mut app, &path, TabEvent::ShowGroundToggled(true));
    assert_eq!(look(&app), (true, true));
}

fn headless(app: &mut Signex, id: &str, operands: &[&str]) -> CommandOutcome {
    let command = AppCommandId::new(id).expect("a catalog id");
    let args = CommandArgs::parse_operands(&command, operands).expect("operands parse");
    let (outcome, task) = app.run_headless_command_in_test(&command, args);
    TaskDriver::new().settle(app, task).expect("settles");
    outcome
}

fn page_objects(bytes: &[u8]) -> usize {
    let text = String::from_utf8_lossy(bytes);
    text.matches("/Type /Page").count() - text.matches("/Type /Pages").count()
}

#[test]
fn export_prints_the_pages_and_paper_asked_for() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = with_rails(&dir);
    let diagram = dir.path().join("dcdc-buck.snxblk");
    std::fs::write(&diagram, DCDC).expect("write");
    let document = format!("document={}", diagram.display());
    let out = dir.path().join("pages.pdf");
    let target = format!("path={}", out.display());
    let answer = match headless(
        &mut app,
        "export_block_diagram",
        &[
            &document,
            &target,
            "pages=blocks,power_tree,budget",
            "paper=a3",
            "orientation=portrait",
        ],
    ) {
        CommandOutcome::Answered(answer) => answer,
        other => panic!("{other:?}"),
    };
    assert_eq!(answer["page_count"], 3, "{answer}");
    assert_eq!(
        answer["pages"],
        serde_json::json!(["blocks", "power_tree", "budget"])
    );
    assert_eq!(page_objects(&std::fs::read(&out).expect("the PDF")), 3);

    for (extra, expected) in [
        (["pages=wiring", "paper=a4"], "is not a page"),
        (["pages=power_tree", "paper=b5"], "is not a paper size"),
        (
            ["pages=power_tree", "orientation=sideways"],
            "is not an orientation",
        ),
        (["pages=power_tree", "variant=Gold"], "is not a variant"),
    ] {
        let refused = headless(
            &mut app,
            "export_block_diagram",
            &[&document, &target, extra[0], extra[1]],
        );
        let CommandOutcome::Disabled { reason } = refused else {
            panic!("{extra:?}: {refused:?}");
        };
        let why = signex_i18n::RenderTranslatable::render_en_us(&reason);
        assert!(why.contains(expected), "{extra:?}: {why}");
    }
    let svg = format!("path={}", dir.path().join("x.svg").display());
    let refused = headless(
        &mut app,
        "export_block_diagram",
        &[&document, &svg, "pages=budget"],
    );
    assert!(
        matches!(refused, CommandOutcome::Disabled { .. }),
        "{refused:?}"
    );
}

#[test]
fn the_popover_sends_its_pages_and_paper() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = with_rails(&dir);
    send(&mut app, &path, TabEvent::AddBlockPressed);
    send(&mut app, &path, TabEvent::SavePressed);
    let pdf = super::state::ExportKind::Pdf;
    send(&mut app, &path, TabEvent::Export(ExportEvent::Opened(pdf)));
    let shown = texts(&app, &path);
    for expected in [
        "Pages",
        "Block diagram",
        "Power tree",
        "Budget table",
        "Landscape",
    ] {
        assert!(shown.iter().any(|t| t == expected), "{expected}: {shown:?}");
    }
    let budget = ExportEvent::PageToggled(ExportPage::Budget, true);
    send(&mut app, &path, TabEvent::Export(budget));
    let paper = ExportEvent::PaperPicked(ExportPaper::A3);
    send(&mut app, &path, TabEvent::Export(paper));
    let draft = app
        .open_diagram_view(&path)
        .expect("open")
        .export
        .clone()
        .expect("open");
    assert_eq!(
        draft.pages,
        [
            ExportPage::Blocks,
            ExportPage::PowerTree,
            ExportPage::Budget
        ]
    );
    assert_eq!(draft.paper, ExportPaper::A3);
    send(
        &mut app,
        &path,
        TabEvent::Export(ExportEvent::ExportPressed),
    );
    let written = dir.path().join("outputs").join("ProjectA_blocks.pdf");
    let bytes = std::fs::read(&written).expect("the PDF");
    assert!(page_objects(&bytes) >= 3);
}

#[test]
fn a_closed_diagram_in_the_folder_is_a_tree_row_that_opens() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(&dir);
    let sub = dir.path().join("docs");
    std::fs::create_dir_all(&sub).expect("folder");
    let diagram = sub.join("system.snxblk");
    std::fs::write(&diagram, DCDC).expect("write");
    app.refresh_panel_ctx();
    let task = app.start_project_fs_probe(true).expect("a probe");
    TaskDriver::new().settle(&mut app, task).expect("settles");
    app.refresh_panel_ctx();
    let tree = crate::panels::build_project_tree(&app.document_view.panel_ctx);
    let row = find(&tree, "docs/system.snxblk").expect("a row for the closed diagram");
    assert!(!row.is_open && !row.is_dirty);
    assert!(
        app.document_state.block_diagrams.editors.is_empty(),
        "listed, not read"
    );

    let task = app
        .open_project_tree_document(&[0], "docs/system.snxblk".to_string())
        .expect("opens");
    TaskDriver::new().settle(&mut app, task).expect("settles");
    assert!(app.block_diagram_tab(&diagram).is_some());
}

fn find<'a>(
    nodes: &'a [signex_widgets::tree_view::TreeNode],
    label: &str,
) -> Option<&'a signex_widgets::tree_view::TreeNode> {
    nodes.iter().find_map(|node| {
        (node.label == label)
            .then_some(node)
            .or_else(|| find(&node.children, label))
    })
}
