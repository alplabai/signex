use super::*;

/// Verifies that component picker matches reference order and labels.
#[test]
fn component_picker_matches_reference_order_and_labels() {
    let labels = CircuitComponentKind::PICKER_OPTIONS
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();

    assert_eq!(
        labels,
        vec![
            "Shorted Capacitor",
            "Series Capacitor",
            "Shorted Inductor",
            "Series Inductor",
            "Shorted Resistor",
            "Series Resistor",
            "Parallel RLC",
            "Custom Z(f)",
            "Transmission Line (Z_0)",
            "Stub",
            "Shorted Stub",
            "Transformer (L1 L2)",
            "S-Parameters",
        ]
    );
}

/// Verifies that black box is fixed at dp0.
#[test]
fn black_box_is_fixed_at_dp0() {
    let mut state = SmithChartState::default();

    state.update(SmithChartMessage::RemoveCircuitComponent(0));
    state.update(SmithChartMessage::MoveCircuitComponentRight(0));

    assert_eq!(state.circuit_components.len(), 1);
    assert_eq!(
        state.circuit_components[0].kind,
        CircuitComponentKind::BlackBox
    );
}

/// Verifies that components can be duplicated reordered and deleted independently.
#[test]
fn components_can_be_duplicated_reordered_and_deleted_independently() {
    let mut state = SmithChartState::default();
    state.update(SmithChartMessage::AddCircuitComponent(
        CircuitComponentKind::SeriesResistor,
    ));
    state.update(SmithChartMessage::AddCircuitComponent(
        CircuitComponentKind::SeriesCapacitor,
    ));
    state.update(SmithChartMessage::AddCircuitComponent(
        CircuitComponentKind::SeriesResistor,
    ));
    state.update(SmithChartMessage::CircuitComponentFieldChanged {
        index: 1,
        field: CircuitComponentField::Primary,
        value: "10".to_string(),
    });
    state.update(SmithChartMessage::CircuitComponentFieldChanged {
        index: 3,
        field: CircuitComponentField::Primary,
        value: "20".to_string(),
    });

    state.update(SmithChartMessage::MoveCircuitComponentLeft(3));
    state.update(SmithChartMessage::RemoveCircuitComponent(3));

    assert_eq!(state.circuit_components.len(), 3);
    assert_eq!(
        state
            .circuit_components
            .iter()
            .map(|component| component.kind)
            .collect::<Vec<_>>(),
        vec![
            CircuitComponentKind::BlackBox,
            CircuitComponentKind::SeriesResistor,
            CircuitComponentKind::SeriesResistor,
        ]
    );
    assert_eq!(state.circuit_components[1].primary, "10");
    assert_eq!(state.circuit_components[2].primary, "20");
}

/// Verifies that component value edits recalculate impedance and reflection.
#[test]
fn component_value_edits_recalculate_impedance_and_reflection() {
    let mut state = SmithChartState::default();
    let initial = state.solve().unwrap().nominal;

    state.update(SmithChartMessage::AddCircuitComponent(
        CircuitComponentKind::SeriesResistor,
    ));
    state.update(SmithChartMessage::CircuitComponentFieldChanged {
        index: 1,
        field: CircuitComponentField::Primary,
        value: "25".to_string(),
    });

    let changed = state.solve().unwrap().nominal;
    assert_close(changed.impedance.re, 75.0);
    assert_close(changed.impedance.im, 0.0);
    assert_ne!(
        initial.reflection_coefficient,
        changed.reflection_coefficient
    );
}

/// Verifies that every picker component builds a solver element.
#[test]
fn every_picker_component_builds_a_solver_element() {
    for kind in CircuitComponentKind::PICKER_OPTIONS {
        let component = CircuitEditorComponent::new(kind);
        assert!(
            component.to_element().is_ok(),
            "default {kind} component should be valid"
        );
    }
}

/// Asserts that close.
fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1.0e-9,
        "expected {actual} to be close to {expected}"
    );
}
