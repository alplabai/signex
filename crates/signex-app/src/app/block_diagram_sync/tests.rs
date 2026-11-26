//! #2968: generate and regenerate on a project on disk (the run itself),
//! from the palette (the GUI route through the translator) and with operands
//! (the headless route).

use std::path::{Path, PathBuf};

use signex_i18n::RenderTranslatable;
use signex_types::block_diagram::{LinkState, read_block_diagram};
use signex_types::schematic::{ChildSheet, FillType, Point, SchematicSheet};

use super::BlockDiagramSyncMsg;
use super::job::{SyncJob, SyncMode};
use crate::app::command::bridge::core_to_message;
use crate::app::command::{CommandArgs, CommandOutcome, translator};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{Message, ProjectMsg, Signex};
use crate::keymap::AppCommandId;

fn symbol(name: &str, file: &str, x: f64) -> ChildSheet {
    ChildSheet {
        uuid: uuid::Uuid::now_v7(),
        name: name.into(),
        filename: file.into(),
        position: Point::new(x, 20.0),
        size: (20.0, 15.0),
        stroke_width: 0.0,
        fill: FillType::None,
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: Vec::new(),
        instances: Vec::new(),
    }
}

fn save(sheet: SchematicSheet, path: &Path) {
    signex_engine::Engine::new_with_path(sheet, Some(path.to_path_buf()))
        .expect("engine")
        .save()
        .expect("write sheet");
}

/// `top.snxsch` in `dir` placing `children`, each an empty sheet written
/// once.
pub(crate) fn write_hierarchy(dir: &Path, children: &[(&str, &str)]) -> PathBuf {
    let mut top = SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        ..Default::default()
    };
    for (i, (name, file)) in children.iter().enumerate() {
        top.child_sheets.push(symbol(name, file, 40.0 * i as f64));
        let child = dir.join(file);
        if !child.exists() {
            let sheet = SchematicSheet {
                uuid: uuid::Uuid::now_v7(),
                ..Default::default()
            };
            save(sheet, &child);
        }
    }
    let root = dir.join("top.snxsch");
    if root.exists() {
        std::fs::remove_file(&root).expect("the old top sheet");
    }
    save(top, &root);
    root
}

fn job(dir: &Path, mode: SyncMode) -> SyncJob {
    let root = dir.join("top.snxsch");
    SyncJob {
        mode,
        target: dir.join("demo.snxblk"),
        project_dir: dir.to_path_buf(),
        open: Default::default(),
        pages: vec![root.clone()],
        root,
        fold: Default::default(),
        naming: Default::default(),
        unlink: true,
    }
}

#[test]
fn a_run_generates_then_regenerates_keeping_the_layout() {
    let dir = tempfile::tempdir().expect("dir");
    write_hierarchy(dir.path(), &[("Alpha", "a.snxsch"), ("Beta", "b.snxsch")]);
    let answer = job(dir.path(), SyncMode::Generate { overwrite: false })
        .run()
        .expect("generated");
    assert_eq!(answer["summary"]["blocks"], 2, "{answer}");
    let file = dir.path().join("demo.snxblk");
    let generated = read_block_diagram(&file).expect("written").diagram;
    assert!(generated.blocks.iter().all(|b| {
        b.sheet
            .as_ref()
            .is_some_and(|l| l.state == LinkState::Linked)
    }));

    write_hierarchy(
        dir.path(),
        &[
            ("Alpha", "a.snxsch"),
            ("Beta", "b.snxsch"),
            ("Gamma", "c.snxsch"),
        ],
    );
    let bytes = std::fs::read(&file).expect("bytes");
    let listed = job(dir.path(), SyncMode::Regenerate { apply: false })
        .run()
        .expect("listed");
    assert_eq!(
        listed["changes"].as_array().map(Vec::len),
        Some(1),
        "{listed}"
    );
    assert_eq!(listed["changes"][0]["change"], "block_added", "{listed}");
    assert_eq!(
        std::fs::read(&file).expect("bytes"),
        bytes,
        "nothing written"
    );

    let applied = job(dir.path(), SyncMode::Regenerate { apply: true })
        .run()
        .expect("applied");
    assert_eq!(applied["written"], true, "{applied}");
    let after = read_block_diagram(&file).expect("written").diagram;
    assert_eq!(after.blocks.len(), 3);
    for block in &generated.blocks {
        let kept = after.block(block.id).expect("kept");
        assert_eq!(kept.position, block.position);
    }
}

#[test]
fn generate_refuses_an_existing_file_without_overwrite() {
    let dir = tempfile::tempdir().expect("dir");
    write_hierarchy(dir.path(), &[("Alpha", "a.snxsch")]);
    let file = dir.path().join("demo.snxblk");
    std::fs::write(&file, "kept").expect("seed");
    let refused = job(dir.path(), SyncMode::Generate { overwrite: false }).run();
    assert!(refused.is_err_and(|why| why.render_en_us().contains("overwrite=true")));
    assert_eq!(std::fs::read_to_string(&file).expect("kept"), "kept");
    job(dir.path(), SyncMode::Generate { overwrite: true })
        .run()
        .expect("replaced");
    assert!(read_block_diagram(&file).is_ok());
}

/// A sheet that is there but does not read refuses the run, so its block
/// is never marked unlinked by mistake.
#[test]
fn a_sheet_that_does_not_read_refuses_the_run() {
    let dir = tempfile::tempdir().expect("dir");
    write_hierarchy(dir.path(), &[("Alpha", "a.snxsch")]);
    job(dir.path(), SyncMode::Generate { overwrite: false })
        .run()
        .expect("generated");
    std::fs::write(dir.path().join("a.snxsch"), "not a sheet").expect("break it");
    let refused = job(dir.path(), SyncMode::Regenerate { apply: true }).run();
    assert!(refused.is_err());
    let diagram = read_block_diagram(&dir.path().join("demo.snxblk"))
        .expect("still there")
        .diagram;
    assert_eq!(
        diagram.blocks[0].sheet.as_ref().map(|l| l.state),
        Some(LinkState::Linked)
    );
}

/// An app with the project of `dir` (root `top.snxsch`) active.
pub(crate) fn app_on(dir: &Path) -> Signex {
    let mut app = app_workspace(&dir.to_string_lossy(), &["top.snxsch"]);
    app.ui_state.first_run_tour_open = false;
    app.document_state.active_project = Some(app.document_state.projects[0].id);
    app
}

/// The palette and a key run the operand handler on the project in view.
#[test]
fn the_palette_runs_the_operand_handler_on_the_project_in_view() {
    for (id, regenerate) in [
        ("generate_block_diagram", false),
        ("regenerate_block_diagram", true),
    ] {
        let dir = tempfile::tempdir().expect("dir");
        let mut app = app_on(dir.path());
        let command = AppCommandId::new(id).expect("catalog id");
        let message = core_to_message(&command, &CommandArgs::none()).expect("bridged");
        let Message::Project(ProjectMsg::BlockDiagramSync(msg)) = message else {
            panic!("{id} bridges elsewhere");
        };
        assert!(matches!(
            msg,
            BlockDiagramSyncMsg::Requested { regenerate: r, apply: false } if r == regenerate
        ));
        let _task = app.dispatch_block_diagram_sync_message(msg);
        assert!(translator::ran(id), "{id}");
    }
}

fn headless(app: &mut Signex, id: &str, operands: &[&str]) -> CommandOutcome {
    let command = AppCommandId::new(id).expect("catalog id");
    let args = CommandArgs::parse_operands(&command, operands).expect("operands parse");
    app.run_headless_command_in_test(&command, args).0
}

/// With operands the run is answered when it lands, by its ticket: a
/// headless run settles the ticket and gives the answer the run made.
#[test]
fn with_operands_the_run_waits_on_a_ticket() {
    let dir = tempfile::tempdir().expect("dir");
    write_hierarchy(dir.path(), &[("Alpha", "a.snxsch")]);
    let mut app = app_on(dir.path());
    let project = format!("document={}", dir.path().join("ProjectA.snxprj").display());
    for id in ["generate_block_diagram", "regenerate_block_diagram"] {
        let outcome = headless(&mut app, id, &[project.as_str()]);
        let CommandOutcome::Answered(answer) = outcome else {
            panic!("{id}: {outcome:?}");
        };
        assert!(
            answer["path"]
                .as_str()
                .is_some_and(|p| p.ends_with("ProjectA.snxblk")),
            "{id}: {answer}"
        );
    }
}

/// A run that writes is refused while its diagram is open in a tab, so the
/// tab's next save cannot put back what the run wrote over; a regenerate
/// that only lists its changes still runs.
#[test]
fn a_write_under_a_diagram_open_in_a_tab_is_refused() {
    use crate::app::block_diagram::BlockDiagramEditor;
    use signex_types::block_diagram::BlockDiagram;

    let dir = tempfile::tempdir().expect("dir");
    write_hierarchy(dir.path(), &[("Alpha", "a.snxsch")]);
    let mut app = app_on(dir.path());
    let project = format!("document={}", dir.path().join("ProjectA.snxprj").display());
    let generated = headless(&mut app, "generate_block_diagram", &[project.as_str()]);
    assert!(
        matches!(generated, CommandOutcome::Answered(_)),
        "{generated:?}"
    );
    let file = dir.path().join("ProjectA.snxblk");
    let before = std::fs::read(&file).expect("generated");
    let editors = &mut app.document_state.block_diagrams.editors;
    editors.insert(
        file.clone(),
        BlockDiagramEditor::created(BlockDiagram::new()),
    );
    for (id, extra) in [
        ("generate_block_diagram", "overwrite=true"),
        ("regenerate_block_diagram", "apply=true"),
    ] {
        let outcome = headless(&mut app, id, &[project.as_str(), extra]);
        let CommandOutcome::Disabled { reason } = outcome else {
            panic!("{id}: {outcome:?}");
        };
        assert!(reason.render_en_us().contains("open in a tab"), "{id}");
    }
    let kept = std::fs::read(&file).expect("kept");
    assert_eq!(kept, before, "nothing written");
    let listed = headless(&mut app, "regenerate_block_diagram", &[project.as_str()]);
    let CommandOutcome::Answered(answer) = listed else {
        panic!("{listed:?}");
    };
    assert_eq!(answer["written"], false, "{answer}");
}

#[test]
fn a_path_that_is_not_a_block_diagram_is_refused() {
    let dir = tempfile::tempdir().expect("dir");
    let mut app = app_on(dir.path());
    let project = format!("document={}", dir.path().join("ProjectA.snxprj").display());
    let path = format!("path={}", dir.path().join("diagram.txt").display());
    let outcome = headless(
        &mut app,
        "generate_block_diagram",
        &[project.as_str(), path.as_str()],
    );
    assert!(
        matches!(outcome, CommandOutcome::Disabled { .. }),
        "{outcome:?}"
    );
}
