//! The limits a written rule can set (#2307, #2435): one variant per
//! checked value, each a minimum or a maximum in nm. The base rules they
//! back are listed in `docs/DRC_RULES.md`, "The base rules".

/// Declares [`Limit`] (with its value) and [`LimitKind`] (without) from one
/// list, so the two never drift.
macro_rules! limits {
    ($($variant:ident => $id:literal, $max:literal;)*) => {
        /// One limit a rule sets, in nm.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum Limit {
            $($variant(i64),)*
        }

        /// Which limit, without its value.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum LimitKind {
            $($variant,)*
        }

        impl Limit {
            /// The limit of `kind` at `nm`.
            pub const fn of(kind: LimitKind, nm: i64) -> Limit {
                match kind {
                    $(LimitKind::$variant => Limit::$variant(nm),)*
                }
            }

            pub const fn split(self) -> (LimitKind, i64) {
                match self {
                    $(Limit::$variant(nm) => (LimitKind::$variant, nm),)*
                }
            }

            pub const fn kind(self) -> LimitKind {
                self.split().0
            }
        }

        impl LimitKind {
            pub const ALL: &'static [LimitKind] = &[$(LimitKind::$variant,)*];

            /// Stable name in answers (`min_clearance`, `max_track_width`, ...).
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(LimitKind::$variant => $id,)*
                }
            }

            /// Whether it is a maximum (the item must stay at or under it).
            pub const fn is_maximum(self) -> bool {
                match self {
                    $(LimitKind::$variant => $max,)*
                }
            }
        }
    };
}

limits! {
    MinClearance => "min_clearance", false;
    MinTrackWidth => "min_track_width", false;
    MaxTrackWidth => "max_track_width", true;
    MinViaDiameter => "min_via_diameter", false;
    MinViaDrill => "min_via_drill", false;
    MinHoleSize => "min_hole_size", false;
    MaxHoleSize => "max_hole_size", true;
    MinAnnularRing => "min_annular_ring", false;
    MinHoleToHole => "min_hole_to_hole", false;
    MinEdgeClearance => "min_edge_clearance", false;
    MinViaPadGap => "min_via_pad_gap", false;
    MinSilkToPad => "min_silk_to_pad", false;
    MinSilkToSilk => "min_silk_to_silk", false;
    MinMaskSliver => "min_mask_sliver", false;
    MinMaskExpansion => "min_mask_expansion", false;
    MaxMaskExpansion => "max_mask_expansion", true;
    MinPasteExpansion => "min_paste_expansion", false;
    MaxPasteExpansion => "max_paste_expansion", true;
    MinCourtyardGap => "min_courtyard_gap", false;
    MaxComponentHeight => "max_component_height", true;
    MinBodyClearance => "min_body_clearance", false;
}

impl LimitKind {
    /// For a maximum the smaller value is the stricter one.
    pub fn stricter(self, a: i64, b: i64) -> bool {
        if self.is_maximum() { a < b } else { a > b }
    }

    pub fn from_str_id(id: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|k| k.as_str() == id)
    }

    /// Whether a minimum of 0 still finds something: a gap kind, where
    /// touching or overlapping objects break even a limit of 0, or an
    /// expansion, which may be negative.
    pub const fn zero_still_checks(self) -> bool {
        matches!(
            self,
            LimitKind::MinHoleToHole
                | LimitKind::MinEdgeClearance
                | LimitKind::MinViaPadGap
                | LimitKind::MinSilkToPad
                | LimitKind::MinSilkToSilk
                | LimitKind::MinCourtyardGap
                | LimitKind::MinBodyClearance
                | LimitKind::MinMaskExpansion
                | LimitKind::MinPasteExpansion
        )
    }
}

impl LimitKind {
    /// The kind of finding that breaking it makes.
    pub const fn violation(self) -> crate::ViolationKind {
        use crate::ViolationKind as V;
        match self {
            LimitKind::MinClearance => V::Clearance,
            LimitKind::MinTrackWidth | LimitKind::MaxTrackWidth => V::TrackWidth,
            LimitKind::MinViaDiameter => V::ViaDiameter,
            LimitKind::MinViaDrill => V::ViaDrill,
            LimitKind::MinHoleSize | LimitKind::MaxHoleSize => V::HoleSize,
            LimitKind::MinAnnularRing => V::AnnularRing,
            LimitKind::MinHoleToHole => V::HoleToHole,
            LimitKind::MinEdgeClearance => V::EdgeClearance,
            LimitKind::MinViaPadGap => V::ViaInPad,
            LimitKind::MinSilkToPad => V::SilkToPad,
            LimitKind::MinSilkToSilk => V::SilkToSilk,
            LimitKind::MinMaskSliver => V::MaskSliver,
            LimitKind::MinMaskExpansion | LimitKind::MaxMaskExpansion => V::MaskExpansion,
            LimitKind::MinPasteExpansion | LimitKind::MaxPasteExpansion => V::PasteExpansion,
            LimitKind::MinCourtyardGap => V::CourtyardOverlap,
            LimitKind::MaxComponentHeight => V::ComponentHeight,
            LimitKind::MinBodyClearance => V::BodyClearance,
        }
    }

    /// Whether `measured` breaks a limit of this kind at `limit`.
    pub const fn is_broken(self, measured: i64, limit: i64) -> bool {
        if self.is_maximum() {
            measured > limit
        } else {
            measured < limit
        }
    }
}
