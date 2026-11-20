//! #158: creating, renaming, locking, re-membering and breaking a named
//! union, the one-union-per-object rule, a delete dropping its member, the
//! lock a locked union puts on its members, and the selection a click grows
//! into.

use crate::test_support::test_sheet;
use crate::unions::{expand_to_unions, normalized_tags, union_of};
use crate::*;
use signex_types::schematic::{Junction, NoConnect, Point, SchUnion, SelectedItem, SelectedKind};
use uuid::Uuid;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

/// A sheet with junctions 1-3 and no-connects 11-12.
fn sheet() -> SchematicSheet {
    let mut s = test_sheet();
    for n in 1..=3 {
        s.junctions.push(Junction {
            uuid: id(n),
            position: Point::new(10.0 * n as f64, 10.0),
            ..Default::default()
        });
    }
    for n in 11..=12 {
        s.no_connects.push(NoConnect {
            uuid: id(n),
            position: Point::new(10.0 * n as f64, 20.0),
        });
    }
    s
}

fn union(uuid: u128, name: &str, members: &[u128]) -> SchUnion {
    SchUnion {
        uuid: id(uuid),
        name: name.into(),
        tags: vec!["#emc".into(), " power-in ".into(), "emc".into()],
        members: members.iter().map(|n| id(*n)).collect(),
        locked: false,
    }
}

fn created(members: &[u128]) -> Engine {
    let mut engine = Engine::new(sheet()).unwrap();
    engine
        .execute(Command::CreateUnion {
            union: union(100, "Input filter", members),
        })
        .unwrap();
    engine
}

/// The engine's sheet as the file would hold it (`SchematicSheet` has no
/// `PartialEq`).
fn written(engine: &Engine) -> String {
    signex_types::format::SnxSchematic::new(engine.document().clone())
        .write_string()
        .unwrap()
}

fn junction(n: u128) -> SelectedItem {
    SelectedItem::new(id(n), SelectedKind::Junction)
}

fn refusal(result: Result<CommandResult, EngineError>) -> UnionRefusal {
    match result {
        Err(EngineError::Union(refusal)) => refusal,
        other => panic!("expected a union refusal, got {other:?}"),
    }
}

#[test]
fn creating_a_union_is_one_undo_step_with_clean_tags() {
    let mut engine = created(&[1, 2, 11]);
    let made = &engine.document().unions[0];
    assert_eq!(made.name, "Input filter");
    assert_eq!(made.tags, ["emc", "power-in"]);
    assert_eq!(made.members, [id(1), id(2), id(11)]);
    engine.undo().unwrap();
    assert!(engine.document().unions.is_empty());
    assert!(!engine.can_undo());
}

#[test]
fn a_blank_name_an_empty_union_or_a_missing_object_is_refused() {
    let mut engine = Engine::new(sheet()).unwrap();
    let mut blank = union(100, "  ", &[1]);
    assert_eq!(
        refusal(engine.execute(Command::CreateUnion {
            union: blank.clone()
        })),
        UnionRefusal::NameEmpty
    );
    blank.name = "Divider".into();
    blank.members.clear();
    assert_eq!(
        refusal(engine.execute(Command::CreateUnion {
            union: blank.clone()
        })),
        UnionRefusal::NoMembers
    );
    blank.members = vec![id(1), id(999)];
    assert_eq!(
        refusal(engine.execute(Command::CreateUnion { union: blank })),
        UnionRefusal::MemberNotOnSheet
    );
    assert!(engine.document().unions.is_empty());
    assert!(!engine.can_undo());
}

#[test]
fn an_object_is_in_one_union_at_most() {
    let mut engine = created(&[1, 2]);
    let refused = engine.execute(Command::CreateUnion {
        union: union(200, "Bulk caps", &[2, 3]),
    });
    assert_eq!(
        refusal(refused),
        UnionRefusal::MemberInAnotherUnion {
            objects: 1,
            unions: vec!["Input filter".into()],
        }
    );
    engine
        .execute(Command::CreateUnion {
            union: union(200, "Bulk caps", &[3]),
        })
        .unwrap();
    let refused = engine.execute(Command::SetUnionMembers {
        uuid: id(200),
        members: vec![id(3), id(1)],
    });
    assert!(matches!(
        refusal(refused),
        UnionRefusal::MemberInAnotherUnion { .. }
    ));
    assert_eq!(engine.document().unions[1].members, [id(3)]);
}

#[test]
fn renaming_is_one_typing_run_and_a_lock_is_its_own_step() {
    let mut engine = created(&[1]);
    let tags = vec!["emc".to_string()];
    for name in ["I", "In", "Input"] {
        engine
            .execute(Command::UpdateUnion {
                uuid: id(100),
                name: name.into(),
                tags: tags.clone(),
                locked: false,
            })
            .unwrap();
    }
    engine
        .execute(Command::UpdateUnion {
            uuid: id(100),
            name: "Input".into(),
            tags: tags.clone(),
            locked: true,
        })
        .unwrap();
    assert!(engine.document().unions[0].locked);
    engine.undo().unwrap();
    assert!(!engine.document().unions[0].locked);
    assert_eq!(engine.document().unions[0].name, "Input");
    engine.undo().unwrap();
    assert_eq!(engine.document().unions[0].name, "Input filter");
}

#[test]
fn an_unchanged_update_changes_nothing() {
    let mut engine = created(&[1]);
    let done = engine
        .execute(Command::UpdateUnion {
            uuid: id(100),
            name: "Input filter".into(),
            tags: vec!["emc".into(), "power-in".into()],
            locked: false,
        })
        .unwrap();
    assert!(!done.changed);
}

#[test]
fn setting_members_and_breaking_are_undo_steps() {
    let mut engine = created(&[1, 2]);
    engine
        .execute(Command::SetUnionMembers {
            uuid: id(100),
            members: vec![id(1), id(2), id(3)],
        })
        .unwrap();
    assert_eq!(engine.document().unions[0].members.len(), 3);
    engine
        .execute(Command::BreakUnion { uuid: id(100) })
        .unwrap();
    assert!(engine.document().unions.is_empty());
    assert_eq!(engine.document().junctions.len(), 3, "objects stay");
    engine.undo().unwrap();
    assert_eq!(engine.document().unions[0].members.len(), 3);
    engine.undo().unwrap();
    assert_eq!(engine.document().unions[0].members.len(), 2);
}

#[test]
fn removing_every_member_breaks_the_union() {
    let mut engine = created(&[1]);
    engine
        .execute(Command::SetUnionMembers {
            uuid: id(100),
            members: Vec::new(),
        })
        .unwrap();
    assert!(engine.document().unions.is_empty());
}

#[test]
fn deleting_a_member_drops_it_from_its_union_in_the_same_step() {
    let mut engine = created(&[1, 2]);
    let done = engine
        .execute(Command::DeleteSelection {
            items: vec![junction(1)],
        })
        .unwrap();
    assert!(
        done.patch_pair
            .unwrap()
            .document
            .contains(DocumentPatch::UNIONS)
    );
    assert_eq!(engine.document().unions[0].members, [id(2)]);
    engine.undo().unwrap();
    assert_eq!(engine.document().unions[0].members, [id(1), id(2)]);
    assert_eq!(engine.document().junctions.len(), 3);
}

#[test]
fn deleting_the_last_member_removes_the_union() {
    let mut engine = created(&[1, 2]);
    engine
        .execute(Command::DeleteSelection {
            items: vec![junction(1), junction(2)],
        })
        .unwrap();
    assert!(engine.document().unions.is_empty());
    engine.undo().unwrap();
    assert_eq!(engine.document().unions.len(), 1);
}

#[test]
fn a_batch_delete_prunes_once_and_undoes_as_one() {
    let mut engine = created(&[1, 2]);
    engine
        .execute_batch([
            Command::DeleteSelection {
                items: vec![junction(1)],
            },
            Command::DeleteSelection {
                items: vec![junction(2)],
            },
        ])
        .unwrap();
    assert!(engine.document().unions.is_empty());
    engine.undo().unwrap();
    assert_eq!(engine.document().unions[0].members, [id(1), id(2)]);
}

fn locked(members: &[u128]) -> Engine {
    let mut engine = created(members);
    engine
        .execute(Command::UpdateUnion {
            uuid: id(100),
            name: "Input filter".into(),
            tags: Vec::new(),
            locked: true,
        })
        .unwrap();
    engine
}

#[test]
fn a_locked_unions_members_cannot_be_moved_or_deleted() {
    let mut engine = locked(&[1, 2]);
    let before = written(&engine);
    let moved = engine.execute(Command::MoveSelection {
        items: vec![junction(1)],
        dx: 2.54,
        dy: 0.0,
        keep_wires: false,
        wiring: signex_types::corner::DragWiringMode::Auto,
    });
    match moved {
        Err(EngineError::ItemLocked { references, action }) => {
            assert_eq!(references, ["union Input filter"]);
            assert_eq!(action, "moved");
        }
        other => panic!("expected a lock refusal, got {other:?}"),
    }
    assert!(matches!(
        engine.execute(Command::DeleteSelection {
            items: vec![junction(2)],
        }),
        Err(EngineError::ItemLocked { .. })
    ));
    assert_eq!(
        written(&engine),
        before,
        "the refused edits changed nothing"
    );
    // An object outside the union is not held.
    engine
        .execute(Command::DeleteSelection {
            items: vec![junction(3)],
        })
        .unwrap();
}

#[test]
fn a_locked_union_keeps_its_members_until_unlocked() {
    let mut engine = locked(&[1, 2]);
    assert!(matches!(
        refusal(engine.execute(Command::BreakUnion { uuid: id(100) })),
        UnionRefusal::Locked { .. }
    ));
    assert!(matches!(
        refusal(engine.execute(Command::SetUnionMembers {
            uuid: id(100),
            members: vec![id(1)],
        })),
        UnionRefusal::Locked { .. }
    ));
    let (kept, skipped) = engine.without_locked(Command::DeleteSelection {
        items: vec![junction(1), junction(3)],
    });
    assert_eq!(skipped, ["union Input filter"]);
    assert!(matches!(
        kept,
        Some(Command::DeleteSelection { items }) if items == [junction(3)]
    ));
}

#[test]
fn a_click_on_a_member_selects_the_whole_union() {
    let engine = created(&[1, 11]);
    let doc = engine.document();
    let grown = expand_to_unions(doc, &[junction(1)]);
    assert_eq!(
        grown,
        [
            junction(1),
            SelectedItem::new(id(11), SelectedKind::NoConnect)
        ]
    );
    assert_eq!(expand_to_unions(doc, &[junction(3)]), [junction(3)]);
    assert_eq!(union_of(doc, id(11)).map(|u| u.uuid), Some(id(100)));
}

#[test]
fn tags_are_trimmed_unhashed_and_unique() {
    let tags = ["#usb".to_string(), "usb".into(), "  ".into(), "esd".into()];
    assert_eq!(normalized_tags(&tags), ["usb", "esd"]);
}
