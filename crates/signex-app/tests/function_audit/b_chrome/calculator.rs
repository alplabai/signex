//! m17 Passive Network Calculator.

use signex_app::app::{Message, OverlayMsg, Signex};
use signex_app::menu_bar::MenuMessage;
use signex_widgets::passive_calculator::{
    CalculatorMessage as Calc, CalculatorTab, ComponentKind, ESeries, MAX_PARTS, OffTargetTone,
    RkmEncoderMessage, SiPrefix, TargetError, Tolerance, off_target,
};

use super::support::{app, desk, esc, is_dirty};
use crate::settle_support::settle;

fn calc(app: &mut Signex, msg: Calc) {
    let _ = app.update(Message::PassiveCalculator(msg));
}

fn target(app: &mut Signex, text: &str) {
    calc(app, Calc::TargetChanged(text.to_string()));
}

/// The mockup's case: 4.52 kΩ from E24 in at most two parts.
fn solved() -> Signex {
    let mut app = app();
    target(&mut app, "4.52");
    calc(&mut app, Calc::PrefixChanged(SiPrefix::Kilo));
    calc(&mut app, Calc::SeriesChanged(ESeries::E24));
    calc(&mut app, Calc::MaxComponentsChanged(2));
    calc(&mut app, Calc::Calculate);
    app
}

#[test]
fn m17_the_calculator_opens_from_tools_and_close_closes_it() {
    let mut app = app();
    settle(&mut app, Message::Menu(MenuMessage::OpenPassiveCalculator));
    assert!(app.ui_state.passive_calculator_open);
    let _ = app.update(Message::Overlay(OverlayMsg::ClosePassiveCalculator));
    assert!(!app.ui_state.passive_calculator_open);
}

#[test]
fn m17_esc_closes_the_calculator() {
    let mut app = app();
    settle(&mut app, Message::Menu(MenuMessage::OpenPassiveCalculator));
    esc(&mut app);
    assert!(!app.ui_state.passive_calculator_open);
}

#[test]
fn m17_calculate_finds_a_two_part_network_on_the_target() {
    let app = solved();
    let state = app.ui_state.passive_calculator.active_state();
    assert_eq!(state.validation_error, None);
    let network = state.result.as_ref().expect("a network");
    assert!(network.part_count() <= 2);
    let nominal = network.nominal(ComponentKind::Resistor);
    assert!((nominal - 4520.0).abs() < 0.5, "nominal {nominal}");
}

#[test]
fn m17_a_target_that_is_not_a_number_is_refused() {
    let mut app = solved();
    target(&mut app, "abc");
    calc(&mut app, Calc::Calculate);
    let state = app.ui_state.passive_calculator.active_state();
    assert_eq!(state.validation_error, Some(TargetError::NotANumber));
    assert!(state.result.is_none());
}

#[test]
fn m17_a_negative_target_is_refused() {
    let mut app = solved();
    target(&mut app, "-1");
    calc(&mut app, Calc::Calculate);
    let state = app.ui_state.passive_calculator.active_state();
    assert_eq!(state.validation_error, Some(TargetError::Negative));
    assert!(state.result.is_none());
}

#[test]
fn m17_changing_any_input_drops_the_result_it_no_longer_matches() {
    let edits = [
        Calc::TargetChanged("10".to_string()),
        Calc::PrefixChanged(SiPrefix::Mega),
        Calc::SeriesChanged(ESeries::E96),
        Calc::MaxComponentsChanged(1),
    ];
    for edit in edits {
        let mut app = solved();
        calc(&mut app, edit.clone());
        assert!(
            app.ui_state
                .passive_calculator
                .active_state()
                .result
                .is_none(),
            "{edit:?} left a stale result"
        );
    }
}

#[test]
fn m17_maximum_parts_stays_within_its_limits() {
    let mut app = app();
    calc(&mut app, Calc::MaxComponentsChanged(0));
    assert_eq!(
        app.ui_state
            .passive_calculator
            .active_state()
            .max_components,
        1
    );
    calc(&mut app, Calc::MaxComponentsChanged(MAX_PARTS + 5));
    assert_eq!(
        app.ui_state
            .passive_calculator
            .active_state()
            .max_components,
        MAX_PARTS
    );
}

#[test]
fn m17_a_tighter_part_tolerance_narrows_the_range_and_keeps_the_nominal() {
    let mut app = solved();
    let kind = ComponentKind::Resistor;
    let (nominal, minimum) = {
        let network = app
            .ui_state
            .passive_calculator
            .active_state()
            .result
            .as_ref()
            .expect("a network");
        (network.nominal(kind), network.minimum(kind))
    };

    calc(&mut app, Calc::ToleranceChanged(0, Tolerance::Percent0_1));

    let network = app
        .ui_state
        .passive_calculator
        .active_state()
        .result
        .as_ref()
        .expect("the result stays");
    assert_eq!(network.nominal(kind), nominal);
    assert!(network.minimum(kind) > minimum);
}

/// The 2026-11-19 decision: ok within the series' tolerance (E24 5 %), warn
/// past it, and no pill against a zero target.
#[test]
fn m17_the_off_target_pill_follows_the_series_tolerance() {
    let ok = off_target(104.0, Some(100.0), ESeries::E24);
    assert_eq!(ok.map(|(_, tone)| tone), Some(OffTargetTone::Ok));
    let warn = off_target(106.0, Some(100.0), ESeries::E24);
    assert_eq!(warn.map(|(_, tone)| tone), Some(OffTargetTone::Warn));
    assert!(off_target(1.0, Some(0.0), ESeries::E24).is_none());
    assert!(off_target(1.0, None, ESeries::E24).is_none());
}

#[test]
fn m17_each_component_tab_keeps_its_own_inputs() {
    let mut app = app();
    target(&mut app, "4.7");
    calc(
        &mut app,
        Calc::TabChanged(CalculatorTab::Passive(ComponentKind::Capacitor)),
    );
    target(&mut app, "100");

    let control = &app.ui_state.passive_calculator;
    assert_eq!(control.kind, ComponentKind::Capacitor);
    assert_eq!(control.state(ComponentKind::Resistor).target_input, "4.7");
    assert_eq!(control.state(ComponentKind::Capacitor).target_input, "100");
}

#[test]
fn m17_the_rkm_tab_keeps_the_component_tab_inputs() {
    let mut app = solved();
    calc(&mut app, Calc::TabChanged(CalculatorTab::RkmEncoder));
    assert_eq!(
        app.ui_state.passive_calculator.active_tab,
        CalculatorTab::RkmEncoder
    );
    let state = app.ui_state.passive_calculator.active_state();
    assert_eq!(state.target_input, "4.52");
    assert!(state.result.is_some());
}

/// The 2026-11-19 decision: the production date is off by default; off gives
/// no code and keeps the picks for when it is on again.
#[test]
fn m17_the_production_date_is_off_first_and_keeps_its_picks_when_turned_off() {
    let mut app = app();
    assert!(
        app.ui_state
            .passive_calculator
            .rkm_encoder
            .production_date_code()
            .is_none()
    );

    calc(
        &mut app,
        Calc::RkmEncoder(RkmEncoderMessage::ProductionDateToggled(true)),
    );
    let encoder = &app.ui_state.passive_calculator.rkm_encoder;
    assert!(encoder.production_date_code().is_some());
    let year = encoder.production_year;
    assert!(year.is_some(), "the first switch-on starts at this year");

    calc(
        &mut app,
        Calc::RkmEncoder(RkmEncoderMessage::ProductionDateToggled(false)),
    );
    let encoder = &app.ui_state.passive_calculator.rkm_encoder;
    assert!(encoder.production_date_code().is_none());
    assert_eq!(encoder.production_year, year);
}

#[test]
fn m17_the_optional_section_folds_and_unfolds() {
    let mut app = app();
    assert!(
        app.ui_state
            .passive_calculator
            .rkm_encoder
            .is_optional_expanded
    );
    calc(
        &mut app,
        Calc::RkmEncoder(RkmEncoderMessage::OptionalSectionToggled),
    );
    assert!(
        !app.ui_state
            .passive_calculator
            .rkm_encoder
            .is_optional_expanded
    );
}

#[test]
fn m17_using_the_calculator_does_not_touch_the_open_sheet() {
    let mut desk = desk();
    let path = desk.sheet("a.snxsch", "A4");
    target(&mut desk.app, "4.52");
    calc(&mut desk.app, Calc::Calculate);
    assert!(!is_dirty(&desk.app, &path));
}
