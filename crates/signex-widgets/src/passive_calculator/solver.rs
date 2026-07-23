use std::cmp::Ordering;
use std::collections::HashSet;

use super::domain::{ComponentKind, ESeries, PreferredComponent, Tolerance};
use super::network::{Connection, Network};

pub const MAX_PARTS: usize = 4;
const FRONTIER_LIMIT: usize = 400;

#[derive(Debug, Clone, Copy)]
pub struct SolveOptions {
    pub kind: ComponentKind,
    pub target: f64,
    pub series: ESeries,
    pub max_parts: usize,
    pub default_tolerance: Tolerance,
    pub result_limit: usize,
}

impl Default for SolveOptions {
    fn default() -> Self {
        Self {
            kind: ComponentKind::Resistor,
            target: 1_000.0,
            series: ESeries::E24,
            max_parts: 3,
            default_tolerance: Tolerance::Percent5,
            result_limit: 10,
        }
    }
}

pub fn solve(options: SolveOptions) -> Vec<Network> {
    if !options.target.is_finite() || options.target <= 0.0 || options.result_limit == 0 {
        return Vec::new();
    }

    let max_parts = options.max_parts.clamp(1, MAX_PARTS);
    let leaves = generate_leaves(options);
    if leaves.is_empty() {
        return Vec::new();
    }

    let mut levels = vec![Vec::<Network>::new(); max_parts + 1];
    levels[1] = leaves;

    for part_count in 2..=max_parts {
        let mut candidates = Vec::new();
        for left_count in 1..=part_count / 2 {
            let right_count = part_count - left_count;
            for (left_index, left) in levels[left_count].iter().enumerate() {
                let right_start = if left_count == right_count {
                    left_index
                } else {
                    0
                };
                for right in levels[right_count].iter().skip(right_start) {
                    candidates.push(Network::connected(
                        Connection::Series,
                        left.clone(),
                        right.clone(),
                    ));
                    candidates.push(Network::connected(
                        Connection::Parallel,
                        left.clone(),
                        right.clone(),
                    ));
                }
            }
        }
        levels[part_count] = retain_frontier(candidates, options.kind, options.target);
    }

    let mut results = levels.into_iter().skip(1).flatten().collect::<Vec<_>>();
    sort_networks(&mut results, options.kind, options.target);
    let mut expressions = HashSet::new();
    results.retain(|network| expressions.insert(network.plain_expression(options.kind)));
    results.truncate(options.result_limit);
    results
}

fn generate_leaves(options: SolveOptions) -> Vec<Network> {
    let target_decade = options.target.log10().floor() as i8;
    let mut leaves = Vec::new();
    for decade in (target_decade - 1)..=(target_decade + 1) {
        for number in options.series.preferred_numbers() {
            let component = PreferredComponent { number, decade };
            let value = component.value();
            if value.is_finite() && value > 0.0 {
                leaves.push(Network::component(component, options.default_tolerance));
            }
        }
    }
    sort_networks(&mut leaves, options.kind, options.target);
    leaves
}

fn retain_frontier(candidates: Vec<Network>, kind: ComponentKind, target: f64) -> Vec<Network> {
    let mut seen_values = HashSet::new();
    let mut candidates = candidates;
    sort_networks(&mut candidates, kind, target);
    candidates.retain(|network| {
        let value = network.nominal(kind);
        let quantized = format!("{value:.12e}");
        seen_values.insert(quantized)
    });
    candidates.truncate(FRONTIER_LIMIT);
    candidates
}

fn sort_networks(networks: &mut [Network], kind: ComponentKind, target: f64) {
    networks.sort_by(|left, right| compare_networks(left, right, kind, target));
}

fn compare_networks(left: &Network, right: &Network, kind: ComponentKind, target: f64) -> Ordering {
    let left_error = relative_error(left.nominal(kind), target);
    let right_error = relative_error(right.nominal(kind), target);
    let error_order = if (left_error - right_error).abs() <= 1e-12 {
        Ordering::Equal
    } else {
        left_error.total_cmp(&right_error)
    };
    error_order
        .then_with(|| left.part_count().cmp(&right.part_count()))
        .then_with(|| {
            let left_span = left.maximum(kind) - left.minimum(kind);
            let right_span = right.maximum(kind) - right.minimum(kind);
            left_span.total_cmp(&right_span)
        })
}

fn relative_error(value: f64, target: f64) -> f64 {
    ((value - target) / target).abs()
}
