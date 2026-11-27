//! Preferences text on the GUI path: the dialog opened and navigated through
//! `Signex::update`, the body laid out with the app's own fonts (mockup rows
//! 144a and 146).

use iced::Widget as _;
use iced::widget::pick_list;
use signex_i18n::LanguagePref;
use signex_widgets::dropdown_style::GraphiteDropdown as _;

use crate::app::{Message, PreferencesMsg, Signex};
use crate::layout_probe::{find, lay_out_texts, natural};
use crate::preferences::{PrefMsg, PrefNav};
use crate::styles::Spacing;

/// The detached Preferences window at its opening size.
const WINDOW: iced::Size = iced::Size::new(900.0, 620.0);

fn inner(msg: PrefMsg) -> Message {
    Message::Preferences(PreferencesMsg::Inner(msg))
}

/// Preferences opened and switched to `nav`, both through `update`.
fn open_on(nav: PrefNav) -> Signex {
    let (mut app, _task) = Signex::new();
    let _ = app.update(Message::Preferences(PreferencesMsg::Open));
    let _ = app.update(inner(PrefMsg::Nav(nav)));
    assert_eq!(app.ui_state.preferences_nav, nav);
    app
}

/// The width the Language page's dropdown asks for to show `label` whole
/// beside its handle: the same dropdown, sized to its value.
fn natural_dropdown_width(label: &str) -> f32 {
    let sp = Spacing::DEFAULT;
    let dropdown = pick_list(
        Some(label.to_owned()),
        vec![label.to_owned()],
        |s: &String| s.clone(),
    )
    .graphite()
    .padding([sp.s, sp.m])
    .width(iced::Length::Shrink);
    natural::<()>(dropdown.boxed(), label).width
}

/// #3003, mockup row 144a: with Follow the system drafted, the Interface
/// language dropdown is wide enough to show the whole language it resolves
/// to.
#[test]
fn the_language_dropdown_shows_the_whole_follow_the_system_value() {
    let mut app = open_on(PrefNav::Language);
    let _ = app.update(inner(PrefMsg::DraftLanguage(LanguagePref::System)));
    // Fixed, so the label does not depend on the language of the machine.
    app.ui_state.preferences_pages.system_language_name = "English (United States)".to_owned();
    let label = "Follow the system: English (United States)";

    let texts = lay_out_texts(app.view_preferences_body(), WINDOW);

    let field = find(&texts, label).bounds;
    let needed = natural_dropdown_width(label);
    assert!(
        needed <= field.width + 0.5,
        "the value is cut off: it needs {needed} px, the field is {field:?}"
    );
}

/// #3004, mockup row 146: the Annotation help bolds the option names it
/// explains, and the fonts the app draws with hold the bold face of the UI
/// family, so the bold runs stay in Inter.
#[test]
fn the_annotation_help_bold_runs_have_an_inter_bold_face() {
    let app = open_on(PrefNav::Annotation);

    let texts = lay_out_texts(app.view_preferences_body(), WINDOW);

    let help = texts.iter().find(|t| {
        t.text.contains("One step for the whole project") && t.text.contains("Per sheet")
    });
    assert!(help.is_some(), "no annotate undo help: {texts:#?}");
    let mut fonts = iced::advanced::graphics::text::font_system()
        .write()
        .expect("font system");
    let has_inter_bold = fonts
        .raw()
        .db()
        .faces()
        .any(|face| face.post_script_name == "Inter-Bold" && face.weight.0 == 700);
    assert!(has_inter_bold, "the app's fonts hold no Inter Bold face");
}
