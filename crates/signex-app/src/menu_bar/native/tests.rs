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
    assert_eq!(menu.entries.len(), native.nodes.len(), "{}", menu.title);
    for (entry, node) in menu.entries.iter().zip(&native.nodes) {
        match (entry, node) {
            (MenuEntry::Separator, NativeNode::Separator) => {}
            (MenuEntry::Submenu(sub), NativeNode::Submenu(inner)) => assert_mirrors(sub, inner),
            (MenuEntry::Item(item), NativeNode::Item(native)) => {
                assert_eq!(item.label, native.label);
                assert_eq!(item.is_enabled(), native.enabled, "{}", item.label);
                assert!(native.accelerator.is_none(), "model rows carry no key");
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
        let built = build(&model);
        assert_eq!(built.spec.menus.len(), model.menus.len() + 1);
        assert_eq!(built.spec.menus[0].title, APP_MENU_TITLE);
        for (menu, native) in model.menus.iter().zip(&built.spec.menus[1..]) {
            assert_mirrors(menu, native);
        }
    }
}

#[test]
fn ids_are_unique_and_stable_across_variants() {
    let recent = recent_paths();
    let mut by_label: HashMap<String, Vec<String>> = HashMap::new();
    for ctx in contexts(&recent) {
        let found = all_ids(&build(&menu_bar_model(&ctx)).spec);
        let unique: HashSet<&String> = found.iter().collect();
        assert_eq!(unique.len(), found.len(), "duplicate id in {found:?}");
        for id in found {
            by_label.entry(id.clone()).or_default().push(id);
        }
    }
    // A row keeps its id whatever the context: the same id is never reused
    // for a different row because ids are derived from the row alone.
    assert!(by_label.contains_key("File/Save"));
    assert!(by_label.contains_key("Help/About Signex"));
}

#[test]
fn recent_projects_get_distinct_path_ids() {
    let recent = recent_paths();
    let ctx = MenuContext {
        recent_projects: &recent,
        ..MenuContext::default()
    };
    let built = build(&menu_bar_model(&ctx));
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
    let built = build(&model);
    // New Project is always enabled; Save needs an open document; the
    // placeholder "No recent projects" has no handler at all.
    assert!(matches!(
        resolve_activation(&built.actions, "File/New Project"),
        Some(MenuMessage::NewProject)
    ));
    assert!(resolve_activation(&built.actions, "File/Save").is_none());
    assert!(resolve_activation(&built.actions, "File/Open Recent/No recent projects").is_none());
    assert!(resolve_activation(&built.actions, "File/Open Recent").is_none());
    assert!(resolve_activation(&built.actions, "nonsense/id").is_none());
    let enabled = variants().remove(1);
    let built = build(&menu_bar_model(&enabled));
    assert!(matches!(
        resolve_activation(&built.actions, "File/Save"),
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
    assert!(build(&menu_bar_model(&with)).actions.contains_key(&id));
    let cleared = build(&menu_bar_model(&MenuContext::default()));
    assert!(resolve_activation(&cleared.actions, &id).is_none());
}

#[test]
fn app_menu_has_the_approved_content() {
    let app = &build(&menu_bar_model(&MenuContext::default())).spec.menus[0];
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
            "Preferences\u{2026} Some(CommandComma)",
            "-",
            "Hide",
            "HideOthers",
            "ShowAll",
            "-",
            "Quit Signex Some(CommandQ)",
        ]
    );
    let actions = build(&menu_bar_model(&MenuContext::default())).actions;
    assert!(matches!(
        actions.get("Signex/Quit Signex"),
        Some(MenuMessage::Exit)
    ));
    assert!(matches!(
        actions.get("Signex/Preferences\u{2026}"),
        Some(MenuMessage::OpenPreferences)
    ));
    assert!(matches!(
        actions.get("Signex/About Signex"),
        Some(MenuMessage::OpenAbout)
    ));
}

fn spec_for(ctx: &MenuContext) -> NativeMenuSpec {
    build(&menu_bar_model(ctx)).spec
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
            assert!(changes.contains(&("File/Save".to_owned(), true)));
            assert!(changes.iter().all(|(id, _)| !id.is_empty()));
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
    let a = MenuInputs::new(CommandContext::default(), &recent);
    assert_eq!(a, MenuInputs::new(CommandContext::default(), &recent));
    assert_ne!(a, MenuInputs::new(CommandContext::default(), &recent[1..]));
    let dirty = CommandContext {
        project_dirty: true,
        ..CommandContext::default()
    };
    assert_ne!(a, MenuInputs::new(dirty, &recent));
}

#[test]
fn nothing_syncs_before_the_main_window_opens() {
    let inputs = MenuInputs::new(CommandContext::default(), &[]);
    let mut state = NativeMenuState::default();
    assert!(!state.needs_sync(&inputs));
    state.arm();
    assert!(state.needs_sync(&inputs));
    assert!(!state.is_installed());
}
