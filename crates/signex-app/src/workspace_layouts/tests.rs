//! The layout list's rules and its `prefs.json` form (mockup row 166).

use super::builtins::{shipped, shipped_dock};
use super::model::*;
use super::store;
use crate::dock::{DockLayoutSnapshot, RegionLayout};
use crate::panels::PanelKind;

/// The English names, as the catalog renders them.
fn english() -> Naming {
    Naming {
        builtins: [
            "Schematic".into(),
            "Layout".into(),
            "Review".into(),
            "Notebook".into(),
        ],
        numbered: Box::new(|n| format!("Layout {n}")),
        copy_of: Box::new(|name| format!("{name} copy")),
        nth: Box::new(|name, n| format!("{name} ({n})")),
    }
}

fn region(panels: &[PanelKind], width: Option<u32>) -> RegionLayout {
    RegionLayout {
        panels: panels.to_vec(),
        collapsed: false,
        width,
    }
}

fn dock(left: &[PanelKind]) -> DockLayoutSnapshot {
    DockLayoutSnapshot {
        left: region(left, Some(300)),
        right: region(&[PanelKind::Properties], Some(260)),
        bottom: region(&[PanelKind::Messages], None),
    }
}

fn captured(left: &[PanelKind]) -> Captured {
    Captured {
        dock: Some(dock(left)),
        split: None,
        saved_at: "2026-11-19 10:00".into(),
    }
}

fn names(layouts: &Layouts) -> Vec<&str> {
    layouts.user().iter().map(|l| l.name.as_str()).collect()
}

/// Three user layouts, "A", "B" and "C", in that order.
fn three() -> Layouts {
    let naming = english();
    let mut layouts = Layouts::default();
    for name in ["A", "B", "C"] {
        layouts
            .save(name, captured(&[PanelKind::Projects]), &naming)
            .expect("saved");
    }
    layouts
}

#[test]
fn the_built_ins_hold_the_panels_the_design_names() {
    use PanelKind::*;
    let schematic = shipped_dock(BuiltinLayout::Schematic);
    assert_eq!(schematic.left.panels, [Projects, Components]);
    assert_eq!(schematic.right.panels, [Properties]);
    assert_eq!(schematic.bottom.panels, [Messages]);
    let layout = shipped_dock(BuiltinLayout::Layout);
    assert_eq!(layout.left.panels, [Projects]);
    assert_eq!(layout.right.panels, [Properties, PcbNets]);
    assert_eq!(layout.bottom.panels, [Layers]);
    let review = shipped_dock(BuiltinLayout::Review);
    assert!(review.left.panels.is_empty());
    assert_eq!(review.right.panels, [History]);
    assert_eq!(review.bottom.panels, [Messages, CommandLog]);
    let notebook = shipped_dock(BuiltinLayout::Notebook);
    assert_eq!(notebook.left.panels, [Projects]);
    assert_eq!(notebook.right.panels, [Annotations, Properties]);
    assert!(notebook.bottom.panels.is_empty());
    for builtin in BuiltinLayout::ALL {
        let layout = shipped(builtin);
        assert_eq!(layout.id, LayoutId::Builtin(builtin));
        assert_eq!(layout.split, None);
        assert!(!layout.from_current);
    }
}

#[test]
fn save_offers_the_first_free_layout_number() {
    let naming = english();
    let mut layouts = Layouts::default();
    assert_eq!(layouts.first_free_name(&naming), "Layout 1");
    layouts
        .save("Layout 1", captured(&[]), &naming)
        .expect("saved");
    layouts
        .save("layout 3", captured(&[]), &naming)
        .expect("saved");
    assert_eq!(layouts.first_free_name(&naming), "Layout 2");
    layouts
        .save("Layout 2", captured(&[]), &naming)
        .expect("saved");
    assert_eq!(layouts.first_free_name(&naming), "Layout 4");
}

#[test]
fn saving_under_the_name_of_your_layout_replaces_it_in_place() {
    let naming = english();
    let mut layouts = three();
    let b = layouts.user()[1].id.clone();
    assert_eq!(
        layouts.name_clash(" b ", &naming, None),
        NameClash::User(b.clone())
    );
    let outcome = layouts
        .save(" b ", captured(&[PanelKind::Components]), &naming)
        .expect("replaced");
    assert_eq!(outcome, SaveOutcome::Replaced(b.clone()));
    assert_eq!(names(&layouts), ["A", "b", "C"]);
    let replaced = layouts.get(&b).expect("still there");
    assert_eq!(
        replaced.dock.expect("panels").left.panels,
        [PanelKind::Components]
    );
}

#[test]
fn a_built_in_name_is_refused_and_an_empty_one_too() {
    let naming = english();
    let mut layouts = Layouts::default();
    assert_eq!(
        layouts.save("review", captured(&[]), &naming),
        Err(SaveRefusal::BuiltinName(BuiltinLayout::Review))
    );
    assert_eq!(
        layouts.save("   ", captured(&[]), &naming),
        Err(SaveRefusal::EmptyName)
    );
    assert!(layouts.user().is_empty());
}

#[test]
fn update_from_current_on_a_built_in_stores_an_override_and_reset_drops_it() {
    let mut layouts = Layouts::default();
    let schematic = LayoutId::Builtin(BuiltinLayout::Schematic);
    assert!(!layouts.is_edited(BuiltinLayout::Schematic));
    assert!(layouts.update_from_current(&schematic, captured(&[PanelKind::History])));
    assert!(layouts.is_edited(BuiltinLayout::Schematic));
    let edited = layouts.builtin_layout(BuiltinLayout::Schematic);
    assert_eq!(
        edited.dock.expect("panels").left.panels,
        [PanelKind::History]
    );
    assert!(edited.from_current);

    // Only the active built-in offers Reset in the menu.
    assert_eq!(layouts.resettable(), None);
    layouts.set_active(Some(schematic.clone()));
    assert_eq!(layouts.resettable(), Some(BuiltinLayout::Schematic));

    assert!(layouts.reset_builtin(BuiltinLayout::Schematic));
    assert!(!layouts.reset_builtin(BuiltinLayout::Schematic));
    assert_eq!(
        layouts.builtin_layout(BuiltinLayout::Schematic),
        shipped(BuiltinLayout::Schematic)
    );
    assert_eq!(layouts.resettable(), None);
}

#[test]
fn built_ins_cannot_be_renamed_or_deleted() {
    let naming = english();
    let mut layouts = Layouts::default();
    let layout = LayoutId::Builtin(BuiltinLayout::Layout);
    assert_eq!(
        layouts.rename(&layout, "Board", &naming),
        Err(RenameRefusal::Builtin)
    );
    assert_eq!(layouts.delete(&layout), None);
}

#[test]
fn rename_keeps_names_unique() {
    let naming = english();
    let mut layouts = three();
    let a = layouts.user()[0].id.clone();
    assert_eq!(layouts.rename(&a, "c", &naming), Err(RenameRefusal::Taken));
    assert_eq!(
        layouts.rename(&a, "Notebook", &naming),
        Err(RenameRefusal::Taken)
    );
    assert_eq!(
        layouts.rename(&a, " ", &naming),
        Err(RenameRefusal::EmptyName)
    );
    // Its own name in another case is not a clash.
    assert_eq!(layouts.rename(&a, "a", &naming), Ok(()));
    assert_eq!(layouts.rename(&a, "Bench", &naming), Ok(()));
    assert_eq!(names(&layouts), ["Bench", "B", "C"]);
}

#[test]
fn duplicate_names_the_copy_and_puts_it_after_its_source() {
    let naming = english();
    let mut layouts = three();
    let a = layouts.user()[0].id.clone();
    let copy = layouts.duplicate(&a, &naming).expect("copied");
    assert_eq!(names(&layouts), ["A", "A copy", "B", "C"]);
    let again = layouts.duplicate(&a, &naming).expect("copied");
    assert_eq!(names(&layouts), ["A", "A copy (2)", "A copy", "B", "C"]);
    assert_ne!(copy, again);
    // A built-in's copy is yours and goes last.
    layouts
        .duplicate(&LayoutId::Builtin(BuiltinLayout::Review), &naming)
        .expect("copied");
    assert_eq!(names(&layouts).last(), Some(&"Review copy"));
    let review_copy = layouts.user().last().expect("a copy");
    assert!(review_copy.builtin().is_none());
    assert_eq!(review_copy.dock, Some(shipped_dock(BuiltinLayout::Review)));
}

#[test]
fn delete_hands_back_a_token_and_undo_restores_the_index() {
    let naming = english();
    let mut layouts = three();
    let b = layouts.user()[1].id.clone();
    layouts.set_active(Some(b.clone()));
    let deleted = layouts.delete(&b).expect("deleted");
    assert_eq!(deleted.index, 1);
    assert!(deleted.was_active);
    assert_eq!(names(&layouts), ["A", "C"]);
    assert_eq!(layouts.active(), None);

    let restored = layouts.undo_delete(deleted, &naming);
    assert_eq!(restored, b);
    assert_eq!(names(&layouts), ["A", "B", "C"]);
    assert_eq!(layouts.active(), Some(&b));
}

#[test]
fn undo_after_the_name_was_reused_keeps_both() {
    let naming = english();
    let mut layouts = three();
    let c = layouts.user()[2].id.clone();
    let deleted = layouts.delete(&c).expect("deleted");
    layouts.save("C", captured(&[]), &naming).expect("saved");
    layouts.undo_delete(deleted, &naming);
    assert_eq!(names(&layouts), ["A", "B", "C (2)", "C"]);
}

#[test]
fn reorder_moves_a_row_to_its_final_index() {
    let mut layouts = three();
    assert!(layouts.reorder(0, 2));
    assert_eq!(names(&layouts), ["B", "C", "A"]);
    assert!(layouts.reorder(2, 0));
    assert_eq!(names(&layouts), ["A", "B", "C"]);
    assert!(!layouts.reorder(1, 1));
    assert!(!layouts.reorder(7, 0));
    assert!(layouts.reorder(1, 9));
    assert_eq!(names(&layouts), ["A", "C", "B"]);
}

#[test]
fn import_adds_with_fresh_ids_and_numbers_duplicate_names() {
    let naming = english();
    let mut layouts = three();
    let before: Vec<LayoutId> = layouts.user().iter().map(|l| l.id.clone()).collect();
    let incoming: Vec<WorkspaceLayout> = ["B", "D", "Schematic", "D"]
        .into_iter()
        .map(|name| WorkspaceLayout {
            id: before[0].clone(),
            name: name.into(),
            dock: Some(dock(&[PanelKind::Navigator])),
            split: Some(DocumentSplit::Stacked),
            saved_at: None,
            from_current: false,
        })
        .collect();
    let added = layouts.import(incoming, &naming);
    assert_eq!(added.len(), 4);
    assert_eq!(
        names(&layouts),
        ["A", "B", "C", "B (2)", "D", "Schematic (2)", "D (2)"]
    );
    for id in &added {
        assert!(!before.contains(id), "an imported layout keeps a fresh id");
    }
}

#[test]
fn the_list_round_trips_through_its_prefs_form() {
    let naming = english();
    let mut layouts = three();
    let b = layouts.user()[1].id.clone();
    layouts.update_from_current(
        &LayoutId::Builtin(BuiltinLayout::Notebook),
        Captured {
            dock: None,
            split: Some(DocumentSplit::SideBySide),
            saved_at: "2026-11-18 08:30".into(),
        },
    );
    layouts.set_active(Some(b));
    let json = store::to_json(&layouts);
    assert_eq!(
        json["order"],
        serde_json::json!(["user-1", "user-2", "user-3"])
    );
    assert_eq!(
        json["user"]["user-1"]["dock"]["left"]["panels"],
        serde_json::json!(["projects"])
    );
    assert_eq!(
        json["overrides"]["notebook"]["split"],
        serde_json::json!("side_by_side")
    );

    let (back, problems) = store::from_json(&json);
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(back, layouts);
    assert_eq!(
        back.name_of(&back.user()[0].id, &naming).as_deref(),
        Some("A")
    );
}

#[test]
fn a_stored_list_is_read_tolerantly() {
    let json = serde_json::json!({
        "active": "user-9",
        "order": ["user-2", "user-1", "user-gone"],
        "user": {
            "user-1": { "name": "Bench", "dock": {
                "left": { "panels": ["projects", "signal_scope"], "collapsed": false, "width": 280 },
                "right": { "panels": ["properties"], "collapsed": true }
            }, "from_current": true, "saved_at": "2026-11-17 16:42" },
            "user-2": { "dock": { "left": { "panels": ["projects"] } } },
            "user-3": { "name": "Split only", "split": "stacked" },
            "user-4": "not a layout"
        },
        "overrides": {
            "schematic": { "dock": { "bottom": { "panels": ["messages"] } } },
            "pcb": { "dock": {} }
        }
    });
    let (layouts, problems) = store::from_json(&json);
    // The nameless entry, the string entry and the unknown built-in are
    // skipped and said; the unknown panel key drops that panel only.
    assert_eq!(problems.len(), 3, "{problems:?}");
    assert_eq!(names(&layouts), ["Bench", "Split only"]);
    let bench = &layouts.user()[0];
    let dock = bench.dock.as_ref().expect("panels");
    assert_eq!(dock.left.panels, [PanelKind::Projects]);
    assert_eq!(dock.left.width, Some(280));
    assert!(dock.right.collapsed);
    assert!(dock.bottom.panels.is_empty());
    assert_eq!(layouts.user()[1].split, Some(DocumentSplit::Stacked));
    assert!(layouts.is_edited(BuiltinLayout::Schematic));
    // The active layout is gone, so none is active.
    assert_eq!(layouts.active(), None);

    let (none, said) = store::from_prefs(None);
    assert_eq!(none, Layouts::default());
    assert!(said.is_empty());
    let (garbage, said) = store::from_json(&serde_json::json!(["not", "a", "list"]));
    assert_eq!(garbage, Layouts::default());
    assert_eq!(said.len(), 1);
}

#[test]
fn the_export_file_reads_back_and_a_foreign_file_is_refused() {
    let layouts = three();
    let value = store::export_json(layouts.user());
    let (read, problems) = store::import_json(&value).expect("a layouts file");
    assert!(problems.is_empty());
    let read_names: Vec<&str> = read.iter().map(|l| l.name.as_str()).collect();
    assert_eq!(read_names, ["A", "B", "C"]);
    assert_eq!(read[0].dock, layouts.user()[0].dock);

    assert!(store::import_json(&serde_json::json!({ "dock": {} })).is_err());
    let mut newer = store::export_json(&[]);
    newer[store::EXPORT_FORMAT_KEY] = serde_json::json!(store::EXPORT_FORMAT + 1);
    assert!(store::import_json(&newer).is_err());
}

/// The real store writes under its key and keeps the other settings.
#[test]
fn the_prefs_store_writes_its_key_and_keeps_the_rest() {
    use super::WorkspaceLayoutsStore as _;
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("prefs.json");
    std::fs::write(&path, r#"{"theme":"graphite_dark"}"#).expect("write prefs");
    let layouts = three();
    super::PrefsWorkspaceLayoutsStore::at(path.clone())
        .write(&store::to_json(&layouts))
        .expect("written");
    let text = std::fs::read_to_string(&path).expect("read prefs");
    let json: serde_json::Value = serde_json::from_str(&text).expect("parses");
    assert_eq!(json["theme"], serde_json::json!("graphite_dark"));
    let (back, problems) = store::from_prefs(Some(&json));
    assert!(problems.is_empty());
    assert_eq!(back, layouts);
}
