//! Mockup row 158: named unions. The create popover, the Unions panel's
//! filter, tags, scope and row actions, the foot's member edits, the lock and
//! the view-only Hide.

use signex_app::app::unions::{UnionPopoverMsg, UnionScope, UnionsMsg};
use signex_app::app::{EditMsg, Message};
use signex_app::menu_bar::MenuMessage;
use signex_app::panels::PanelMsg;
use signex_types::schematic::SchUnion;
use uuid::Uuid;

use super::support::{Bench, bench, empty_sheet, part, project_bench};

fn parts_bench() -> Bench {
    let mut sheet = empty_sheet();
    sheet.symbols = vec![
        part("R1", (10.16, 10.16)),
        part("R2", (20.32, 10.16)),
        part("R3", (30.48, 10.16)),
    ];
    bench(sheet)
}

fn unions(b: &mut Bench, msg: UnionsMsg) {
    b.post(Message::Edit(EditMsg::Unions(msg)));
}

fn popover(b: &mut Bench, msg: UnionPopoverMsg) {
    unions(b, UnionsMsg::Popover(msg));
}

fn row(b: &mut Bench, msg: UnionsMsg) {
    b.panel(PanelMsg::Unions(msg));
}

fn open_panel(b: &mut Bench) {
    b.post(Message::Menu(MenuMessage::OpenUnionsPanel));
    row(b, UnionsMsg::FilterEdited(String::new()));
}

/// R1 and R2 as "Input filter", tagged `emc`, at a fresh save point.
fn grouped() -> (Bench, Uuid) {
    let mut sheet = empty_sheet();
    sheet.symbols = vec![
        part("R1", (10.16, 10.16)),
        part("R2", (20.32, 10.16)),
        part("R3", (30.48, 10.16)),
    ];
    let id = Uuid::now_v7();
    sheet.unions.push(SchUnion {
        uuid: id,
        name: "Input filter".to_string(),
        tags: vec!["emc".to_string()],
        members: vec![sheet.symbols[0].uuid, sheet.symbols[1].uuid],
        locked: false,
    });
    (bench(sheet), id)
}

fn union(b: &Bench) -> &SchUnion {
    &b.sheet().unions[0]
}

fn pick(b: &mut Bench, id: Uuid) {
    let sheet = b.path.clone();
    row(b, UnionsMsg::RowPicked { sheet, union: id });
}

fn lock(b: &mut Bench, id: Uuid) {
    let sheet = b.path.clone();
    row(b, UnionsMsg::LockToggled { sheet, union: id });
}

fn shown_names(b: &Bench) -> Vec<String> {
    b.app
        .document_state
        .panel_ctx
        .unions
        .groups
        .iter()
        .flat_map(|g| g.rows.iter().map(|r| r.name.clone()))
        .collect()
}

#[test]
fn m158_create_makes_a_named_tagged_union_in_one_undo_step() {
    let mut b = parts_bench();
    b.select_parts(&["R1", "R2"]);
    unions(&mut b, UnionsMsg::CreateRequested);
    popover(
        &mut b,
        UnionPopoverMsg::NameEdited("Input filter".to_string()),
    );
    popover(&mut b, UnionPopoverMsg::TagInputEdited("#emc".to_string()));
    popover(&mut b, UnionPopoverMsg::TagSubmitted);
    let steps = b.steps();
    popover(&mut b, UnionPopoverMsg::Confirmed);
    assert!(b.app.ui_state.unions.popover.is_none(), "Create closes it");
    assert_eq!(union(&b).name, "Input filter");
    assert_eq!(union(&b).tags, ["emc"]);
    assert_eq!(union(&b).members.len(), 2);
    assert_eq!(b.steps(), steps + 1);
    assert!(b.dirty());
    b.undo();
    assert!(b.sheet().unions.is_empty());
    assert!(!b.dirty());
}

#[test]
fn m158_lock_after_creating_makes_the_union_locked() {
    let mut b = parts_bench();
    b.select_parts(&["R1"]);
    unions(&mut b, UnionsMsg::CreateRequested);
    popover(&mut b, UnionPopoverMsg::NameEdited("Divider".to_string()));
    popover(&mut b, UnionPopoverMsg::LockToggled(true));
    popover(&mut b, UnionPopoverMsg::Confirmed);
    assert!(union(&b).locked);
}

#[test]
fn m158_cancel_closes_the_popover_and_makes_nothing() {
    let mut b = parts_bench();
    b.select_parts(&["R1", "R2"]);
    unions(&mut b, UnionsMsg::CreateRequested);
    popover(
        &mut b,
        UnionPopoverMsg::NameEdited("Input filter".to_string()),
    );
    popover(&mut b, UnionPopoverMsg::Cancelled);
    assert!(b.app.ui_state.unions.popover.is_none());
    assert!(b.sheet().unions.is_empty());
    assert!(!b.dirty());
}

#[test]
fn m158_create_without_a_name_is_refused_and_the_popover_stays() {
    let mut b = parts_bench();
    b.select_parts(&["R1"]);
    unions(&mut b, UnionsMsg::CreateRequested);
    popover(&mut b, UnionPopoverMsg::NameEdited("   ".to_string()));
    popover(&mut b, UnionPopoverMsg::Confirmed);
    assert!(b.sheet().unions.is_empty());
    let open = b.app.ui_state.unions.popover.as_ref().expect("it stays");
    assert_eq!(open.name, "   ", "the text is kept");
    assert!(!b.dirty());
}

#[test]
fn m158_create_with_nothing_selected_opens_no_popover() {
    let mut b = parts_bench();
    unions(&mut b, UnionsMsg::CreateRequested);
    assert!(b.app.ui_state.unions.popover.is_none());
}

#[test]
fn m158_an_object_of_another_union_cannot_join_a_new_one() {
    let (mut b, _) = grouped();
    b.select_parts(&["R1", "R3"]);
    unions(&mut b, UnionsMsg::CreateRequested);
    popover(&mut b, UnionPopoverMsg::NameEdited("Second".to_string()));
    popover(&mut b, UnionPopoverMsg::Confirmed);
    assert_eq!(b.sheet().unions.len(), 1, "R1 is in Input filter");
    assert!(
        b.app.ui_state.unions.popover.is_some(),
        "the popover stays open on a refusal"
    );
    assert!(!b.dirty());
}

#[test]
fn m158_the_panel_filters_by_name_and_by_tag() {
    let (mut b, _) = grouped();
    open_panel(&mut b);
    assert_eq!(shown_names(&b), ["Input filter"]);
    row(&mut b, UnionsMsg::FilterEdited("divider".to_string()));
    assert!(shown_names(&b).is_empty());
    row(&mut b, UnionsMsg::FilterEdited("#emc".to_string()));
    assert_eq!(shown_names(&b), ["Input filter"]);
    row(&mut b, UnionsMsg::FilterEdited(String::new()));
    row(&mut b, UnionsMsg::TagChipToggled("usb".to_string()));
    assert!(shown_names(&b).is_empty(), "no union has the tag");
    row(&mut b, UnionsMsg::TagChipToggled("usb".to_string()));
    assert_eq!(shown_names(&b), ["Input filter"]);
    assert!(!b.dirty(), "filtering is not an edit");
}

#[test]
fn m158_project_scope_lists_the_unions_of_the_other_open_sheet() {
    let root = empty_sheet();
    let mut child = empty_sheet();
    child.symbols.push(part("R5", (10.16, 10.16)));
    let member = child.symbols[0].uuid;
    child.unions.push(SchUnion {
        uuid: Uuid::now_v7(),
        name: "Feedback divider".to_string(),
        tags: Vec::new(),
        members: vec![member],
        locked: false,
    });
    let mut b = project_bench(root, child);
    b.post(Message::Menu(MenuMessage::OpenUnionsPanel));
    b.panel(PanelMsg::Unions(UnionsMsg::ScopePicked(
        UnionScope::ThisSheet,
    )));
    let count = |b: &super::support::ProjectBench| {
        b.app
            .document_state
            .panel_ctx
            .unions
            .groups
            .iter()
            .map(|g| g.rows.len())
            .sum::<usize>()
    };
    assert_eq!(count(&b), 0, "the root sheet has none");
    b.panel(PanelMsg::Unions(UnionsMsg::ScopePicked(
        UnionScope::Project,
    )));
    assert_eq!(count(&b), 1);
}

#[test]
fn m158_select_on_a_row_selects_every_member() {
    let (mut b, id) = grouped();
    let sheet = b.path.clone();
    row(&mut b, UnionsMsg::MembersSelected { sheet, union: id });
    let selected = &b.app.interaction_state.active_canvas().selected;
    assert_eq!(selected.len(), 2);
    assert!(!b.dirty(), "selecting is not an edit");
}

#[test]
fn m158_hide_is_a_view_setting_with_no_undo_step_and_no_dirty_mark() {
    let (mut b, id) = grouped();
    open_panel(&mut b);
    pick(&mut b, id);
    let (steps, before) = (b.steps(), b.sheet().clone());
    let sheet = b.path.clone();
    row(
        &mut b,
        UnionsMsg::HideToggled {
            sheet: sheet.clone(),
            union: id,
        },
    );
    assert!(b.app.ui_state.unions.is_hidden(&sheet, id));
    let foot = b.app.document_state.panel_ctx.unions.foot.clone();
    assert!(foot.expect("the picked union's foot").hidden);
    assert_eq!(b.steps(), steps);
    assert_eq!(b.sheet().unions, before.unions);
    assert!(!b.dirty());
    row(
        &mut b,
        UnionsMsg::HideToggled {
            sheet: sheet.clone(),
            union: id,
        },
    );
    assert!(!b.app.ui_state.unions.is_hidden(&sheet, id));
}

#[test]
fn m158_lock_is_one_undo_step() {
    let (mut b, id) = grouped();
    lock(&mut b, id);
    assert!(union(&b).locked);
    assert!(b.dirty());
    b.undo();
    assert!(!union(&b).locked);
    assert!(!b.dirty());
}

#[test]
fn m158_a_locked_union_keeps_its_members_and_cannot_be_broken() {
    let (mut b, id) = grouped();
    pick(&mut b, id);
    lock(&mut b, id);
    b.select_parts(&["R1"]);
    b.post(Message::Edit(EditMsg::DeleteSelected));
    assert_eq!(b.sheet().symbols.len(), 3, "a locked member stays");
    b.select_parts(&["R3"]);
    unions(&mut b, UnionsMsg::AddSelectionRequested);
    assert_eq!(union(&b).members.len(), 2, "no member joins");
    unions(&mut b, UnionsMsg::BreakRequested);
    assert_eq!(b.sheet().unions.len(), 1, "it is not broken");
}

/// Row 158: a locked union "cannot be moved or edited until unlocked".
#[test]
fn m158_a_locked_union_cannot_be_renamed() {
    let (mut b, id) = grouped();
    pick(&mut b, id);
    lock(&mut b, id);
    row(&mut b, UnionsMsg::RenameEdited("Renamed".to_string()));
    assert_eq!(union(&b).name, "Input filter");
}

#[test]
fn m158_the_foot_renames_the_picked_union_as_one_step() {
    let (mut b, id) = grouped();
    pick(&mut b, id);
    for typed in ["I", "In", "Input"] {
        row(&mut b, UnionsMsg::RenameEdited(typed.to_string()));
    }
    assert_eq!(union(&b).name, "Input");
    assert!(b.dirty());
    b.undo();
    assert_eq!(union(&b).name, "Input filter");
    assert!(!b.dirty());
}

#[test]
fn m158_a_blank_name_in_the_foot_is_refused() {
    let (mut b, id) = grouped();
    pick(&mut b, id);
    row(&mut b, UnionsMsg::RenameEdited("   ".to_string()));
    assert_eq!(union(&b).name, "Input filter");
    assert!(!b.dirty());
}

#[test]
fn m158_add_and_remove_selection_change_the_members_and_undo() {
    let (mut b, id) = grouped();
    pick(&mut b, id);
    b.select_parts(&["R3"]);
    unions(&mut b, UnionsMsg::AddSelectionRequested);
    assert_eq!(union(&b).members.len(), 3);
    b.select_parts(&["R1"]);
    unions(&mut b, UnionsMsg::RemoveSelectionRequested);
    assert_eq!(union(&b).members.len(), 2);
    b.undo();
    assert_eq!(union(&b).members.len(), 3);
    b.undo();
    assert_eq!(union(&b).members.len(), 2);
    assert!(!b.dirty());
}

#[test]
fn m158_break_removes_the_union_keeps_the_objects_and_undo_restores_it() {
    let (mut b, id) = grouped();
    pick(&mut b, id);
    unions(&mut b, UnionsMsg::BreakRequested);
    assert!(b.sheet().unions.is_empty());
    assert_eq!(b.sheet().symbols.len(), 3);
    assert!(b.dirty());
    b.undo();
    assert_eq!(b.sheet().unions.len(), 1);
    assert!(!b.dirty());
}
