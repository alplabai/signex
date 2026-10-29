use std::cmp::Ordering;
use std::collections::HashSet;

use super::domain::{ComponentKind, ESeries, PreferredComponent, Tolerance};
use super::network::{BoundaryCondition, Connection, Network, is_additive, parallel_value};

pub const MAX_PARTS: usize = 4;
const FRONTIER_LIMIT: usize = 400;
const ERROR_EQUIVALENCE: f64 = 1e-12;

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

#[derive(Debug, Clone, Copy)]
struct LevelTwoCandidate {
    left_index: usize,
    right_index: usize,
    connection: Connection,
    nominal: f64,
}

#[derive(Debug, Clone, Copy)]
struct LevelThreeCandidate {
    leaf_index: usize,
    level_two_index: usize,
    connection: Connection,
    nominal: f64,
}

#[derive(Debug, Clone, Copy)]
struct BalancedLevelFourCandidate {
    left_index: usize,
    right_index: usize,
    connection: Connection,
}

#[derive(Debug, Clone, Copy)]
struct UnbalancedLevelFourCandidate {
    leaf_index: usize,
    level_three: LevelThreeCandidate,
    connection: Connection,
}

#[derive(Debug, Clone, Copy)]
struct ValueBrackets<T> {
    below: Option<(f64, T)>,
    above: Option<(f64, T)>,
}

impl<T: Copy> ValueBrackets<T> {
    fn new() -> Self {
        Self {
            below: None,
            above: None,
        }
    }

    fn consider(&mut self, nominal: f64, target: f64, candidate: T) {
        if nominal <= target {
            if self.below.is_none_or(|(current, _)| nominal > current) {
                self.below = Some((nominal, candidate));
            }
        } else if self.above.is_none_or(|(current, _)| nominal < current) {
            self.above = Some((nominal, candidate));
        }
    }

    fn candidates(self) -> impl Iterator<Item = T> {
        self.below
            .into_iter()
            .chain(self.above)
            .map(|(_, candidate)| candidate)
    }
}

pub fn solve(options: SolveOptions) -> Vec<Network> {
    if options.target.is_nan()
        || options.target < 0.0
        || options.target == f64::NEG_INFINITY
        || options.result_limit == 0
    {
        return Vec::new();
    }

    if let Some(condition) = BoundaryCondition::exact_for_target(options.kind, options.target) {
        return vec![Network::boundary(condition)];
    }

    let max_parts = options.max_parts.clamp(1, MAX_PARTS);
    let leaves = generate_leaves(options);
    if leaves.is_empty() {
        return Vec::new();
    }

    let closest = find_closest_network(&leaves, options.kind, options.target, max_parts);
    if options.result_limit == 1 {
        return vec![closest];
    }

    let mut levels = vec![Vec::<Network>::new(); max_parts + 1];
    levels[1] = leaves;

    for part_count in 2..=max_parts {
        let mut candidates = Vec::new();
        for left_count in 1..=part_count / 2 {
            let right_count = part_count - left_count;
            candidates.extend(generate_target_brackets(
                &levels[left_count],
                &levels[right_count],
                left_count == right_count,
                options.kind,
                options.target,
            ));
        }
        levels[part_count] = retain_frontier(candidates, options.kind, options.target);
    }

    let mut results = levels.into_iter().skip(1).flatten().collect::<Vec<_>>();
    results.push(closest);
    sort_networks(&mut results, options.kind, options.target);
    let mut expressions = HashSet::new();
    results.retain(|network| expressions.insert(network.plain_expression(options.kind)));
    results.truncate(options.result_limit);
    results
}

fn find_closest_network(
    leaves: &[Network],
    kind: ComponentKind,
    target: f64,
    max_parts: usize,
) -> Network {
    let minimum_leaf = leaves
        .iter()
        .map(|leaf| leaf.nominal(kind))
        .min_by(f64::total_cmp)
        .expect("the caller guarantees at least one leaf");
    let minimum_network = minimum_leaf / max_parts as f64;
    if target <= minimum_network / 2.0 {
        let condition = BoundaryCondition::exact_for_target(kind, 0.0)
            .expect("every supported component kind has a zero-valued boundary");
        return Network::boundary(condition);
    }

    let mut candidates = leaves.to_vec();
    sort_networks(&mut candidates, kind, target);
    if max_parts == 1 || is_effectively_exact(&candidates[0], kind, target) {
        return candidates.remove(0);
    }

    if max_parts == 2 {
        candidates.extend(generate_target_brackets(leaves, leaves, true, kind, target));
        sort_networks(&mut candidates, kind, target);
        return candidates.remove(0);
    }

    // Three- and four-part trees can always be split around a two-part subtree.
    // Keep that level compact and materialize only the bracketing trees selected below.
    let level_two = generate_level_two_candidates(leaves, kind);
    let mut level_two_brackets = ValueBrackets::new();
    let insertion = lower_bound_level_two(&level_two, 0, target);
    for index in neighboring_indices(insertion, 0, level_two.len()) {
        level_two_brackets.consider(level_two[index].nominal, target, index);
    }
    candidates.extend(
        level_two_brackets
            .candidates()
            .map(|index| materialize_level_two(leaves, level_two[index])),
    );
    sort_networks(&mut candidates, kind, target);
    if is_effectively_exact(&candidates[0], kind, target) {
        return candidates.remove(0);
    }

    candidates.extend(
        find_level_three_brackets(leaves, &level_two, kind, target)
            .candidates()
            .map(|candidate| materialize_level_three(leaves, &level_two, candidate)),
    );
    sort_networks(&mut candidates, kind, target);
    if max_parts == 3 || is_effectively_exact(&candidates[0], kind, target) {
        return candidates.remove(0);
    }

    candidates.extend(
        find_balanced_level_four_brackets(&level_two, kind, target)
            .candidates()
            .map(|candidate| {
                Network::connected(
                    candidate.connection,
                    materialize_level_two(leaves, level_two[candidate.left_index]),
                    materialize_level_two(leaves, level_two[candidate.right_index]),
                )
            }),
    );
    candidates.extend(
        find_unbalanced_level_four_brackets(leaves, &level_two, kind, target)
            .candidates()
            .map(|candidate| {
                Network::connected(
                    candidate.connection,
                    leaves[candidate.leaf_index].clone(),
                    materialize_level_three(leaves, &level_two, candidate.level_three),
                )
            }),
    );

    sort_networks(&mut candidates, kind, target);
    candidates.remove(0)
}

fn generate_level_two_candidates(
    leaves: &[Network],
    kind: ComponentKind,
) -> Vec<LevelTwoCandidate> {
    let mut candidates = Vec::with_capacity(leaves.len() * (leaves.len() + 1));
    for (left_index, left) in leaves.iter().enumerate() {
        for (right_index, right) in leaves.iter().enumerate().skip(left_index) {
            for connection in [Connection::Series, Connection::Parallel] {
                candidates.push(LevelTwoCandidate {
                    left_index,
                    right_index,
                    connection,
                    nominal: combine_values(
                        kind,
                        connection,
                        left.nominal(kind),
                        right.nominal(kind),
                    ),
                });
            }
        }
    }
    candidates.sort_by(|left, right| left.nominal.total_cmp(&right.nominal));
    candidates
}

fn find_level_three_brackets(
    leaves: &[Network],
    level_two: &[LevelTwoCandidate],
    kind: ComponentKind,
    target: f64,
) -> ValueBrackets<LevelThreeCandidate> {
    let mut brackets = ValueBrackets::new();
    for (leaf_index, leaf) in leaves.iter().enumerate() {
        let leaf_value = leaf.nominal(kind);
        for connection in [Connection::Series, Connection::Parallel] {
            let desired = desired_right_value(kind, connection, leaf_value, target);
            let insertion = lower_bound_level_two(level_two, 0, desired);
            for level_two_index in neighboring_indices(insertion, 0, level_two.len()) {
                let nominal = combine_values(
                    kind,
                    connection,
                    leaf_value,
                    level_two[level_two_index].nominal,
                );
                brackets.consider(
                    nominal,
                    target,
                    LevelThreeCandidate {
                        leaf_index,
                        level_two_index,
                        connection,
                        nominal,
                    },
                );
            }
        }
    }
    brackets
}

fn find_balanced_level_four_brackets(
    level_two: &[LevelTwoCandidate],
    kind: ComponentKind,
    target: f64,
) -> ValueBrackets<BalancedLevelFourCandidate> {
    let mut brackets = ValueBrackets::new();
    for (left_index, left) in level_two.iter().enumerate() {
        for connection in [Connection::Series, Connection::Parallel] {
            let desired = desired_right_value(kind, connection, left.nominal, target);
            let insertion = lower_bound_level_two(level_two, left_index, desired);
            for right_index in neighboring_indices(insertion, left_index, level_two.len()) {
                let nominal = combine_values(
                    kind,
                    connection,
                    left.nominal,
                    level_two[right_index].nominal,
                );
                brackets.consider(
                    nominal,
                    target,
                    BalancedLevelFourCandidate {
                        left_index,
                        right_index,
                        connection,
                    },
                );
            }
        }
    }
    brackets
}

fn find_unbalanced_level_four_brackets(
    leaves: &[Network],
    level_two: &[LevelTwoCandidate],
    kind: ComponentKind,
    target: f64,
) -> ValueBrackets<UnbalancedLevelFourCandidate> {
    let mut brackets = ValueBrackets::new();
    for (leaf_index, leaf) in leaves.iter().enumerate() {
        let leaf_value = leaf.nominal(kind);
        for connection in [Connection::Series, Connection::Parallel] {
            let desired = desired_right_value(kind, connection, leaf_value, target);
            for level_three in
                find_level_three_brackets(leaves, level_two, kind, desired).candidates()
            {
                let nominal = combine_values(kind, connection, leaf_value, level_three.nominal);
                brackets.consider(
                    nominal,
                    target,
                    UnbalancedLevelFourCandidate {
                        leaf_index,
                        level_three,
                        connection,
                    },
                );
            }
        }
    }
    brackets
}

fn materialize_level_two(leaves: &[Network], candidate: LevelTwoCandidate) -> Network {
    Network::connected(
        candidate.connection,
        leaves[candidate.left_index].clone(),
        leaves[candidate.right_index].clone(),
    )
}

fn materialize_level_three(
    leaves: &[Network],
    level_two: &[LevelTwoCandidate],
    candidate: LevelThreeCandidate,
) -> Network {
    Network::connected(
        candidate.connection,
        leaves[candidate.leaf_index].clone(),
        materialize_level_two(leaves, level_two[candidate.level_two_index]),
    )
}

fn lower_bound_level_two(candidates: &[LevelTwoCandidate], start: usize, target: f64) -> usize {
    let mut low = start;
    let mut high = candidates.len();
    while low < high {
        let middle = low + (high - low) / 2;
        if candidates[middle].nominal < target {
            low = middle + 1;
        } else {
            high = middle;
        }
    }
    low
}

fn neighboring_indices(
    insertion: usize,
    start: usize,
    length: usize,
) -> impl Iterator<Item = usize> {
    let below = insertion.checked_sub(1).filter(|index| *index >= start);
    let above = (insertion < length).then_some(insertion);
    below.into_iter().chain(above)
}

fn generate_target_brackets(
    left_networks: &[Network],
    right_networks: &[Network],
    same_networks: bool,
    kind: ComponentKind,
    target: f64,
) -> Vec<Network> {
    let mut left_indices = (0..left_networks.len()).collect::<Vec<_>>();
    left_indices.sort_by(|left, right| {
        left_networks[*left]
            .nominal(kind)
            .total_cmp(&left_networks[*right].nominal(kind))
    });
    let mut right_indices = (0..right_networks.len()).collect::<Vec<_>>();
    right_indices.sort_by(|left, right| {
        right_networks[*left]
            .nominal(kind)
            .total_cmp(&right_networks[*right].nominal(kind))
    });

    let mut candidates = Vec::new();
    for (left_position, left_index) in left_indices.into_iter().enumerate() {
        let left = &left_networks[left_index];
        let right_start = if same_networks { left_position } else { 0 };
        for connection in [Connection::Series, Connection::Parallel] {
            let desired = desired_right_value(kind, connection, left.nominal(kind), target);
            let insertion =
                lower_bound_networks(right_networks, &right_indices, right_start, desired, kind);
            for right_position in neighboring_indices(insertion, right_start, right_indices.len()) {
                candidates.push(Network::connected(
                    connection,
                    left.clone(),
                    right_networks[right_indices[right_position]].clone(),
                ));
            }
        }
    }
    candidates
}

fn lower_bound_networks(
    networks: &[Network],
    sorted_indices: &[usize],
    start: usize,
    target: f64,
    kind: ComponentKind,
) -> usize {
    let mut low = start;
    let mut high = sorted_indices.len();
    while low < high {
        let middle = low + (high - low) / 2;
        if networks[sorted_indices[middle]].nominal(kind) < target {
            low = middle + 1;
        } else {
            high = middle;
        }
    }
    low
}

fn desired_right_value(kind: ComponentKind, connection: Connection, left: f64, target: f64) -> f64 {
    // Both series addition and the harmonic/parallel equation are monotone in the
    // right operand. Values below and above this inverse are therefore the only
    // pairs that can be closest; every more distant architecture is dominated.
    if is_additive(kind, connection) {
        return target - left;
    }
    if target >= left {
        return f64::INFINITY;
    }
    target * left / (left - target)
}

fn combine_values(kind: ComponentKind, connection: Connection, left: f64, right: f64) -> f64 {
    if is_additive(kind, connection) {
        left + right
    } else {
        parallel_value(left, right)
    }
}

fn generate_leaves(options: SolveOptions) -> Vec<Network> {
    let target_decade = options.target.log10().floor() as i32;
    let first_decade = target_decade
        .saturating_sub(1)
        .clamp(i32::from(i8::MIN), i32::from(i8::MAX));
    let last_decade = target_decade
        .saturating_add(1)
        .clamp(i32::from(i8::MIN), i32::from(i8::MAX));
    let mut leaves = Vec::new();
    for decade in first_decade..=last_decade {
        for number in options.series.preferred_numbers() {
            let component = PreferredComponent {
                number,
                decade: decade as i8,
            };
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
    candidates.retain(|network| seen_values.insert(network.nominal(kind).to_bits()));
    candidates.truncate(FRONTIER_LIMIT);
    candidates
}

fn sort_networks(networks: &mut [Network], kind: ComponentKind, target: f64) {
    networks.sort_by(|left, right| compare_networks(left, right, kind, target));
}

fn compare_networks(left: &Network, right: &Network, kind: ComponentKind, target: f64) -> Ordering {
    let left_error = relative_error(left.nominal(kind), target);
    let right_error = relative_error(right.nominal(kind), target);
    let error_order = if (left_error - right_error).abs() <= ERROR_EQUIVALENCE {
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

fn is_effectively_exact(network: &Network, kind: ComponentKind, target: f64) -> bool {
    relative_error(network.nominal(kind), target) <= ERROR_EQUIVALENCE
}

fn relative_error(value: f64, target: f64) -> f64 {
    if value == target {
        0.0
    } else if target == 0.0 || target == f64::INFINITY {
        f64::INFINITY
    } else {
        ((value - target) / target).abs()
    }
}
