//! #1269 - Tools ▸ Net Inspector opens a panel that lists every net of the
//! active project from the stitched net index, filterable, each expandable
//! to its members; a member click goes to it.
//!
//! The project of `net_nav`: `a.snxsch` and `b.snxsch`, each a wire with the
//! global label `VCC`, both open with `a` focused.

use signex_app::app::command_palette::{CommandAction, build_catalog};
use signex_app::app::{CommandPaletteMsg, Message, Signex, UiMsg};
use signex_app::dock::DockMessage;
use signex_app::keymap::ShortcutTrigger;
use signex_app::panels::net_inspector::{NetListStatus, NetMemberRowKind};
use signex_app::panels::{NetInspectorMsg, PanelKind, PanelMsg};
use signex_types::schematic::{SelectedItem, SelectedKind};
use tempfile::TempDir;

use signex_app::task_driver::TaskDriver;

use crate::net_nav::flows::open_project;

const COMMAND: &str = "open_net_inspector";

/// `settle_support::settle`, except the dock layout save is dropped: opening
/// a panel would otherwise write the layout into the test run's shared config
/// folder, and the next `Signex::new()` in this process would start with the
/// panel open. `settle_support` feeds every message back and has no way to
/// skip one, so this thin wrapper runs the same `TaskDriver` one level at a
/// time.
fn settle(app: &mut Signex, message: Message) {
    let task = app.update(message);
    settle_task(app, task);
}

fn settle_task(app: &mut Signex, task: iced::Task<Message>) {
    let mut driver = TaskDriver::new();
    let mut queue = std::collections::VecDeque::from([task]);
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
    settle(
        app,
        Message::Dock(DockMessage::Panel(PanelMsg::NetInspector(msg))),
    );
}

/// The panel is open and lists the project's one net.
fn assert_listed(app: &Signex) {
    assert!(
        app.document_state
            .dock
            .locate(PanelKind::NetInspector)
            .is_some(),
        "panel open"
    );
    let view = &app.document_state.panel_ctx.net_inspector;
    assert_eq!(view.status, NetListStatus::Ready);
    assert_eq!(view.total, 1, "VCC is the project's one net");
    assert_eq!(view.nets[0].name, "VCC");
}

#[test]
fn the_menu_row_opens_the_panel_and_lists_the_projects_nets() {
    let tmp = TempDir::new().unwrap();
    let (mut app, _a, _b) = open_project(tmp.path());
    settle(&mut app, crate::menu_support::menu_row(COMMAND));
    assert_listed(&app);
    let vcc = &app.document_state.panel_ctx.net_inspector.nets[0];
    assert_eq!(vcc.pins, 0, "two labels, no component pin");
    assert!(!vcc.expanded && vcc.members.is_empty());
}

#[test]
fn a_net_expands_to_its_members_on_every_sheet_and_the_filter_narrows_the_list() {
    let tmp = TempDir::new().unwrap();
    let (mut app, _a, _b) = open_project(tmp.path());
    settle(&mut app, crate::menu_support::menu_row(COMMAND));
    inspector(&mut app, NetInspectorMsg::NetToggled("VCC".into()));

    let vcc = &app.document_state.panel_ctx.net_inspector.nets[0];
    assert!(vcc.expanded);
    let mut sheets: Vec<&str> = vcc.members.iter().map(|m| m.sheet.as_str()).collect();
    sheets.sort_unstable();
    assert_eq!(sheets.len(), 2, "one label per sheet: {sheets:?}");
    assert!(
        sheets[0].contains('a') && sheets[1].contains('b'),
        "{sheets:?}"
    );
    assert!(
        vcc.members
            .iter()
            .all(|m| m.kind == NetMemberRowKind::Label && m.text == "VCC")
    );

    inspector(&mut app, NetInspectorMsg::FilterChanged("gnd".into()));
    let view = &app.document_state.panel_ctx.net_inspector;
    assert!(view.nets.is_empty(), "no net matches");
    assert_eq!(view.total, 1, "the total still counts every net");
    inspector(&mut app, NetInspectorMsg::FilterChanged("vc".into()));
    assert_eq!(app.document_state.panel_ctx.net_inspector.nets.len(), 1);
}

#[test]
fn clicking_a_member_on_another_sheet_goes_there_and_highlights_the_net() {
    let tmp = TempDir::new().unwrap();
    let (mut app, _a, b) = open_project(tmp.path());
    settle(&mut app, crate::menu_support::menu_row(COMMAND));
    inspector(&mut app, NetInspectorMsg::NetToggled("VCC".into()));
    let on_b = app.document_state.panel_ctx.net_inspector.nets[0]
        .members
        .iter()
        .find(|m| m.sheet.contains('b'))
        .expect("the label on b")
        .clone();

    inspector(
        &mut app,
        NetInspectorMsg::MemberClicked {
            net: "VCC".into(),
            anchor: on_b.anchor.clone(),
        },
    );

    assert_eq!(app.document_state.active_path.as_deref(), Some(b.as_path()));
    assert_eq!(
        app.interaction_state.active_canvas().selected,
        vec![SelectedItem::new(on_b.anchor.id, SelectedKind::Label)]
    );
    let highlight = app.document_state.net_nav.highlight.as_ref();
    assert_eq!(highlight.map(|h| h.name.as_str()), Some("VCC"));
}

#[test]
fn the_palette_lists_and_runs_it() {
    let tmp = TempDir::new().unwrap();
    let (mut app, _a, _b) = open_project(tmp.path());
    let listed = build_catalog(&app)
        .iter()
        .any(|entry| matches!(&entry.action, CommandAction::Command(id) if id.as_str() == COMMAND));
    assert!(listed, "the palette offers the Net Inspector");
    settle(&mut app, Message::CommandPalette(CommandPaletteMsg::Open));
    settle(
        &mut app,
        Message::CommandPalette(CommandPaletteMsg::QueryChanged(
            "Open Net Inspector panel".into(),
        )),
    );
    settle(
        &mut app,
        Message::CommandPalette(CommandPaletteMsg::ExecuteSelected),
    );
    assert_listed(&app);
}

#[test]
fn a_user_key_binding_runs_it() {
    let tmp = TempDir::new().unwrap();
    let (mut app, _a, _b) = open_project(tmp.path());
    crate::assign_footprints::bind(&mut app, COMMAND, "Ctrl+Alt+Shift+F10");
    let ShortcutTrigger::KeySequence(mut strokes) =
        ShortcutTrigger::parse("Ctrl+Alt+Shift+F10").unwrap()
    else {
        panic!("a key");
    };
    let task = app.update(Message::Ui(UiMsg::KeymapStroke {
        window: None,
        stroke: strokes.remove(0),
    }));
    settle_task(&mut app, task);
    assert_listed(&app);
}

#[test]
fn a_loose_sheet_has_no_project_nets() {
    let (mut app, _boot) = Signex::new();
    settle(&mut app, crate::menu_support::menu_row(COMMAND));
    assert!(
        app.document_state
            .dock
            .locate(PanelKind::NetInspector)
            .is_some()
    );
    assert_eq!(
        app.document_state.panel_ctx.net_inspector.status,
        NetListStatus::NoProject
    );
}

/// Open the panel, expand VCC and return its member on sheet `b`.
fn member_on_b(app: &mut Signex) -> signex_app::panels::net_inspector::NetMemberRow {
    settle(app, crate::menu_support::menu_row(COMMAND));
    inspector(app, NetInspectorMsg::NetToggled("VCC".into()));
    app.document_state.panel_ctx.net_inspector.nets[0]
        .members
        .iter()
        .find(|m| m.sheet.contains('b'))
        .expect("the label on b")
        .clone()
}

/// A click on a member whose sheet is not open opens it (`go_to_member`
/// reads it on a worker) and selects the member there.
#[test]
fn clicking_a_member_on_a_closed_sheet_opens_it() {
    let tmp = TempDir::new().unwrap();
    let (mut app, _a, b) = open_project(tmp.path());
    let window = app.ui_state.main_window_id.expect("window");
    let b_tab = app
        .document_state
        .tabs
        .iter()
        .position(|t| t.path == b)
        .expect("b's tab");
    settle(
        &mut app,
        Message::Tab {
            window_id: window,
            msg: signex_app::tab_bar::TabMessage::CloseClicked(b_tab),
        },
    );
    assert!(!app.document_state.engines.contains_key(&b), "b is closed");

    let on_b = member_on_b(&mut app);
    inspector(
        &mut app,
        NetInspectorMsg::MemberClicked {
            net: "VCC".into(),
            anchor: on_b.anchor.clone(),
        },
    );

    assert_eq!(app.document_state.active_path.as_deref(), Some(b.as_path()));
    assert_eq!(
        app.interaction_state.active_canvas().selected,
        vec![SelectedItem::new(on_b.anchor.id, SelectedKind::Label)]
    );
}

/// A member that is no longer on the net is said in Messages, and nothing
/// moves or lights up.
#[test]
fn a_member_that_left_its_net_is_said_and_nothing_moves() {
    let _ = signex_app::diagnostics::init_logging();
    let tmp = TempDir::new().unwrap();
    let (mut app, a, _b) = open_project(tmp.path());
    let mut gone = member_on_b(&mut app).anchor;
    gone.id = uuid::Uuid::from_u128(0xDEAD);
    inspector(
        &mut app,
        NetInspectorMsg::MemberClicked {
            net: "VCC".into(),
            anchor: gone,
        },
    );

    assert_eq!(app.document_state.active_path.as_deref(), Some(a.as_path()));
    assert!(app.document_state.net_nav.highlight.is_none());
    assert!(
        signex_app::diagnostics::recent_entries()
            .iter()
            .any(|e| e.message.contains("no longer on net 'VCC'")),
        "the Messages line names the net"
    );
}
