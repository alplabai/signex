//! The Unions panel's pure view builder and the hidden set (mockup row 158).

use std::path::{Path, PathBuf};

use signex_types::schematic::{
    Junction, Point, SchUnion, SchematicSheet, SelectedItem, SelectedKind,
};
use uuid::Uuid;

use super::state::{ScopeSheet, UnionScope, UnionsState, panel_view};

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn sheet(unions: &[(u128, &str, &[&str], &[u128])]) -> SchematicSheet {
    let mut s = SchematicSheet::default();
    for n in 1..=6 {
        s.junctions.push(Junction {
            uuid: id(n),
            position: Point::new(n as f64, 0.0),
            ..Default::default()
        });
    }
    for (uuid, name, tags, members) in unions {
        s.unions.push(SchUnion {
            uuid: id(*uuid),
            name: (*name).into(),
            tags: tags.iter().map(|t| (*t).to_string()).collect(),
            members: members.iter().map(|m| id(*m)).collect(),
            locked: false,
        });
    }
    s
}

fn names(view: &super::UnionsPanelView) -> Vec<String> {
    view.groups
        .iter()
        .flat_map(|g| g.rows.iter().map(|r| r.name.clone()))
        .collect()
}

#[test]
fn the_filter_matches_names_and_hash_tags_and_the_chips_any_tag() {
    let a = sheet(&[
        (100, "Input filter", &["emc", "power-in"], &[1, 2]),
        (101, "Feedback divider", &["buck"], &[3]),
        (102, "USB ESD", &["emc", "usb"], &[4]),
    ]);
    let path = PathBuf::from("/p/a.snxsch");
    let sheets = [ScopeSheet {
        path: &path,
        name: "a.snxsch".into(),
        sheet: &a,
    }];
    let mut state = UnionsState::default();
    let all = panel_view(&state, &sheets, Some(path.as_path()), &[]);
    assert_eq!(names(&all).len(), 3);
    assert_eq!(
        all.tags.iter().map(|(t, _)| t.as_str()).collect::<Vec<_>>(),
        ["buck", "emc", "power-in", "usb"]
    );
    let shown = |state: &UnionsState| names(&panel_view(state, &sheets, Some(path.as_path()), &[]));
    state.filter = "divider".into();
    assert_eq!(shown(&state), ["Feedback divider"]);
    state.filter = "#us".into();
    assert_eq!(shown(&state), ["USB ESD"]);
    state.filter.clear();
    state.tags_on.insert("power-in".into());
    state.tags_on.insert("buck".into());
    assert_eq!(shown(&state), ["Input filter", "Feedback divider"]);
}

#[test]
fn project_scope_groups_the_rows_by_sheet() {
    let a = sheet(&[(100, "Input filter", &[], &[1])]);
    let b = sheet(&[(200, "Bulk caps", &[], &[2])]);
    let (pa, pb) = (PathBuf::from("/p/a.snxsch"), PathBuf::from("/p/b.snxsch"));
    let sheets = [
        ScopeSheet {
            path: &pa,
            name: "a.snxsch".into(),
            sheet: &a,
        },
        ScopeSheet {
            path: &pb,
            name: "b.snxsch".into(),
            sheet: &b,
        },
    ];
    let state = UnionsState {
        scope: UnionScope::Project,
        ..UnionsState::default()
    };
    let view = panel_view(&state, &sheets, Some(pa.as_path()), &[]);
    let titles: Vec<Option<String>> = view.groups.iter().map(|g| g.title.clone()).collect();
    assert_eq!(
        titles,
        [Some("a.snxsch".to_string()), Some("b.snxsch".to_string())]
    );
}

#[test]
fn the_selected_union_is_picked_and_its_foot_counts_the_members() {
    let a = sheet(&[(100, "Input filter", &[], &[1, 2])]);
    let path = PathBuf::from("/p/a.snxsch");
    let sheets = [ScopeSheet {
        path: &path,
        name: "a.snxsch".into(),
        sheet: &a,
    }];
    let selected = [
        SelectedItem::new(id(1), SelectedKind::Junction),
        SelectedItem::new(id(2), SelectedKind::Junction),
    ];
    let view = panel_view(
        &UnionsState::default(),
        &sheets,
        Some(path.as_path()),
        &selected,
    );
    assert!(view.groups[0].rows[0].picked);
    let foot = view.foot.expect("the selection is the union");
    assert_eq!(foot.members, 2);
    assert_eq!(foot.others, 2, "junctions are neither parts nor wires");
    assert!(foot.can_remove && !foot.can_add);
}

#[test]
fn hiding_is_a_view_setting_per_sheet() {
    let mut state = UnionsState::default();
    let (a, b) = (Path::new("/p/a.snxsch"), Path::new("/p/b.snxsch"));
    state.toggle_hidden(a, id(100));
    assert!(state.is_hidden(a, id(100)));
    assert!(!state.is_hidden(b, id(100)));
    state.toggle_hidden(a, id(100));
    assert!(state.hidden_in(a).is_empty());
    assert!(state.hidden.is_empty(), "an emptied sheet entry is dropped");
}
