//! Preferences ▸ Keyboard shortcuts as drawn after `Signex::update` (mockup
//! row 145): the recorder after Clear (#3048), the category names (#3049)
//! and the status line in the interface language (#3050).

use iced::Size;
use signex_i18n::{LanguagePref, shortcut_ids as sids};

use crate::app::{Message, PreferencesMsg, Signex, UiMsg};
use crate::layout_probe::lay_out_texts;
use crate::preferences::{PrefMsg, PrefNav};

const WINDOW: Size = Size::new(1000.0, 4000.0);

fn send(app: &mut Signex, msg: PrefMsg) {
    let _ = app.update(Message::Preferences(PreferencesMsg::Inner(msg)));
}

/// The app with Preferences open on the Keyboard shortcuts page.
fn on_the_keyboard_page() -> Signex {
    let (mut app, _boot) = Signex::new();
    let _ = app.update(Message::Preferences(PreferencesMsg::Open));
    send(&mut app, PrefMsg::Nav(PrefNav::Keyboard));
    app
}

/// Every text the Preferences body lays out, without isolation marks.
fn page_texts(app: &Signex) -> Vec<String> {
    let body = crate::preferences::view_body(app.preferences_view_inputs());
    lay_out_texts(body, WINDOW)
        .into_iter()
        .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect()
}

/// #3050: Create's status line is catalog text, and a language switch
/// redraws it in the new language.
#[test]
fn the_status_line_follows_the_interface_language() {
    let mut app = on_the_keyboard_page();
    send(&mut app, PrefMsg::KeymapCreateCustomProfile);
    assert_eq!(
        app.ui_state.preferences_keymap_status.id(),
        Some(sids::PREF_KEYMAP_STATUS_CREATED)
    );
    let english = "Created a custom profile draft.";
    let shown = page_texts(&app);
    assert!(shown.iter().any(|t| t == english), "{shown:?}");

    let pseudo = LanguagePref::Locale("en-XA".into());
    let _ = app.update(Message::Ui(UiMsg::LanguageChanged(pseudo)));
    let translated = app
        .ui_state
        .i18n
        .message_plain(sids::PREF_KEYMAP_STATUS_CREATED, &[]);
    assert_ne!(translated, english, "the pseudo-locale rewrites the text");
    let shown = page_texts(&app);
    assert!(
        shown.contains(&translated),
        "{translated:?} not in {shown:?}"
    );
    assert!(!shown.iter().any(|t| t == english), "{shown:?}");
}

/// #3050: an export's status names the written file, from the catalog.
#[test]
fn the_export_status_names_the_file_from_the_catalog() {
    let mut app = on_the_keyboard_page();
    send(
        &mut app,
        PrefMsg::KeymapExportFinished(Ok(std::path::PathBuf::from("/tmp/mine.toml"))),
    );
    assert_eq!(
        app.ui_state.preferences_keymap_status.id(),
        Some(sids::PREF_KEYMAP_STATUS_EXPORTED)
    );
    let shown = page_texts(&app);
    assert!(
        shown
            .iter()
            .any(|t| t == "Exported keyboard shortcuts to /tmp/mine.toml."),
        "{shown:?}"
    );
}

/// #3048, mockup row 145: after Clear the recorder still records, so its
/// empty box asks for a keystroke; after Stop it names Record, which the
/// popover then shows.
#[test]
fn the_cleared_recorder_says_what_its_state_is() {
    let mut app = on_the_keyboard_page();
    send(&mut app, PrefMsg::KeymapCreateCustomProfile);
    let row = app
        .ui_state
        .preferences_keymap_editor
        .rows()
        .into_iter()
        .find(|r| r.keyboard_editable && r.command.is_some() && !r.trigger_config.is_empty())
        .expect("a keyboard binding of the custom copy");
    send(&mut app, PrefMsg::KeymapSearchChanged(row.label.clone()));
    send(
        &mut app,
        PrefMsg::KeymapRecorderOpen {
            slot: row.slot,
            command: row.command.clone().expect("a command"),
            label: row.label.clone(),
            context: row.context,
            trigger: row.trigger_config.clone(),
            unassigned: row.unassigned,
        },
    );
    send(&mut app, PrefMsg::KeymapRecorderClear);
    let recorder = app
        .ui_state
        .preferences_keymap_recorder
        .as_ref()
        .expect("the recorder is open");
    assert!(recorder.recording && recorder.strokes.is_empty());
    let waiting = "Press a keystroke to record it";
    let stopped = "Press Record, then type a shortcut";
    let shown = page_texts(&app);
    assert!(shown.iter().any(|t| t == waiting), "{shown:?}");
    assert!(!shown.iter().any(|t| t == stopped), "{shown:?}");
    assert!(shown.iter().any(|t| t == "Stop"), "{shown:?}");

    send(&mut app, PrefMsg::KeymapRecorderStop);
    let shown = page_texts(&app);
    assert!(shown.iter().any(|t| t == stopped), "{shown:?}");
    assert!(!shown.iter().any(|t| t == waiting), "{shown:?}");
    assert!(
        shown.iter().any(|t| t == "Record"),
        "the hint names a button the popover shows: {shown:?}"
    );
}

/// #3049: a board command's Category reads PCB, as every other surface
/// writes it.
#[test]
fn a_board_command_says_pcb_in_the_category_column() {
    let mut app = on_the_keyboard_page();
    send(&mut app, PrefMsg::KeymapSearchChanged("repour".into()));
    let shown = page_texts(&app);
    assert!(
        shown.iter().any(|t| t.to_lowercase().contains("repour")),
        "a board command is listed: {shown:?}"
    );
    assert!(shown.iter().any(|t| t == "PCB"), "{shown:?}");
    assert!(!shown.iter().any(|t| t == "Pcb"), "{shown:?}");
}
