//! #2868: Preferences ▸ PCB Defaults — typed numbers, picks and Save.

use signex_engine::pcb::primitive_edit::DimensionStyleEdit;
use signex_i18n::I18n;
use signex_types::pcb_dimension_style::{ArrowStyle, DimensionDefaults};

use super::pcb_defaults::apply_message;
use crate::app::{Message, PreferencesMsg, Signex};
use crate::panels::pcb_primitive_properties::PrimitiveField as F;
use crate::pcb_defaults_prefs::PcbDefaultsDraft;
use crate::preferences::{PcbDefaultsMsg, PrefMsg};

fn send(app: &mut Signex, msg: PrefMsg) {
    let _ = app.update(Message::Preferences(PreferencesMsg::Inner(msg)));
}

fn typed(draft: &mut PcbDefaultsDraft, field: F, text: &str) {
    let msg = PcbDefaultsMsg::Typed(field, text.into());
    apply_message(draft, msg, &I18n::en_us());
}

#[test]
fn a_typed_number_sets_the_draft_and_a_bad_one_is_refused() {
    let mut draft = PcbDefaultsDraft::of(&DimensionDefaults::default());
    typed(&mut draft, F::ArrowLength, "2.5");
    assert_eq!(draft.defaults.style.arrow_length, 2.5);
    assert!(draft.refused.is_empty());
    typed(&mut draft, F::ArrowLength, "abc");
    assert_eq!(
        draft.defaults.style.arrow_length, 2.5,
        "the last good value stays"
    );
    assert!(draft.refused.contains_key(&F::ArrowLength));
    typed(&mut draft, F::ArrowLength, "-1");
    assert!(
        draft.refused.contains_key(&F::ArrowLength),
        "a negative length"
    );
    typed(&mut draft, F::LineWidth, "0");
    assert!(
        draft.refused.contains_key(&F::LineWidth),
        "a line width of zero"
    );
    typed(&mut draft, F::ArrowLength, "3");
    assert!(!draft.refused.contains_key(&F::ArrowLength));
    typed(&mut draft, F::Prefix, "W=");
    assert_eq!(draft.defaults.style.prefix, "W=");
}

#[test]
fn picks_set_the_draft() {
    let mut draft = PcbDefaultsDraft::of(&DimensionDefaults::default());
    let en = I18n::en_us();
    let pick = PcbDefaultsMsg::Picked(DimensionStyleEdit::ArrowStyle(ArrowStyle::Filled));
    apply_message(&mut draft, pick, &en);
    apply_message(&mut draft, PcbDefaultsMsg::PrecisionPicked(4), &en);
    assert_eq!(draft.defaults.style.arrow_style, ArrowStyle::Filled);
    assert_eq!(draft.defaults.precision, 4);
}

/// Save commits the draft to what the next dimension starts from; a
/// refused field stops Save and keeps the saved defaults.
#[test]
fn save_commits_the_draft_and_a_refused_field_stops_it() {
    let (mut app, _boot) = Signex::new();
    let saved = app.ui_state.pcb_dimension_defaults.clone();
    let pick = PcbDefaultsMsg::Picked(DimensionStyleEdit::ArrowStyle(ArrowStyle::None));
    send(&mut app, PrefMsg::PcbDefaults(pick));
    let bad = PcbDefaultsMsg::Typed(F::TextWidth, "0".into());
    send(&mut app, PrefMsg::PcbDefaults(bad));
    assert!(app.ui_state.preferences_dirty);
    send(&mut app, PrefMsg::Save);
    assert_eq!(app.ui_state.pcb_dimension_defaults, saved, "refused");
    let good = PcbDefaultsMsg::Typed(F::TextWidth, "0.7".into());
    send(&mut app, PrefMsg::PcbDefaults(good));
    send(&mut app, PrefMsg::Save);
    let now = &app.ui_state.pcb_dimension_defaults;
    assert_eq!(now.style.arrow_style, ArrowStyle::None);
    assert_eq!(now.style.text_width, 0.7);
}
