//! The bake preview of a board sketch (mockup row 179): what Bake writes,
//! baked at the solve the editor already holds, so the preview never solves
//! the sketch a second time. Bake itself still solves (`bake_board`).

use signex_sketch::parameter::resolve;
use signex_sketch::sketch::SketchData;
use signex_sketch::solver::FullSolveOutput;
use signex_types::pcb::PcbBoard;
use signex_types::pcb_sketch::BakedBoard;
use signex_types::sketch::board_attr::SketchBoardItems;
use uuid::Uuid;

use super::{BoardBakeError, Slot, at_solve, baked_with_slots, items};

/// What a board bake writes, for its preview.
#[derive(Debug, Clone, PartialEq)]
pub struct BoardPreviewBake {
    pub baked: BakedBoard,
    pub items: SketchBoardItems,
    /// Each slot among `baked.cutouts`: the cutout's uuid and its size.
    pub slots: Vec<(Uuid, Slot)>,
}

/// Bake `sketch` at `solve` against `board`: its nets, and the pours,
/// drawings and anchors it takes. Without a board (it is closing), the
/// sketch's own geometry only. Pure.
pub fn bake_board_preview(
    sketch: &SketchData,
    solve: &FullSolveOutput,
    board: Option<&PcbBoard>,
) -> Result<BoardPreviewBake, BoardBakeError> {
    let params = resolve(&sketch.parameters).map_err(BoardBakeError::Parameters)?;
    at_solve(sketch, solve, &params, |ctx| {
        let nets = board.map_or(&[][..], |board| board.nets.as_slice());
        let (baked, slots) = baked_with_slots(ctx, nets)?;
        let items = match board {
            Some(board) => items::items(ctx, board)?,
            None => SketchBoardItems::default(),
        };
        Ok(BoardPreviewBake {
            baked,
            items,
            slots,
        })
    })
}

#[cfg(test)]
mod tests {
    use signex_sketch::Solver;
    use signex_sketch::attr::BoardCutoutAttr;
    use signex_sketch::entity::{Entity, EntityKind};
    use signex_sketch::id::SketchEntityId;
    use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
    use signex_types::sketch::board_attr::BoardSlotAttr;

    use super::*;
    use crate::board::{bake_board, bake_board_with_items};

    struct Sketch {
        data: SketchData,
        plane: PlaneId,
    }

    impl Sketch {
        fn new() -> Self {
            let plane = PlaneId::new();
            let mut data = SketchData::default();
            data.planes.push(Plane {
                id: plane,
                kind: PlaneKind::BoardTop,
            });
            Sketch { data, plane }
        }

        fn add(&mut self, kind: EntityKind) -> SketchEntityId {
            let id = SketchEntityId::new();
            self.data.entities.push(Entity::new(id, self.plane, kind));
            id
        }

        fn point(&mut self, x: f64, y: f64) -> SketchEntityId {
            self.add(EntityKind::Point { x, y })
        }

        fn line(&mut self, start: SketchEntityId, end: SketchEntityId) -> SketchEntityId {
            self.add(EntityKind::Line { start, end })
        }

        fn arc(&mut self, center: SketchEntityId, start: SketchEntityId, end: SketchEntityId) {
            self.add(EntityKind::Arc {
                center,
                start,
                end,
                sweep_ccw: true,
            });
        }

        fn rect(&mut self, x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<SketchEntityId> {
            let c = [
                self.point(x0, y0),
                self.point(x1, y0),
                self.point(x1, y1),
                self.point(x0, y1),
            ];
            (0..4).map(|i| self.line(c[i], c[(i + 1) % 4])).collect()
        }

        /// A 12 x 3 slot along X from (`x`, `y`); its bottom line.
        fn slot(&mut self, x: f64, y: f64) -> SketchEntityId {
            let (a, b) = (self.point(x, y), self.point(x + 9.0, y));
            let (c, d) = (self.point(x + 9.0, y + 3.0), self.point(x, y + 3.0));
            let (right, left) = (self.point(x + 9.0, y + 1.5), self.point(x, y + 1.5));
            let bottom = self.line(a, b);
            self.arc(right, b, c);
            self.line(c, d);
            self.arc(left, d, a);
            bottom
        }

        fn get(&mut self, id: SketchEntityId) -> &mut Entity {
            self.data.entities.iter_mut().find(|e| e.id == id).unwrap()
        }

        fn cutout(&mut self, id: SketchEntityId) {
            self.get(id).board_cutout = Some(BoardCutoutAttr {
                edge_radius_expr: None,
                through: true,
            });
        }

        fn solve(&self) -> FullSolveOutput {
            let params = resolve(&self.data.parameters).unwrap();
            Solver::default().solve(&self.data, &params).unwrap()
        }
    }

    /// The preview writes what Bake writes.
    #[test]
    fn the_preview_at_the_solve_is_the_bake() {
        let mut s = Sketch::new();
        let edges = s.rect(0.0, 0.0, 40.0, 30.0);
        s.get(edges[0]).board_outline = true;
        let hole = s.rect(5.0, 5.0, 8.0, 8.0);
        s.cutout(hole[0]);
        let board: PcbBoard =
            serde_json::from_value(serde_json::json!({ "uuid": Uuid::nil() })).expect("a board");
        let preview = bake_board_preview(&s.data, &s.solve(), Some(&board)).unwrap();
        let (baked, items) = bake_board_with_items(&s.data, &board).unwrap();
        assert_eq!((preview.baked, preview.items), (baked, items));
        assert!(preview.slots.is_empty(), "a plain cutout is no slot");
    }

    /// Mockup row 179: the preview reads the solve it is given; a sketch
    /// moved since is not solved again.
    #[test]
    fn the_preview_never_solves_again() {
        let mut s = Sketch::new();
        let edges = s.rect(0.0, 0.0, 40.0, 30.0);
        s.get(edges[0]).board_outline = true;
        let solve = s.solve();
        for entity in &mut s.data.entities {
            if let EntityKind::Point { x, .. } = &mut entity.kind {
                *x += 100.0;
            }
        }
        let preview = bake_board_preview(&s.data, &solve, None).unwrap();
        let outline = preview.baked.outline.expect("the outline bakes");
        assert!(outline.iter().all(|p| p.x <= 40.0 + 1e-9), "{outline:?}");
        let resolved = bake_board(&s.data, &[]).unwrap().outline.unwrap();
        assert!(resolved.iter().any(|p| p.x > 100.0 - 1e-9));
    }

    /// A slot is one of the cutouts, listed once more with its size so the
    /// preview can show it as a slot and not as a cutout too.
    #[test]
    fn a_slot_is_listed_with_its_size_under_its_cutout_uuid() {
        let mut s = Sketch::new();
        let slot = s.slot(10.0, 10.0);
        s.cutout(slot);
        s.get(slot).board_slot = Some(BoardSlotAttr::default());
        let plain = s.rect(30.0, 0.0, 35.0, 5.0);
        s.cutout(plain[0]);
        let preview = bake_board_preview(&s.data, &s.solve(), None).unwrap();
        assert_eq!(preview.baked.cutouts.len(), 2);
        assert_eq!(preview.slots.len(), 1);
        let (uuid, size) = preview.slots[0];
        assert_eq!(uuid, slot.0, "the slot cutout's uuid");
        assert!((size.length - 12.0).abs() < 1e-6, "{size:?}");
        assert!((size.width - 3.0).abs() < 1e-6, "{size:?}");
    }
}
