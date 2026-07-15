use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Complex {
    pub re: f64,
    pub im: f64,
}

impl Complex {
    pub const ZERO: Self = Self { re: 0.0, im: 0.0 };
    pub const ONE: Self = Self { re: 1.0, im: 0.0 };

    pub const fn new(re: f64, im: f64) -> Self {
        Self { re, im }
    }

    pub fn magnitude(self) -> f64 {
        self.re.hypot(self.im)
    }

    pub fn phase_degrees(self) -> f64 {
        self.im.atan2(self.re).to_degrees()
    }

    pub fn conjugate(self) -> Self {
        Self::new(self.re, -self.im)
    }

    pub fn from_polar(magnitude: f64, angle_degrees: f64) -> Self {
        let angle = angle_degrees.to_radians();
        Self::new(magnitude * angle.cos(), magnitude * angle.sin())
    }

    pub fn reciprocal(self) -> Option<Self> {
        let denominator = self.re.mul_add(self.re, self.im * self.im);
        (denominator > f64::EPSILON).then_some(Self {
            re: self.re / denominator,
            im: -self.im / denominator,
        })
    }

    pub fn tan(self) -> Option<Self> {
        let denom = (2.0 * self.re).cos() + (2.0 * self.im).cosh();
        (denom.abs() > f64::EPSILON).then_some(Self {
            re: (2.0 * self.re).sin() / denom,
            im: (2.0 * self.im).sinh() / denom,
        })
    }
}

impl std::ops::Add for Complex {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.re + rhs.re, self.im + rhs.im)
    }
}

impl std::ops::Sub for Complex {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.re - rhs.re, self.im - rhs.im)
    }
}

impl std::ops::Mul for Complex {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Self::new(
            self.re.mul_add(rhs.re, -(self.im * rhs.im)),
            self.re.mul_add(rhs.im, self.im * rhs.re),
        )
    }
}

impl std::ops::Mul<f64> for Complex {
    type Output = Self;

    fn mul(self, rhs: f64) -> Self::Output {
        Self::new(self.re * rhs, self.im * rhs)
    }
}

impl std::ops::Div<f64> for Complex {
    type Output = Self;

    fn div(self, rhs: f64) -> Self::Output {
        Self::new(self.re / rhs, self.im / rhs)
    }
}

impl std::ops::Div for Complex {
    type Output = Self;

    fn div(self, rhs: Self) -> Self::Output {
        let denominator = rhs.re.mul_add(rhs.re, rhs.im * rhs.im);
        Self::new(
            (self.re * rhs.re + self.im * rhs.im) / denominator,
            (self.im * rhs.re - self.re * rhs.im) / denominator,
        )
    }
}
