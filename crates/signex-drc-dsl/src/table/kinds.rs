//! What a rule can limit: its `on` target ([`RuleKind`]) and the limit
//! fields each target takes ([`LimitField`]), with the words of the rule
//! text and the limit the check resolves. The one table the compiler, the
//! rule table and the Rules tab read. Only what Run DRC checks is here.

use signex_drc::LimitKind;

use crate::ast::Bound;

/// What a rule limits: its `on` target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RuleKind {
    Clearance,
    Track,
    Via,
    /// Every drilled hole, pad or via (#2435).
    Hole,
    /// Copper against the board outline.
    Edge,
    Silk,
    Mask,
    Paste,
    Courtyard,
}

impl RuleKind {
    pub const ALL: [RuleKind; 9] = [
        RuleKind::Clearance,
        RuleKind::Track,
        RuleKind::Via,
        RuleKind::Hole,
        RuleKind::Edge,
        RuleKind::Silk,
        RuleKind::Mask,
        RuleKind::Paste,
        RuleKind::Courtyard,
    ];

    /// The word after `on`.
    pub const fn keyword(self) -> &'static str {
        match self {
            RuleKind::Clearance => "clearance",
            RuleKind::Track => "track",
            RuleKind::Via => "via",
            RuleKind::Hole => "hole",
            RuleKind::Edge => "edge",
            RuleKind::Silk => "silk",
            RuleKind::Mask => "mask",
            RuleKind::Paste => "paste",
            RuleKind::Courtyard => "courtyard",
        }
    }

    pub fn from_keyword(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.keyword() == word)
    }

    /// The limits a rule of this kind can set, in column order.
    pub fn fields(self) -> Vec<LimitField> {
        LimitField::ALL
            .into_iter()
            .filter(|f| f.kind() == self)
            .collect()
    }

    /// Whether a net, class or pattern condition and a layer condition can
    /// narrow it. Silkscreen, mask, paste and courtyards carry no net and
    /// sit on no copper layer, so their rules are board-wide.
    pub const fn is_scoped(self) -> bool {
        matches!(
            self,
            RuleKind::Clearance | RuleKind::Track | RuleKind::Via | RuleKind::Hole | RuleKind::Edge
        )
    }
}

/// One limit column: `(field, kind, field word, bound, id, limit)`.
type FieldRow = (
    LimitField,
    RuleKind,
    &'static str,
    Bound,
    &'static str,
    LimitKind,
);

const FIELDS: [FieldRow; 19] = {
    use Bound::{AtLeast, AtMost};
    use LimitField as F;
    use LimitKind as L;
    use RuleKind as K;
    [
        (
            F::MinClearance,
            K::Clearance,
            "clearance",
            AtLeast,
            "min_clearance",
            L::MinClearance,
        ),
        (
            F::MinWidth,
            K::Track,
            "width",
            AtLeast,
            "min_width",
            L::MinTrackWidth,
        ),
        (
            F::MaxWidth,
            K::Track,
            "width",
            AtMost,
            "max_width",
            L::MaxTrackWidth,
        ),
        (
            F::MinDiameter,
            K::Via,
            "diameter",
            AtLeast,
            "min_diameter",
            L::MinViaDiameter,
        ),
        (
            F::MinDrill,
            K::Via,
            "drill",
            AtLeast,
            "min_drill",
            L::MinViaDrill,
        ),
        (
            F::MinPadGap,
            K::Via,
            "pad_gap",
            AtLeast,
            "min_pad_gap",
            L::MinViaPadGap,
        ),
        (
            F::MinHole,
            K::Hole,
            "diameter",
            AtLeast,
            "min_hole",
            L::MinHoleSize,
        ),
        (
            F::MaxHole,
            K::Hole,
            "diameter",
            AtMost,
            "max_hole",
            L::MaxHoleSize,
        ),
        (
            F::MinAnnular,
            K::Hole,
            "annular",
            AtLeast,
            "min_annular",
            L::MinAnnularRing,
        ),
        (
            F::MinHoleSpacing,
            K::Hole,
            "spacing",
            AtLeast,
            "min_hole_spacing",
            L::MinHoleToHole,
        ),
        (
            F::MinEdgeClearance,
            K::Edge,
            "clearance",
            AtLeast,
            "min_edge_clearance",
            L::MinEdgeClearance,
        ),
        (
            F::MinSilkPad,
            K::Silk,
            "pad_clearance",
            AtLeast,
            "min_silk_pad",
            L::MinSilkToPad,
        ),
        (
            F::MinSilkSpacing,
            K::Silk,
            "spacing",
            AtLeast,
            "min_silk_spacing",
            L::MinSilkToSilk,
        ),
        (
            F::MinSliver,
            K::Mask,
            "sliver",
            AtLeast,
            "min_sliver",
            L::MinMaskSliver,
        ),
        (
            F::MinMaskExpansion,
            K::Mask,
            "expansion",
            AtLeast,
            "min_mask_expansion",
            L::MinMaskExpansion,
        ),
        (
            F::MaxMaskExpansion,
            K::Mask,
            "expansion",
            AtMost,
            "max_mask_expansion",
            L::MaxMaskExpansion,
        ),
        (
            F::MinPasteExpansion,
            K::Paste,
            "expansion",
            AtLeast,
            "min_paste_expansion",
            L::MinPasteExpansion,
        ),
        (
            F::MaxPasteExpansion,
            K::Paste,
            "expansion",
            AtMost,
            "max_paste_expansion",
            L::MaxPasteExpansion,
        ),
        (
            F::MinCourtyardSpacing,
            K::Courtyard,
            "spacing",
            AtLeast,
            "min_courtyard_spacing",
            L::MinCourtyardGap,
        ),
    ]
};

/// One limit column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LimitField {
    MinClearance,
    MinWidth,
    MaxWidth,
    MinDiameter,
    MinDrill,
    MinPadGap,
    MinHole,
    MaxHole,
    MinAnnular,
    MinHoleSpacing,
    MinEdgeClearance,
    MinSilkPad,
    MinSilkSpacing,
    MinSliver,
    MinMaskExpansion,
    MaxMaskExpansion,
    MinPasteExpansion,
    MaxPasteExpansion,
    MinCourtyardSpacing,
}

impl LimitField {
    pub const ALL: [LimitField; 19] = {
        let mut out = [LimitField::MinClearance; 19];
        let mut i = 0;
        while i < FIELDS.len() {
            out[i] = FIELDS[i].0;
            i += 1;
        }
        out
    };

    /// Its row in [`FIELDS`]: the enum is declared in that order.
    const fn row(self) -> FieldRow {
        FIELDS[self as usize]
    }

    pub const fn kind(self) -> RuleKind {
        self.row().1
    }

    /// The field word in the rule text.
    pub const fn word(self) -> &'static str {
        self.row().2
    }

    pub const fn bound(self) -> Bound {
        self.row().3
    }

    /// Stable operand and JSON name.
    pub const fn id(self) -> &'static str {
        self.row().4
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|f| f.id() == id)
    }

    /// The limit the check resolves for this column.
    pub const fn limit_kind(self) -> LimitKind {
        self.row().5
    }

    /// The field `word` with `bound` of a `kind` rule.
    pub(crate) fn of(kind: RuleKind, word: &str, bound: Bound) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|f| f.kind() == kind && f.word() == word && f.bound() == bound)
    }

    /// The field words a `kind` rule takes, for a message: `width`,
    /// `diameter, drill or pad_gap`.
    pub(crate) fn words_of(kind: RuleKind) -> String {
        let mut words: Vec<&str> = kind.fields().iter().map(|f| f.word()).collect();
        words.dedup();
        match words.split_last() {
            Some((last, [])) => (*last).to_string(),
            Some((last, rest)) => format!("{} or {last}", rest.join(", ")),
            None => String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_field_table_is_in_declaration_order_and_ids_are_unique() {
        for (i, row) in FIELDS.iter().enumerate() {
            assert_eq!(row.0 as usize, i, "{:?} out of order", row.0);
        }
        let mut ids: Vec<&str> = LimitField::ALL.iter().map(|f| f.id()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), LimitField::ALL.len());
        let mut limits: Vec<LimitKind> = LimitField::ALL.iter().map(|f| f.limit_kind()).collect();
        limits.sort_unstable();
        limits.dedup();
        assert_eq!(
            limits.len(),
            LimitKind::ALL.len(),
            "every limit has one column"
        );
    }
}
