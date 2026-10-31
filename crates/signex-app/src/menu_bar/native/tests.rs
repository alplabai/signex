use std::collections::HashSet;
use std::path::PathBuf;

use super::state::{MenuInputs, Refresh, plan_refresh};
use super::*;
use crate::app::command::CommandContext;
use crate::menu_bar::MenuContext;
use crate::menu_bar::golden_tests::variants;
use crate::menu_bar::model::menu_bar_model;

fn recent_paths() -> Vec<PathBuf> {
    ["/a/one/p.snxprj", "/b/two/p.snxprj", "/c/three/q.snxprj"]
        .into_iter()
        .map(PathBuf::from)
        .collect()
}

/// Every context the tests cover: the golden variants, plus recent projects
/// (two with the same file name).
fn contexts(recent: &[PathBuf]) -> Vec<MenuContext<'_>> {
    let mut all = variants();
    all.push(MenuContext {
        recent_projects: recent,
        ..MenuContext::default()
    });
    all
}

fn ids(menu: &NativeMenu, out: &mut Vec<String>) {
    for node in &menu.nodes {
        match node {
            NativeNode::Item(item) => out.push(item.id.clone()),
            NativeNode::Submenu(sub) => ids(sub, out),
            NativeNode::Predefined(_) | NativeNode::Separator => {}
        }
    }
}

fn all_ids(spec: &NativeMenuSpec) -> Vec<String> {
    let mut out = Vec::new();
    spec.menus.iter().for_each(|m| ids(m, &mut out));
    out
}

/// Spec nodes mirror model entries: same order, separators, submenus,
/// labels and enablement; a placeholder is disabled.
fn assert_mirrors(menu: &Menu, native: &NativeMenu) {
    assert_eq!(menu.title, native.title);
    // The app menu carries Quit, so the native File menu drops Exit and the
    // separator above it.
    let entries: Vec<&MenuEntry> = if menu.title == "File" {
        let mut kept: Vec<&MenuEntry> = menu.entries.iter().collect();
        kept.truncate(kept.len() - 2);
        kept
    } else {
        menu.entries.iter().collect()
    };
    assert_eq!(entries.len(), native.nodes.len(), "{}", menu.title);
    for (entry, node) in entries.into_iter().zip(&native.nodes) {
        match (entry, node) {
            (MenuEntry::Separator, NativeNode::Separator) => {}
            (MenuEntry::Submenu(sub), NativeNode::Submenu(inner)) => assert_mirrors(sub, inner),
            (MenuEntry::Item(item), NativeNode::Item(native)) => {
                assert_eq!(item.label, native.label);
                assert_eq!(item.is_enabled(), native.enabled, "{}", item.label);
                assert!(native.accelerator.is_none(), "model rows carry no key");
                assert_eq!(
                    item.shortcut.as_ref().map(|s| s.text.as_str()),
                    native.shortcut.as_deref(),
                    "{}",
                    item.label
                );
            }
            (entry, node) => panic!("shape differs: {entry:?} vs {node:?}"),
        }
    }
}

#[test]
fn spec_mirrors_the_model_for_every_variant() {
    let recent = recent_paths();
    for ctx in contexts(&recent) {
        let model = menu_bar_model(&ctx);
        let built = build(&model, false);
        assert_eq!(built.spec.menus.len(), model.menus.len() + 1);
        assert_eq!(built.spec.menus[0].title, APP_MENU_TITLE);
        for (menu, native) in model.menus.iter().zip(&built.spec.menus[1..]) {
            assert_mirrors(menu, native);
        }
    }
}

/// id -> (label, message kind if live) of every row built for `ctx`.
fn rows_by_id(ctx: &MenuContext) -> HashMap<String, (String, Option<String>)> {
    fn walk(
        menu: &NativeMenu,
        actions: &ActionMap,
        out: &mut HashMap<String, (String, Option<String>)>,
    ) {
        for node in &menu.nodes {
            match node {
                NativeNode::Item(item) => {
                    let kind = actions.get(&item.id).map(|action| {
                        let text = format!("{action:?}");
                        text.split('(').next().unwrap_or_default().to_owned()
                    });
                    out.insert(item.id.clone(), (item.label.clone(), kind));
                }
                NativeNode::Submenu(sub) => walk(sub, actions, out),
                NativeNode::Predefined(_) | NativeNode::Separator => {}
            }
        }
    }
    let built = build(&menu_bar_model(ctx), false);
    let mut out = HashMap::new();
    for menu in &built.spec.menus {
        walk(menu, &built.actions, &mut out);
    }
    out
}

#[test]
fn ids_are_unique_and_stable_across_variants() {
    let recent = recent_paths();
    let mut seen: HashMap<String, String> = HashMap::new();
    for ctx in contexts(&recent) {
        let found = all_ids(&build(&menu_bar_model(&ctx), false).spec);
        let unique: HashSet<&String> = found.iter().collect();
        assert_eq!(unique.len(), found.len(), "duplicate id in {found:?}");
        // The same id always names the same row: its label never changes.
        for (id, (label, _)) in rows_by_id(&ctx) {
            if let Some(before) = seen.insert(id.clone(), label.clone()) {
                assert_eq!(before, label, "id {id} names two rows");
            }
        }
    }
    assert!(seen.contains_key("File/Save"));
    assert!(seen.contains_key("Help/About Signex"));
}

/// Whenever a row is live its message kind is the same in every context: an
/// id never changes what it does.
#[test]
fn a_live_id_always_dispatches_the_same_message_kind() {
    let recent = recent_paths();
    let mut kinds: HashMap<String, String> = HashMap::new();
    for ctx in contexts(&recent) {
        for (id, (_, kind)) in rows_by_id(&ctx) {
            let Some(kind) = kind else { continue };
            if let Some(before) = kinds.insert(id.clone(), kind.clone()) {
                assert_eq!(before, kind, "id {id} changed message");
            }
        }
    }
    assert!(kinds.len() > 40);
}

#[test]
fn a_locked_bar_greys_the_model_rows_and_keeps_the_app_menu() {
    let ctx = variants().remove(1);
    let built = build(&menu_bar_model(&ctx), true);
    assert_eq!(built.actions.len(), 3, "only the app menu stays live");
    assert!(built.actions.keys().all(|id| is_app_menu_id(id)));
    assert!(!is_app_menu_id("File/Save"));
    assert!(!is_app_menu_id("Signexx/File"));
    let unlocked = spec_for(&ctx);
    match plan_refresh(Some(&unlocked), &built.spec) {
        Refresh::Enablement(changes) => assert!(changes.iter().all(|change| !change.enabled)),
        other => panic!("locking must only flip enablement, got {other:?}"),
    }
}

#[test]
fn recent_projects_get_distinct_path_ids() {
    let recent = recent_paths();
    let ctx = MenuContext {
        recent_projects: &recent,
        ..MenuContext::default()
    };
    let built = build(&menu_bar_model(&ctx), false);
    for path in &recent {
        let id = format!("File/Open Recent/{}", path.display());
        assert!(
            matches!(
                built.actions.get(&id),
                Some(MenuMessage::OpenRecentProject(p)) if p == path
            ),
            "no live row for {id}"
        );
    }
}

#[test]
fn only_enabled_live_rows_activate() {
    let model = menu_bar_model(&MenuContext::default());
    let built = build(&model, false);
    // New Project is always enabled; Save needs an open document; the
    // placeholder "No recent projects" has no handler at all.
    assert!(matches!(
        built.actions.get("File/New Project"),
        Some(MenuMessage::NewProject)
    ));
    assert!(!built.actions.contains_key("File/Save"));
    assert!(
        !built
            .actions
            .contains_key("File/Open Recent/No recent projects")
    );
    assert!(!built.actions.contains_key("File/Open Recent"));
    assert!(!built.actions.contains_key("nonsense/id"));
    let enabled = variants().remove(1);
    let built = build(&menu_bar_model(&enabled), false);
    assert!(matches!(
        built.actions.get("File/Save"),
        Some(MenuMessage::Save)
    ));
}

#[test]
fn a_stale_id_misses_after_the_row_goes_away() {
    let recent = recent_paths();
    let with = MenuContext {
        recent_projects: &recent,
        ..MenuContext::default()
    };
    let id = format!("File/Open Recent/{}", recent[0].display());
    assert!(
        build(&menu_bar_model(&with), false)
            .actions
            .contains_key(&id)
    );
    let cleared = build(&menu_bar_model(&MenuContext::default()), false);
    assert!(!cleared.actions.contains_key(&id));
}

#[test]
fn app_menu_has_the_approved_content() {
    let app = &build(&menu_bar_model(&MenuContext::default()), false)
        .spec
        .menus[0];
    let shape: Vec<String> = app
        .nodes
        .iter()
        .map(|node| match node {
            NativeNode::Item(i) => format!("{} {:?}", i.label, i.accelerator),
            NativeNode::Predefined(p) => format!("{p:?}"),
            NativeNode::Separator => "-".into(),
            NativeNode::Submenu(_) => "submenu".into(),
        })
        .collect();
    assert_eq!(
        shape,
        [
            "About Signex None",
            "-",
            "Preferences... Some(CommandComma)",
            "-",
            "Hide",
            "HideOthers",
            "ShowAll",
            "-",
            "Quit Signex Some(CommandQ)",
        ]
    );
    let actions = build(&menu_bar_model(&MenuContext::default()), false).actions;
    assert!(matches!(
        actions.get("Signex/Quit Signex"),
        Some(MenuMessage::Exit)
    ));
    assert!(matches!(
        actions.get("Signex/Preferences..."),
        Some(MenuMessage::OpenPreferences)
    ));
    assert!(matches!(
        actions.get("Signex/About Signex"),
        Some(MenuMessage::OpenAbout)
    ));
}

fn spec_for(ctx: &MenuContext) -> NativeMenuSpec {
    build(&menu_bar_model(ctx), false).spec
}

#[test]
fn refresh_plan_follows_what_changed() {
    let base = spec_for(&MenuContext::default());
    assert_eq!(plan_refresh(None, &base), Refresh::Rebuild);
    assert_eq!(plan_refresh(Some(&base), &base.clone()), Refresh::Unchanged);

    // Enablement only: in place, and exactly the rows that moved.
    let open = spec_for(&variants().remove(1));
    match plan_refresh(Some(&base), &open) {
        Refresh::Enablement(changes) => {
            assert!(changes.iter().any(|c| c.id == "File/Save" && c.enabled));
            assert!(changes.iter().all(|c| !c.id.is_empty()));
        }
        other => panic!("expected an enablement update, got {other:?}"),
    }

    // Recent projects change the rows themselves.
    let recent = recent_paths();
    let listed = spec_for(&MenuContext {
        recent_projects: &recent,
        ..MenuContext::default()
    });
    assert_eq!(plan_refresh(Some(&base), &listed), Refresh::Rebuild);

    // A relabelled row is a rebuild too.
    let mut renamed = base.clone();
    if let NativeNode::Item(item) = &mut renamed.menus[1].nodes[0] {
        item.label.push('!');
    }
    assert_eq!(plan_refresh(Some(&base), &renamed), Refresh::Rebuild);
}

#[test]
fn inputs_compare_by_what_the_menu_reads() {
    let recent = recent_paths();
    let a = MenuInputs::new(CommandContext::default(), &recent, false, Vec::new());
    assert_eq!(
        a,
        MenuInputs::new(CommandContext::default(), &recent, false, Vec::new())
    );
    assert_ne!(
        a,
        MenuInputs::new(CommandContext::default(), &recent, true, Vec::new())
    );
    assert_ne!(
        a,
        MenuInputs::new(CommandContext::default(), &recent[1..], false, Vec::new())
    );
    let dirty = CommandContext {
        project_dirty: true,
        ..CommandContext::default()
    };
    assert_ne!(a, MenuInputs::new(dirty, &recent, false, Vec::new()));
    let panels = vec![crate::panels::PanelKind::Properties];
    assert_ne!(
        a,
        MenuInputs::new(CommandContext::default(), &recent, false, panels)
    );
}

#[test]
fn nothing_syncs_before_the_main_window_opens() {
    let inputs = MenuInputs::new(CommandContext::default(), &[], false, Vec::new());
    let mut state = NativeMenuState::default();
    assert!(!state.needs_sync(&inputs));
    state.arm();
    assert!(state.needs_sync(&inputs));
}

/// #103 - the panel rows' check marks reach the native spec and refresh in place.
#[test]
fn the_native_menu_carries_and_refreshes_the_check_mark() {
    let closed = build(&menu_bar_model(&MenuContext::default()), false);
    let open = build(
        &menu_bar_model(&MenuContext {
            open_panels: vec![crate::panels::PanelKind::Properties],
            ..MenuContext::default()
        }),
        false,
    );
    let checked_of = |spec: &NativeMenuSpec| {
        spec.menus
            .iter()
            .find(|m| m.title == "View")
            .unwrap()
            .nodes
            .iter()
            .find_map(|node| match node {
                NativeNode::Item(i) if i.label == "Properties" => Some(i.checked),
                _ => None,
            })
            .unwrap()
    };
    assert_eq!(checked_of(&closed.spec), Some(false));
    assert_eq!(checked_of(&open.spec), Some(true));

    match plan_refresh(Some(&closed.spec), &open.spec) {
        Refresh::Enablement(changes) => {
            assert_eq!(changes.len(), 1);
            assert_eq!(changes[0].id, "View/Properties");
            assert_eq!(changes[0].checked, Some(true));
        }
        other => panic!("a check mark moving must refresh in place, got {other:?}"),
    }
}

fn built_in_keymap_context() -> MenuContext<'static> {
    let keymap = crate::keymap::ShortcutProfileSet::built_ins()
        .expect("built-in profiles")
        .compile_active();
    MenuContext {
        active_keymap: Some(std::sync::Arc::new(keymap)),
        ..MenuContext::default()
    }
}

fn file_menu(spec: &NativeMenuSpec) -> &NativeMenu {
    spec.menus.iter().find(|m| m.title == "File").unwrap()
}

fn item_labelled<'a>(menu: &'a NativeMenu, label: &str) -> Option<&'a NativeItem> {
    menu.nodes.iter().find_map(|node| match node {
        NativeNode::Item(item) if item.label == label => Some(item),
        _ => None,
    })
}

/// #1765: a native row shows its keymap shortcut but registers no key
/// equivalent, so AppKit cannot take the key before the keymap sees it.
#[test]
fn native_rows_show_the_keymap_shortcut_without_a_key_equivalent() {
    let spec = build(&menu_bar_model(&built_in_keymap_context()), false).spec;
    let save = item_labelled(file_menu(&spec), "Save").unwrap();
    assert_eq!(save.shortcut.as_deref(), Some("Ctrl+S"));
    assert!(save.accelerator.is_none());
    let redo = spec.menus.iter().find(|m| m.title == "Edit").unwrap();
    assert_eq!(
        item_labelled(redo, "Redo").unwrap().shortcut.as_deref(),
        Some("Ctrl+Y")
    );
}

/// The shortcut is part of what is on screen: a rebind must rebuild.
#[test]
fn a_changed_shortcut_rebuilds_the_native_menu() {
    let with = build(&menu_bar_model(&built_in_keymap_context()), false).spec;
    let mut rebound = with.clone();
    for menu in &mut rebound.menus {
        for node in &mut menu.nodes {
            if let NativeNode::Item(item) = node
                && item.shortcut.is_some()
            {
                item.shortcut = Some("F2".into());
            }
        }
    }
    assert_ne!(with, rebound);
    assert_eq!(plan_refresh(Some(&with), &rebound), Refresh::Rebuild);
}

#[test]
fn the_menu_inputs_follow_the_active_keymap() {
    let first = std::sync::Arc::new(
        crate::keymap::ShortcutProfileSet::built_ins()
            .unwrap()
            .compile_active(),
    );
    let again = std::sync::Arc::new(
        crate::keymap::ShortcutProfileSet::built_ins()
            .unwrap()
            .compile_active(),
    );
    let base = || MenuInputs::new(CommandContext::default(), &[], false, Vec::new());
    assert_eq!(
        base().with_keymap(first.clone()),
        base().with_keymap(first.clone())
    );
    assert_ne!(base().with_keymap(first), base().with_keymap(again));
}

/// #1765: File gains Close Document, Close Project and Print, and loses Exit
/// natively (Quit Signex lives in the app menu).
#[test]
fn the_native_file_menu_has_the_approved_rows() {
    let built = build(&menu_bar_model(&variants().remove(1)), false);
    let file = file_menu(&built.spec);
    for label in ["Close Document", "Close Project", "Print..."] {
        assert!(item_labelled(file, label).is_some(), "{label}");
    }
    assert!(item_labelled(file, "Exit").is_none());
    assert!(!matches!(file.nodes.last(), Some(NativeNode::Separator)));
    let at = |id: &str| built.actions.get(id);
    assert!(matches!(
        at("File/Close Document"),
        Some(MenuMessage::CloseDocument)
    ));
    assert!(matches!(
        at("File/Close Project"),
        Some(MenuMessage::CloseProject)
    ));
    assert!(matches!(at("File/Print..."), Some(MenuMessage::Print)));
    let print = item_labelled(file, "Print...").unwrap();
    assert!(print.accelerator.is_none());
}
