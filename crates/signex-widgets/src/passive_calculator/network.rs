use super::domain::{ComponentKind, PreferredComponent, SiPrefix, Tolerance};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Connection {
    Series,
    Parallel,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Network {
    Component {
        component: PreferredComponent,
        tolerance: Tolerance,
    },
    Connected {
        connection: Connection,
        left: Box<Network>,
        right: Box<Network>,
    },
}

impl Network {
    pub fn component(component: PreferredComponent, tolerance: Tolerance) -> Self {
        Self::Component {
            component,
            tolerance,
        }
    }

    pub fn connected(connection: Connection, left: Self, right: Self) -> Self {
        Self::Connected {
            connection,
            left: Box::new(left),
            right: Box::new(right),
        }
    }

    pub fn part_count(&self) -> usize {
        match self {
            Self::Component { .. } => 1,
            Self::Connected { left, right, .. } => left.part_count() + right.part_count(),
        }
    }

    pub fn nominal(&self, kind: ComponentKind) -> f64 {
        self.evaluate(kind, Bound::Nominal)
    }

    pub fn minimum(&self, kind: ComponentKind) -> f64 {
        self.evaluate(kind, Bound::Minimum)
    }

    pub fn maximum(&self, kind: ComponentKind) -> f64 {
        self.evaluate(kind, Bound::Maximum)
    }

    pub fn tolerance_interval(&self, kind: ComponentKind) -> (f64, f64) {
        (self.minimum(kind), self.maximum(kind))
    }

    pub fn set_tolerance(&mut self, leaf_index: usize, tolerance: Tolerance) -> bool {
        let mut current = 0;
        self.set_tolerance_at(leaf_index, tolerance, &mut current)
    }

    pub fn components(&self) -> Vec<(PreferredComponent, Tolerance)> {
        let mut components = Vec::with_capacity(self.part_count());
        self.collect_components(&mut components);
        components
    }

    pub fn expression(&self, kind: ComponentKind) -> String {
        let mut index = 1;
        self.format_expression(kind.symbol(), &mut index, true)
    }

    pub fn plain_expression(&self, kind: ComponentKind) -> String {
        let mut index = 1;
        self.format_expression(kind.symbol(), &mut index, false)
    }

    fn evaluate(&self, kind: ComponentKind, bound: Bound) -> f64 {
        match self {
            Self::Component {
                component,
                tolerance,
            } => {
                let nominal = component.value();
                match bound {
                    Bound::Nominal => nominal,
                    Bound::Minimum => nominal * (1.0 - tolerance.fraction()),
                    Bound::Maximum => nominal * (1.0 + tolerance.fraction()),
                }
            }
            Self::Connected {
                connection,
                left,
                right,
            } => {
                let left = left.evaluate(kind, bound);
                let right = right.evaluate(kind, bound);
                if is_additive(kind, *connection) {
                    left + right
                } else {
                    parallel_value(left, right)
                }
            }
        }
    }

    fn set_tolerance_at(
        &mut self,
        target: usize,
        tolerance: Tolerance,
        current: &mut usize,
    ) -> bool {
        match self {
            Self::Component {
                tolerance: current_tolerance,
                ..
            } => {
                if *current == target {
                    *current_tolerance = tolerance;
                    true
                } else {
                    *current += 1;
                    false
                }
            }
            Self::Connected { left, right, .. } => {
                left.set_tolerance_at(target, tolerance, current)
                    || right.set_tolerance_at(target, tolerance, current)
            }
        }
    }

    fn collect_components(&self, output: &mut Vec<(PreferredComponent, Tolerance)>) {
        match self {
            Self::Component {
                component,
                tolerance,
            } => output.push((*component, *tolerance)),
            Self::Connected { left, right, .. } => {
                left.collect_components(output);
                right.collect_components(output);
            }
        }
    }

    fn format_expression(&self, symbol: &str, index: &mut usize, subscripts: bool) -> String {
        match self {
            Self::Component { .. } => {
                let result = if subscripts {
                    format!("{symbol}{}", to_subscript(*index))
                } else {
                    format!("{symbol}{index}")
                };
                *index += 1;
                result
            }
            Self::Connected {
                connection,
                left,
                right,
            } => {
                let left = left.format_expression(symbol, index, subscripts);
                let right = right.format_expression(symbol, index, subscripts);
                let operator = match connection {
                    Connection::Series => "+",
                    Connection::Parallel => "∥",
                };
                format!("({left} {operator} {right})")
            }
        }
    }
}

pub fn is_additive(kind: ComponentKind, connection: Connection) -> bool {
    match kind {
        ComponentKind::Resistor | ComponentKind::Inductor => connection == Connection::Series,
        ComponentKind::Capacitor => connection == Connection::Parallel,
    }
}

pub fn parallel_value(left: f64, right: f64) -> f64 {
    left * right / (left + right)
}

pub fn format_value(value: f64, kind: ComponentKind) -> String {
    let prefix = best_prefix(value);
    format!(
        "{} {}",
        format_number(value / prefix.multiplier()),
        prefix.unit(kind)
    )
}

pub fn format_number(value: f64) -> String {
    if value == 0.0 {
        return "0".to_string();
    }
    let magnitude = value.abs();
    let decimals = if magnitude >= 100.0 {
        1
    } else if magnitude >= 10.0 {
        2
    } else {
        3
    };
    let formatted = format!("{value:.decimals$}");
    formatted
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string()
}

fn best_prefix(value: f64) -> SiPrefix {
    let exponent = if value == 0.0 {
        0
    } else {
        value.abs().log10().floor() as i8
    };
    match exponent {
        ..=-10 => SiPrefix::Pico,
        -9..=-7 => SiPrefix::Nano,
        -6..=-4 => SiPrefix::Micro,
        -3..=-1 => SiPrefix::Milli,
        0..=2 => SiPrefix::None,
        3..=5 => SiPrefix::Kilo,
        6..=8 => SiPrefix::Mega,
        _ => SiPrefix::Giga,
    }
}

fn to_subscript(value: usize) -> String {
    value
        .to_string()
        .chars()
        .map(|character| match character {
            '0' => '₀',
            '1' => '₁',
            '2' => '₂',
            '3' => '₃',
            '4' => '₄',
            '5' => '₅',
            '6' => '₆',
            '7' => '₇',
            '8' => '₈',
            '9' => '₉',
            _ => character,
        })
        .collect()
}

#[derive(Debug, Clone, Copy)]
enum Bound {
    Nominal,
    Minimum,
    Maximum,
}
