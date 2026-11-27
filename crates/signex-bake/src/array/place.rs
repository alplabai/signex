//! Where one pattern instance puts what it repeats: a turn `m` about a
//! pivot `c`, then a shift `t`, and how a pad's rotation turns with it.

/// An instance's placement: the source point `p` goes to
/// `c + m · (p − c) + t`, a pad's rotation to `sign · deg + offset_deg`.
/// Arrays move and turn positions but keep pads unrotated; a mirror
/// reflects the rotation too.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Place {
    m: [f64; 4],
    c: [f64; 2],
    t: [f64; 2],
    sign: f64,
    offset_deg: f64,
}

impl Place {
    const IDENTITY: Self = Self {
        m: [1.0, 0.0, 0.0, 1.0],
        c: [0.0, 0.0],
        t: [0.0, 0.0],
        sign: 1.0,
        offset_deg: 0.0,
    };

    pub(super) fn translation(dx: f64, dy: f64) -> Self {
        Self {
            t: [dx, dy],
            ..Self::IDENTITY
        }
    }

    /// A turn by `theta` about `centre`, kept about its pivot so a polar
    /// instance lands where `centre + m · (source − centre)` puts it.
    pub(super) fn rotation_about(centre: (f64, f64), theta: f64) -> Self {
        let (sin, cos) = theta.sin_cos();
        Self {
            m: [cos, -sin, sin, cos],
            c: [centre.0, centre.1],
            ..Self::IDENTITY
        }
    }

    /// The reflection about the line through `a` and `b`; `None` when
    /// they coincide.
    pub(super) fn reflection(a: (f64, f64), b: (f64, f64)) -> Option<Self> {
        let (vx, vy) = (b.0 - a.0, b.1 - a.1);
        let len2 = vx * vx + vy * vy;
        if len2 <= 1e-12 {
            return None;
        }
        let (ux, uy) = (vx / len2.sqrt(), vy / len2.sqrt());
        let m = [
            2.0 * ux * ux - 1.0,
            2.0 * ux * uy,
            2.0 * ux * uy,
            2.0 * uy * uy - 1.0,
        ];
        Some(Self {
            m,
            t: [
                a.0 - (m[0] * a.0 + m[1] * a.1),
                a.1 - (m[2] * a.0 + m[3] * a.1),
            ],
            sign: -1.0,
            offset_deg: 2.0 * vy.atan2(vx).to_degrees(),
            ..Self::IDENTITY
        })
    }

    /// The shift that, with no pivot, places as this does: `p ↦ m · p + t'`.
    fn shift(&self) -> [f64; 2] {
        let (m, c) = (self.m, self.c);
        [
            c[0] - (m[0] * c[0] + m[1] * c[1]) + self.t[0],
            c[1] - (m[2] * c[0] + m[3] * c[1]) + self.t[1],
        ]
    }

    /// `self` after `inner`: place by `inner`, then by `self`.
    pub(super) fn after(&self, inner: &Self) -> Self {
        let (a, b) = (self.m, inner.m);
        let (ta, tb) = (self.shift(), inner.shift());
        Self {
            m: [
                a[0] * b[0] + a[1] * b[2],
                a[0] * b[1] + a[1] * b[3],
                a[2] * b[0] + a[3] * b[2],
                a[2] * b[1] + a[3] * b[3],
            ],
            c: [0.0, 0.0],
            t: [
                a[0] * tb[0] + a[1] * tb[1] + ta[0],
                a[2] * tb[0] + a[3] * tb[1] + ta[1],
            ],
            sign: self.sign * inner.sign,
            offset_deg: self.sign * inner.offset_deg + self.offset_deg,
        }
    }

    pub(crate) fn apply(&self, p: (f64, f64)) -> (f64, f64) {
        let (m, c) = (self.m, self.c);
        let d = (p.0 - c[0], p.1 - c[1]);
        (
            c[0] + d.0 * m[0] + d.1 * m[1] + self.t[0],
            c[1] + d.0 * m[2] + d.1 * m[3] + self.t[1],
        )
    }

    /// How far this placement moves `p`. A placement that turns nothing
    /// moves by its shift exactly, so a linear or grid step is the step.
    pub(super) fn offset_from(&self, p: (f64, f64)) -> (f64, f64) {
        if self.m == Self::IDENTITY.m {
            let t = self.shift();
            return (t[0], t[1]);
        }
        let (x, y) = self.apply(p);
        (x - p.0, y - p.1)
    }

    /// Whether this placement reflects what it places.
    pub(crate) fn mirrors(&self) -> bool {
        self.sign < 0.0
    }

    /// A pad rotation (degrees) as this placement leaves it.
    pub(crate) fn turn_deg(&self, deg: f64) -> f64 {
        if self.sign > 0.0 && self.offset_deg == 0.0 {
            return deg;
        }
        (self.sign * deg + self.offset_deg).rem_euclid(360.0)
    }
}
