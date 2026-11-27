//! The tab's view switch, Power tree, export popover, saves and Projects
//! tree row (mockup row 171), driven the way a person drives them: messages
//! through `Signex::update`, and the laid-out view read and clicked with the
//! layout probe.

use std::path::{Path, PathBuf};

use iced::Size;
use signex_types::block_diagram::read_block_diagram;
use signex_widgets::tree_view::{TreeIcon, TreeNode};

use super::state::{DiagramView, ExportKind};
use super::{BlockDiagramMsg, ExportEvent, TabEvent};
use crate::app::block_diagram_sync::tests::{app_on, write_hierarchy};
use crate::app::command::project_name_tests::project_app;
use crate::app::command::queries::power_tree_tests::write_child_with_rails;
use crate::app::command::translator;
use crate::app::{Message, Signex};
use crate::layout_probe::{lay_out_inputs, lay_out_texts, messages_on_click};
use crate::task_driver::TaskDriver;

const WINDOW: Size = Size::new(1600.0, 900.0);

fn send(app: &mut Signex, path: &Path, event: TabEvent) {
    let task = app.update(Message::from(BlockDiagramMsg::Tab {
        path: path.to_path_buf(),
        event,
    }));
    TaskDriver::new().settle(app, task).expect("settles");
}

/// The project of `project_app` with a new diagram open in its tab.
fn with_diagram(dir: &tempfile::TempDir) -> (Signex, PathBuf) {
    let mut app = project_app(dir);
    app.document_state.active_project = Some(app.document_state.projects[0].id);
    let _ = app.update(Message::from(BlockDiagramMsg::NewRequested));
    let path = dir.path().join("ProjectA.snxblk");
    assert!(app.block_diagram_tab(&path).is_some());
    (app, path)
}

fn texts(app: &Signex, path: &Path) -> Vec<String> {
    lay_out_texts(app.view_block_diagram_tab(path.to_path_buf()), WINDOW)
        .into_iter()
        // Without the isolation marks around a message's arguments.
        .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect()
}

fn clicked(app: &Signex, path: &Path, label: &str) -> Vec<TabEvent> {
    messages_on_click(
        app.view_block_diagram_tab(path.to_path_buf()),
        WINDOW,
        label,
    )
    .into_iter()
    .filter_map(|message| match message {
        Message::BlockDiagram(BlockDiagramMsg::Tab { event, .. }) => Some(event),
        _ => None,
    })
    .collect()
}

#[test]
fn the_view_switch_reads_and_draws_the_projects_power_tree() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = with_diagram(&dir);
    // After `project_app`, which writes the plain child sheet.
    write_child_with_rails(&dir);
    let before = app.document_state.block_diagrams.editors[&path]
        .diagram
        .clone();

    assert_eq!(
        clicked(&app, &path, "Power tree"),
        [TabEvent::ViewPicked(DiagramView::PowerTree)]
    );
    send(
        &mut app,
        &path,
        TabEvent::ViewPicked(DiagramView::PowerTree),
    );
    let editor = app.open_diagram(&path).expect("open");
    assert_eq!(editor.tab.view, DiagramView::PowerTree);
    assert!(!editor.tab.power.loading, "the run landed");
    let read = editor
        .tab
        .power
        .read
        .as_ref()
        .expect("read")
        .as_ref()
        .expect("ok");
    assert!(read.tree.rail("+3V3").is_some());
    assert_eq!(
        editor.doc.diagram, before,
        "showing the tree changes nothing"
    );

    let shown = texts(&app, &path);
    for expected in [
        "Rails",
        "+3V3",
        "VIN_12V",
        "Converter",
        "0.64 A max",
        "3.30 V",
    ] {
        assert!(shown.iter().any(|t| t == expected), "{expected}: {shown:?}");
    }
    let picked = clicked(&app, &path, "VIN_12V");
    assert_eq!(picked, [TabEvent::RailPicked("VIN_12V".into())]);
    send(&mut app, &path, picked[0].clone());
    let editor = app.open_diagram(&path).expect("open");
    assert_eq!(editor.tab.power.rail.as_deref(), Some("VIN_12V"));

    send(&mut app, &path, TabEvent::ViewPicked(DiagramView::Blocks));
    assert!(texts(&app, &path).iter().any(|t| t == "Add Block"));
}

#[test]
fn resync_on_the_toolbar_sends_the_tabs_regenerate() {
    let dir = tempfile::tempdir().unwrap();
    write_hierarchy(dir.path(), &[("Alpha", "a.snxsch"), ("Beta", "b.snxsch")]);
    let mut app = app_on(dir.path());
    let _ = app.update(Message::from(BlockDiagramMsg::NewRequested));
    let path = dir.path().join("ProjectA.snxblk");
    assert_eq!(clicked(&app, &path, "Re-sync"), [TabEvent::ResyncPressed]);
    send(&mut app, &path, TabEvent::ResyncPressed);
    assert!(translator::ran("regenerate_block_diagram"));
    // The sheets are blocks now, merged into the open diagram as one undo
    // step; nothing is written until Save.
    let editor = app.open_diagram(&path).expect("open");
    assert_eq!(
        editor.doc.diagram.blocks.len(),
        2,
        "{:?}",
        editor.doc.diagram
    );
    assert_eq!(editor.doc.undo.len(), 1);
    assert!(!path.exists(), "nothing written");
}

#[test]
fn the_export_popover_runs_export_block_diagram_with_its_settings() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = with_diagram(&dir);
    send(&mut app, &path, TabEvent::AddBlockPressed);
    send(&mut app, &path, TabEvent::SavePressed);

    assert_eq!(
        clicked(&app, &path, "PDF…"),
        [TabEvent::Export(ExportEvent::Opened(ExportKind::Pdf))]
    );
    send(
        &mut app,
        &path,
        TabEvent::Export(ExportEvent::Opened(ExportKind::Pdf)),
    );
    let shown = texts(&app, &path);
    for expected in ["Export the saved diagram", "Landscape", "Title block"] {
        assert!(shown.iter().any(|t| t == expected), "{expected}: {shown:?}");
    }
    let inputs = lay_out_inputs(app.view_block_diagram_tab(path.clone()), WINDOW);
    assert!(
        inputs
            .iter()
            .any(|i| i.text == "outputs/ProjectA_blocks.pdf"),
        "{inputs:?}"
    );

    // SVG keeps the file name the popover offered, with its extension.
    send(
        &mut app,
        &path,
        TabEvent::Export(ExportEvent::KindPicked(ExportKind::Svg)),
    );
    send(
        &mut app,
        &path,
        TabEvent::Export(ExportEvent::MonochromePicked(true)),
    );
    let draft = app.diagram_view(&path).export.clone().expect("open");
    assert_eq!(draft.file, "outputs/ProjectA_blocks.svg");
    assert!(draft.monochrome);

    assert_eq!(
        clicked(&app, &path, "Export"),
        [TabEvent::Export(ExportEvent::ExportPressed)]
    );
    send(
        &mut app,
        &path,
        TabEvent::Export(ExportEvent::ExportPressed),
    );
    assert!(translator::ran("export_block_diagram"));
    let written = dir.path().join("outputs").join("ProjectA_blocks.svg");
    let svg = std::fs::read_to_string(&written).expect("the SVG");
    assert!(svg.contains("Block 1"), "{svg}");
    assert!(
        app.diagram_view(&path).export.is_none(),
        "the popover closed"
    );
}

#[test]
fn a_save_is_not_written_over_a_file_changed_on_disk() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = with_diagram(&dir);
    send(&mut app, &path, TabEvent::AddBlockPressed);
    send(&mut app, &path, TabEvent::SavePressed);
    assert!(!app.document_state.is_document_dirty(&path));
    // A second save of our own write is fine: the tracker knows it.
    send(&mut app, &path, TabEvent::AddBlockPressed);
    send(&mut app, &path, TabEvent::SavePressed);
    assert_eq!(
        read_block_diagram(&path)
            .expect("reads")
            .diagram
            .blocks
            .len(),
        2
    );

    // Another program writes the file; our next save is refused.
    let theirs = std::fs::read_to_string(&path)
        .expect("bytes")
        .replace("Block 1", "Their block");
    std::fs::write(&path, &theirs).expect("their write");
    send(&mut app, &path, TabEvent::AddBlockPressed);
    send(&mut app, &path, TabEvent::SavePressed);
    assert_eq!(std::fs::read_to_string(&path).expect("kept"), theirs);
    assert!(app.document_state.is_document_dirty(&path), "edits kept");
    assert!(app.document_view.error_notice.is_some());
    assert!(!app.document_state.document_saves.is_saving(&path));
    assert!(app.document_state.document_saves.failure(&path).is_some());
}

#[test]
fn a_save_holds_the_quit_barrier_until_it_lands() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = with_diagram(&dir);
    let task = app
        .begin_block_diagram_save(&path)
        .expect("the save starts");
    assert!(app.document_state.document_saves.is_saving(&path));
    TaskDriver::new().settle(&mut app, task).expect("settles");
    assert!(!app.document_state.document_saves.is_saving(&path));
    assert!(path.exists());
}

fn find<'a>(nodes: &'a [TreeNode], label: &str) -> Option<&'a TreeNode> {
    nodes.iter().find_map(|node| {
        (node.label == label)
            .then_some(node)
            .or_else(|| find(&node.children, label))
    })
}

#[test]
fn the_projects_tree_lists_a_held_diagram_with_its_own_icon_and_opens_it() {
    assert_eq!(TreeIcon::for_path("Top.SNXBLK"), TreeIcon::SnxBlockDiagram);
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = with_diagram(&dir);
    app.refresh_panel_ctx();
    let tree = crate::panels::build_project_tree(&app.document_view.panel_ctx);
    let row = find(&tree, "ProjectA.snxblk").expect("a row");
    assert_eq!(row.icon, TreeIcon::SnxBlockDiagram);
    assert!(row.is_open && row.is_dirty && row.is_active);

    send(&mut app, &path, TabEvent::SavePressed);
    let idx = app.block_diagram_tab(&path).expect("a tab");
    let _ = app.close_block_diagram_tab(idx, false);
    assert!(app.block_diagram_tab(&path).is_none());
    let task = app
        .open_project_tree_document(&[0], "ProjectA.snxblk".to_string())
        .expect("opens");
    TaskDriver::new().settle(&mut app, task).expect("settles");
    assert!(app.block_diagram_tab(&path).is_some());
}
