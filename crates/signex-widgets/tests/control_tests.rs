use signex_widgets::passive_calculator::{
    CalculatorControl, CalculatorMessage, ComponentKind, ESeries, SiPrefix, Tolerance,
};

#[test]
fn default_control_calculates_a_bounded_result() {
    let mut control = CalculatorControl::default();
    assert_eq!(control.active_state().target_input, "220");
    control.update(CalculatorMessage::Calculate);
    let state = control.active_state();
    assert!(state.validation_error.is_none());
    assert!(state.result.as_ref().unwrap().part_count() <= state.max_components);
}

#[test]
fn decimal_comma_is_accepted() {
    let mut control = CalculatorControl::default();
    control.update(CalculatorMessage::TargetChanged("2,2".to_string()));
    control.update(CalculatorMessage::PrefixChanged(SiPrefix::Kilo));
    assert_eq!(control.target_value().unwrap(), 2_200.0);
}

#[test]
fn invalid_or_non_positive_input_is_rejected() {
    for input in ["", "abc", "0", "-2", "NaN"] {
        let mut control = CalculatorControl::default();
        control.update(CalculatorMessage::TargetChanged(input.to_string()));
        control.update(CalculatorMessage::Calculate);
        assert!(control.active_state().result.is_none(), "{input}");
        assert!(control.active_state().validation_error.is_some(), "{input}");
    }
}

#[test]
fn changing_kind_selects_a_practical_default_prefix() {
    let mut control = CalculatorControl::default();
    control.update(CalculatorMessage::KindChanged(ComponentKind::Capacitor));
    assert_eq!(control.active_state().prefix, SiPrefix::Micro);
    control.update(CalculatorMessage::KindChanged(ComponentKind::Inductor));
    assert_eq!(control.active_state().prefix, SiPrefix::Milli);
}

#[test]
fn changing_a_result_tolerance_updates_only_that_leaf() {
    let mut control = CalculatorControl::default();
    control.active_state_mut().series = ESeries::E24;
    control.active_state_mut().max_components = 3;
    control.update(CalculatorMessage::Calculate);
    let before = control.active_state().result.as_ref().unwrap().components();
    control.update(CalculatorMessage::ToleranceChanged(0, Tolerance::Percent1));
    let after = control.active_state().result.as_ref().unwrap().components();
    assert_eq!(after[0].1, Tolerance::Percent1);
    assert_eq!(before[1..], after[1..]);
}

#[test]
fn every_tab_preserves_its_own_complete_state() {
    let mut control = CalculatorControl::default();
    control.update(CalculatorMessage::TargetChanged("123".to_string()));
    control.update(CalculatorMessage::SeriesChanged(ESeries::E12));
    control.update(CalculatorMessage::MaxComponentsChanged(2));
    control.update(CalculatorMessage::Calculate);

    control.update(CalculatorMessage::KindChanged(ComponentKind::Capacitor));
    control.update(CalculatorMessage::TargetChanged("456".to_string()));
    control.update(CalculatorMessage::SeriesChanged(ESeries::E96));
    control.update(CalculatorMessage::MaxComponentsChanged(4));
    control.update(CalculatorMessage::Calculate);

    let capacitor = control.state(ComponentKind::Capacitor);
    assert_eq!(capacitor.target_input, "456");
    assert_eq!(capacitor.series, ESeries::E96);
    assert_eq!(capacitor.max_components, 4);
    assert!(capacitor.result.is_some());

    control.update(CalculatorMessage::KindChanged(ComponentKind::Resistor));
    let resistor = control.active_state();
    assert_eq!(resistor.target_input, "123");
    assert_eq!(resistor.series, ESeries::E12);
    assert_eq!(resistor.max_components, 2);
    assert!(resistor.result.is_some());
}
