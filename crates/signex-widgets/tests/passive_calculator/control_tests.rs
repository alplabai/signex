use signex_widgets::passive_calculator::control::format_difference;
use signex_widgets::passive_calculator::{
    BoundaryCondition, CalculatorControl, CalculatorMessage, CalculatorTab, ComponentKind, ESeries,
    Network, ProductionDateCycle, ProductionMonth, RatedPower, RkmEncoderMessage, SiPrefix,
    TemperatureCoefficient, Tolerance,
};

#[test]
fn default_control_calculates_a_bounded_result() {
    let mut control = CalculatorControl::default();
    assert_eq!(control.active_state().target_input, "220");
    control.update(CalculatorMessage::Calculate);
    let state = control.active_state();
    assert!(state.validation_error.is_none());
    assert!(state.result.as_ref().unwrap().part_count() <= state.max_components);
    assert_eq!(
        control.active_tab,
        CalculatorTab::Passive(ComponentKind::Resistor)
    );
}

#[test]
fn decimal_comma_is_accepted() {
    let mut control = CalculatorControl::default();
    control.update(CalculatorMessage::TargetChanged("2,2".to_string()));
    control.update(CalculatorMessage::PrefixChanged(SiPrefix::Kilo));
    assert_eq!(control.target_value().unwrap(), 2_200.0);
}

#[test]
fn differences_use_compact_signed_percentages() {
    assert_eq!(format_difference(120.0, 100.0), "20 (+20%)");
    assert_eq!(format_difference(80.0, 100.0), "20 (-20%)");
    assert_eq!(format_difference(120.56, 100.0), "20.56 (+20.56%)");
    assert_eq!(format_difference(84.5, 100.0), "15.5 (-15.5%)");
    assert_eq!(format_difference(100.0, 100.0), "0 (0%)");
    assert_eq!(format_difference(20.0, 0.0), "unbounded");
}

#[test]
fn invalid_or_negative_input_is_rejected() {
    for input in ["", "abc", "-2", "NaN", "-inf"] {
        let mut control = CalculatorControl::default();
        control.update(CalculatorMessage::TargetChanged(input.to_string()));
        control.update(CalculatorMessage::Calculate);
        assert!(control.active_state().result.is_none(), "{input}");
        assert!(control.active_state().validation_error.is_some(), "{input}");
    }
}

#[test]
fn zero_and_infinity_inputs_produce_terminal_networks() {
    for (input, expected) in [
        ("0", BoundaryCondition::WireBridge),
        ("∞", BoundaryCondition::OpenCircuit),
        ("infinity", BoundaryCondition::OpenCircuit),
    ] {
        let mut control = CalculatorControl::default();
        control.update(CalculatorMessage::TargetChanged(input.to_string()));
        control.update(CalculatorMessage::Calculate);
        assert!(control.active_state().validation_error.is_none(), "{input}");
        assert!(matches!(
            control.active_state().result.as_ref(),
            Some(Network::Boundary { condition }) if *condition == expected
        ));
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
fn changing_a_result_tolerance_updates_the_selected_leaf() {
    let mut control = CalculatorControl::default();
    control.active_state_mut().series = ESeries::E24;
    control.active_state_mut().max_components = 3;
    control.update(CalculatorMessage::Calculate);
    control.update(CalculatorMessage::ToleranceChanged(0, Tolerance::Percent1));
    let after = control.active_state().result.as_ref().unwrap().components();
    assert_eq!(after[0].1, Tolerance::Percent1);
}

#[test]
fn changing_a_result_tolerance_leaves_other_leaves_unchanged() {
    let mut control = CalculatorControl::default();
    control.update(CalculatorMessage::TargetChanged("235".to_string()));
    control.active_state_mut().series = ESeries::E24;
    control.active_state_mut().max_components = 3;
    control.update(CalculatorMessage::Calculate);
    let before = control.active_state().result.as_ref().unwrap().components();
    assert_eq!(before.len(), 2, "the fixture must produce two components");
    let mut expected = before.clone();
    expected[0].1 = Tolerance::Percent1;

    control.update(CalculatorMessage::ToleranceChanged(0, Tolerance::Percent1));

    let after = control.active_state().result.as_ref().unwrap().components();
    assert_eq!(after, expected);
}

#[test]
fn complete_rkm_properties_are_isolated_in_the_encoder_tab() {
    let mut control = CalculatorControl::default();
    control.update(CalculatorMessage::Calculate);
    let passive_result = control.active_state().result.clone();

    control.update(CalculatorMessage::TabChanged(CalculatorTab::RkmEncoder));
    assert!(control.rkm_encoder.temperature_coefficient.is_none());
    assert!(control.rkm_encoder.rated_power.is_none());
    control.update(CalculatorMessage::RkmEncoder(
        RkmEncoderMessage::TemperatureCoefficientChanged(TemperatureCoefficient::Ppm5),
    ));
    control.update(CalculatorMessage::RkmEncoder(
        RkmEncoderMessage::RatedPowerChanged(RatedPower::Watts0_63),
    ));
    control.update(CalculatorMessage::RkmEncoder(
        RkmEncoderMessage::ProductionDateCycleChanged(ProductionDateCycle::TwentyYear),
    ));
    control.update(CalculatorMessage::RkmEncoder(
        RkmEncoderMessage::ProductionYearChanged(2026),
    ));
    control.update(CalculatorMessage::RkmEncoder(
        RkmEncoderMessage::ProductionMonthChanged(ProductionMonth::July),
    ));
    assert_eq!(control.rkm_encoder.code().to_string(), "4K70 FM 0W63");
    assert_eq!(
        control
            .rkm_encoder
            .production_date_code()
            .unwrap()
            .to_string(),
        "U7"
    );

    control.update(CalculatorMessage::KindChanged(ComponentKind::Resistor));
    assert_eq!(
        control.active_tab,
        CalculatorTab::Passive(ComponentKind::Resistor)
    );
    assert_eq!(control.active_state().result, passive_result);
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
