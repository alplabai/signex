//! Board pours, mechanical drawings and component anchors of a board sketch
//! (decision 2, Caner 2026-11-20). Unlike the other targets they bake into
//! the board's own lists: a pour is a zone, a mechanical drawing a board
//! drawing, an anchor a footprint position. A zone takes the uuid of its
//! seed edge and a drawing that of its edge, so the board engine tells the
//! sketch's own items from the rest. See `docs/PCB_SKETCH.md`, "Targets".

use signex_sketch::baked_kinds::BakedKind;
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::sketch::SketchData;
use signex_types::format::{UnknownFields, pcb_layer_token};
use signex_types::pcb::{
    BoardGraphic, Footprint, NetDef, PCB_DEFAULT_GRAPHIC_WIDTH_MM, PcbBoard, Point, Zone,
};
use signex_types::pcb_graphic::BoardGraphicKind;
use signex_types::pcb_layer::PcbLayer;
use signex_types::pcb_sketch::BakedBoard;
use signex_types::sketch::board_attr::{BakedAnchor, SketchBoardItems};
use signex_types::zone_pour::POUR_FILL;

use super::edge_chains::EdgeChains;
use super::features::{expression, finite, named_net};
use super::geometry::{boundary, circle, point};
use super::patterned::{boundary_places, own_sources, places, refuse_pattern};
use super::{BoardBakeError, Context, baked, solved};
use crate::eval_util::{build_ctx, opt_eval_mm};
use crate::traced_loops::TracedLoops;

/// Bake the pours, mechanical drawings and component anchors of `sketch`
/// against `board`: its net table names a pour's net, its footprints are
/// what an anchor moves. Pure, like `bake_board`.
pub fn bake_board_items(
    sketch: &SketchData,
    board: &PcbBoard,
) -> Result<SketchBoardItems, BoardBakeError> {
    solved(sketch, |ctx| items(ctx, board))
}

/// `bake_board` and [`bake_board_items`] from one solve.
pub fn bake_board_with_items(
    sketch: &SketchData,
    board: &PcbBoard,
) -> Result<(BakedBoard, SketchBoardItems), BoardBakeError> {
    solved(sketch, |ctx| {
        Ok((baked(ctx, &board.nets)?, items(ctx, board)?))
    })
}

/// How many items of `kind` a bake wrote, for the bake preview; `None` for
/// a kind these items never hold (`baked_count` answers for those).
pub fn baked_item_count(items: &SketchBoardItems, kind: BakedKind) -> Option<usize> {
    items.count(kind)
}

pub(super) fn items(
    ctx: &Context<'_>,
    board: &PcbBoard,
) -> Result<SketchBoardItems, BoardBakeError> {
    Ok(SketchBoardItems {
        zones: pours(ctx, &board.nets)?,
        graphics: mechanical(ctx)?,
        anchors: anchors(ctx, &board.footprints)?,
    })
}

/// A pour zone for each closed profile tagged `pour`, on its copper layer
/// and named net, with the pour's priority and thermal settings.
fn pours(ctx: &Context<'_>, nets: &[NetDef]) -> Result<Vec<Zone>, BoardBakeError> {
    let eval = build_ctx(ctx.sketch, ctx.params);
    let mut loops = TracedLoops::default();
    let mut out = Vec::new();
    for entity in ctx.tagged(|e| e.pour.is_some()) {
        let Some(attr) = entity.pour.as_ref() else {
            continue;
        };
        if !attr.layer.is_copper() {
            return Err(BoardBakeError::PourNotCopper { entity: entity.id });
        }
        let attribute = format!("{attr:?}");
        let Some(outline) = boundary(ctx, &mut loops, &attribute, entity, BakedKind::BoardPours)?
        else {
            continue;
        };
        let net = named_net(entity.id, attr.net.as_deref(), nets, |entity, net| {
            BoardBakeError::PourUnknownNet { entity, net }
        })?;
        let length = |expr: &Option<String>, what| {
            opt_eval_mm(expr, &eval)
                .and_then(|v| finite(v.unwrap_or(0.0)))
                .map(|v| v.max(0.0))
                .map_err(|message| expression(entity.id, what, message))
        };
        let thermal = &attr.thermal_relief;
        let thermal_gap = length(&Some(thermal.gap_expr.clone()), "pour thermal gap")?;
        let thermal_width = length(
            &Some(thermal.spoke_width_expr.clone()),
            "pour thermal spoke width",
        )?;
        let zone = Zone {
            uuid: entity.id.0,
            net: net.map_or(0, |n| n.number),
            net_name: net.map(|n| n.name.clone()).unwrap_or_default(),
            layer: pcb_layer_token(attr.layer),
            outline: Vec::new(),
            holes: Vec::new(),
            priority: attr.priority,
            fill_type: POUR_FILL.to_string(),
            thermal_relief: thermal.enabled,
            thermal_gap,
            thermal_width,
            // 0: the board's clearance (`Zone::pour_clearance_mm`).
            clearance: length(&attr.clearance_expr, "pour clearance")?,
            min_thickness: length(&attr.min_thickness_expr, "pour minimum width")?,
            min_island_area: 0.0,
            remove_dead_islands: false,
            // A sketch pour bakes live copper; shelving and unfilling are
            // board edits.
            shelved: false,
            unfilled: false,
            corners: Default::default(),
            corner_size: 0.0,
            fill_mode: Default::default(),
            hatch_width: 0.0,
            hatch_gap: 0.0,
            unknown: UnknownFields::default(),
        };
        let kind = BakedKind::BoardPours;
        for place in boundary_places(ctx, entity, &outline, kind, "pour pattern")? {
            out.push(Zone {
                uuid: place.uuid,
                outline: place.ring(&outline),
                ..zone.clone()
            });
        }
    }
    Ok(out)
}

/// A mechanical edge at its solved place.
#[derive(Clone, Copy)]
struct Drawn {
    kind: BoardGraphicKind,
    start: Point,
    end: Point,
    center: Option<Point>,
    radius: f64,
}

impl Drawn {
    /// The points that say where it is; a circle turned about its centre
    /// stays the same circle.
    fn shape(&self) -> Vec<Point> {
        match (self.kind, self.center) {
            (BoardGraphicKind::Circle, Some(c)) => vec![c],
            (_, Some(c)) => vec![self.start, self.end, c],
            (_, None) => vec![self.start, self.end],
        }
    }
}

fn drawn(ctx: &Context<'_>, entity: &Entity) -> Result<Drawn, BoardBakeError> {
    const KIND: BakedKind = BakedKind::Mechanical;
    Ok(match entity.kind {
        EntityKind::Line { start, end } => Drawn {
            kind: BoardGraphicKind::Line,
            start: point(ctx, start, KIND)?,
            end: point(ctx, end, KIND)?,
            center: None,
            radius: 0.0,
        },
        // A board arc runs counter-clockwise from `start` to `end`.
        EntityKind::Arc {
            center,
            start,
            end,
            sweep_ccw,
        } => {
            let (from, to) = if sweep_ccw {
                (start, end)
            } else {
                (end, start)
            };
            let (c, from) = (point(ctx, center, KIND)?, point(ctx, from, KIND)?);
            Drawn {
                kind: BoardGraphicKind::Arc,
                start: from,
                end: point(ctx, to, KIND)?,
                center: Some(c),
                radius: (from.x - c.x).hypot(from.y - c.y),
            }
        }
        EntityKind::Circle { .. } => {
            let (c, radius) = circle(ctx, entity, KIND)?;
            let through = Point {
                x: c.x + radius,
                y: c.y,
            };
            Drawn {
                kind: BoardGraphicKind::Circle,
                start: through,
                end: through,
                center: Some(c),
                radius,
            }
        }
        EntityKind::Point { .. } => {
            return Err(BoardBakeError::NotAnEdge {
                kind: KIND,
                entity: entity.id,
            });
        }
    })
}

/// What a pattern of a connected mechanical shape names, and the points that
/// say where the whole shape is.
fn chain_reach(
    ctx: &Context<'_>,
    chain: &[&Entity],
) -> Result<(Vec<SketchEntityId>, Vec<Point>), BoardBakeError> {
    let (mut sources, mut shape) = (Vec::new(), Vec::new());
    for edge in chain {
        sources.extend(own_sources(edge));
        shape.extend(drawn(ctx, edge)?.shape());
    }
    Ok((sources, shape))
}

/// A board drawing on its mechanical layer for each line, arc or circle
/// tagged `mechanical`, at each place of the connected shape it belongs to.
fn mechanical(ctx: &Context<'_>) -> Result<Vec<BoardGraphic>, BoardBakeError> {
    let chains = EdgeChains::new(ctx.tagged(|e| e.mechanical.is_some()));
    let reaches = chains
        .chains()
        .iter()
        .map(|chain| chain_reach(ctx, chain))
        .collect::<Result<Vec<_>, _>>()?;
    let mut out = Vec::new();
    for entity in ctx.tagged(|e| e.mechanical.is_some()) {
        let Some(attr) = entity.mechanical else {
            continue;
        };
        let edge = drawn(ctx, entity)?;
        let (sources, shape) = &reaches[chains.index_of(entity.id)];
        for place in places(ctx, entity, sources, shape, "mechanical pattern")? {
            // A reflection turns an arc clockwise: swap its ends to keep it
            // counter-clockwise.
            let flip = place.mirrors() && edge.kind == BoardGraphicKind::Arc;
            let (start, end) = if flip {
                (edge.end, edge.start)
            } else {
                (edge.start, edge.end)
            };
            out.push(BoardGraphic {
                uuid: place.uuid,
                graphic_type: edge.kind.token().to_string(),
                layer: pcb_layer_token(PcbLayer::Mechanical(attr.layer)),
                width: PCB_DEFAULT_GRAPHIC_WIDTH_MM,
                start: (edge.kind != BoardGraphicKind::Circle).then(|| place.at(start)),
                end: Some(place.at(end)),
                center: edge.center.map(|c| place.at(c)),
                radius: edge.radius,
                points: Vec::new(),
                text: String::new(),
                font_size: 0.0,
            });
        }
    }
    Ok(out)
}

/// Each point tagged `component_anchor`: the footprint it names and the
/// solved place its origin goes to.
fn anchors(
    ctx: &Context<'_>,
    footprints: &[Footprint],
) -> Result<Vec<BakedAnchor>, BoardBakeError> {
    let mut out = Vec::new();
    for entity in ctx.tagged(|e| e.component_anchor.is_some()) {
        let Some(attr) = entity.component_anchor else {
            continue;
        };
        if !matches!(entity.kind, EntityKind::Point { .. }) {
            return Err(BoardBakeError::AnchorNotAPoint { entity: entity.id });
        }
        refuse_pattern(ctx, entity, BakedKind::ComponentAnchors)?;
        let Some(wanted) = attr.footprint else {
            return Err(BoardBakeError::AnchorWithoutComponent { entity: entity.id });
        };
        let footprint = footprints.iter().find(|fp| fp.uuid == wanted).ok_or(
            BoardBakeError::UnknownComponent {
                entity: entity.id,
                footprint: wanted,
            },
        )?;
        out.push(BakedAnchor {
            entity: entity.id.0,
            footprint: footprint.uuid,
            reference: footprint.reference.clone(),
            position: point(ctx, entity.id, BakedKind::ComponentAnchors)?,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use signex_sketch::attr::{BoardCutoutAttr, PourAttr, ThermalRelief};
    use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
    use signex_types::pcb_layer::{CopperLayer, MechanicalLayer, TechnicalLayer};
    use signex_types::sketch::board_attr::{BoardSlotAttr, ComponentAnchorAttr, MechanicalAttr};
    use uuid::Uuid;

    use super::*;
    use crate::board::bake_board;

    const U1: Uuid = Uuid::from_u128(0x0001_0001);

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

        fn arc(
            &mut self,
            center: SketchEntityId,
            start: SketchEntityId,
            end: SketchEntityId,
        ) -> SketchEntityId {
            self.add(EntityKind::Arc {
                center,
                start,
                end,
                sweep_ccw: true,
            })
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

        /// A 12 x 3 slot along X from (`x`, `y`): bottom line, right arc,
        /// top line, left arc.
        fn slot(&mut self, x: f64, y: f64) -> Vec<SketchEntityId> {
            let (a, b) = (self.point(x, y), self.point(x + 9.0, y));
            let (c, d) = (self.point(x + 9.0, y + 3.0), self.point(x, y + 3.0));
            let (right, left) = (self.point(x + 9.0, y + 1.5), self.point(x, y + 1.5));
            vec![
                self.line(a, b),
                self.arc(right, b, c),
                self.line(c, d),
                self.arc(left, d, a),
            ]
        }

        fn get(&mut self, id: SketchEntityId) -> &mut Entity {
            self.data.entities.iter_mut().find(|e| e.id == id).unwrap()
        }

        fn items(&self) -> Result<SketchBoardItems, BoardBakeError> {
            bake_board_items(&self.data, &board())
        }
    }

    /// A board with nets VCC (1) and GND (4) and one placed U1 at the origin.
    fn board() -> PcbBoard {
        serde_json::from_value(serde_json::json!({
            "uuid": Uuid::nil(),
            "nets": [{"number": 1, "name": "VCC"}, {"number": 4, "name": "GND"}],
            "footprints": [{
                "uuid": U1,
                "reference": "U1",
                "position": {"x": 0.0, "y": 0.0},
                "layer": "TopCopper",
            }],
        }))
        .expect("a board")
    }

    fn pour(layer: PcbLayer, net: Option<&str>) -> PourAttr {
        PourAttr {
            layer,
            net: net.map(str::to_string),
            fill_type: Default::default(),
            thermal_relief: ThermalRelief::default(),
            clearance_expr: Some("0.3mm".into()),
            min_thickness_expr: None,
            priority: 3,
            corners: Default::default(),
            corner_size: 0.0,
        }
    }

    #[test]
    fn a_pour_is_a_board_zone_under_its_seed_uuid_on_its_layer_and_net() {
        let mut s = Sketch::new();
        let edges = s.rect(0.0, 0.0, 10.0, 5.0);
        for &edge in &edges {
            let attr = pour(PcbLayer::Copper(CopperLayer::Bottom), Some("GND"));
            s.get(edge).pour = Some(attr);
        }
        let zones = s.items().unwrap().zones;
        assert_eq!(zones.len(), 1, "one loop, one zone");
        let zone = &zones[0];
        assert_eq!(zone.uuid, edges[0].0, "the seed edge's uuid");
        assert_eq!(zone.layer, "BottomCopper");
        assert_eq!((zone.net, zone.net_name.as_str()), (4, "GND"));
        assert!(zone.is_pour());
        assert_eq!(zone.priority, 3);
        assert!((zone.clearance - 0.3).abs() < 1e-12);
        assert!((zone.thermal_gap - 0.508).abs() < 1e-12);
        assert_eq!(zone.outline.len(), 4);
    }

    #[test]
    fn a_pour_without_a_net_is_net_zero() {
        let mut s = Sketch::new();
        let edges = s.rect(0.0, 0.0, 10.0, 5.0);
        s.get(edges[2]).pour = Some(pour(PcbLayer::Copper(CopperLayer::Top), None));
        let zone = &s.items().unwrap().zones[0];
        assert_eq!((zone.net, zone.net_name.as_str()), (0, ""));
        assert_eq!(zone.layer, "TopCopper");
    }

    #[test]
    fn a_pour_naming_a_net_the_board_lacks_is_refused_by_name() {
        let mut s = Sketch::new();
        let edges = s.rect(0.0, 0.0, 10.0, 5.0);
        s.get(edges[0]).pour = Some(pour(PcbLayer::Copper(CopperLayer::Top), Some("AGND")));
        match s.items() {
            Err(BoardBakeError::PourUnknownNet { entity, net }) => {
                assert_eq!((entity, net.as_str()), (edges[0], "AGND"));
            }
            other => panic!("expected an unknown net, got {other:?}"),
        }
    }

    #[test]
    fn a_pour_off_copper_is_refused() {
        let mut s = Sketch::new();
        let edges = s.rect(0.0, 0.0, 10.0, 5.0);
        let silk = PcbLayer::Technical(TechnicalLayer::SilkscreenTop);
        s.get(edges[0]).pour = Some(pour(silk, None));
        let error = s.items().unwrap_err();
        assert!(matches!(error, BoardBakeError::PourNotCopper { entity } if entity == edges[0]));
        assert_eq!(error.place(), Some(edges[0]));
    }

    #[test]
    fn a_mechanical_edge_is_a_drawing_on_its_layer_under_its_uuid() {
        let mut s = Sketch::new();
        let (a, b) = (s.point(0.0, 0.0), s.point(20.0, 0.0));
        let line = s.line(a, b);
        let layer = MechanicalLayer::new(3).unwrap();
        s.get(line).mechanical = Some(MechanicalAttr { layer });
        let graphics = s.items().unwrap().graphics;
        assert_eq!(graphics.len(), 1);
        let drawing = &graphics[0];
        assert_eq!(drawing.uuid, line.0);
        assert_eq!(drawing.layer, "Mechanical3");
        assert_eq!(drawing.kind(), Some(BoardGraphicKind::Line));
        assert_eq!(drawing.start, Some(Point { x: 0.0, y: 0.0 }));
        assert_eq!(drawing.end, Some(Point { x: 20.0, y: 0.0 }));
    }

    #[test]
    fn a_clockwise_mechanical_arc_is_stored_counter_clockwise() {
        let mut s = Sketch::new();
        let (c, a, b) = (s.point(0.0, 0.0), s.point(5.0, 0.0), s.point(0.0, 5.0));
        let arc = s.add(EntityKind::Arc {
            center: c,
            start: a,
            end: b,
            sweep_ccw: false,
        });
        s.get(arc).mechanical = Some(MechanicalAttr::default());
        let drawing = &s.items().unwrap().graphics[0];
        assert_eq!(drawing.kind(), Some(BoardGraphicKind::Arc));
        assert_eq!(drawing.layer, "Mechanical1");
        assert_eq!(drawing.start, Some(Point { x: 0.0, y: 5.0 }));
        assert_eq!(drawing.end, Some(Point { x: 5.0, y: 0.0 }));
        assert_eq!(drawing.center, Some(Point { x: 0.0, y: 0.0 }));
        assert!((drawing.radius - 5.0).abs() < 1e-12);
    }

    #[test]
    fn an_anchor_moves_its_component_to_the_solved_point() {
        let mut s = Sketch::new();
        let at = s.point(12.5, 7.0);
        s.get(at).component_anchor = Some(ComponentAnchorAttr {
            footprint: Some(U1),
        });
        let anchors = s.items().unwrap().anchors;
        assert_eq!(
            anchors,
            vec![BakedAnchor {
                entity: at.0,
                footprint: U1,
                reference: "U1".into(),
                position: Point { x: 12.5, y: 7.0 },
            }]
        );
    }

    #[test]
    fn an_anchor_naming_a_component_the_board_lacks_is_refused() {
        let mut s = Sketch::new();
        let at = s.point(1.0, 1.0);
        let gone = Uuid::from_u128(0xdead);
        s.get(at).component_anchor = Some(ComponentAnchorAttr {
            footprint: Some(gone),
        });
        match s.items() {
            Err(BoardBakeError::UnknownComponent { entity, footprint }) => {
                assert_eq!((entity, footprint), (at, gone));
            }
            other => panic!("expected an unknown component, got {other:?}"),
        }
    }

    #[test]
    fn a_construction_anchor_does_not_bake() {
        let mut s = Sketch::new();
        let at = s.point(1.0, 1.0);
        s.get(at).component_anchor = Some(ComponentAnchorAttr {
            footprint: Some(U1),
        });
        s.get(at).construction = true;
        assert!(s.items().unwrap().anchors.is_empty());
    }

    /// The role is set before a component is picked: the bake says so.
    #[test]
    fn an_anchor_without_a_component_is_refused_by_name() {
        let mut s = Sketch::new();
        let at = s.point(1.0, 1.0);
        s.get(at).component_anchor = Some(ComponentAnchorAttr::default());
        let error = s.items().unwrap_err();
        assert!(matches!(error, BoardBakeError::AnchorWithoutComponent { entity } if entity == at));
        assert_eq!(error.place(), Some(at));
    }

    #[test]
    fn one_solve_gives_what_the_two_bakes_give() {
        let mut s = Sketch::new();
        let edges = s.rect(0.0, 0.0, 10.0, 5.0);
        s.get(edges[0]).pour = Some(pour(PcbLayer::Copper(CopperLayer::Top), Some("VCC")));
        s.get(edges[1]).board_outline = true;
        let (baked, items) = bake_board_with_items(&s.data, &board()).unwrap();
        assert_eq!(baked, bake_board(&s.data, &board().nets).unwrap());
        assert_eq!(items, s.items().unwrap());
        assert_eq!(baked_item_count(&items, BakedKind::BoardPours), Some(1));
        assert_eq!(baked_item_count(&items, BakedKind::Mechanical), Some(0));
        assert_eq!(baked_item_count(&items, BakedKind::Cutouts), None);
    }

    #[test]
    fn a_slot_bakes_as_a_cutout_routed_at_its_own_radius() {
        let mut s = Sketch::new();
        let edges = s.slot(10.0, 10.0);
        s.get(edges[0]).board_cutout = Some(BoardCutoutAttr {
            edge_radius_expr: None,
            through: true,
        });
        s.get(edges[0]).board_slot = Some(BoardSlotAttr::default());
        let cutouts = bake_board(&s.data, &[]).unwrap().cutouts;
        assert_eq!(cutouts.len(), 1);
        assert_eq!(cutouts[0].uuid, edges[0].0);
        assert!((cutouts[0].edge_radius - 1.5).abs() < 1e-9);
        assert!(cutouts[0].boundary.len() > 4, "the arcs are tessellated");
    }

    #[test]
    fn a_slot_that_is_not_a_slot_is_refused_by_name() {
        let mut s = Sketch::new();
        let edges = s.rect(0.0, 0.0, 12.0, 3.0);
        s.get(edges[1]).board_cutout = Some(BoardCutoutAttr {
            edge_radius_expr: None,
            through: true,
        });
        s.get(edges[1]).board_slot = Some(BoardSlotAttr::default());
        let error = bake_board(&s.data, &[]).unwrap_err();
        assert!(
            matches!(error, BoardBakeError::NotASlot { entity } if entity == edges[1]),
            "{error:?}"
        );
        assert_eq!(error.place(), Some(edges[1]));
    }

    /// A plain cutout keeps its own edge radius and any profile: only
    /// the Slot role checks the profile.
    #[test]
    fn a_plain_cutout_is_not_checked_as_a_slot() {
        let mut s = Sketch::new();
        let edges = s.rect(0.0, 0.0, 12.0, 3.0);
        s.get(edges[0]).board_cutout = Some(BoardCutoutAttr {
            edge_radius_expr: None,
            through: true,
        });
        assert_eq!(
            bake_board(&s.data, &[]).unwrap().cutouts[0].edge_radius,
            0.0
        );
    }
}
