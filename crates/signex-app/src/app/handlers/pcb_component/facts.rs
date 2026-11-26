//! The Component view's facts, copied from the board and the selection.

use std::path::Path;

use pcb_geom::{ItemRef, PointNm, Shape, nm_to_mm};
use signex_engine::pcb::component;
use signex_engine::pcb::designators::FootprintTextField;
use signex_types::format::parse_pcb_layer;
use signex_types::pcb::{Footprint, FpGraphic, PcbBoard, Point, SymbolLink};
use signex_types::pcb_layer::{PcbLayer, TechnicalLayer};

use crate::app::Signex;
use crate::panels::pcb_component::{ComponentDrawing, PadOutline};
use crate::panels::{
    ComponentSettingFacts, FootprintLibrary, ParameterFacts, PartIdentity, PcbComponentFacts,
    Shared,
};

/// Points per quarter turn of a round pad edge in the illustration.
const PAD_ARC_STEPS: usize = 4;

fn mm(p: PointNm) -> Point {
    Point::new(nm_to_mm(p.x), nm_to_mm(p.y))
}

/// Whether `graphic` draws the part's body: a fabrication or assembly
/// drawing.
fn is_body_outline(graphic: &FpGraphic) -> bool {
    matches!(
        parse_pcb_layer(&graphic.layer),
        Some(PcbLayer::Technical(
            TechnicalLayer::FabTop
                | TechnicalLayer::FabBottom
                | TechnicalLayer::AssemblyTop
                | TechnicalLayer::AssemblyBottom
        ))
    )
}

/// The illustration's drawing of `footprint`: placed at the origin and not
/// turned, so the drawing shows no rotation, but on its own side, so a
/// bottom-side part is drawn mirrored. Pads, drawings and pin 1 go through
/// the same `pcb_geom` placement the canvas draws them by; a pad or drawing
/// the geometry refuses is left out.
fn drawing(footprint: &Footprint) -> ComponentDrawing {
    let mut frame = footprint.clone();
    frame.position = Point::new(0.0, 0.0);
    frame.rotation = 0.0;
    let pads = frame
        .pads
        .iter()
        .filter_map(|pad| match pcb_geom::pad_shape(&frame, pad).ok()? {
            Shape::Disc { center, radius } => Some(PadOutline::Disc {
                center: mm(center),
                radius: nm_to_mm(radius),
            }),
            shape => Some(PadOutline::Ring(
                shape.ring(PAD_ARC_STEPS).into_iter().map(mm).collect(),
            )),
        })
        .collect();
    let pin_one = frame
        .pads
        .iter()
        .find(|pad| pad.number.trim() == "1")
        .and_then(|pad| pcb_geom::pad_world_center(&frame, pad).ok())
        .map(mm);
    let strokes = |keep: fn(&FpGraphic) -> bool| -> Vec<(Point, Point)> {
        frame
            .graphics
            .iter()
            .filter(|graphic| keep(graphic))
            .filter_map(|graphic| pcb_geom::footprint_graphic_placement(&frame, graphic).ok()?)
            .flat_map(|placed| pcb_geom::graphic_strokes(&placed))
            .filter_map(|stroke| match stroke {
                Shape::Capsule { a, b, .. } => Some((mm(a), mm(b))),
                _ => None,
            })
            .collect()
    };
    ComponentDrawing {
        side: component::side_of(footprint),
        pads,
        pin_one,
        courtyard: strokes(pcb_geom::is_courtyard),
        outline: strokes(is_body_outline),
    }
}

/// The value every footprint gives, or mixed. Only called on a non-empty
/// list, where it always has an answer.
fn shared<T: PartialEq>(footprints: &[&Footprint], read: impl Fn(&Footprint) -> T) -> Shared<T> {
    Shared::of(footprints.iter().map(|fp| read(fp))).unwrap_or(Shared::Mixed)
}

/// Whether every footprint has the text, then whether it shows.
fn text_shown(footprints: &[&Footprint], field: FootprintTextField) -> Option<Shared<bool>> {
    let shown: Option<Vec<bool>> = footprints
        .iter()
        .map(|fp| component::text_shown(fp, field))
        .collect();
    Shared::of(shown?)
}

/// The sheet path and the part's unit uuids, `/` joined: where the link
/// points in the schematic (#1312).
fn hierarchical_path(link: &SymbolLink) -> String {
    let sheet = link.sheet.to_string();
    let base = sheet.trim_end_matches('/');
    let units: Vec<String> = link.units.iter().map(ToString::to_string).collect();
    format!("{base}/{}", units.join(","))
}

/// The component settings the footprints share (#2365).
fn settings(footprints: &[&Footprint]) -> ComponentSettingFacts {
    ComponentSettingFacts {
        component_type: shared(footprints, |fp| fp.component.component_type),
        height_mm: shared(footprints, |fp| fp.component.height_mm),
        body_opacity: shared(footprints, |fp| fp.component.body_opacity),
        lock_primitives: shared(footprints, |fp| fp.component.lock_primitives),
        lock_strings: shared(footprints, |fp| fp.component.lock_strings),
        pin_swapping: shared(footprints, |fp| fp.component.pin_swapping),
        part_swapping: shared(footprints, |fp| fp.component.part_swapping),
        channel_offset: shared(footprints, |fp| fp.component.channel_offset),
        description: shared(footprints, |fp| fp.component.description.clone()),
    }
}

/// The parameters every footprint has, in the first one's order.
fn parameters(footprints: &[&Footprint]) -> Vec<ParameterFacts> {
    let Some(first) = footprints.first() else {
        return Vec::new();
    };
    component::parameter_keys(first)
        .into_iter()
        .filter_map(|key| {
            let values: Option<Vec<String>> = footprints
                .iter()
                .map(|fp| component::parameter(fp, key).map(|p| p.value.clone()))
                .collect();
            Some(ParameterFacts {
                key: key.to_string(),
                value: Shared::of(values?)?,
            })
        })
        .collect()
}

impl Signex {
    /// The Component view's facts of the footprints selected on the board at
    /// `path`, when the selection is footprints only.
    pub(crate) fn pcb_component_facts(
        &self,
        path: &Path,
        board: &PcbBoard,
    ) -> Option<PcbComponentFacts> {
        let selection = &self.interaction_state.pcb_selection;
        if selection.document() != Some(path) {
            return None;
        }
        let mut ids = Vec::new();
        for item in selection.selection().items() {
            let ItemRef::Footprint(id) = item else {
                return None;
            };
            ids.push(id);
        }
        ids.sort_unstable();
        ids.dedup();
        let footprints: Vec<&Footprint> = ids
            .iter()
            .filter_map(|id| board.footprints.iter().find(|fp| fp.uuid == *id))
            .collect();
        if footprints.is_empty() || footprints.len() != ids.len() {
            return None;
        }
        let fps = footprints.as_slice();
        let state = &self.interaction_state.pcb_component;
        let drafts = state
            .draft
            .as_ref()
            .filter(|d| d.is_for(path, &ids))
            .map(|d| d.texts.clone())
            .unwrap_or_default();
        let identities: Vec<PartIdentity> =
            fps.iter().map(|fp| self.part_identity(path, fp)).collect();
        let designator_mismatches = fps
            .iter()
            .zip(&identities)
            .filter(|(fp, identity)| {
                matches!(identity, PartIdentity::Read { designator, .. } if *designator != fp.reference.trim())
            })
            .count();
        Some(PcbComponentFacts {
            settings: settings(fps),
            identity: Shared::of(identities).unwrap_or(Shared::Mixed),
            designator_mismatches,
            board: path.to_path_buf(),
            // Shown from the board origin, Y up (#2384).
            x: shared(fps, |fp| board.options.origin.user_x(fp.position.x)),
            y: shared(fps, |fp| board.options.origin.user_y(fp.position.y)),
            rotation: shared(fps, |fp| fp.rotation),
            locked: shared(fps, |fp| fp.locked),
            side: shared(fps, component::side_of),
            designator: shared(fps, |fp| fp.reference.clone()),
            designator_shown: text_shown(fps, FootprintTextField::Designator),
            comment: shared(fps, |fp| fp.value.clone()),
            comment_shown: text_shown(fps, FootprintTextField::Value),
            area_mm2: shared(fps, component::body_area_mm2),
            footprint_name: shared(fps, |fp| fp.footprint_id.clone()),
            library: shared(fps, |fp| {
                fp.library_ref
                    .as_ref()
                    .map(|source| self.footprint_library(source.library))
            }),
            schematic_path: shared(fps, |fp| fp.schematic_link.as_ref().map(hierarchical_path)),
            parameters: parameters(fps),
            drafts,
            read_only: board.unknown.newer_minor_source.is_some(),
            search: state.search.clone(),
            // One component only: the mockup draws no merged drawing.
            drawing: match fps {
                [one] => Some(drawing(one)),
                _ => None,
            },
            illustration_hover: state.illustration_hover.clone(),
            footprints: ids,
        })
    }

    /// The library `id` as the Library row names it: an open library by its
    /// display name, else by its id.
    fn footprint_library(&self, id: uuid::Uuid) -> FootprintLibrary {
        self.library
            .open_libraries
            .iter()
            .find(|lib| lib.library_id == id)
            .map_or(FootprintLibrary::NotOpen(id), |lib| {
                FootprintLibrary::Open(lib.display_name.clone())
            })
    }
}

#[cfg(test)]
mod tests {
    use signex_types::pcb::{Pad, PadShape, PadType};
    use signex_types::pcb_layer::BoardSide;
    use signex_types::schematic::InstancePath;
    use uuid::Uuid;

    use super::*;

    fn pad(n: u128, number: &str, x: f64) -> Pad {
        Pad {
            uuid: Uuid::from_u128(n),
            number: number.into(),
            pad_type: PadType::Smd,
            shape: PadShape::Rect,
            position: Point::new(x, 0.0),
            size: Point::new(1.0, 1.0),
            drill: None,
            layers: vec!["TopCopper".into()],
            net: None,
            roundrect_ratio: 0.25,
            trapezoid_delta: Default::default(),
            custom_outline: Vec::new(),
            rotation: 0.0,
            layer_sizes: None,
            net_locked: false,
            thermal: Default::default(),
            unknown: Default::default(),
            unknown_columns: Default::default(),
        }
    }

    /// A courtyard rectangle 3 mm either side of the origin.
    fn courtyard() -> FpGraphic {
        FpGraphic {
            graphic_type: "rect".into(),
            layer: "TopCourtyard".into(),
            width: 0.05,
            start: Some(Point::new(-3.0, -1.0)),
            end: Some(Point::new(3.0, 1.0)),
            center: None,
            mid: None,
            radius: 0.0,
            points: Vec::new(),
            text: String::new(),
            font_size: 0.0,
            position: None,
            rotation: 0.0,
            fill: String::new(),
        }
    }

    /// A two-pad part at (10, 10), pin 1 on the left.
    fn part(rotation: f64, layer: &str, graphics: Vec<FpGraphic>) -> Footprint {
        Footprint {
            uuid: Uuid::from_u128(1),
            reference: "R1".into(),
            value: "10k".into(),
            footprint_id: "R_0603".into(),
            position: Point::new(10.0, 10.0),
            rotation,
            layer: layer.into(),
            locked: false,
            schematic_link: None,
            library_ref: None,
            component: Default::default(),
            pads: vec![pad(2, "1", -2.0), pad(3, "2", 2.0)],
            graphics,
            properties: Vec::new(),
            unknown: Default::default(),
            unknown_columns: Default::default(),
        }
    }

    #[test]
    fn the_drawing_has_pin_one_and_a_courtyard_only_when_the_footprint_draws_one() {
        let bare = drawing(&part(0.0, "TopCopper", Vec::new()));
        assert_eq!(bare.pads.len(), 2);
        assert_eq!(bare.pin_one, Some(Point::new(-2.0, 0.0)));
        assert!(bare.courtyard.is_empty(), "no courtyard is made up");
        let drawn = drawing(&part(0.0, "TopCopper", vec![courtyard()]));
        assert_eq!(drawn.courtyard.len(), 4, "the rectangle's four sides");
        assert!(drawn.outline.is_empty(), "a courtyard is not the body");
    }

    #[test]
    fn the_drawing_leaves_the_rotation_out_and_mirrors_the_bottom_side() {
        let turned = drawing(&part(90.0, "TopCopper", Vec::new()));
        assert_eq!(turned.side, BoardSide::Top);
        assert_eq!(
            turned.pin_one,
            Some(Point::new(-2.0, 0.0)),
            "pin 1 where the footprint has it, unturned and at the origin"
        );
        let bottom = drawing(&part(0.0, "BottomCopper", Vec::new()));
        assert_eq!(bottom.side, BoardSide::Bottom);
        assert_eq!(bottom.pin_one, Some(Point::new(2.0, 0.0)), "mirrored");
    }

    #[test]
    fn a_footprint_without_a_pad_one_has_no_pin_one() {
        let mut fp = part(0.0, "TopCopper", Vec::new());
        fp.pads[0].number = "A".into();
        assert_eq!(drawing(&fp).pin_one, None);
    }

    #[test]
    fn the_hierarchical_path_joins_the_sheet_path_and_the_units() {
        let unit = Uuid::from_u128(7);
        let root = SymbolLink::new(InstancePath::root(), [unit]);
        assert_eq!(hierarchical_path(&root), format!("/{unit}"));
        let sheet = Uuid::from_u128(3);
        let nested = SymbolLink::new(InstancePath::from_chain(vec![sheet]), [unit]);
        assert_eq!(hierarchical_path(&nested), format!("/{sheet}/{unit}"));
    }
}
