//! The role every board layer plays in a whole-job export (IPC-2581, and
//! ODB++ next, #2485), keyed by the typed layer id: one table both writers
//! read, so renaming a layer row never changes an output. The mapping:
//! `docs/FABRICATION_OUTPUTS.md`, "Layer roles".

use signex_types::format::{parse_pcb_layer, pcb_layer_token};
use signex_types::pcb::PcbBoard;
use signex_types::pcb_layer::{BoardSide, CopperLayer, PcbLayer, TechnicalLayer};
use signex_types::pcb_stack::{LayerStack, StackLayerKind};

/// What a layer is for in a whole-job export.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LayerRole {
    /// Copper that carries tracks.
    Signal,
    /// Copper the stack pours as a plane.
    Plane,
    Legend,
    SolderMask,
    SolderPaste,
    Glue,
    BoardOutline,
    Assembly,
    Courtyard,
    /// Drawings only: fab, dimensions, notes, drill drawing, every
    /// mechanical layer, and every other technical layer.
    Document,
    /// The holes of one drilled span. No board layer has this role; the
    /// writers add one drill layer per span.
    Drill,
}

impl LayerRole {
    /// The role of `layer`; `plane` is whether the stack pours that copper
    /// as a plane (see [`plane_layers`]).
    pub(crate) fn of(layer: PcbLayer, plane: bool) -> LayerRole {
        use TechnicalLayer as T;
        match layer {
            PcbLayer::Copper(_) if plane => LayerRole::Plane,
            PcbLayer::Copper(_) => LayerRole::Signal,
            PcbLayer::Technical(T::SilkscreenTop | T::SilkscreenBottom) => LayerRole::Legend,
            PcbLayer::Technical(T::SolderMaskTop | T::SolderMaskBottom) => LayerRole::SolderMask,
            PcbLayer::Technical(T::PasteTop | T::PasteBottom) => LayerRole::SolderPaste,
            PcbLayer::Technical(T::AdhesiveTop | T::AdhesiveBottom) => LayerRole::Glue,
            PcbLayer::Technical(T::EdgeCuts) => LayerRole::BoardOutline,
            PcbLayer::Technical(T::AssemblyTop | T::AssemblyBottom) => LayerRole::Assembly,
            PcbLayer::Technical(T::CourtyardTop | T::CourtyardBottom) => LayerRole::Courtyard,
            PcbLayer::Technical(_) | PcbLayer::Mechanical(_) => LayerRole::Document,
        }
    }

    /// The IPC-2581 `layerFunction`.
    pub(crate) fn ipc2581(self) -> &'static str {
        match self {
            LayerRole::Signal => "SIGNAL",
            LayerRole::Plane => "PLANE",
            LayerRole::Legend => "LEGEND",
            LayerRole::SolderMask => "SOLDERMASK",
            LayerRole::SolderPaste => "SOLDERPASTE",
            LayerRole::Glue => "GLUE",
            LayerRole::BoardOutline => "BOARD_OUTLINE",
            LayerRole::Assembly => "ASSEMBLY",
            LayerRole::Courtyard => "COURTYARD",
            LayerRole::Document => "DOCUMENT",
            LayerRole::Drill => "DRILL",
        }
    }

    /// The ODB++ matrix layer type; `None` for the board outline, which is
    /// the step profile there, not a matrix layer.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "the ODB++ writer (#2485) reads it")
    )]
    pub(crate) fn odb_matrix_type(self) -> Option<&'static str> {
        Some(match self {
            LayerRole::Signal => "SIGNAL",
            LayerRole::Plane => "POWER_GROUND",
            LayerRole::Legend => "SILK_SCREEN",
            LayerRole::SolderMask => "SOLDER_MASK",
            LayerRole::SolderPaste => "SOLDER_PASTE",
            LayerRole::Drill => "DRILL",
            LayerRole::BoardOutline => return None,
            LayerRole::Glue | LayerRole::Assembly | LayerRole::Courtyard | LayerRole::Document => {
                "DOCUMENT"
            }
        })
    }
}

/// Where a layer sits in the board.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LayerPlace {
    Top,
    Bottom,
    Inner,
    /// Board-wide: the outline, a drawing layer, a drill span.
    Board,
}

impl LayerPlace {
    pub(crate) fn of(layer: PcbLayer) -> LayerPlace {
        match (layer, layer.side()) {
            (_, Some(BoardSide::Top)) => LayerPlace::Top,
            (_, Some(BoardSide::Bottom)) => LayerPlace::Bottom,
            (PcbLayer::Copper(_), None) => LayerPlace::Inner,
            _ => LayerPlace::Board,
        }
    }
}

/// The name a layer carries in an export: its file token (`TopCopper`,
/// `Inner1Copper`, `Mechanical3`), or for a mechanical layer the name the
/// layer table gives it, when it gives one.
pub(crate) fn layer_label(board: &PcbBoard, layer: PcbLayer) -> String {
    let token = pcb_layer_token(layer);
    if let PcbLayer::Mechanical(_) = layer {
        let named = board
            .layers
            .iter()
            .find(|def| def.name == token)
            .map(|def| def.display_name.trim())
            .filter(|name| !name.is_empty());
        if let Some(name) = named {
            return name.to_string();
        }
    }
    token
}

/// The board's stack: the stored one, or the one derived from its layer
/// table when it has none.
pub(crate) fn board_stack(board: &PcbBoard) -> LayerStack {
    board
        .stackup
        .clone()
        .unwrap_or_else(|| LayerStack::derived(&board.layers, board.thickness))
}

/// The copper layers `stack` pours as planes.
pub(crate) fn plane_layers(stack: &LayerStack) -> Vec<CopperLayer> {
    stack
        .copper()
        .filter(|row| row.kind == StackLayerKind::Plane)
        .filter_map(|row| parse_pcb_layer(&row.layer)?.copper())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::pcb::LayerDef;
    use signex_types::pcb_layer::{InnerCopper, MechanicalLayer};

    fn tech(t: TechnicalLayer) -> PcbLayer {
        PcbLayer::Technical(t)
    }

    #[test]
    fn every_role_maps_the_way_the_plan_table_says() {
        use TechnicalLayer as T;
        let cases = [
            (PcbLayer::Copper(CopperLayer::Top), false, "SIGNAL", Some("SIGNAL")),
            (
                PcbLayer::Copper(CopperLayer::Bottom),
                true,
                "PLANE",
                Some("POWER_GROUND"),
            ),
            (tech(T::SilkscreenBottom), false, "LEGEND", Some("SILK_SCREEN")),
            (tech(T::SolderMaskTop), false, "SOLDERMASK", Some("SOLDER_MASK")),
            (tech(T::PasteBottom), false, "SOLDERPASTE", Some("SOLDER_PASTE")),
            (tech(T::AdhesiveTop), false, "GLUE", Some("DOCUMENT")),
            (tech(T::EdgeCuts), false, "BOARD_OUTLINE", None),
            (tech(T::DrillDrawing), false, "DOCUMENT", Some("DOCUMENT")),
            (tech(T::FabBottom), false, "DOCUMENT", Some("DOCUMENT")),
            (tech(T::AssemblyTop), false, "ASSEMBLY", Some("DOCUMENT")),
            (tech(T::CourtyardBottom), false, "COURTYARD", Some("DOCUMENT")),
            (
                PcbLayer::Mechanical(MechanicalLayer::new(7).unwrap()),
                false,
                "DOCUMENT",
                Some("DOCUMENT"),
            ),
        ];
        for (layer, plane, ipc, odb) in cases {
            let role = LayerRole::of(layer, plane);
            assert_eq!(role.ipc2581(), ipc, "{layer:?}");
            assert_eq!(role.odb_matrix_type(), odb, "{layer:?}");
        }
        assert_eq!(LayerRole::Drill.ipc2581(), "DRILL");
    }

    #[test]
    fn a_mechanical_layer_keeps_its_name_and_the_rest_their_token() {
        let mut board = super::super::tests::fixture::empty_board();
        board.layers.push(LayerDef {
            id: 70,
            name: "Mechanical3".into(),
            layer_type: "user".into(),
            display_name: "Board Fold".into(),
        });
        let mech = |n| PcbLayer::Mechanical(MechanicalLayer::new(n).unwrap());
        assert_eq!(layer_label(&board, mech(3)), "Board Fold");
        assert_eq!(layer_label(&board, mech(4)), "Mechanical4");
        let inner = PcbLayer::Copper(CopperLayer::Inner(InnerCopper::new(1).unwrap()));
        assert_eq!(layer_label(&board, inner), "Inner1Copper");
        assert_eq!(LayerPlace::of(inner), LayerPlace::Inner);
        assert_eq!(LayerPlace::of(mech(3)), LayerPlace::Board);
        assert_eq!(
            LayerPlace::of(tech(TechnicalLayer::PasteBottom)),
            LayerPlace::Bottom
        );
    }

    #[test]
    fn the_planes_come_from_the_stack_and_its_layer_types() {
        let mut board = super::super::tests::fixture::empty_board();
        board.layers[1].layer_type = "plane".into();
        let stack = board_stack(&board);
        assert_eq!(plane_layers(&stack), [CopperLayer::Bottom]);
    }
}
