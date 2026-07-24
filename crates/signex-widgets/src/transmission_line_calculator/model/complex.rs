use serde::{Deserialize, Serialize};

/// Represents a complex scalar in rectangular form.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Complex {
    pub re: f64,
    pub im: f64,
}

impl Complex {
    pub const ZERO: Self = Self { re: 0.0, im: 0.0 };
    pub const ONE: Self = Self { re: 1.0, im: 0.0 };

    /// Creates a complex value from its real and imaginary components.
    pub const fn new(re: f64, im: f64) -> Self {
        Self { re, im }
    }

    /// Returns the complex magnitude using the Euclidean norm.
    pub fn magnitude(self) -> f64 {
        self.re.hypot(self.im)
    }

    /// Returns the complex phase angle in degrees.
    pub fn phase_degrees(self) -> f64 {
        self.im.atan2(self.re).to_degrees()
    }

    /// Returns the complex conjugate.
    pub fn conjugate(self) -> Self {
        Self::new(self.re, -self.im)
    }

    /// Creates a value from polar.
    pub fn from_polar(magnitude: f64, angle_degrees: f64) -> Self {
        let angle = angle_degrees.to_radians();
        Self::new(magnitude * angle.cos(), magnitude * angle.sin())
    }

    /// Returns the multiplicative inverse of the complex value.
    pub fn reciprocal(self) -> Option<Self> {
        let denominator = self.re.mul_add(self.re, self.im * self.im);
        (denominator > f64::EPSILON).then_some(Self {
            re: self.re / denominator,
            im: -self.im / denominator,
        })
    }

    /// Returns the complex tangent.
    pub fn tan(self) -> Option<Self> {
        let denom = (2.0 * self.re).cos() + (2.0 * self.im).cosh();
        (denom.abs() > f64::EPSILON).then_some(Self {
            re: (2.0 * self.re).sin() / denom,
            im: (2.0 * self.im).sinh() / denom,
        })
    }
}

impl std::ops::Add for Complex {
    /// Defines the associated `Output` type for this implementation.
    type Output = Self;

    /// Adds two complex values component-wise.
    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.re + rhs.re, self.im + rhs.im)
    }
}

impl std::ops::Sub for Complex {
    /// Defines the associated `Output` type for this implementation.
    type Output = Self;

    /// Subtracts two complex values component-wise.
    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.re - rhs.re, self.im - rhs.im)
    }
}

impl std::ops::Mul for Complex {
    /// Defines the associated `Output` type for this implementation.
    type Output = Self;

    /// Multiplies the operands using complex arithmetic.
    fn mul(self, rhs: Self) -> Self::Output {
        Self::new(
            self.re.mul_add(rhs.re, -(self.im * rhs.im)),
            self.re.mul_add(rhs.im, self.im * rhs.re),
        )
    }
}

impl std::ops::Mul<f64> for Complex {
    /// Defines the associated `Output` type for this implementation.
    type Output = Self;

    /// Multiplies the operands using complex arithmetic.
    fn mul(self, rhs: f64) -> Self::Output {
        Self::new(self.re * rhs, self.im * rhs)
    }
}

impl std::ops::Div<f64> for Complex {
    /// Defines the associated `Output` type for this implementation.
    type Output = Self;

    /// Divides the operands using complex arithmetic.
    fn div(self, rhs: f64) -> Self::Output {
        Self::new(self.re / rhs, self.im / rhs)
    }
}

impl std::ops::Div for Complex {
    /// Defines the associated `Output` type for this implementation.
    type Output = Self;

    /// Divides the operands using complex arithmetic.
    fn div(self, rhs: Self) -> Self::Output {
        let denominator = rhs.re.mul_add(rhs.re, rhs.im * rhs.im);
        Self::new(
            (self.re * rhs.re + self.im * rhs.im) / denominator,
            (self.im * rhs.re - self.re * rhs.im) / denominator,
        )
    }
}
