//! Where an installed extension shows up besides its own surfaces (#2495,
//! the maintainer's answers of 2026-11-25): its own `.ftl` messages, its
//! panels in the panel list and under View ▸ Extensions, its commands in the
//! palette and the keyboard shortcuts, bindable to keys, and its Preferences
//! page.

use iced::{Element, Widget as _};

use super::points::{ExtensionPanelKind, RegisterError};
use super::test_extension::{GREETING, NOTES, SAY_HELLO, sample_host, sample_id};
use super::*;
use crate::app::{Message, Signex};
use crate::dock::PanelPosition;
use crate::keymap::{AppCommandId, KeymapEditorModel, ShortcutProfileSet, ShortcutTrigger};
use crate::menu_bar::model::{MenuEntry, menu_bar_model};
use crate::menu_bar::{MenuContext, MenuMessage};
use crate::panels::PanelKind;
use crate::panels::panel_groups::{PanelGroup, grouped_panels, with_extension_panels};

fn app_with_sample() -> Signex {
    let (host, _seen) = sample_host();
    let (app, _boot) = Signex::new();
    app.with_extensions(host)
}

// ── (8) An extension's own messages ──────────────────────────────────────

const OWN_FTL: &str = "ext-told-title = Told panel\n";

struct Blank;

impl ExtensionView for Blank {
    fn view<'a>(&'a self, _cx: ViewContext<'a>) -> Element<'a, ExtensionMsg> {
        iced::widget::Space::new().boxed()
    }
}

/// An extension named `told` that ships `source` and titles its panel with
/// its own message.
struct Told(&'static str);

impl Extension for Told {
    fn name(&self) -> &str {
        "told"
    }

    fn register(&mut self, points: &mut ExtensionPoints) {
        let _ = points.messages.add("en-US", self.0);
        if let Some(title) = points.messages.id("ext-told-title") {
            let _ = points
                .panels
                .add("ext.told.notes", title, PanelPosition::Right, Blank);
        }
    }
}

#[test]
fn an_extension_titles_its_panel_with_its_own_message_in_the_app_language() {
    let host = ExtensionHost::install(vec![Box::new(Told(OWN_FTL))]).expect("installs");
    let (app, _boot) = Signex::new();
    let mut app = app.with_extensions(host);
    let panel = app.extensions().panel("ext.told.notes").expect("the panel");
    assert!(panel.title().is_extension());
    let shown = |app: &Signex| app.ui_state.i18n.message(panel.title(), &[]).into_string();
    assert_eq!(shown(&app), "Told panel");
    // The panel context the dock draws with carries it too.
    assert_eq!(
        app.document_state
            .panel_ctx
            .i18n
            .message(panel.title(), &[])
            .into_string(),
        "Told panel"
    );

    // A language switch keeps it; the pseudolocale reads the en-US file.
    let _task = app.set_language(signex_i18n::LanguagePref::Locale("en-XA".to_owned()));
    let pseudo = shown(&app);
    assert!(
        pseudo.starts_with('[') && pseudo != "ext-told-title",
        "{pseudo}"
    );
}

#[test]
fn a_message_file_must_parse_and_stay_in_its_extensions_namespace() {
    let refused = |source: &'static str| match ExtensionHost::install(vec![Box::new(Told(source))])
    {
        Err(InstallError::Refused { error, .. }) => error,
        Err(other) => panic!("refused for another reason: {other}"),
        Ok(_) => panic!("{source:?} was accepted"),
    };
    // A built-in id, a built-in term, another extension's namespace.
    assert_eq!(
        refused("menu-file = Mine\n"),
        RegisterError::NotOwned("menu-file".to_owned())
    );
    assert_eq!(
        refused("-brand-name = Mine\n"),
        RegisterError::NotOwned("-brand-name".to_owned())
    );
    assert_eq!(
        refused("ext-other-title = Theirs\n"),
        RegisterError::NotOwned("ext-other-title".to_owned())
    );
    assert!(matches!(
        refused("ext-told-title\n"),
        RegisterError::InvalidMessages(_)
    ));
    // An id outside the namespace is not handed out either.
    let mut points = ExtensionPoints::new(ExtensionId::new("told").expect("a name"));
    assert!(points.messages.id("ext-other-title").is_none());
    assert!(points.messages.id("menu-file").is_none());
    assert!(points.messages.add("en-US", OWN_FTL).is_ok());
    assert!(points.messages.id("ext-told-title").is_some());
}

// ── (9) The panel list and View ▸ Extensions ─────────────────────────────

#[test]
fn extension_panels_are_listed_under_an_extensions_group_last() {
    let app = app_with_sample();
    let notes = app.extensions().panel(NOTES).expect("the sample's panel");
    let built_in = grouped_panels(|_| true);
    let listed = with_extension_panels(built_in.clone(), &app.extensions().panels());
    assert_eq!(listed[..built_in.len()], built_in[..]);
    assert_eq!(
        listed.last(),
        Some(&(PanelGroup::Extensions, vec![PanelKind::Extension(notes)]))
    );
    // With no extension panel the list is the built-in one.
    assert_eq!(with_extension_panels(built_in.clone(), &[]), built_in);
    assert!(!PanelGroup::ALL.contains(&PanelGroup::Extensions));
}

/// The View menu's Extensions submenu and its rows; `None` when there is
/// no such submenu.
fn view_extensions(panels: Vec<ExtensionPanelKind>) -> Option<Vec<MenuEntry>> {
    let model = menu_bar_model(&MenuContext::default().with_extension_panels(panels));
    let view = model.menus.iter().find(|menu| menu.title == "View")?;
    view.entries.iter().find_map(|entry| match entry {
        MenuEntry::Submenu(menu) if menu.title == "Extensions" => Some(menu.entries.clone()),
        _ => None,
    })
}

#[test]
fn view_lists_extension_panels_in_an_extensions_submenu_that_opens_them() {
    let mut app = app_with_sample();
    let notes = app.extensions().panel(NOTES).expect("the sample's panel");
    assert!(view_extensions(Vec::new()).is_none(), "Community has none");
    let rows = view_extensions(app.extensions().panels()).expect("the submenu");
    let [MenuEntry::Item(row)] = rows.as_slice() else {
        panic!("one row: {rows:?}");
    };
    assert_eq!(row.label, "Wiki");
    let picked = row.dispatches().cloned().expect("a live row");
    assert!(matches!(picked, MenuMessage::OpenExtensionPanel(kind) if kind == notes));

    let _task = app.update(Message::Menu(picked));
    assert!(
        app.document_state
            .dock
            .locate(PanelKind::Extension(notes))
            .is_some()
    );
}

// ── (20) The palette and the keyboard shortcuts ──────────────────────────

#[test]
fn the_palette_lists_extension_commands_with_their_extension_and_panels() {
    use crate::app::command_palette::{CommandAction, build_catalog};
    let app = app_with_sample();
    let notes = app.extensions().panel(NOTES).expect("the sample's panel");
    let catalog = build_catalog(&app);
    let command = catalog
        .iter()
        .find(
            |entry| matches!(&entry.action, CommandAction::Command(id) if id.as_str() == SAY_HELLO),
        )
        .expect("the sample's command row");
    assert!(command.label.starts_with("sample: "), "{}", command.label);
    assert!(catalog.iter().any(|entry| matches!(
        entry.action,
        CommandAction::Panel(PanelKind::Extension(kind)) if kind == notes
    )));

    // The Community palette has neither.
    let (community, _boot) = Signex::new();
    assert!(
        build_catalog(&community)
            .iter()
            .all(|entry| !matches!(&entry.action, CommandAction::Command(id) if id.is_extension()))
    );
}

/// A custom profile that binds Ctrl+Alt+H to the sample's command.
const BOUND: &str = r#"[signex_settings]
application = "signex"
file_kind = "keyboard_shortcuts"
version = 1

[keyboard_shortcuts]
schema_version = 1
profile_id = "mine"
profile_name = "Mine"
profile_kind = "custom"
base_profile = "signex_standard"

[[keyboard_shortcuts.sections]]
context = "global"

[keyboard_shortcuts.sections.bindings]
"Ctrl+Alt+H" = "ext.sample.say_hello"
"#;

fn bound_profiles() -> ShortcutProfileSet {
    let mine = crate::keymap::import_custom_profile(BOUND).expect("the profile parses");
    let mut set = ShortcutProfileSet::built_ins().expect("built-ins");
    set.insert_custom_profile(mine).expect("inserted");
    set.set_active_profile("mine").expect("active");
    set
}

#[test]
fn an_extension_command_binds_to_a_key_and_the_binding_is_saved_under_its_id() {
    let set = bound_profiles();
    let ShortcutTrigger::KeySequence(strokes) =
        ShortcutTrigger::parse("Ctrl+Alt+H").expect("a trigger")
    else {
        panic!("a key sequence");
    };
    let lookup = set
        .compile_active()
        .lookup(&strokes, &[crate::keymap::ShortcutContext::Global]);
    assert_eq!(
        lookup.command.as_ref().map(AppCommandId::as_str),
        Some(SAY_HELLO)
    );
    let saved = crate::keymap::export_custom_profile(set.active_profile()).expect("exports");
    assert!(saved.contains("ext.sample.say_hello"), "{saved}");
}

#[test]
fn a_binding_shows_its_extension_or_is_kept_and_shown_unavailable() {
    let set = bound_profiles();
    let app = app_with_sample();
    let i18n = signex_i18n::I18n::en_us();
    let editor = KeymapEditorModel::new(set.clone());
    let row_for = |rows: Vec<crate::keymap::KeymapEditorRow>| {
        rows.into_iter()
            .find(|row| {
                row.command
                    .as_ref()
                    .is_some_and(|c| c.as_str() == SAY_HELLO)
            })
            .expect("the binding's row")
    };

    // Installed: the extension's name and the command's label.
    let installed = row_for(editor.rows_in_with(&i18n, &app.ui_state.extension_commands));
    assert!(
        installed.label.starts_with("sample: "),
        "{}",
        installed.label
    );
    assert_eq!(installed.category, "sample");

    // Not installed: kept, and labelled unavailable.
    let missing = row_for(editor.rows_in(&i18n));
    assert!(missing.label.contains("unavailable"), "{}", missing.label);
    assert!(missing.label.contains(SAY_HELLO), "{}", missing.label);

    // The Keyboard Shortcuts sheet lists it either way.
    for names in [
        app.ui_state.extension_commands.clone(),
        crate::keymap::ExtensionCommandNames::default(),
    ] {
        let sections = crate::keyboard_shortcuts_modal::build_sections_with(&i18n, &set, &names);
        assert!(
            sections.iter().flat_map(|s| &s.rows).any(|row| row
                .command
                .as_ref()
                .is_some_and(|c| c.as_str() == SAY_HELLO)),
            "{names:?}"
        );
    }
}

// ── (13) The extension's Preferences page ────────────────────────────────

#[test]
fn an_extension_with_preferences_has_a_page_with_its_values() {
    use crate::preferences::PrefNav;
    let mut app = app_with_sample();
    let pages: Vec<_> = app.extensions().preference_pages().collect();
    let [page] = pages.as_slice() else {
        panic!("one page: {pages:?}");
    };
    assert_eq!((page.index, page.name), (0, "sample"));
    assert_eq!(page.values.get(GREETING), Some(&serde_json::json!("hi")));
    // Its row reads the sample's own label and help, not the key.
    let (label, help) =
        crate::preferences::extension_row_texts(&app.ui_state.i18n, page.preferences, GREETING);
    assert_eq!(label.into_string(), "Greeting");
    assert_eq!(
        help.map(signex_i18n::Localized::into_string).as_deref(),
        Some("What the sample says first.")
    );
    assert_eq!(
        crate::preferences::extension_value_kind(&serde_json::json!("hi")),
        signex_i18n::ids::PREFERENCES_EXTENSION_TYPE_TEXT
    );
    assert_eq!(PrefNav::Extension(page.index).group(), "Extensions");
    assert!(!PrefNav::ALL.contains(&PrefNav::Extension(0)));

    // A save that landed shows on the page.
    let saved = ExtensionMsg::new(
        sample_id(),
        PreferenceSaved {
            key: GREETING,
            value: serde_json::json!("hello"),
            result: Ok(()),
        },
    );
    let _task = app.update(Message::Extension(saved));
    let page = app.extensions().preference_page(0).expect("the page");
    assert_eq!(page.values.get(GREETING), Some(&serde_json::json!("hello")));

    // The Community binary has no extension page.
    let (community, _boot) = Signex::new();
    assert!(community.extensions().preference_pages().next().is_none());
}

/// An extension named `told` that ships `OWN_PREFS_FTL` and adds the
/// preference `ext.told.color` with the label and help it is given.
struct ToldPrefs {
    label: &'static str,
    help: Option<&'static str>,
}

const OWN_PREFS_FTL: &str =
    "ext-told-color = Accent colour\next-told-color-help = The colour the panel draws in.\n";

impl Extension for ToldPrefs {
    fn name(&self) -> &str {
        "told"
    }

    fn register(&mut self, points: &mut ExtensionPoints) {
        let _ = points.messages.add("en-US", OWN_PREFS_FTL);
        let id = |text| MessageId::extension(text).expect("an extension message id");
        let _ = points.preferences.add(
            "ext.told.color",
            id(self.label),
            self.help.map(id),
            serde_json::json!("blue"),
        );
    }
}

/// The label and help the `told` page's row shows.
fn told_row(label: &'static str, help: Option<&'static str>) -> (String, Option<String>) {
    let host = ExtensionHost::install(vec![Box::new(ToldPrefs { label, help })]).expect("installs");
    let (app, _boot) = Signex::new();
    let app = app.with_extensions(host);
    let page = app.extensions().preference_page(0).expect("the page");
    let (label, help) = crate::preferences::extension_row_texts(
        &app.ui_state.i18n,
        page.preferences,
        "ext.told.color",
    );
    (
        label.into_string(),
        help.map(signex_i18n::Localized::into_string),
    )
}

#[test]
fn a_preference_row_shows_its_label_and_help() {
    assert_eq!(
        told_row("ext-told-color", Some("ext-told-color-help")),
        (
            "Accent colour".to_owned(),
            Some("The colour the panel draws in.".to_owned())
        )
    );
}

/// No help: none is shown. A label the extension's files lack: the key.
#[test]
fn a_preference_row_falls_back_to_its_key_when_its_label_does_not_resolve() {
    assert_eq!(
        told_row("ext-told-color", None),
        ("Accent colour".to_owned(), None)
    );
    let (label, help) = told_row("ext-told-missing", Some("ext-told-also-missing"));
    assert!(label.contains("ext.told.color"), "{label}");
    assert_eq!(help, None);
}

#[test]
fn a_preference_label_or_help_from_another_namespace_is_refused() {
    let refused =
        |label, help| match ExtensionHost::install(vec![Box::new(ToldPrefs { label, help })]) {
            Err(InstallError::Refused { error, .. }) => error,
            Err(other) => panic!("refused for another reason: {other}"),
            Ok(_) => panic!("{label:?} / {help:?} was accepted"),
        };
    assert_eq!(
        refused("ext-other-color", None),
        RegisterError::ForeignMessage("ext-other-color".to_owned())
    );
    assert_eq!(
        refused("ext-told-color", Some("ext-other-help")),
        RegisterError::ForeignMessage("ext-other-help".to_owned())
    );
    // A built-in id is not one of the extension's messages either.
    let mut points = ExtensionPoints::new(ExtensionId::new("told").expect("a name"));
    assert_eq!(
        points.preferences.add(
            "ext.told.color",
            signex_i18n::ids::PANELS_KIND_WIKI,
            None,
            serde_json::json!(1)
        ),
        Err(RegisterError::ForeignMessage("panels-kind-wiki".to_owned()))
    );
}
