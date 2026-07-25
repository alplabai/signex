use signex_widgets::passive_calculator::{
    BoundaryCondition, ComponentKind, ESeries, Network, PreferredComponent, SolveOptions,
    Tolerance, solve,
};

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
    for target in [-1.0, f64::NAN, f64::NEG_INFINITY] {
        assert!(solve(options(ComponentKind::Resistor, target, 3)).is_empty());
    }
}

#[test]
fn boundary_targets_return_exact_structured_networks() {
    for (kind, target, expected) in [
        (ComponentKind::Resistor, 0.0, BoundaryCondition::WireBridge),
        (ComponentKind::Inductor, 0.0, BoundaryCondition::WireBridge),
        (
            ComponentKind::Capacitor,
            0.0,
            BoundaryCondition::OpenCircuit,
        ),
        (
            ComponentKind::Resistor,
            f64::INFINITY,
            BoundaryCondition::OpenCircuit,
        ),
        (
            ComponentKind::Inductor,
            f64::INFINITY,
            BoundaryCondition::OpenCircuit,
        ),
        (
            ComponentKind::Capacitor,
            f64::INFINITY,
            BoundaryCondition::WireBridge,
        ),
    ] {
        let results = solve(options(kind, target, 4));
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].nominal(kind), target);
        assert_eq!(results[0].part_count(), 0);
        assert!(matches!(
            results[0],
            Network::Boundary { condition } if condition == expected
        ));
    }
}

#[test]
fn unreachable_target_still_returns_a_closest_network() {
    let results = solve(SolveOptions {
        result_limit: 1,
        series: ESeries::E3,
        ..options(ComponentKind::Resistor, 137.0, 1)
    });
    assert_eq!(results.len(), 1);
    assert!((results[0].nominal(ComponentKind::Resistor) - 137.0).abs() > 1e-12);
}

#[test]
fn extreme_finite_targets_always_return_a_result() {
    for (kind, expected) in [
        (ComponentKind::Resistor, BoundaryCondition::WireBridge),
        (ComponentKind::Capacitor, BoundaryCondition::OpenCircuit),
        (ComponentKind::Inductor, BoundaryCondition::WireBridge),
    ] {
        let tiny = solve(SolveOptions {
            kind,
            target: f64::MIN_POSITIVE,
            series: ESeries::E3,
            max_parts: 4,
            default_tolerance: Tolerance::Percent20,
            result_limit: 1,
        });
        assert!(matches!(
            tiny.as_slice(),
            [Network::Boundary { condition }] if *condition == expected
        ));

        let huge = solve(SolveOptions {
            kind,
            target: f64::MAX,
            series: ESeries::E3,
            max_parts: 4,
            default_tolerance: Tolerance::Percent20,
            result_limit: 1,
        });
        assert_eq!(huge.len(), 1);
        assert!(huge[0].nominal(kind).is_finite());
    }
}

#[test]
fn optimized_search_matches_an_exhaustive_small_series_oracle() {
    for (kind, target) in [
        (ComponentKind::Resistor, 137.0),
        (ComponentKind::Capacitor, 13.7e-6),
        (ComponentKind::Inductor, 1.37e-3),
    ] {
        for max_parts in 1..=4 {
            assert_matches_exhaustive_oracle(ESeries::E3, kind, target, max_parts);
        }
        assert_matches_exhaustive_oracle(ESeries::E6, kind, target, 4);
    }
}

#[test]
fn largest_preferred_series_finds_a_four_part_closest_result() {
    let results = solve(SolveOptions {
        series: ESeries::E192,
        max_parts: 4,
        result_limit: 1,
        ..options(ComponentKind::Resistor, 123.456, 4)
    });
    assert_eq!(results.len(), 1);
    assert!(results[0].nominal(ComponentKind::Resistor).is_finite());
    assert!(results[0].part_count() <= 4);
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

fn assert_matches_exhaustive_oracle(
    series: ESeries,
    kind: ComponentKind,
    target: f64,
    max_parts: usize,
) {
    let result = solve(SolveOptions {
        kind,
        target,
        series,
        max_parts,
        default_tolerance: Tolerance::Percent20,
        result_limit: 1,
    })
    .into_iter()
    .next()
    .unwrap();
    let expected_error = exhaustive_best_error(series, kind, target, max_parts);
    let actual_error = relative_error(result.nominal(kind), target);
    assert!(
        (actual_error - expected_error).abs() <= 1e-12,
        "{series} {kind} with {max_parts} parts: expected {expected_error:e}, got {actual_error:e}"
    );
}

fn exhaustive_best_error(
    series: ESeries,
    kind: ComponentKind,
    target: f64,
    max_parts: usize,
) -> f64 {
    let target_decade = target.log10().floor() as i8;
    let mut levels = vec![Vec::<f64>::new(); max_parts + 1];
    for decade in (target_decade - 1)..=(target_decade + 1) {
        for number in series.preferred_numbers() {
            levels[1].push(PreferredComponent { number, decade }.value());
        }
    }

    for part_count in 2..=max_parts {
        let mut candidates = Vec::new();
        for left_count in 1..=part_count / 2 {
            let right_count = part_count - left_count;
            for (left_index, left) in levels[left_count].iter().copied().enumerate() {
                let right_start = if left_count == right_count {
                    left_index
                } else {
                    0
                };
                for right in levels[right_count].iter().copied().skip(right_start) {
                    candidates.push(additive_value(kind, left, right));
                    candidates.push(harmonic_value(kind, left, right));
                }
            }
        }
        levels[part_count] = candidates;
    }

    levels
        .into_iter()
        .skip(1)
        .flatten()
        .map(|value| relative_error(value, target))
        .min_by(f64::total_cmp)
        .unwrap()
}

fn additive_value(kind: ComponentKind, left: f64, right: f64) -> f64 {
    match kind {
        ComponentKind::Resistor | ComponentKind::Inductor => left + right,
        ComponentKind::Capacitor => parallel(left, right),
    }
}

fn harmonic_value(kind: ComponentKind, left: f64, right: f64) -> f64 {
    match kind {
        ComponentKind::Resistor | ComponentKind::Inductor => parallel(left, right),
        ComponentKind::Capacitor => left + right,
    }
}

fn parallel(left: f64, right: f64) -> f64 {
    let (smaller, larger) = if left <= right {
        (left, right)
    } else {
        (right, left)
    };
    smaller / (1.0 + smaller / larger)
}

fn relative_error(value: f64, target: f64) -> f64 {
    ((value - target) / target).abs()
}
