//! The drill pairs of a board (mockup row 105): one per via kind and span,
//! with its depth through the layer stack, smallest drill and via count.
//! Derived from the board's own vias; nothing here is stored.

use pcb_geom::via_span;
use signex_types::pcb::{PcbBoard, ViaType};
use signex_types::pcb_stack::{LayerStack, board_copper_layers};

use super::via_types::{ViaTypeDef, type_of};

/// One drill pair: every via of `kind` running `span`.
#[derive(Debug, Clone, PartialEq)]
pub struct DrillPair {
    pub kind: ViaType,
    /// The 1-based copper places it runs between, top first.
    pub span: (usize, usize),
    /// Stack thickness from the top of its first copper row to the bottom
    /// of its last, in mm; `None` when the stack lacks one of those rows.
    pub depth_mm: Option<f64>,
    /// The smallest drill of its vias, in mm; `None` with no via.
    pub min_drill: Option<f64>,
    pub vias: usize,
}

impl DrillPair {
    /// Depth over the smallest drill; `None` without both.
    pub fn aspect_ratio(&self) -> Option<f64> {
        let drill = self.min_drill.filter(|d| *d > 0.0)?;
        Some(self.depth_mm? / drill)
    }
}

/// The thickness of `stack` from copper place `from` through `to`, 1-based
/// and inclusive.
pub fn span_depth_mm(stack: &LayerStack, (from, to): (usize, usize)) -> Option<f64> {
    let copper_row = |place: usize| {
        let index = place.checked_sub(1)?;
        stack
            .layers
            .iter()
            .enumerate()
            .filter(|(_, layer)| layer.kind.is_copper())
            .nth(index)
            .map(|(row, _)| row)
    };
    let (first, last) = (copper_row(from)?, copper_row(to)?);
    let rows = stack.layers.get(first..=last)?;
    Some(rows.iter().map(|layer| layer.thickness_mm).sum())
}

/// The board's drill pairs, widest span first. The through pair is always
/// listed, as the Via tool places it.
pub fn drill_pairs(board: &PcbBoard, stack: &LayerStack) -> Vec<DrillPair> {
    let copper = board_copper_layers(&board.layers);
    let through = (1, copper.len());
    let mut pairs = vec![DrillPair {
        kind: ViaType::Through,
        span: through,
        depth_mm: span_depth_mm(stack, through),
        min_drill: None,
        vias: 0,
    }];
    for via in &board.vias {
        let Some(span) = via_span(&copper, via) else {
            continue;
        };
        let at = pairs
            .iter()
            .position(|p| p.kind == via.via_type && p.span == span);
        let pair = match at {
            Some(at) => &mut pairs[at],
            None => {
                pairs.push(DrillPair {
                    kind: via.via_type,
                    span,
                    depth_mm: span_depth_mm(stack, span),
                    min_drill: None,
                    vias: 0,
                });
                let last = pairs.len() - 1;
                &mut pairs[last]
            }
        };
        pair.vias += 1;
        pair.min_drill = Some(pair.min_drill.map_or(via.drill, |d| d.min(via.drill)));
    }
    pairs.sort_by_key(|p| (p.span.0, std::cmp::Reverse(p.span.1)));
    pairs
}

/// How many of the board's vias are of each of `types`, in their order.
pub fn via_type_counts(board: &PcbBoard, types: &[ViaTypeDef]) -> Vec<usize> {
    let mut counts = vec![0; types.len()];
    for via in &board.vias {
        if let Some(index) = type_of(types, via) {
            counts[index] += 1;
        }
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pcb::via_types::via_types;

    fn board() -> PcbBoard {
        let via = |id: u128, kind: &str, layers: [&str; 2], drill: f64| {
            serde_json::json!({
                "uuid": uuid::Uuid::from_u128(id),
                "position": {"x": 0.0, "y": 0.0},
                "diameter": 0.6,
                "drill": drill,
                "via_type": kind,
                "layers": layers,
            })
        };
        serde_json::from_value(serde_json::json!({
            "uuid": uuid::Uuid::from_u128(1),
            "thickness": 1.6,
            "layers": [
                {"id": 0, "name": "TopCopper", "layer_type": "signal"},
                {"id": 2, "name": "Inner1Copper", "layer_type": "signal"},
                {"id": 3, "name": "Inner2Copper", "layer_type": "signal"},
                {"id": 31, "name": "BottomCopper", "layer_type": "signal"},
            ],
            "vias": [
                via(2, "through", ["TopCopper", "BottomCopper"], 0.3),
                via(3, "through", ["TopCopper", "BottomCopper"], 0.25),
                via(4, "blind", ["TopCopper", "Inner1Copper"], 0.1),
            ],
        }))
        .unwrap()
    }

    #[test]
    fn pairs_group_vias_by_kind_and_span_with_their_smallest_drill() {
        let board = board();
        let stack = LayerStack::derived(&board.layers, board.thickness);
        let pairs = drill_pairs(&board, &stack);
        assert_eq!(pairs.len(), 2);
        let through = &pairs[0];
        assert_eq!((through.kind, through.span), (ViaType::Through, (1, 4)));
        assert_eq!((through.vias, through.min_drill), (2, Some(0.25)));
        let blind = &pairs[1];
        assert_eq!((blind.kind, blind.span), (ViaType::Blind, (1, 2)));
        assert_eq!(blind.vias, 1);
    }

    #[test]
    fn depth_runs_from_the_first_copper_row_through_the_last() {
        let board = board();
        let stack = LayerStack::derived(&board.layers, board.thickness);
        let pairs = drill_pairs(&board, &stack);
        let mask = 2.0 * signex_types::pcb_stack::DEFAULT_MASK_MM;
        let depth = pairs[0].depth_mm.unwrap();
        assert!((depth - (1.6 - mask)).abs() < 1e-9, "{depth}");
        let ratio = pairs[0].aspect_ratio().unwrap();
        assert!((ratio - depth / 0.25).abs() < 1e-9);
    }

    #[test]
    fn a_board_with_no_via_still_lists_the_through_pair() {
        let mut board = board();
        board.vias.clear();
        let stack = LayerStack::derived(&board.layers, board.thickness);
        let pairs = drill_pairs(&board, &stack);
        assert_eq!(pairs.len(), 1);
        assert_eq!((pairs[0].vias, pairs[0].min_drill), (0, None));
        assert_eq!(pairs[0].aspect_ratio(), None);
    }

    #[test]
    fn every_via_is_counted_under_its_type() {
        let board = board();
        let types = via_types(&board);
        let counts = via_type_counts(&board, &types);
        assert_eq!(counts.iter().sum::<usize>(), board.vias.len());
    }
}
