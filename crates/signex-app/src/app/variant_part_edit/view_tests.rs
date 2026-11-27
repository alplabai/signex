//! Layout probes of the Alternate part picker (mockup row 208) and the Edit
//! value popover (mockup row 209), in Graphite Light and Dark: the Base part,
//! the candidates with the library alternates first, the compare strip and
//! its warning with Apply still live, the scope on a channel tab, and the
//! popover's fields, parameter list and Reset to base.

use iced::Size;
use signex_types::theme::{ThemeId, theme_tokens};

use super::VariantPartEditMsg;
use super::candidates::{CandidateSource, Candidates};
use super::picker::EditCard;
use super::state::VariantEditKind;
use super::update_tests::{alternate, app, open, send, strip};
use crate::app::{Message, ProjectMsg, Signex};
use crate::layout_probe::{lay_out_inputs, lay_out_texts, messages_on_click};

const WINDOW: Size = Size::new(1200.0, 900.0);
/// The Apply button's text as laid out, the variant in isolation marks.
const APPLY: &str = "Apply to \u{2068}Lite\u{2069}";

fn plain(text: &str) -> String {
    text.replace(['\u{2068}', '\u{2069}'], "")
}

fn card(app: &Signex) -> iced::Element<'_, Message> {
    let window = app.ui_state.main_window_id.expect("main window");
    match app.variant_part_edit_card(window).expect("a card is open") {
        EditCard::Centred(card) | EditCard::Anchored(card, _) => card,
    }
}

fn texts(app: &Signex) -> Vec<String> {
    lay_out_texts(card(app), WINDOW)
        .into_iter()
        .map(|t| plain(&t.text))
        .collect()
}

fn has(shown: &[String], words: &str) -> bool {
    shown.iter().any(|t| t == words)
}

fn set_theme(app: &mut Signex, theme: ThemeId) {
    app.document_view.panel_ctx.tokens = theme_tokens(theme);
}

/// The candidates landing for the open picker.
fn land(app: &mut Signex, candidates: Candidates) {
    let Some(super::VariantPartEdit::Alternate(picker)) = app.ui_state.variant_part_edit.as_deref()
    else {
        panic!("the picker is open");
    };
    let msg = VariantPartEditMsg::CandidatesRead {
        generation: picker.generation,
        part: picker.part.this_part.designator.clone(),
        query: picker.query.trim().to_string(),
        read: Ok(Box::new(candidates)),
    };
    send(app, msg);
}

fn applies(messages: &[Message]) -> bool {
    messages.iter().any(|message| {
        matches!(
            message,
            Message::Project(ProjectMsg::VariantPartEdit(
                VariantPartEditMsg::ApplyPressed
            ))
        )
    })
}

#[test]
fn the_picker_lists_alternates_first_and_compares_the_picked_one() {
    for theme in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
        let (mut app, _id, _path, _dir) = app();
        set_theme(&mut app, theme);
        open(&mut app, VariantEditKind::Alternate);
        assert!(
            has(&texts(&app), "Reading the project libraries…"),
            "{theme:?}"
        );
        let mut found = alternate("RC0603FR-0731K6L-ALT");
        found.source = CandidateSource::Search;
        land(
            &mut app,
            Candidates {
                library: Some("passives.snxlib".into()),
                alternates: vec![alternate("ERJ-3EKF3162V")],
                found: vec![found],
                found_total: 4,
                ..Candidates::default()
            },
        );
        send(&mut app, VariantPartEditMsg::CandidatePicked(0));
        let shown = texts(&app);
        for words in [
            "Alternate part",
            "R5 in Lite · power.snxsch",
            "Base part",
            "RC0603FR-0731K6L",
            "Library alternates of R5 in passives.snxlib · 1",
            "ERJ-3EKF3162V",
            "Search results for \"31.6k\" in the project libraries · 1 of 4",
            "RC0603FR-0731K6L-ALT",
            "Approved",
            "Same footprint",
            "Lite gets",
            "MPN ERJ-3EKF3162V · value 31.6k · footprint RES_0603",
            "Apply is one undo step. Nothing is written until you save the project.",
            "Apply to Lite",
        ] {
            assert!(has(&shown, words), "{theme:?}: {words} in {shown:?}");
        }
        let alternates = shown.iter().position(|t| t == "ERJ-3EKF3162V");
        let search = shown.iter().position(|t| t == "RC0603FR-0731K6L-ALT");
        assert!(alternates < search, "library alternates first");
        assert!(applies(&messages_on_click(card(&app), WINDOW, APPLY)));
    }
}

/// DECISIONS 2026-11-26: a footprint that differs warns in the strip and
/// the footer, and Apply stays live.
#[test]
fn a_differing_footprint_warns_and_apply_stays_live() {
    for theme in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
        let (mut app, _id, _path, _dir) = app();
        set_theme(&mut app, theme);
        open(&mut app, VariantEditKind::Alternate);
        let mut other = alternate("NFM31PC276B0J3L");
        other.footprint = "CAP_3T_1206".into();
        land(
            &mut app,
            Candidates {
                alternates: vec![other],
                ..Candidates::default()
            },
        );
        send(&mut app, VariantPartEditMsg::CandidatePicked(0));
        let shown = texts(&app);
        for words in [
            "Footprint differs",
            "CAP_3T_1206 vs RES_0603: the board changes in Lite",
            "Pins or footprint differ: check the wiring and the board after Apply.",
        ] {
            assert!(has(&shown, words), "{theme:?}: {words} in {shown:?}");
        }
        assert!(applies(&messages_on_click(card(&app), WINDOW, APPLY)));
    }
}

/// Before a part is picked Apply does nothing; on a channel tab the scope
/// is shown.
#[test]
fn apply_waits_for_a_pick_and_a_channel_tab_shows_the_scope() {
    let (mut app, _id, path, _dir) = app();
    app.document_view.panel_ctx.compiled_design.strip = Some(strip(path, Some(1)));
    open(&mut app, VariantEditKind::Alternate);
    land(&mut app, Candidates::default());
    let shown = texts(&app);
    // On a channel tab the part is the channel's copy.
    assert!(
        has(&shown, "CH1_R5 in Lite · CH1 tab of power.snxsch"),
        "{shown:?}"
    );
    assert!(has(&shown, "Scope"), "{shown:?}");
    assert!(!applies(&messages_on_click(card(&app), WINDOW, APPLY)));
}

#[test]
fn the_popover_shows_base_the_variants_value_and_reset_to_base() {
    for theme in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
        let (mut app, _id, _path, _dir) = app();
        set_theme(&mut app, theme);
        open(&mut app, VariantEditKind::Value);
        send(&mut app, VariantPartEditMsg::ValueEdited("47k".into()));
        let shown = texts(&app);
        for words in [
            "Edit value for this variant",
            "R5 · Lite",
            "Parameter",
            "Value",
            "Base",
            "31.6k",
            "Lite",
            "Enter OK · Esc Cancel · one undo step",
            "Reset to base",
            "OK",
        ] {
            assert!(has(&shown, words), "{theme:?}: {words} in {shown:?}");
        }
        let inputs = lay_out_inputs(card(&app), WINDOW);
        assert!(inputs.iter().any(|input| input.text == "47k"), "{inputs:?}");

        send(&mut app, VariantPartEditMsg::ParameterListToggled);
        let shown = texts(&app);
        for words in ["Parameters of R5 · Base value", "Tolerance", "1%", "MPN"] {
            assert!(has(&shown, words), "{theme:?}: {words} in {shown:?}");
        }
    }
}

/// DECISIONS 2026-11-26: an empty field says so and OK does nothing.
#[test]
fn an_empty_field_greys_ok() {
    let (mut app, _id, _path, _dir) = app();
    open(&mut app, VariantEditKind::Value);
    send(&mut app, VariantPartEditMsg::ValueEdited(String::new()));
    let shown = texts(&app);
    assert!(
        has(&shown, "Type a value, or use Reset to base."),
        "{shown:?}"
    );
    assert!(!applies(&messages_on_click(card(&app), WINDOW, "OK")));
}

/// A parameter Lite already gives a value of its own reads it, in the list.
#[test]
fn the_parameter_list_marks_a_value_lite_already_sets() {
    let (mut app, _id, _path, _dir) = app();
    open(&mut app, VariantEditKind::Value);
    send(
        &mut app,
        VariantPartEditMsg::ParameterPicked("Tolerance".into()),
    );
    send(&mut app, VariantPartEditMsg::ValueEdited("0.5%".into()));
    send(&mut app, VariantPartEditMsg::ApplyPressed);
    open(&mut app, VariantEditKind::Value);
    send(&mut app, VariantPartEditMsg::ParameterListToggled);
    let shown = texts(&app);
    assert!(has(&shown, "0.5% in Lite"), "{shown:?}");
}

/// The Base card says whether the variant fits the part (mockup row 208):
/// the variant's own fit, else what the sheets say.
#[test]
fn the_base_card_says_whether_the_variant_fits_the_part() {
    for theme in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
        let (mut app, _id, _path, _dir) = app();
        set_theme(&mut app, theme);
        open(&mut app, VariantEditKind::Alternate);
        assert!(has(&texts(&app), "Fitted in Lite"), "{theme:?}");
        app.document_state.projects[0]
            .data
            .set_variant_part_fit("Lite", "R5", Some(false))
            .expect("Lite is defined");
        assert!(has(&texts(&app), "Not fitted in Lite"), "{theme:?}");
    }
}

/// With the part's wiring read, a candidate whose pins differ names the pin
/// that would have no wire (mockup row 208, "pin 3 (GND) has no wire").
#[test]
fn the_compare_strip_names_the_pin_left_without_a_wire() {
    let (mut app, _id, _path, _dir) = app();
    open(&mut app, VariantEditKind::Alternate);
    let pin = |number: &str, name: &str| (number.to_string(), name.to_string());
    let mut three = alternate("NFM31PC276B0J3L");
    three.pins = Some(vec![pin("1", "~"), pin("2", "~"), pin("3", "GND")]);
    land(
        &mut app,
        Candidates {
            alternates: vec![three],
            wired: Some(vec!["1".to_string(), "2".to_string()]),
            ..Candidates::default()
        },
    );
    if let Some(super::VariantPartEdit::Alternate(picker)) =
        app.ui_state.variant_part_edit.as_deref_mut()
    {
        picker.part.base.pins = vec![pin("1", "~"), pin("2", "~")];
    }
    send(&mut app, VariantPartEditMsg::CandidatePicked(0));
    let shown = texts(&app);
    assert!(
        has(&shown, "3 pins vs 2: pin 3 (GND) has no wire"),
        "{shown:?}"
    );
}
