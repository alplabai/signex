use signex_widgets::passive_calculator::{
    BoundaryCondition, ComponentKind, Connection, ESeries, Network, PreferredComponent, Tolerance,
};

fn component(significand: u16, decade: i8, tolerance: Tolerance) -> Network {
    let number = ESeries::E24
        .preferred_numbers()
        .find(|number| number.significand == significand)
        .unwrap();
    Network::component(PreferredComponent { number, decade }, tolerance)
}

#[test]
fn resistance_and_uncoupled_inductance_use_the_same_connection_equations() {
    let network = Network::connected(
        Connection::Series,
        component(10, 1, Tolerance::Percent5),
        component(22, 1, Tolerance::Percent5),
    );
    assert!((network.nominal(ComponentKind::Resistor) - 32.0).abs() < 1e-12);
    assert!((network.nominal(ComponentKind::Inductor) - 32.0).abs() < 1e-12);

    let parallel = Network::connected(
        Connection::Parallel,
        component(10, 1, Tolerance::Percent5),
        component(10, 1, Tolerance::Percent5),
    );
    assert!((parallel.nominal(ComponentKind::Resistor) - 5.0).abs() < 1e-12);
}

#[test]
fn capacitance_inverts_series_and_parallel_equations() {
    let series = Network::connected(
        Connection::Series,
        component(10, -6, Tolerance::Percent5),
        component(10, -6, Tolerance::Percent5),
    );
    assert!((series.nominal(ComponentKind::Capacitor) - 0.5e-6).abs() < 1e-18);

    let parallel = Network::connected(
        Connection::Parallel,
        component(10, -6, Tolerance::Percent5),
        component(10, -6, Tolerance::Percent5),
    );
    assert!((parallel.nominal(ComponentKind::Capacitor) - 2.0e-6).abs() < 1e-18);
}

#[test]
fn mixed_tolerances_produce_exact_monotone_bounds() {
    let network = Network::connected(
        Connection::Series,
        component(10, 1, Tolerance::Percent10),
        component(22, 1, Tolerance::Percent5),
    );
    assert!((network.minimum(ComponentKind::Resistor) - 29.9).abs() < 1e-12);
    assert!((network.maximum(ComponentKind::Resistor) - 34.1).abs() < 1e-12);
}

#[test]
fn individual_leaf_tolerance_can_be_changed() {
    let mut network = Network::connected(
        Connection::Series,
        component(10, 1, Tolerance::Percent10),
        component(22, 1, Tolerance::Percent10),
    );
    assert!(network.set_tolerance(1, Tolerance::Percent1));
    assert_eq!(network.components()[1].1, Tolerance::Percent1);
    assert!(!network.set_tolerance(2, Tolerance::Percent1));
}

#[test]
fn expressions_use_unicode_subscripts_and_plain_text_fallback() {
    let mut network = component(10, 1, Tolerance::Percent5);
    for _ in 1..10 {
        network = Network::connected(
            Connection::Series,
            network,
            component(10, 1, Tolerance::Percent5),
        );
    }
    assert!(network.expression(ComponentKind::Resistor).contains("R₁₀"));
    assert!(
        network
            .plain_expression(ComponentKind::Resistor)
            .contains("R10")
    );
}

#[test]
fn boundary_networks_own_their_electrical_behavior() {
    let mut wire = Network::boundary(BoundaryCondition::WireBridge);
    assert_eq!(wire.nominal(ComponentKind::Resistor), 0.0);
    assert_eq!(wire.nominal(ComponentKind::Inductor), 0.0);
    assert_eq!(wire.nominal(ComponentKind::Capacitor), f64::INFINITY);
    assert_eq!(wire.expression(ComponentKind::Resistor), "Wire bridge");
    assert!(wire.components().is_empty());
    assert!(!wire.set_tolerance(0, Tolerance::Percent1));

    let open = Network::boundary(BoundaryCondition::OpenCircuit);
    assert_eq!(open.nominal(ComponentKind::Resistor), f64::INFINITY);
    assert_eq!(open.nominal(ComponentKind::Inductor), f64::INFINITY);
    assert_eq!(open.nominal(ComponentKind::Capacitor), 0.0);
    assert_eq!(open.expression(ComponentKind::Capacitor), "Open circuit");

    let parallel_open = Network::connected(Connection::Parallel, open.clone(), open);
    assert_eq!(
        parallel_open.nominal(ComponentKind::Resistor),
        f64::INFINITY
    );
    let parallel_wire = Network::connected(Connection::Parallel, wire.clone(), wire);
    assert_eq!(parallel_wire.nominal(ComponentKind::Resistor), 0.0);
}
