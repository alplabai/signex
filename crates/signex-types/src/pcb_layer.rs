//! Typed board layer id (#2173).
//!
//! Board objects keep their layer as the on-disk string, so a name this
//! build does not know survives a save unchanged. [`PcbLayer`] is the
//! typed view of a known name: 32 copper layers, the technical and
//! documentation layers, and six user layers. The string mapping lives in
//! `format/pcb_rows.rs` ([`crate::format::parse_pcb_layer`] and
//! [`crate::format::pcb_layer_token`]).

/// Which face of the board a sided layer belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BoardSide {
    Top,
    Bottom,
}

/// 1-based index of an inner copper layer, `1..=InnerCopper::MAX`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct InnerCopper(u8);

impl InnerCopper {
    /// Inner layers between top and bottom copper: 32 copper layers in all.
    pub const MAX: u8 = 30;

    /// `None` when `index` is outside `1..=MAX`.
    pub fn new(index: u8) -> Option<Self> {
        (1..=Self::MAX).contains(&index).then_some(Self(index))
    }

    pub fn index(self) -> u8 {
        self.0
    }
}

/// A copper layer, ordered top to bottom.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CopperLayer {
    Top,
    Inner(InnerCopper),
    Bottom,
}

impl CopperLayer {
    /// Total copper layers the type can name.
    pub const COUNT: usize = InnerCopper::MAX as usize + 2;

    /// Every copper layer, top to bottom.
    pub fn all() -> impl Iterator<Item = CopperLayer> {
        std::iter::once(Self::Top)
            .chain((1..=InnerCopper::MAX).map(|n| Self::Inner(InnerCopper(n))))
            .chain(std::iter::once(Self::Bottom))
    }

    /// `None` for inner layers.
    pub fn side(self) -> Option<BoardSide> {
        match self {
            Self::Top => Some(BoardSide::Top),
            Self::Bottom => Some(BoardSide::Bottom),
            Self::Inner(_) => None,
        }
    }
}

/// A non-copper layer: fabrication, assembly and documentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TechnicalLayer {
    EdgeCuts,
    Margin,
    SilkscreenTop,
    SilkscreenBottom,
    SolderMaskTop,
    SolderMaskBottom,
    PasteTop,
    PasteBottom,
    AdhesiveTop,
    AdhesiveBottom,
    FabTop,
    FabBottom,
    AssemblyTop,
    AssemblyBottom,
    CourtyardTop,
    CourtyardBottom,
    Body3dTop,
    Body3dBottom,
    KeepoutTop,
    KeepoutBottom,
    FiducialTop,
    FiducialBottom,
    DesignatorTop,
    DesignatorBottom,
    Dimensions,
    Notes,
    DrillDrawing,
    Eco1,
    Eco2,
}

impl TechnicalLayer {
    /// Every technical layer in display order.
    pub const ALL: [TechnicalLayer; 29] = [
        Self::EdgeCuts,
        Self::Margin,
        Self::SilkscreenTop,
        Self::SilkscreenBottom,
        Self::SolderMaskTop,
        Self::SolderMaskBottom,
        Self::PasteTop,
        Self::PasteBottom,
        Self::AdhesiveTop,
        Self::AdhesiveBottom,
        Self::FabTop,
        Self::FabBottom,
        Self::AssemblyTop,
        Self::AssemblyBottom,
        Self::CourtyardTop,
        Self::CourtyardBottom,
        Self::Body3dTop,
        Self::Body3dBottom,
        Self::KeepoutTop,
        Self::KeepoutBottom,
        Self::FiducialTop,
        Self::FiducialBottom,
        Self::DesignatorTop,
        Self::DesignatorBottom,
        Self::Dimensions,
        Self::Notes,
        Self::DrillDrawing,
        Self::Eco1,
        Self::Eco2,
    ];

    /// `None` for board-wide layers (outline, margin, documentation).
    pub fn side(self) -> Option<BoardSide> {
        use TechnicalLayer as T;
        match self {
            T::SilkscreenTop
            | T::SolderMaskTop
            | T::PasteTop
            | T::AdhesiveTop
            | T::FabTop
            | T::AssemblyTop
            | T::CourtyardTop
            | T::Body3dTop
            | T::KeepoutTop
            | T::FiducialTop
            | T::DesignatorTop => Some(BoardSide::Top),
            T::SilkscreenBottom
            | T::SolderMaskBottom
            | T::PasteBottom
            | T::AdhesiveBottom
            | T::FabBottom
            | T::AssemblyBottom
            | T::CourtyardBottom
            | T::Body3dBottom
            | T::KeepoutBottom
            | T::FiducialBottom
            | T::DesignatorBottom => Some(BoardSide::Bottom),
            T::EdgeCuts | T::Margin | T::Dimensions | T::Notes | T::DrillDrawing => None,
            T::Eco1 | T::Eco2 => None,
        }
    }

    /// Drawing-only layers that never reach manufacturing output.
    pub fn is_documentation(self) -> bool {
        matches!(
            self,
            Self::Dimensions | Self::Notes | Self::DrillDrawing | Self::Eco1 | Self::Eco2
        )
    }
}

/// 1-based index of a user layer, `1..=UserLayer::MAX`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct UserLayer(u8);

impl UserLayer {
    pub const MAX: u8 = 6;

    /// `None` when `index` is outside `1..=MAX`.
    pub fn new(index: u8) -> Option<Self> {
        (1..=Self::MAX).contains(&index).then_some(Self(index))
    }

    pub fn index(self) -> u8 {
        self.0
    }
}

/// A board layer this build knows by name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PcbLayer {
    Copper(CopperLayer),
    Technical(TechnicalLayer),
    User(UserLayer),
}

impl PcbLayer {
    /// Total layers the type can name.
    pub const COUNT: usize =
        CopperLayer::COUNT + TechnicalLayer::ALL.len() + UserLayer::MAX as usize;

    /// Every layer: copper top to bottom, technical, then user.
    pub fn all() -> impl Iterator<Item = PcbLayer> {
        CopperLayer::all()
            .map(Self::Copper)
            .chain(TechnicalLayer::ALL.into_iter().map(Self::Technical))
            .chain((1..=UserLayer::MAX).map(|n| Self::User(UserLayer(n))))
    }

    pub fn is_copper(self) -> bool {
        matches!(self, Self::Copper(_))
    }

    pub fn copper(self) -> Option<CopperLayer> {
        match self {
            Self::Copper(c) => Some(c),
            _ => None,
        }
    }

    /// `None` for inner copper, board-wide and user layers.
    pub fn side(self) -> Option<BoardSide> {
        match self {
            Self::Copper(c) => c.side(),
            Self::Technical(t) => t.side(),
            Self::User(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn index_newtypes_reject_out_of_range() {
        assert_eq!(InnerCopper::new(0), None);
        assert_eq!(InnerCopper::new(1).map(InnerCopper::index), Some(1));
        assert_eq!(InnerCopper::new(30).map(InnerCopper::index), Some(30));
        assert_eq!(InnerCopper::new(31), None);
        assert_eq!(UserLayer::new(0), None);
        assert_eq!(UserLayer::new(6).map(UserLayer::index), Some(6));
        assert_eq!(UserLayer::new(7), None);
    }

    #[test]
    fn all_names_every_layer_once() {
        let all: Vec<_> = PcbLayer::all().collect();
        assert_eq!(all.len(), PcbLayer::COUNT);
        assert_eq!(PcbLayer::COUNT, 32 + 29 + 6);
        assert_eq!(all.iter().collect::<HashSet<_>>().len(), all.len());
        assert_eq!(all.iter().filter(|l| l.is_copper()).count(), 32);
    }

    #[test]
    fn copper_runs_top_to_bottom() {
        let copper: Vec<_> = CopperLayer::all().collect();
        assert_eq!(copper.first(), Some(&CopperLayer::Top));
        assert_eq!(copper.last(), Some(&CopperLayer::Bottom));
        assert!(copper.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn sides_pair_up() {
        let top = PcbLayer::all()
            .filter(|l| l.side() == Some(BoardSide::Top))
            .count();
        let bottom = PcbLayer::all()
            .filter(|l| l.side() == Some(BoardSide::Bottom))
            .count();
        assert_eq!(top, 12);
        assert_eq!(bottom, 12);
        assert_eq!(
            PcbLayer::Copper(CopperLayer::Top).side(),
            Some(BoardSide::Top)
        );
        let inner = CopperLayer::Inner(InnerCopper::new(3).unwrap());
        assert_eq!(PcbLayer::Copper(inner).side(), None);
        assert_eq!(PcbLayer::Technical(TechnicalLayer::EdgeCuts).side(), None);
    }

    #[test]
    fn documentation_layers_are_board_wide() {
        for t in TechnicalLayer::ALL
            .into_iter()
            .filter(|t| t.is_documentation())
        {
            assert_eq!(t.side(), None, "{t:?}");
        }
        assert_eq!(
            TechnicalLayer::ALL
                .into_iter()
                .filter(|t| t.is_documentation())
                .count(),
            5
        );
    }
}
