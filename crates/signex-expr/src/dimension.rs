//! Dimension vectors: the unit algebra behind `*`, `/`, `^` and `sqrt`.
//!
//! A dimension is a small integer exponent over five base dimensions:
//! length (mm), angle (rad), time (s), voltage (V) and current (A).
//! Voltage and current stand in for SI's kilogram because mass never
//! appears in an EDA expression, and with mm as the length base a sketch's
//! numbers stay exactly what they were before unit algebra. `*` adds
//! exponents, `/` subtracts them; a named unit such as Ω is just a symbol
//! for one vector (V/A).

use serde::{Deserialize, Serialize};

use crate::ExprError;

/// Largest exponent magnitude any base dimension may reach. A long chain
/// of multiplications fails with [`ExprError::Domain`] instead of
/// overflowing the `i8`.
pub const MAX_DIM_EXPONENT: i8 = 8;

/// Exponents of the five base dimensions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Dimension {
    pub length: i8,
    pub angle: i8,
    pub time: i8,
    pub voltage: i8,
    pub current: i8,
}

/// Base-unit symbols in exponent order, used to spell a compound unit.
const BASE_SYMBOLS: [&str; 5] = ["mm", "rad", "s", "V", "A"];

impl Dimension {
    /// A plain number.
    pub const NONE: Self = Self::new(0, 0, 0, 0, 0);
    pub const LENGTH: Self = Self::new(1, 0, 0, 0, 0);
    pub const ANGLE: Self = Self::new(0, 1, 0, 0, 0);
    pub const TIME: Self = Self::new(0, 0, 1, 0, 0);
    pub const VOLTAGE: Self = Self::new(0, 0, 0, 1, 0);
    pub const CURRENT: Self = Self::new(0, 0, 0, 0, 1);
    /// Ω = V/A.
    pub const RESISTANCE: Self = Self::new(0, 0, 0, 1, -1);
    /// W = V·A.
    pub const POWER: Self = Self::new(0, 0, 0, 1, 1);
    /// F = A·s/V.
    pub const CAPACITANCE: Self = Self::new(0, 0, 1, -1, 1);
    /// H = V·s/A.
    pub const INDUCTANCE: Self = Self::new(0, 0, 1, 1, -1);
    /// C = A·s.
    pub const CHARGE: Self = Self::new(0, 0, 1, 0, 1);
    /// Hz = 1/s.
    pub const FREQUENCY: Self = Self::new(0, 0, -1, 0, 0);

    pub const fn new(length: i8, angle: i8, time: i8, voltage: i8, current: i8) -> Self {
        Self {
            length,
            angle,
            time,
            voltage,
            current,
        }
    }

    const fn exponents(self) -> [i8; 5] {
        [
            self.length,
            self.angle,
            self.time,
            self.voltage,
            self.current,
        ]
    }

    const fn from_exponents(e: [i8; 5]) -> Self {
        Self::new(e[0], e[1], e[2], e[3], e[4])
    }

    /// True for a plain number.
    pub fn is_none(self) -> bool {
        self == Self::NONE
    }

    /// The dimension of a product.
    pub fn times(self, other: Self) -> Result<Self, ExprError> {
        self.combine(other, |a, b| i32::from(a) + i32::from(b))
    }

    /// The dimension of a quotient.
    pub fn per(self, other: Self) -> Result<Self, ExprError> {
        self.combine(other, |a, b| i32::from(a) - i32::from(b))
    }

    /// The dimension of `self ^ n`.
    pub fn raised_to(self, n: i32) -> Result<Self, ExprError> {
        self.combine(Self::NONE, |a, _| i32::from(a).saturating_mul(n))
    }

    /// The dimension of a square root, when every exponent is even.
    pub fn root(self) -> Option<Self> {
        let e = self.exponents();
        if e.iter().any(|x| x % 2 != 0) {
            return None;
        }
        Some(Self::from_exponents(e.map(|x| x / 2)))
    }

    fn combine(self, other: Self, f: impl Fn(i8, i8) -> i32) -> Result<Self, ExprError> {
        let mut out = [0i8; 5];
        let pairs = self.exponents().into_iter().zip(other.exponents());
        for (slot, (a, b)) in out.iter_mut().zip(pairs) {
            let v = f(a, b);
            *slot = i8::try_from(v)
                .ok()
                .filter(|x| x.unsigned_abs() <= MAX_DIM_EXPONENT.unsigned_abs())
                .ok_or_else(|| {
                    ExprError::Domain(format!("unit exponent {v} is beyond ±{MAX_DIM_EXPONENT}"))
                })?;
        }
        Ok(Self::from_exponents(out))
    }

    /// The dimension spelled in base units: `mm²`, `V/s`, `1/mm`.
    pub fn compound_symbol(self) -> String {
        let mut numerator = Vec::new();
        let mut denominator = Vec::new();
        for (exp, sym) in self.exponents().into_iter().zip(BASE_SYMBOLS) {
            let part = |n: i8| {
                if n == 1 {
                    sym.to_string()
                } else {
                    format!("{sym}{}", superscript(n))
                }
            };
            if exp > 0 {
                numerator.push(part(exp));
            } else if exp < 0 {
                denominator.push(part(-exp));
            }
        }
        let top = if numerator.is_empty() {
            "1".to_string()
        } else {
            numerator.join("·")
        };
        if denominator.is_empty() {
            top
        } else {
            format!("{top}/{}", denominator.join("·"))
        }
    }
}

/// `n` (1..=8) in superscript digits.
fn superscript(n: i8) -> String {
    const DIGITS: [char; 10] = ['⁰', '¹', '²', '³', '⁴', '⁵', '⁶', '⁷', '⁸', '⁹'];
    n.to_string()
        .chars()
        .map(|c| c.to_digit(10).map_or(c, |d| DIGITS[d as usize]))
        .collect()
}
