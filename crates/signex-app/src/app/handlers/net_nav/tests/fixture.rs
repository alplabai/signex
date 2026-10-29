//! Four sheets on disk, all open: `root` places `mid` and `third`.
//!
//! * `VCC` is a global label on root, mid and third (each with one resistor).
//! * `SIG` joins root's sheet entry on `mid`'s symbol to mid's hierarchical
//!   label.
//! * `ALONE` is a global label on mid that nothing else carries.

use std::path::PathBuf;

use signex_types::format::SnxSchematic;
use signex_types::schematic::{
    Label, LabelType, Point, SelectedItem, SelectedKind, SheetPin, Wire,
};
use uuid::Uuid;

use crate::app::handlers::menu::export::tests::{app_workspace, sheet_with_net};
use crate::app::{Message, NetNavMsg, Signex, UiMsg};

pub(super) fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

pub(super) const ROOT_VCC: u128 = 1;
pub(super) const ROOT_R1: u128 = 2;
pub(super) const ROOT_VCC_WIRE: u128 = 3;
pub(super) const SIG_ENTRY: u128 = 10;
pub(super) const MID_SYMBOL: u128 = 500;
pub(super) const MID_VCC: u128 = 21;
pub(super) const MID_R2: u128 = 22;
pub(super) const MID_SIG: u128 = 12;
pub(super) const ALONE: u128 = 24;
pub(super) const THIRD_VCC: u128 = 31;
pub(super) const THIRD_R3: u128 = 32;
/// An unlabelled net on root (R5) and one on third (R4): auto-named.
pub(super) const R5_WIRE: u128 = 42;
pub(super) const R4_WIRE: u128 = 41;
pub(super) const BROKEN: &str = "broken_page_7291.snxsch";
/// A page that breaks late; its own name keeps the shared log ring unambiguous.
pub(super) const PAGE_B: &str = "page_b_3318.snxsch";
pub(super) const CLASS_FIRST_PAGE: &str = "class_first_9921.snxsch";
pub(super) const LATE_PAGE: &str = "late_page_5503.snxsch";

pub(super) struct Project {
    pub app: Signex,
    pub root: PathBuf,
    pub mid: PathBuf,
    pub third: PathBuf,
    _dir: tempfile::TempDir,
}

fn label(text: &str, at: Point, ty: LabelType, uuid: u128) -> Label {
    Label {
        uuid: id(uuid),
        text: text.to_string(),
        position: at,
        label_type: ty,
        ..sheet_with_net("X", "X", &[]).labels.remove(0)
    }
}

fn wire(from: Point, to: Point, uuid: u128) -> Wire {
    Wire {
        uuid: id(uuid),
        start: from,
        end: to,
        ..Default::default()
    }
}

fn write(
    dir: &std::path::Path,
    name: &str,
    sheet: signex_types::schematic::SchematicSheet,
) -> PathBuf {
    let path = dir.join(name);
    let text = SnxSchematic::new(sheet).write_string().expect("encode");
    std::fs::write(&path, text).expect("write sheet");
    path
}

/// The project on disk with every sheet open and `root` focused.
pub(super) fn project() -> Project {
    build(false)
}

/// [`project`] plus a listed page that does not parse.
pub(super) fn project_with_broken_page() -> Project {
    build(true)
}

fn build(broken_page: bool) -> Project {
    // The Messages ring only records once the logger is installed.
    let _ = crate::diagnostics::init_logging();
    let dir = tempfile::tempdir().expect("tempdir");
    let pt = Point::new;

    let mut root = sheet_with_net("R1", "VCC", &["mid.snxsch", "third.snxsch"]);
    root.labels[0].uuid = id(ROOT_VCC);
    root.symbols[0].uuid = id(ROOT_R1);
    root.wires[0].uuid = id(ROOT_VCC_WIRE);
    root.child_sheets[0].uuid = id(MID_SYMBOL);
    root.child_sheets[0].name = "Mid".to_string();
    root.child_sheets[0].position = pt(30.0, 30.0);
    root.child_sheets[0].size = (20.0, 10.0);
    root.child_sheets[0].pins.push(SheetPin {
        uuid: id(SIG_ENTRY),
        name: "SIG".to_string(),
        direction: String::new(),
        position: pt(20.0, 0.0),
        rotation: 0.0,
        auto_generated: false,
        user_moved: false,
    });
    root.child_sheets[1].uuid = id(501);
    root.child_sheets[1].name = "Third".to_string();
    root.wires.push(wire(pt(20.0, 0.0), pt(30.0, 0.0), 11));

    let mut mid = sheet_with_net("R2", "VCC", &[]);
    mid.labels[0].uuid = id(MID_VCC);
    mid.symbols[0].uuid = id(MID_R2);
    mid.wires.push(wire(pt(20.0, 0.0), pt(30.0, 0.0), 13));
    mid.labels.push(label(
        "SIG",
        pt(20.0, 0.0),
        LabelType::Hierarchical,
        MID_SIG,
    ));
    mid.wires.push(wire(pt(0.0, 20.0), pt(5.0, 20.0), 23));
    mid.labels
        .push(label("ALONE", pt(0.0, 20.0), LabelType::Global, ALONE));

    let mut third = sheet_with_net("R3", "VCC", &[]);
    third.labels[0].uuid = id(THIRD_VCC);
    third.symbols[0].uuid = id(THIRD_R3);

    for (sheet, wire_id, sym_id, reference) in [
        (&mut root, R5_WIRE, 43, "R5"),
        (&mut third, R4_WIRE, 33, "R4"),
    ] {
        sheet
            .wires
            .push(wire(pt(0.0, 50.0), pt(5.0, 50.0), wire_id));
        let mut symbol = sheet.symbols[0].clone();
        symbol.uuid = id(sym_id);
        symbol.reference = reference.to_string();
        symbol.position = pt(0.0, 50.0);
        sheet.symbols.push(symbol);
    }

    let root_path = write(dir.path(), "root.snxsch", root);
    let mid_path = write(dir.path(), "mid.snxsch", mid);
    let third_path = write(dir.path(), "third.snxsch", third);

    let mut listed = vec!["root.snxsch", "mid.snxsch", "third.snxsch"];
    if broken_page {
        std::fs::write(dir.path().join(BROKEN), "not a schematic").expect("write broken page");
        listed.push(BROKEN);
    }
    let mut app = app_workspace(&dir.path().to_string_lossy(), &listed);
    for path in [&third_path, &mid_path, &root_path] {
        assert!(app.open_or_focus_sheet_path(path.clone()), "open {path:?}");
    }
    Project {
        app,
        root: root_path,
        mid: mid_path,
        third: third_path,
        _dir: dir,
    }
}

impl Project {
    pub fn send(&mut self, msg: NetNavMsg) {
        let _task = self.app.update(Message::Ui(UiMsg::NetNav(msg)));
    }

    /// Run `msg`, then finish the index build it waited for.
    pub fn send_settled(&mut self, msg: NetNavMsg) {
        self.send(msg);
        self.app.settle_net_index_blocking();
    }

    pub fn focus(&mut self, path: &std::path::Path) {
        assert!(self.app.open_or_focus_sheet_path(path.to_path_buf()));
    }

    pub fn select_uuid(&mut self, uuid: Uuid, kind: SelectedKind) {
        self.app.interaction_state.active_canvas_mut().selected =
            vec![SelectedItem::new(uuid, kind)];
    }

    pub fn select(&mut self, uuid: u128, kind: SelectedKind) {
        self.app.interaction_state.active_canvas_mut().selected =
            vec![SelectedItem::new(id(uuid), kind)];
    }

    pub fn active(&self) -> Option<PathBuf> {
        self.app.document_state.active_path.clone()
    }

    pub fn selected(&self) -> Vec<SelectedItem> {
        self.app.interaction_state.active_canvas().selected.clone()
    }

    pub fn bright(&self) -> Option<std::collections::HashSet<Uuid>> {
        self.app
            .interaction_state
            .active_canvas()
            .net_highlight()
            .cloned()
    }
}

/// Whether the Messages ring holds an entry mentioning every fragment.
pub(super) fn logged(fragments: &[&str]) -> bool {
    crate::diagnostics::recent_entries()
        .iter()
        .any(|e| fragments.iter().all(|f| e.message.contains(f)))
}

pub(super) const TWICE_A: u128 = 700;
pub(super) const TWICE_B: u128 = 701;
pub(super) const TWICE_LABEL: u128 = 710;
pub(super) const TWICE_ENTRY_B: u128 = 721;

/// `root` places `child` twice (A and B); each entry is wired to its own
/// parent net, so each placement is its own net. Returns the project with
/// both sheets open and root focused, and the child's path.
pub(super) fn placed_twice() -> (Project, PathBuf) {
    let _ = crate::diagnostics::init_logging();
    let dir = tempfile::tempdir().expect("tempdir");
    let pt = Point::new;
    let mut root = sheet_with_net("R1", "ROOT_ONLY", &["child.snxsch", "child.snxsch"]);
    for (i, (symbol, name, y)) in [(TWICE_A, "A", 20.0), (TWICE_B, "B", 40.0)]
        .into_iter()
        .enumerate()
    {
        let cs = &mut root.child_sheets[i];
        cs.uuid = id(symbol);
        cs.name = name.to_string();
        cs.pins.push(SheetPin {
            uuid: id(symbol + 20),
            name: "SIG".to_string(),
            direction: String::new(),
            position: pt(0.0, y),
            rotation: 0.0,
            auto_generated: false,
            user_moved: false,
        });
        root.wires
            .push(wire(pt(0.0, y), pt(5.0, y), 800 + i as u128));
    }
    let mut child = sheet_with_net("R9", "CHILD_ONLY", &[]);
    child.wires.push(wire(pt(0.0, 70.0), pt(10.0, 70.0), 811));
    child.labels.push(label(
        "SIG",
        pt(0.0, 70.0),
        LabelType::Hierarchical,
        TWICE_LABEL,
    ));
    let root_path = write(dir.path(), "root.snxsch", root);
    let child_path = write(dir.path(), "child.snxsch", child);
    let mut app = app_workspace(
        &dir.path().to_string_lossy(),
        &["root.snxsch", "child.snxsch"],
    );
    for path in [&child_path, &root_path] {
        assert!(app.open_or_focus_sheet_path(path.clone()));
    }
    let project = Project {
        app,
        root: root_path,
        mid: child_path.clone(),
        third: child_path.clone(),
        _dir: dir,
    };
    (project, child_path)
}

pub(super) const ONLY_B: &str = "ONLYB";
pub(super) const B_WIRE: u128 = 901;
pub(super) const B_SYMBOL: u128 = 902;
pub(super) const B_LABEL: u128 = 900;

impl Project {
    /// Add a second, unrelated project with one open sheet carrying the
    /// global label `ONLYB`, and focus it. Returns that sheet's path and the
    /// label's uuid.
    pub fn add_second_project(&mut self) -> (PathBuf, Uuid) {
        self.add_project_b(None)
    }

    /// [`Self::add_second_project`] listing one more page that does not parse.
    pub fn add_second_project_with_broken_page(&mut self, page: &str) -> (PathBuf, Uuid) {
        self.add_project_b(Some(page))
    }

    fn add_project_b(&mut self, broken: Option<&str>) -> (PathBuf, Uuid) {
        let dir = tempfile::tempdir().expect("tempdir");
        let label_id = id(900);
        let mut sheet = sheet_with_net("RB", ONLY_B, &[]);
        sheet.labels[0].uuid = label_id;
        sheet.wires[0].uuid = id(B_WIRE);
        sheet.symbols[0].uuid = id(B_SYMBOL);
        let path = write(dir.path(), "b_only.snxsch", sheet);
        let mut listed = vec!["b_only.snxsch"];
        if let Some(page) = broken {
            std::fs::write(dir.path().join(page), "not a schematic").expect("write page");
            listed.push(page);
        }
        let mut other = app_workspace(&dir.path().to_string_lossy(), &listed);
        let mut project = other.document_state.projects.remove(0);
        project.id = self.app.document_state.mint_project_id();
        self.app.document_state.projects.push(project);
        assert!(self.app.open_or_focus_sheet_path(path.clone()));
        std::mem::forget(dir);
        (path, label_id)
    }
}

/// `root` and `c.snxsch` each carry two global `VCC` labels, one at each end of
/// a wire: ports 1 and 2 on root, 5 and 6 on the child. Both sheets are open,
/// root focused.
pub(super) fn ring_project() -> Project {
    let _ = crate::diagnostics::init_logging();
    let dir = tempfile::tempdir().expect("tempdir");
    let pt = Point::new;
    let mut root = sheet_with_net("R1", "VCC", &["c.snxsch"]);
    root.labels[0].uuid = id(1);
    root.labels
        .push(label("VCC", pt(10.0, 0.0), LabelType::Global, 2));
    let mut child = sheet_with_net("R2", "VCC", &[]);
    child.labels[0].uuid = id(5);
    child
        .labels
        .push(label("VCC", pt(10.0, 0.0), LabelType::Global, 6));
    let root_path = write(dir.path(), "root.snxsch", root);
    let child_path = write(dir.path(), "c.snxsch", child);
    let mut app = app_workspace(&dir.path().to_string_lossy(), &["root.snxsch", "c.snxsch"]);
    for path in [&child_path, &root_path] {
        assert!(app.open_or_focus_sheet_path(path.clone()));
    }
    Project {
        app,
        root: root_path,
        mid: child_path.clone(),
        third: child_path,
        _dir: dir,
    }
}
