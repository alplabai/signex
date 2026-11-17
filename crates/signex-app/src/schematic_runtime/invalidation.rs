//! Which collections of the render snapshot an edit changed, so the scene
//! cache copies only those.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RenderInvalidation(u32);

impl RenderInvalidation {
    pub const NONE: Self = Self(0);
    pub const SYMBOLS: Self = Self(1 << 0);
    pub const WIRES: Self = Self(1 << 1);
    pub const LABELS: Self = Self(1 << 2);
    pub const TEXT_NOTES: Self = Self(1 << 3);
    pub const BUSES: Self = Self(1 << 4);
    pub const BUS_ENTRIES: Self = Self(1 << 5);
    pub const JUNCTIONS: Self = Self(1 << 6);
    pub const NO_CONNECTS: Self = Self(1 << 7);
    pub const CHILD_SHEETS: Self = Self(1 << 8);
    pub const DRAWINGS: Self = Self(1 << 9);
    pub const LIB_SYMBOLS: Self = Self(1 << 10);
    pub const PAPER: Self = Self(1 << 11);
    pub const NO_ERC_DIRECTIVES: Self = Self(1 << 12);
    /// The sheet's net colours: a repaint with no geometry or ERC change.
    pub const NET_COLORS: Self = Self(1 << 13);
    /// Blankets, Compile Masks and Differential Pair directives (#2420).
    pub const DIRECTIVES: Self = Self(1 << 14);
    pub const FULL: Self = Self(u32::MAX);

    /// True when any bit of `mask` is also set in `self`.
    pub fn intersects(self, mask: Self) -> bool {
        self.0 & mask.0 != 0
    }
}

impl std::ops::BitOr for RenderInvalidation {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for RenderInvalidation {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}
