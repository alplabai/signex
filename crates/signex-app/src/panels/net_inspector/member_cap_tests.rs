//! Mockup row 58, decided 2026-11-24 (docs/DECISIONS.md): an expanded net
//! lists its first 6 members, then a "+ N more" row whose click lists the
//! rest. Driven through the real messages over a project on disk.

use std::path::{Path, PathBuf};

use signex_types::schematic::{Label, LabelType, Point, SchematicSheet, Wire};
use uuid::Uuid;

use crate::app::path_ops::settle;
use crate::app::{FileMsg, Message, Signex, TabKind};
use crate::dock::DockMessage;
use crate::panels::{NetInspectorMsg, PanelMsg};
use crate::task_driver::TaskDriver;

/// Create project `stem` under `dir` through File ▸ New Project and return
/// its `.snxprj` and the schematic it was written with.
fn written_project(dir: &Path, stem: &str) -> (PathBuf, PathBuf) {
    let (mut app, _boot) = Signex::new();
    let picked = dir.join(format!("{stem}.snxprj"));
    settle(&mut app, Message::File(FileMsg::NewProject(Some(picked))));
    let project = app.document_state.projects[0].path.clone();
    let sheet = app
        .document_state
        .tabs
        .iter()
        .find(|t| matches!(t.kind, TabKind::Schematic))
        .expect("New Project opens its schematic")
        .path
        .clone();
    (project, sheet)
}

/// A sheet of `count` short wires, each with the global label `VCC` at its
/// start, so the one net `VCC` has `count` label members.
fn vcc_sheet(count: u32) -> SchematicSheet {
    let mut sheet = SchematicSheet {
        uuid: Uuid::now_v7(),
        version: 1,
        generator: "signex-test".into(),
        generator_version: "0.9".into(),
        paper_size: "A4".into(),
        ..Default::default()
    };
    for i in 0..count {
        let y = f64::from(i) * 5.08;
        sheet.wires.push(Wire {
            uuid: Uuid::from_u128(5800 + u128::from(i)),
            start: Point::new(0.0, y),
            end: Point::new(5.08, y),
            ..Default::default()
        });
        sheet.labels.push(Label {
            uuid: Uuid::from_u128(5900 + u128::from(i)),
            ..Label::new("VCC", Point::new(0.0, y), LabelType::Global)
        });
    }
    sheet
}

/// [`settle`], except the dock layout save is dropped, so opening a panel
/// does not write the layout into the run's shared config folder.
fn settle_quietly(app: &mut Signex, message: Message) {
    let mut driver = TaskDriver::new();
    let mut queue = std::collections::VecDeque::from([app.update(message)]);
    let mut steps = 0;
    while let Some(task) = queue.pop_front() {
        for message in driver.outputs(task).expect("the task chain settled") {
            steps += 1;
            assert!(steps < 1_000, "the task chain settles");
            if !matches!(message, Message::DockLayout(_)) {
                queue.push_back(app.update(message));
            }
        }
    }
}

fn inspector(app: &mut Signex, msg: NetInspectorMsg) {
    settle_quietly(
        app,
        Message::Dock(DockMessage::Panel(PanelMsg::NetInspector(msg))),
    );
}

/// The message the main-menu row of command `id` sends.
fn menu_row(id: &str) -> Message {
    Message::Menu(crate::menu_bar::MenuMessage::Command(
        crate::keymap::AppCommandId::new(id).expect("a command id"),
    ))
}

/// The members `VCC` lists and the count of its "+ N more" row.
fn vcc_listing(app: &Signex) -> (usize, usize) {
    let view = &app.document_state.panel_ctx.net_inspector;
    let vcc = view
        .nets
        .iter()
        .find(|n| n.name == "VCC")
        .expect("VCC is listed");
    (vcc.members.len(), vcc.more)
}

#[test]
fn an_expanded_net_lists_six_members_then_a_more_row_that_lists_the_rest() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let (project, sheet) = written_project(dir.path(), "Members");
    let text = signex_types::format::SnxSchematic::new(vcc_sheet(8))
        .write_string()
        .expect("encode");
    std::fs::write(&sheet, text).expect("write the sheet");

    let (mut app, _boot) = Signex::new();
    settle(&mut app, Message::File(FileMsg::Opened(Some(project))));
    settle(&mut app, Message::File(FileMsg::Opened(Some(sheet))));
    settle_quietly(&mut app, menu_row("open_net_inspector"));
    inspector(&mut app, NetInspectorMsg::NetToggled("VCC".into()));
    assert_eq!(vcc_listing(&app), (6, 2), "six members, then + 2 more");

    inspector(&mut app, NetInspectorMsg::MoreMembersShown("VCC".into()));
    assert_eq!(vcc_listing(&app), (8, 0), "the more row lists the rest");
}
