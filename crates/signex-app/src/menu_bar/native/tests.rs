use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use iced::menu::{Action, Item, Predefined};

use super::probe::{action_at, root_titles, row_message};
use super::*;
use crate::menu_bar::MenuContext;
use crate::menu_bar::golden_tests::variants;
use crate::menu_bar::model::menu_bar_model;

type NativeMenu = iced_menu::Menu<Message>;

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

fn built(ctx: &MenuContext) -> NativeMenu {
    build(&menu_bar_model(ctx), false)
}

/// The model row a native row carries, with the shortcut text appended.
fn label_of(item: &MenuItem) -> String {
    match &item.shortcut {
        Some(shortcut) => format!("{}{SHORTCUT_GAP}{}", item.label, shortcut.text),
        None => item.label.clone(),
    }
}

/// Native items mirror model entries: same order, separators, submenus,
/// labels, enablement and check marks; a placeholder is disabled.
fn assert_mirrors(menu: &Menu, native: &Item<Message>, is_root: bool) {
    let Item::Submenu(sub) = native else {
        panic!("{} is not a submenu", menu.title);
    };
    assert_eq!(menu.title, sub.label);
    // The app menu carries Quit, so the native File menu drops Exit and the
    // separator above it.
    let entries: Vec<&MenuEntry> = if is_root && menu.title == "File" {
        let mut kept: Vec<&MenuEntry> = menu.entries.iter().collect();
        kept.truncate(kept.len() - 2);
        kept
    } else {
        menu.entries.iter().collect()
    };
    assert_eq!(entries.len(), sub.items.len(), "{}", menu.title);
    for (entry, node) in entries.into_iter().zip(&sub.items) {
        match (entry, node) {
            (MenuEntry::Separator, Item::Separator) => {}
            (MenuEntry::Submenu(inner), node @ Item::Submenu(_)) => {
                assert_mirrors(inner, node, false)
            }
            (MenuEntry::Item(item), Item::Action(native)) => {
                assert_eq!(label_of(item), native.label);
                assert_eq!(
                    item.is_enabled(),
                    native.on_activate.is_some(),
                    "{}",
                    item.label
                );
                assert_eq!(item.checked, native.checked, "{}", item.label);
                assert!(native.shortcut.is_none(), "model rows register no key");
            }
            (entry, node) => panic!("shape differs: {entry:?} vs {node:?}"),
        }
    }
}

#[test]
fn menu_mirrors_the_model_for_every_variant() {
    let recent = recent_paths();
    for ctx in contexts(&recent) {
        let model = menu_bar_model(&ctx);
        let menu = build(&model, false);
        assert_eq!(menu.items().len(), model.menus.len() + 1);
        assert_eq!(root_titles(&menu)[0], APP_MENU_TITLE);
        for (model_menu, native) in model.menus.iter().zip(&menu.items()[1..]) {
            assert_mirrors(model_menu, native, true);
        }
    }
}

/// `path -> row label` of every action row, recursing through submenus.
fn rows(items: &[Item<Message>], path: &str, out: &mut Vec<(String, Action<Message>)>) {
    for item in items {
        match item {
            Item::Action(action) => out.push((format!("{path}/{}", action.label), action.clone())),
            Item::Submenu(sub) => rows(&sub.items, &format!("{path}/{}", sub.label), out),
            Item::Separator | Item::Predefined(_) => {}
        }
    }
}

fn all_rows(menu: &NativeMenu) -> Vec<(String, Action<Message>)> {
    let mut out = Vec::new();
    rows(menu.items(), "", &mut out);
    out
}

/// Row labels are unique within one submenu, so the fork's id (the label
/// path) never needs a suffix; the one exception is Open Recent, where two
/// projects can share a file name and the fork suffixes the repeat.
#[test]
fn row_paths_are_unique_outside_open_recent() {
    let recent = recent_paths();
    for ctx in contexts(&recent) {
        let rows: Vec<String> = all_rows(&built(&ctx))
            .into_iter()
            .map(|(path, _)| path)
            .filter(|path| !path.starts_with("/File/Open Recent/"))
            .collect();
        let unique: HashSet<&String> = rows.iter().collect();
        assert_eq!(unique.len(), rows.len(), "duplicate row path in {rows:?}");
    }
}

/// Whenever a row is live its message kind is the same in every context: a
/// row path never changes what it does.
#[test]
fn a_live_row_always_sends_the_same_message_kind() {
    let recent = recent_paths();
    let mut kinds: HashMap<String, String> = HashMap::new();
    for ctx in contexts(&recent) {
        for (path, action) in all_rows(&built(&ctx)) {
            let Some(message) = &action.on_activate else {
                continue;
            };
            let kind = model_kind(message);
            if let Some(before) = kinds.insert(path.clone(), kind.clone()) {
                assert_eq!(before, kind, "row {path} changed message");
            }
        }
    }
    assert!(kinds.len() > 40);
}

/// The model action's variant name, without its payload.
fn model_kind(message: &Message) -> String {
    match message {
        Message::Window(WindowMsg::NativeMenuRowChosen(action)) => {
            let text = format!("{action:?}");
            text.split('(').next().unwrap_or_default().to_owned()
        }
        Message::Menu(action) => format!("{action:?}"),
        other => panic!("a native row sent {other:?}"),
    }
}

#[test]
fn a_locked_bar_greys_the_model_rows_and_keeps_the_app_menu() {
    let ctx = variants().remove(1);
    let unlocked = built(&ctx);
    assert!(row_message(&unlocked, "File/Save").is_some());

    let locked = build(&menu_bar_model(&ctx), true);
    let live: Vec<String> = all_rows(&locked)
        .into_iter()
        .filter(|(_, action)| action.on_activate.is_some())
        .map(|(path, _)| path)
        .collect();
    assert_eq!(
        live,
        ["/Signex/About Signex", "/Signex/Preferences..."],
        "only the app menu stays live"
    );
    assert!(row_message(&locked, "File/Save").is_none());
}

#[test]
fn recent_projects_send_their_own_path() {
    let recent = recent_paths();
    let ctx = MenuContext {
        recent_projects: &recent,
        ..MenuContext::default()
    };
    let menu = built(&ctx);
    let sent: Vec<PathBuf> = all_rows(&menu)
        .into_iter()
        .filter_map(|(_, action)| match action.on_activate {
            Some(Message::Window(WindowMsg::NativeMenuRowChosen(
                MenuMessage::OpenRecentProject(path),
            ))) => Some(path),
            _ => None,
        })
        .collect();
    assert_eq!(sent, recent, "each row opens its own project, in order");
}

fn chosen(menu: &NativeMenu, path: &str) -> Option<MenuMessage> {
    match row_message(menu, path)? {
        Message::Window(WindowMsg::NativeMenuRowChosen(action)) => Some(action),
        other => panic!("{path} sent {other:?}"),
    }
}

#[test]
fn only_enabled_live_rows_send_a_message() {
    let menu = built(&MenuContext::default());
    // New Project is always enabled; Save needs an open document; the
    // placeholder "No recent projects" has no handler at all.
    assert!(matches!(
        chosen(&menu, "File/New Project"),
        Some(MenuMessage::Command(id)) if id.as_str() == "new_document"
    ));
    assert!(chosen(&menu, "File/Save").is_none());
    assert!(
        action_at(&menu, "File/Save").is_some(),
        "greyed, not absent"
    );
    assert!(chosen(&menu, "File/Open Recent/No recent projects").is_none());
    assert!(action_at(&menu, "nonsense/row").is_none());
    let menu = built(&variants().remove(1));
    assert!(matches!(
        chosen(&menu, "File/Save"),
        Some(MenuMessage::Command(id)) if id.as_str() == "save_document"
    ));
}

#[test]
fn app_menu_has_the_approved_content() {
    let menu = built(&MenuContext::default());
    let Item::Submenu(app) = &menu.items()[0] else {
        panic!("the first item is the app menu");
    };
    let shape: Vec<String> = app
        .items
        .iter()
        .map(|item| match item {
            Item::Action(a) => format!("{} {:?}", a.label, a.shortcut),
            Item::Predefined(p) => format!("{p:?}"),
            Item::Separator => "-".into(),
            Item::Submenu(_) => "submenu".into(),
        })
        .collect();
    assert_eq!(
        shape,
        [
            "About Signex None",
            "-",
            "Preferences... Some(\"CmdOrCtrl+,\")",
            "-",
            "Hide",
            "HideOthers",
            "ShowAll",
            "-",
            "Quit",
        ]
    );
    assert!(matches!(
        app.items.last(),
        Some(Item::Predefined(Predefined::Quit))
    ));
    assert!(matches!(
        row_message(&menu, "Signex/About Signex"),
        Some(Message::Menu(MenuMessage::OpenAbout))
    ));
    assert!(matches!(
        row_message(&menu, "Signex/Preferences..."),
        Some(Message::Menu(MenuMessage::Command(id))) if id.as_str() == "open_preferences"
    ));
}

/// #103 - the panel rows' check marks follow the model.
#[test]
fn the_native_menu_carries_the_check_mark() {
    let closed = built(&MenuContext::default());
    let open = built(&MenuContext {
        open_panels: vec![crate::panels::PanelKind::Properties],
        ..MenuContext::default()
    });
    let checked_of = |menu: &NativeMenu| action_at(menu, "View/Properties").unwrap().checked;
    assert_eq!(checked_of(&closed), Some(false));
    assert_eq!(checked_of(&open), Some(true));
    assert_eq!(
        action_at(&closed, "File/New Project").unwrap().checked,
        None,
        "a plain row shows no mark"
    );
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

/// #1765: a native row shows its keymap shortcut as label text but registers
/// no key equivalent, so AppKit cannot take the key before the keymap sees
/// it. Only Preferences keeps a real Command+comma.
#[test]
fn native_rows_show_the_keymap_shortcut_without_a_key_equivalent() {
    let menu = built(&built_in_keymap_context());
    let save = action_at(&menu, "File/Save").unwrap();
    assert_eq!(save.label, format!("Save{SHORTCUT_GAP}Ctrl+S"));
    assert!(save.shortcut.is_none());
    let redo = action_at(&menu, "Edit/Redo").unwrap();
    assert_eq!(redo.label, format!("Redo{SHORTCUT_GAP}Ctrl+Y"));
    assert!(redo.shortcut.is_none());
    let preferences = action_at(&menu, "Signex/Preferences...").unwrap();
    assert_eq!(preferences.shortcut.as_deref(), Some("CmdOrCtrl+,"));
    // No model row anywhere registers a key.
    let model_keys = all_rows(&menu)
        .into_iter()
        .filter(|(path, _)| !path.starts_with("/Signex/"))
        .filter(|(_, action)| action.shortcut.is_some())
        .count();
    assert_eq!(model_keys, 0);
}

/// #1765: File gains Close Document and Print, and loses Exit natively (Quit
/// lives in the app menu).
#[test]
fn the_native_file_menu_has_the_approved_rows() {
    let menu = built(&variants().remove(1));
    for path in ["File/Close Document", "File/Print..."] {
        assert!(action_at(&menu, path).is_some(), "{path}");
    }
    assert!(action_at(&menu, "File/Exit").is_none());
    let Some(Item::Submenu(file)) = menu
        .items()
        .iter()
        .find(|item| matches!(item, Item::Submenu(sub) if sub.label == "File"))
    else {
        panic!("no File menu");
    };
    assert!(!matches!(file.items.last(), Some(Item::Separator)));
    assert!(matches!(
        chosen(&menu, "File/Close Document"),
        Some(MenuMessage::Command(id)) if id.as_str() == "close_active_document"
    ));
    // Close Project moved to the Project menu (2026-11-07 decision).
    assert!(matches!(
        chosen(&menu, "Project/Close Project"),
        Some(MenuMessage::CloseProject)
    ));
    assert!(matches!(
        chosen(&menu, "File/Print..."),
        Some(MenuMessage::Command(id)) if id.as_str() == "print"
    ));
    assert!(
        action_at(&menu, "File/Print...")
            .unwrap()
            .shortcut
            .is_none()
    );
}

fn en() -> std::sync::Arc<signex_i18n::I18n> {
    signex_i18n::I18n::en_us()
}

#[test]
fn inputs_compare_by_what_the_menu_reads() {
    use crate::app::command::CommandContext;
    let recent = recent_paths();
    let new = |ctx, recent: &[PathBuf], locked, panels| {
        MenuInputs::new(ctx, recent, locked, panels, en())
    };
    let a = new(CommandContext::default(), &recent, false, Vec::new());
    assert_eq!(
        a,
        new(CommandContext::default(), &recent, false, Vec::new())
    );
    assert_ne!(a, new(CommandContext::default(), &recent, true, Vec::new()));
    assert_ne!(
        a,
        new(CommandContext::default(), &recent[1..], false, Vec::new())
    );
    let dirty = CommandContext {
        project_dirty: true,
        ..CommandContext::default()
    };
    assert_ne!(a, new(dirty, &recent, false, Vec::new()));
    let panels = vec![crate::panels::PanelKind::Properties];
    assert_ne!(a, new(CommandContext::default(), &recent, false, panels));
}

#[test]
fn the_menu_inputs_follow_the_active_keymap() {
    let compile = || {
        std::sync::Arc::new(
            crate::keymap::ShortcutProfileSet::built_ins()
                .unwrap()
                .compile_active(),
        )
    };
    let (first, again) = (compile(), compile());
    let base = || {
        MenuInputs::new(
            crate::app::command::CommandContext::default(),
            &[],
            false,
            Vec::new(),
            en(),
        )
    };
    assert_eq!(
        base().with_keymap(first.clone()),
        base().with_keymap(first.clone())
    );
    assert_ne!(base().with_keymap(first), base().with_keymap(again));
}

/// #2055: a new language changes the root titles, which the fork sees as a
/// different menu and rebuilds.
#[test]
fn a_new_language_changes_the_native_titles() {
    let titles_in = |tag: &str| {
        let ctx = MenuContext {
            i18n: std::sync::Arc::new(signex_i18n::I18n::new(&signex_i18n::LanguagePref::Locale(
                tag.to_owned(),
            ))),
            ..MenuContext::default()
        };
        root_titles(&built(&ctx))
    };
    let (en, xa) = (titles_in("en-US"), titles_in("en-XA"));
    assert_eq!(en, titles_in("en-US"));
    assert_ne!(en, xa);
}
