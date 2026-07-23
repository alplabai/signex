use signex_widgets::passive_calculator::{ComponentKind, ESeries, SolveOptions, Tolerance, solve};

fn options(kind: ComponentKind, target: f64, max_parts: usize) -> SolveOptions {
    SolveOptions {
        kind,
        target,
        series: ESeries::E24,
        max_parts,
        default_tolerance: Tolerance::Percent5,
        result_limit: 5,
    }
}

#[test]
fn exact_single_component_match_is_ranked_first() {
    let results = solve(options(ComponentKind::Resistor, 220.0, 4));
    assert!(!results.is_empty());
    assert!((results[0].nominal(ComponentKind::Resistor) - 220.0).abs() < 1e-12);
    assert_eq!(results[0].part_count(), 1);
}

#[test]
fn result_never_exceeds_requested_component_count() {
    for max_parts in 1..=4 {
        let results = solve(options(ComponentKind::Resistor, 235.75, max_parts));
        assert!(
            results
                .iter()
                .all(|result| result.part_count() <= max_parts)
        );
    }
}

#[test]
fn adding_parts_can_improve_the_approximation() {
    let one = solve(options(ComponentKind::Resistor, 235.75, 1));
    let three = solve(options(ComponentKind::Resistor, 235.75, 3));
    let one_error = (one[0].nominal(ComponentKind::Resistor) - 235.75).abs();
    let three_error = (three[0].nominal(ComponentKind::Resistor) - 235.75).abs();
    assert!(three_error <= one_error);
}

#[test]
fn solver_is_deterministic() {
    let first = solve(options(ComponentKind::Resistor, 235.75, 3));
    let second = solve(options(ComponentKind::Resistor, 235.75, 3));
    let first = first
        .iter()
        .map(|network| network.plain_expression(ComponentKind::Resistor))
        .collect::<Vec<_>>();
    let second = second
        .iter()
        .map(|network| network.plain_expression(ComponentKind::Resistor))
        .collect::<Vec<_>>();
    assert_eq!(first, second);
}

#[test]
fn capacitor_solver_uses_capacitor_connection_semantics() {
    let results = solve(options(ComponentKind::Capacitor, 25e-6, 2));
    assert!((results[0].nominal(ComponentKind::Capacitor) - 25e-6).abs() < 1e-18);
    assert_eq!(results[0].part_count(), 2);
    assert!(
        results[0]
            .expression(ComponentKind::Capacitor)
            .contains('∥')
    );
}

#[test]
fn invalid_targets_return_no_results() {
    for target in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(solve(options(ComponentKind::Resistor, target, 3)).is_empty());
    }
}

#[test]
fn every_preferred_series_can_seed_a_solution() {
    for series in ESeries::ALL {
        let results = solve(SolveOptions {
            series,
            ..options(ComponentKind::Resistor, 123.45, 1)
        });
        assert!(!results.is_empty(), "{series}");
        assert_eq!(results[0].part_count(), 1, "{series}");
    }
}
