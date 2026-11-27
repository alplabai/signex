//! Mockup row 158 - Named unions: Create Union (the menu row and Ctrl+G) opens a
//! popover whose Create is one undo step; the Unions panel locks, hides and
//! breaks; hiding is a view setting, not an undo step; a locked union's
//! members cannot be deleted.

use signex_app::app::unions::{UnionPopoverMsg, UnionsMsg};
use signex_app::app::{EditMsg, Message, Signex};
use signex_app::dock::DockMessage;
use signex_app::keymap::{AppCommandId, ShortcutProfileSet};
use signex_app::panels::PanelMsg;
use signex_types::format::SnxSchematic;
use signex_types::schematic::SchematicSheet;

use crate::paste_support::sheet::{app_with_sheet, populated_sheet, select};

fn doc(app: &Signex) -> &SchematicSheet {
    app.document_state.active_engine().unwrap().document()
}

/// The active sheet as the file would hold it.
fn written(app: &Signex) -> String {
    SnxSchematic::new(doc(app).clone()).write_string().unwrap()
}

fn undo_position(app: &Signex) -> usize {
    app.document_state.active_engine().unwrap().undo_position()
}

fn unions(app: &mut Signex, msg: UnionsMsg) {
    let _ = app.update(Message::Edit(EditMsg::Unions(msg)));
}

fn panel(app: &mut Signex, msg: UnionsMsg) {
    let _ = app.update(Message::Dock(DockMessage::Panel(PanelMsg::Unions(msg))));
}

/// Mockup row 158: Create Union is the panel's title-bar +, off with nothing
/// selected.
#[test]
fn the_title_bar_plus_opens_the_create_popover_for_the_selection() {
    let mut app = app_with_sheet(populated_sheet());
    let _ = app.update(Message::Menu(
        signex_app::menu_bar::MenuMessage::OpenUnionsPanel,
    ));
    panel(&mut app, UnionsMsg::FilterEdited(String::new()));
    let plus = |app: &Signex| {
        signex_app::panels::title_actions(
            signex_app::panels::PanelKind::Unions,
            &app.document_state.panel_ctx,
        )
        .into_iter()
        .next()
        .expect("the panel has a + in its title bar")
        .on_press
    };
    assert!(plus(&app).is_none(), "nothing is selected");

    select(&mut app, &[("R1", 1), ("R2", 1)], false);
    panel(&mut app, UnionsMsg::FilterEdited(String::new()));
    let press = plus(&app).expect("a selection can be grouped");
    let _ = app.update(Message::Dock(DockMessage::Panel(press)));
    assert!(app.ui_state.unions.popover.is_some());
}

/// R1 and R2 grouped as "Input filter" through the popover.
fn app_with_union() -> Signex {
    let mut app = app_with_sheet(populated_sheet());
    select(&mut app, &[("R1", 1), ("R2", 1)], false);
    unions(&mut app, UnionsMsg::CreateRequested);
    let popover = app
        .ui_state
        .unions
        .popover
        .as_ref()
        .expect("the popover opened");
    assert_eq!(popover.members.len(), 2);
    unions(
        &mut app,
        UnionsMsg::Popover(UnionPopoverMsg::NameEdited("Input filter".into())),
    );
    unions(
        &mut app,
        UnionsMsg::Popover(UnionPopoverMsg::TagInputEdited("#emc".into())),
    );
    unions(&mut app, UnionsMsg::Popover(UnionPopoverMsg::TagSubmitted));
    unions(&mut app, UnionsMsg::Popover(UnionPopoverMsg::Confirmed));
    app
}

#[test]
fn create_union_is_one_undo_step() {
    let mut app = app_with_sheet(populated_sheet());
    let before = undo_position(&app);
    select(&mut app, &[("R1", 1), ("R2", 1)], false);
    unions(&mut app, UnionsMsg::CreateRequested);
    // Create waits for a name.
    unions(&mut app, UnionsMsg::Popover(UnionPopoverMsg::Confirmed));
    assert!(doc(&app).unions.is_empty(), "no union without a name");
    drop(app);

    let mut app = app_with_union();
    assert!(app.ui_state.unions.popover.is_none(), "Create closes it");
    let made = &doc(&app).unions[0];
    assert_eq!(made.name, "Input filter");
    assert_eq!(made.tags, ["emc"]);
    assert_eq!(made.members.len(), 2);
    assert_eq!(undo_position(&app), before + 1, "one undo step");
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert!(doc(&app).unions.is_empty());
}

#[test]
fn ctrl_g_creates_a_union_in_the_default_profile_only() {
    let id = AppCommandId::new("create_union").unwrap();
    // The built-ins start on the default profile.
    let mut set = ShortcutProfileSet::built_ins().expect("built-ins parse");
    let expected = if cfg!(target_os = "macos") {
        "\u{2318}G"
    } else {
        "Ctrl+G"
    };
    assert_eq!(
        set.compile_active().shortcut_label(&id).as_deref(),
        Some(expected)
    );
    // The classic profile keeps Ctrl+G for the search panel.
    set.set_active_profile("classic").expect("profile exists");
    assert_eq!(set.compile_active().shortcut_label(&id), None);
}

#[test]
fn hiding_a_union_is_not_an_undo_step_and_changes_no_document() {
    let mut app = app_with_union();
    let path = app.document_state.active_path.clone().unwrap();
    let union = doc(&app).unions[0].uuid;
    let (steps, sheet) = (undo_position(&app), written(&app));
    panel(
        &mut app,
        UnionsMsg::HideToggled {
            sheet: path.clone(),
            union,
        },
    );
    assert!(app.ui_state.unions.is_hidden(&path, union));
    assert_eq!(undo_position(&app), steps, "hide is not an undo step");
    assert_eq!(written(&app), sheet, "the document is unchanged");
    panel(
        &mut app,
        UnionsMsg::HideToggled {
            sheet: path.clone(),
            union,
        },
    );
    assert!(!app.ui_state.unions.is_hidden(&path, union));
}

#[test]
fn a_locked_union_keeps_its_members_until_unlocked() {
    let mut app = app_with_union();
    let path = app.document_state.active_path.clone().unwrap();
    let union = doc(&app).unions[0].uuid;
    panel(
        &mut app,
        UnionsMsg::LockToggled {
            sheet: path.clone(),
            union,
        },
    );
    assert!(doc(&app).unions[0].locked);
    select(&mut app, &[("R1", 1)], false);
    let _ = app.update(Message::Edit(EditMsg::DeleteSelected));
    assert_eq!(doc(&app).symbols.len(), 5, "a locked union's member stays");
    panel(&mut app, UnionsMsg::LockToggled { sheet: path, union });
    select(&mut app, &[("R1", 1)], false);
    let _ = app.update(Message::Edit(EditMsg::DeleteSelected));
    assert_eq!(doc(&app).symbols.len(), 4);
    assert_eq!(
        doc(&app).unions[0].members.len(),
        1,
        "the deleted part left its union"
    );
}

/// The tab of the active sheet's first union, at `scale` px per mm.
fn first_tab(app: &Signex, scale: f32) -> signex_types::schematic::Aabb {
    use signex_app::schematic_runtime::{Designators, WireWidths, unions::union_frame};
    let sheet = doc(app);
    union_frame(
        sheet,
        &sheet.unions[0],
        &Designators::new(),
        WireWidths::default(),
        scale,
    )
    .expect("the members are on the sheet")
    .tab
}

/// Mockup row 158: only a locked union shows a padlock on its tab, so
/// locking widens the tab and unlocking gives the room back.
#[test]
fn locking_a_union_makes_room_for_the_padlock_on_its_tab() {
    let mut app = app_with_union();
    let path = app.document_state.active_path.clone().unwrap();
    let union = doc(&app).unions[0].uuid;
    let open = first_tab(&app, 4.0);
    let toggle = |app: &mut Signex| {
        panel(
            app,
            UnionsMsg::LockToggled {
                sheet: path.clone(),
                union,
            },
        );
    };
    toggle(&mut app);
    let locked = first_tab(&app, 4.0);
    assert_eq!(locked.min_x, open.min_x);
    assert!(
        locked.max_x > open.max_x,
        "the locked tab holds the padlock"
    );
    toggle(&mut app);
    assert_eq!(first_tab(&app, 4.0), open, "unlocked, the room goes");
}

#[test]
fn break_union_removes_it_and_keeps_the_objects() {
    let mut app = app_with_union();
    select(&mut app, &[("R1", 1)], false);
    unions(&mut app, UnionsMsg::BreakRequested);
    assert!(doc(&app).unions.is_empty());
    assert_eq!(doc(&app).symbols.len(), 5);
}

#[test]
fn add_and_remove_selection_change_the_members() {
    let mut app = app_with_union();
    select(&mut app, &[("R3", 1)], false);
    unions(&mut app, UnionsMsg::AddSelectionRequested);
    assert_eq!(doc(&app).unions[0].members.len(), 3);
    select(&mut app, &[("R1", 1)], false);
    unions(&mut app, UnionsMsg::RemoveSelectionRequested);
    assert_eq!(doc(&app).unions[0].members.len(), 2);
}

/// The name the panel foot's field shows for the picked union.
fn foot_name(app: &Signex) -> String {
    app.document_state
        .panel_ctx
        .unions
        .foot
        .as_ref()
        .expect("the picked union has a foot")
        .name
        .clone()
}

fn open_unions_panel(app: &mut Signex) {
    let _ = app.update(Message::Menu(
        signex_app::menu_bar::MenuMessage::OpenUnionsPanel,
    ));
}

/// #2745: the foot's name field keeps a trailing space while the union takes
/// the trimmed name, so "Input f" can be typed, and the keys are one undo
/// step.
#[test]
fn the_rename_field_keeps_a_trailing_space_and_the_keys_are_one_undo_step() {
    let mut app = app_with_union();
    open_unions_panel(&mut app);
    let before = undo_position(&app);
    panel(&mut app, UnionsMsg::RenameEdited("Input".into()));
    panel(&mut app, UnionsMsg::RenameEdited("Input ".into()));
    assert_eq!(foot_name(&app), "Input ", "the space stays in the field");
    assert_eq!(
        doc(&app).unions[0].name,
        "Input",
        "the name is stored trimmed"
    );
    panel(&mut app, UnionsMsg::RenameEdited("Input f".into()));
    assert_eq!(doc(&app).unions[0].name, "Input f");
    assert_eq!(foot_name(&app), "Input f");
    assert_eq!(undo_position(&app), before + 1, "one undo step");
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(doc(&app).unions[0].name, "Input filter");
    assert_eq!(
        foot_name(&app),
        "Input filter",
        "the field follows the undo"
    );
}

/// #2745: a cleared name field stays empty instead of snapping back, and
/// renames nothing.
#[test]
fn a_cleared_rename_field_stays_empty_and_renames_nothing() {
    let mut app = app_with_union();
    open_unions_panel(&mut app);
    let before = undo_position(&app);
    panel(&mut app, UnionsMsg::RenameEdited(String::new()));
    assert_eq!(foot_name(&app), "");
    assert_eq!(doc(&app).unions[0].name, "Input filter");
    assert_eq!(undo_position(&app), before, "no undo step");
}

/// #2791: a locked union keeps its name, through the panel too; its field
/// shows the stored name.
#[test]
fn a_locked_union_keeps_its_name() {
    let mut app = app_with_union();
    open_unions_panel(&mut app);
    let path = app.document_state.active_path.clone().unwrap();
    let union = doc(&app).unions[0].uuid;
    panel(&mut app, UnionsMsg::LockToggled { sheet: path, union });
    assert!(doc(&app).unions[0].locked);
    let (steps, sheet) = (undo_position(&app), written(&app));
    panel(&mut app, UnionsMsg::RenameEdited("Renamed".into()));
    assert_eq!(doc(&app).unions[0].name, "Input filter");
    assert_eq!(undo_position(&app), steps, "no undo step");
    assert_eq!(written(&app), sheet, "the document is unchanged");
    assert_eq!(foot_name(&app), "Input filter");
}

/// #2745: every canvas gesture that adds a union member to the selection
/// brings the whole union along, so the union moves as one.
mod moves_as_one {
    use signex_app::active_bar::{ActiveBarAction, ActiveBarMsg};
    use signex_app::app::unions::{UnionPopoverMsg, UnionsMsg};
    use signex_app::app::{EditMsg, Message, Signex};
    use signex_app::canvas::{BoxDirection, CanvasEvent};
    use signex_types::schematic::{Point, SelectedItem, SelectedKind, Wire};
    use std::path::PathBuf;
    use uuid::Uuid;

    use crate::click_click_move::{app_with_wires, wire_start};

    fn wire(a: (f64, f64), b: (f64, f64)) -> Wire {
        Wire {
            color: None,
            unknown_columns: Default::default(),
            uuid: Uuid::now_v7(),
            start: Point::new(a.0, a.1),
            end: Point::new(b.0, b.1),
            stroke_width: 0.0,
            locked: false,
        }
    }

    /// A union "Pair" of two wires, nothing selected: `near` lies in the box
    /// x 0..30, y 0..10, `far` well outside it.
    struct Pair {
        app: Signex,
        path: PathBuf,
        near: Uuid,
        far: Uuid,
    }

    fn pair() -> Pair {
        let near = wire((5.0, 5.0), (15.0, 5.0));
        let far = wire((40.0, 20.0), (60.0, 20.0));
        let (near, far, wires) = (near.uuid, far.uuid, vec![near, far]);
        let (mut app, path) = app_with_wires(wires);
        app.interaction_state.active_canvas_mut().selected = vec![
            SelectedItem::new(near, SelectedKind::Wire),
            SelectedItem::new(far, SelectedKind::Wire),
        ];
        let send = |app: &mut Signex, msg: UnionsMsg| {
            let _ = app.update(Message::Edit(EditMsg::Unions(msg)));
        };
        send(&mut app, UnionsMsg::CreateRequested);
        send(
            &mut app,
            UnionsMsg::Popover(UnionPopoverMsg::NameEdited("Pair".into())),
        );
        send(&mut app, UnionsMsg::Popover(UnionPopoverMsg::Confirmed));
        assert_eq!(app.document_state.engines[&path].document().unions.len(), 1);
        app.interaction_state.active_canvas_mut().selected.clear();
        Pair {
            app,
            path,
            near,
            far,
        }
    }

    fn selected(app: &Signex) -> Vec<Uuid> {
        let mut ids: Vec<Uuid> = app
            .interaction_state
            .active_canvas()
            .selected
            .iter()
            .map(|i| i.uuid)
            .collect();
        ids.sort();
        ids
    }

    impl Pair {
        fn both(&self) -> Vec<Uuid> {
            let mut ids = vec![self.near, self.far];
            ids.sort();
            ids
        }

        /// Drop a drag of the selection 2.54 mm to the right, and check that
        /// both wires moved by it.
        fn drag_moves_both(&mut self) {
            let _ = self
                .app
                .update(Message::CanvasEvent(CanvasEvent::MoveSelected {
                    dx: 2.54,
                    dy: 0.0,
                    connections: Default::default(),
                }));
            let near = wire_start(&self.app, &self.path, self.near);
            let far = wire_start(&self.app, &self.path, self.far);
            assert!((near.x - 7.54).abs() < 1e-6, "the gripped member moves");
            assert!(
                (far.x - 42.54).abs() < 1e-6,
                "the other member moves by the same amount"
            );
        }

        fn ctrl_click(&mut self, x: f64, y: f64) {
            let _ = self
                .app
                .update(Message::CanvasEvent(CanvasEvent::CtrlClicked {
                    world_x: x,
                    world_y: y,
                }));
        }
    }

    #[test]
    fn a_box_round_one_member_selects_and_moves_the_whole_union() {
        let mut pair = pair();
        let _ = pair
            .app
            .update(Message::CanvasEvent(CanvasEvent::BoxSelect {
                x1: 0.0,
                y1: 0.0,
                x2: 30.0,
                y2: 10.0,
                direction: BoxDirection::LeftToRight,
            }));
        assert_eq!(selected(&pair.app), pair.both());
        pair.drag_moves_both();
    }

    /// Ctrl+click (and Shift+click, which sends the same event) on a member
    /// adds its whole union; Ctrl+click on a selected member takes that one
    /// off, which is how a selection narrows for Remove Selection.
    #[test]
    fn ctrl_click_adds_the_whole_union_and_takes_off_one_member() {
        let mut pair = pair();
        pair.ctrl_click(50.0, 20.0);
        assert_eq!(selected(&pair.app), pair.both());
        pair.ctrl_click(10.0, 5.0);
        assert_eq!(selected(&pair.app), vec![pair.far]);
        pair.ctrl_click(10.0, 5.0);
        assert_eq!(selected(&pair.app), pair.both());
        pair.drag_moves_both();
    }

    #[test]
    fn a_click_click_move_pick_on_a_member_lifts_the_whole_union() {
        let mut pair = pair();
        let _ = pair.app.update(Message::ActiveBar(ActiveBarMsg::Action(
            ActiveBarAction::Move,
        )));
        let _ = pair
            .app
            .update(Message::CanvasEvent(CanvasEvent::MovePicked {
                world_x: 10.0,
                world_y: 5.0,
            }));
        assert_eq!(selected(&pair.app), pair.both());
        pair.drag_moves_both();
    }
}
