//! The dock panel, menu row, Start-screen and preference points (#2495),
//! driven through the application with the sample extension of
//! `test_extension.rs`; and the refusals that keep a binary from running
//! with half an extension. The Active Bar row is driven in
//! `app/view/pcb_active_bar/extension_row_tests.rs`, the Start-screen slot
//! in `start_screen/extension_slot.rs` and the saved layout in
//! `fonts/dock_layout.rs`.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use super::points::{MenuRoot, RegisterError, StartScreenSlot};
use super::test_extension::{
    BODY, GREETING, GREETING_LABEL, LABEL, NOTES, Pinged, SAY_HELLO, Seen, WELCOME, sample_host,
    sample_id,
};
use super::*;
use crate::app::{Message, Signex};
use crate::dock::{DockMessage, PanelPosition, PanelSite};
use crate::menu_bar::model::{MenuEntry, menu_bar_model};
use crate::menu_bar::{MenuContext, MenuMessage};
use crate::panels::PanelKind;

fn app_with_sample() -> (Signex, Arc<Mutex<Seen>>) {
    let (host, seen) = sample_host();
    let (app, _boot) = Signex::new();
    (app.with_extensions(host), seen)
}

/// The rows of `entries` as text: a label, `---` or a submenu's title and
/// rows, so two menus compare row by row.
fn rows_of(entries: &[MenuEntry]) -> Vec<String> {
    entries
        .iter()
        .flat_map(|entry| match entry {
            MenuEntry::Item(item) => vec![item.label.clone()],
            MenuEntry::Separator => vec!["---".to_owned()],
            MenuEntry::Submenu(menu) => std::iter::once(format!("{} >", menu.title))
                .chain(rows_of(&menu.entries))
                .collect(),
        })
        .collect()
}

#[test]
fn a_panel_opens_in_its_home_region_and_its_messages_reach_its_update_hook() {
    let (mut app, seen) = app_with_sample();
    let notes = app.extensions().panel(NOTES).expect("registered");
    assert_eq!(
        (notes.key(), notes.title(), notes.home()),
        (NOTES, LABEL, PanelPosition::Right)
    );
    assert_eq!(PanelKind::Extension(notes).message(), LABEL);
    assert_eq!(
        PanelPosition::default_for(PanelKind::Extension(notes)),
        PanelPosition::Right
    );

    let _ = app.update(notes.open_message());
    assert!(matches!(
        app.document_state.dock.locate(PanelKind::Extension(notes)),
        Some(PanelSite::Docked(PanelPosition::Right, _))
    ));

    // Its view hook draws it; a click sends a message for the panel.
    let i18n = signex_i18n::I18n::en_us();
    let tokens = &app.document_state.panel_ctx.tokens;
    let body = i18n.message(BODY, &[]).into_string();
    let view = app
        .extensions()
        .view_panel(notes, ViewContext::new(tokens, &i18n));
    let sent = crate::layout_probe::messages_on_click(view, iced::Size::new(400.0, 300.0), &body);
    assert_eq!(sent.len(), 1, "one click, one message");
    assert_eq!(sent[0].view(), Some(NOTES));

    for message in sent {
        let _ = app.update(Message::Dock(DockMessage::Extension(message)));
    }
    let seen = seen.lock().expect("seen");
    assert_eq!(seen.panel, [1]);
    assert!(seen.extension.is_empty() && seen.section.is_empty());
}

/// A message for a key the extension did not register goes to the
/// extension itself.
#[test]
fn a_message_for_an_unknown_view_key_reaches_the_extension() {
    let (mut app, seen) = app_with_sample();
    let _ = app.update(Message::Extension(ExtensionMsg::for_view(
        sample_id(),
        "ext.sample.nowhere",
        Pinged(5),
    )));
    let _ = app.update(Message::Extension(ExtensionMsg::for_view(
        sample_id(),
        WELCOME,
        Pinged(6),
    )));
    let seen = seen.lock().expect("seen");
    assert_eq!(seen.extension, [5]);
    assert_eq!(seen.section, [6]);
    assert!(seen.panel.is_empty());
}

#[test]
fn a_menu_row_ends_its_root_menu_and_runs_its_command_through_dispatch_command() {
    let (mut app, _seen) = app_with_sample();
    let community = menu_bar_model(&MenuContext::default());
    let with = menu_bar_model(&MenuContext {
        extension_rows: app.extensions().menu_rows(),
        ..MenuContext::default()
    });

    let tools = MenuRoot::Tools.index();
    assert_eq!(with.menus.len(), community.menus.len());
    for (index, (before, after)) in community.menus.iter().zip(&with.menus).enumerate() {
        assert_eq!(before.title, after.title);
        if index != tools {
            assert_eq!(
                rows_of(&before.entries),
                rows_of(&after.entries),
                "{}",
                before.title
            );
        }
    }
    let built_in = community.menus[tools].entries.len();
    let (kept, added) = with.menus[tools].entries.split_at(built_in);
    assert_eq!(rows_of(kept), rows_of(&community.menus[tools].entries));
    let [MenuEntry::Separator, MenuEntry::Item(row)] = added else {
        panic!("a separator, then the row: {:?}", rows_of(added));
    };
    assert_eq!(row.label, "Wiki");
    let action = row.dispatches().cloned().expect("a live row");
    let MenuMessage::Command(command) = &action else {
        panic!("the row runs a command: {action:?}");
    };
    assert_eq!(command.as_str(), SAY_HELLO);

    // The click takes the path every command row takes: dispatch_command,
    // which records the run in the Command Log.
    let _ = app.update(Message::Menu(action));
    assert!(
        app.ui_state
            .command_log
            .log
            .rows()
            .any(|logged| logged.command == SAY_HELLO)
    );
}

#[test]
fn preferences_load_from_their_namespace_with_defaults() {
    let (mut host, seen) = sample_host();
    host.load_preferences(&BTreeMap::new());
    let defaults = seen
        .lock()
        .expect("seen")
        .preferences
        .clone()
        .expect("loaded");
    assert_eq!(defaults.get(GREETING), Some(&serde_json::json!("hi")));

    let stored = BTreeMap::from([
        (GREETING.to_owned(), serde_json::json!("hello")),
        ("ext.sample.unknown".to_owned(), serde_json::json!(1)),
        ("ext.other.greeting".to_owned(), serde_json::json!("no")),
    ]);
    host.load_preferences(&stored);
    let values = seen
        .lock()
        .expect("seen")
        .preferences
        .clone()
        .expect("loaded");
    assert_eq!(values.get(GREETING), Some(&serde_json::json!("hello")));
    assert_eq!(values.get("ext.sample.unknown"), None);
    assert_eq!(values.get("ext.other.greeting"), None);
}

/// A save goes through a `Task`, never inline, and only for a key the
/// owner added.
#[test]
fn a_preference_is_saved_only_under_its_owners_prefix() {
    let (mut host, seen) = sample_host();
    host.load_preferences(&BTreeMap::new());
    let values = seen
        .lock()
        .expect("seen")
        .preferences
        .clone()
        .expect("loaded");
    assert_eq!(values.owner(), &sample_id());
    assert!(values.save(GREETING, serde_json::json!("hey")).is_ok());
    assert_eq!(
        values
            .save("ext.other.greeting", serde_json::json!(1))
            .err(),
        Some(RegisterError::NotOwned("ext.other.greeting".to_owned()))
    );
    assert!(values.save("theme", serde_json::json!(1)).is_err());
    // Under the owner's prefix, but never added with `Preferences::add`.
    assert_eq!(
        values
            .save("ext.sample.unknown", serde_json::json!(1))
            .err(),
        Some(RegisterError::NotRegistered(
            "ext.sample.unknown".to_owned()
        ))
    );
}

/// Registers through `add`, for the refusal tests.
struct Adds(fn(&mut ExtensionPoints));

impl Extension for Adds {
    fn name(&self) -> &str {
        "sample"
    }

    fn register(&mut self, points: &mut ExtensionPoints) {
        (self.0)(points);
    }
}

struct Blank;

impl ExtensionView for Blank {
    fn view<'a>(&'a self, _cx: ViewContext<'a>) -> iced::Element<'a, ExtensionMsg> {
        iced::Widget::boxed(iced::widget::Space::new())
    }
}

fn refusal(adds: fn(&mut ExtensionPoints)) -> Option<RegisterError> {
    match ExtensionHost::install(vec![Box::new(Adds(adds))]) {
        Err(InstallError::Refused { extension, error }) => {
            assert_eq!(extension, "sample");
            Some(error)
        }
        Err(other) => panic!("refused for another reason: {other}"),
        Ok(_) => None,
    }
}

#[test]
fn install_refuses_an_addition_outside_the_namespace_or_added_twice() {
    assert_eq!(
        refusal(|p| {
            let _ = p
                .panels
                .add("ext.other.notes", LABEL, PanelPosition::Right, Blank);
        }),
        Some(RegisterError::NotOwned("ext.other.notes".to_owned()))
    );
    assert_eq!(
        refusal(|p| {
            let _ = p.panels.add(NOTES, LABEL, PanelPosition::Right, Blank);
            let _ = p.panels.add(NOTES, LABEL, PanelPosition::Left, Blank);
        }),
        Some(RegisterError::Duplicate(NOTES.to_owned()))
    );
    assert_eq!(
        refusal(|p| {
            let _ = p
                .menu_rows
                .add_menu_row(MenuRoot::Tools, LABEL, "say_hello");
        }),
        Some(RegisterError::NotOwned("say_hello".to_owned()))
    );
    assert_eq!(
        refusal(|p| {
            let _ = p
                .menu_rows
                .add_menu_row(MenuRoot::Tools, LABEL, "ext.sample.Say");
        }),
        Some(RegisterError::InvalidId("ext.sample.Say".to_owned()))
    );
    assert_eq!(
        refusal(|p| {
            let _ = p
                .preferences
                .add("theme", GREETING_LABEL, None, serde_json::json!(1));
        }),
        Some(RegisterError::NotOwned("theme".to_owned()))
    );
    // A panel and a section share one key space.
    assert_eq!(
        refusal(|p| {
            let _ = p.panels.add(NOTES, LABEL, PanelPosition::Right, Blank);
            let _ = p
                .start_screen
                .add(NOTES, LABEL, StartScreenSlot::HomeEnd, Blank);
        }),
        Some(RegisterError::Duplicate(NOTES.to_owned()))
    );
    assert_eq!(
        refusal(|p| {
            let _ = p.panels.add(NOTES, LABEL, PanelPosition::Right, Blank);
        }),
        None
    );
}

/// A row must run a command its extension added: install refuses one that
/// names a command under the owner's prefix that was never added, from the
/// menu bar or the Active Bar, and accepts it once the command is added
/// (in either order).
#[test]
fn install_refuses_a_row_whose_command_the_extension_never_added() {
    use super::points::PcbBarGroup;
    assert_eq!(
        refusal(|p| {
            let _ = p.menu_rows.add_menu_row(MenuRoot::Tools, LABEL, SAY_HELLO);
        }),
        Some(RegisterError::NotRegistered(SAY_HELLO.to_owned()))
    );
    assert_eq!(
        refusal(|p| {
            let _ = p
                .menu_rows
                .add_active_bar_row(PcbBarGroup::Route, LABEL, SAY_HELLO);
        }),
        Some(RegisterError::NotRegistered(SAY_HELLO.to_owned()))
    );
    assert_eq!(
        refusal(|p| {
            let _ = p.menu_rows.add_menu_row(MenuRoot::Tools, LABEL, SAY_HELLO);
            let _ = p.commands.add(&ROW_SAY_HELLO, |_| Ok(Pinged(0)));
        }),
        None
    );
}

/// The catalog row of [`SAY_HELLO`], for the row tests.
static ROW_SAY_HELLO: CommandMetadata = CommandMetadata {
    id: SAY_HELLO,
    category: "sample",
    flags: CommandFlags::RUNS_HEADLESS,
    ..CommandMetadata::DEFAULT
};

/// The Community binary installs nothing: no panel, no row, no section, no
/// preference, and its menu bar is the built-in one row for row.
#[test]
fn with_no_extension_every_surface_is_the_built_in_one() {
    let (app, _boot) = Signex::new();
    let host = app.extensions();
    assert!(host.panel(NOTES).is_none());
    assert!(host.menu_rows().is_empty());
    assert!(host.active_bar_rows().is_empty());
    let i18n = signex_i18n::I18n::en_us();
    let cx = ViewContext::new(&app.document_state.panel_ctx.tokens, &i18n);
    assert!(
        host.view_start_screen_slot(StartScreenSlot::HomeEnd, cx)
            .is_empty()
    );

    let community = menu_bar_model(&MenuContext::default());
    let installed_none = menu_bar_model(&MenuContext {
        extension_rows: host.menu_rows(),
        ..MenuContext::default()
    });
    for (before, after) in community.menus.iter().zip(&installed_none.menus) {
        assert_eq!(rows_of(&before.entries), rows_of(&after.entries));
    }
}
