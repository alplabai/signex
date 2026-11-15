//! The facts the track, via and pad panels show, read from the board. Net
//! lengths, net membership and via spans come from `pcb_geom`; nothing
//! here re-derives board geometry.

use std::path::Path;

use pcb_geom::ItemRef;
use signex_engine::pcb::primitive_edit::track_location;
use signex_i18n::{I18n, ids, t};
use signex_types::format::parse_pcb_layer;
use signex_types::net_class::first_matching_class;
use signex_types::pcb::{Pad, PadShape, PadType, PcbBoard, Segment, Via, ViaType};
use signex_types::pcb_layer::PcbLayer;

use crate::app::Signex;
use crate::panels::pcb_primitive_properties::{
    Editability, LayerChoice, NetChoice, NetInfo, PadFacts, PcbPrimitiveFacts, PrimitiveCommon,
    PrimitiveTarget, ShapeChoice, TrackFacts, ViaFacts, shared,
};
use crate::pcb_canvas::layer_style;

impl Signex {
    /// The Properties panel's facts of the tracks, vias or pads selected on
    /// the board at `path`; `None` unless the selection is all one of those.
    pub(crate) fn pcb_primitive_facts(
        &self,
        path: &Path,
        board: &PcbBoard,
    ) -> Option<PcbPrimitiveFacts> {
        let selection = &self.interaction_state.pcb_selection;
        if selection.document() != Some(path) {
            return None;
        }
        let items: Vec<ItemRef> = selection.selection().items().collect();
        let target = PrimitiveTarget {
            board: path.to_path_buf(),
            items,
        };
        let i18n = &*self.ui_state.i18n;
        let first = *target.items.first()?;
        let same_kind = target
            .items
            .iter()
            .all(|i| std::mem::discriminant(i) == std::mem::discriminant(&first));
        if !same_kind {
            return None;
        }
        Some(match first {
            ItemRef::Track(_) => {
                let tracks: Vec<&Segment> = target
                    .items
                    .iter()
                    .filter_map(|i| match i {
                        ItemRef::Track(id) => board.segments.iter().find(|s| s.uuid == *id),
                        _ => None,
                    })
                    .collect();
                let common = self.primitive_common(board, target, Editability::Editable)?;
                PcbPrimitiveFacts::Track(track_facts(board, common, &tracks, i18n))
            }
            ItemRef::Via(_) => {
                let vias: Vec<&Via> = target
                    .items
                    .iter()
                    .filter_map(|i| match i {
                        ItemRef::Via(id) => board.vias.iter().find(|v| v.uuid == *id),
                        _ => None,
                    })
                    .collect();
                let common = self.primitive_common(board, target, Editability::Editable)?;
                PcbPrimitiveFacts::Via(via_facts(board, common, &vias, i18n))
            }
            ItemRef::Pad { .. } => {
                let pads: Vec<(&signex_types::pcb::Footprint, &Pad)> = target
                    .items
                    .iter()
                    .filter_map(|i| match i {
                        ItemRef::Pad { footprint, pad } => {
                            let fp = board.footprints.iter().find(|f| f.uuid == *footprint)?;
                            Some((fp, fp.pads.iter().find(|p| p.uuid == *pad)?))
                        }
                        _ => None,
                    })
                    .collect();
                // A footprint that locks its primitives (#2365) locks its pads too.
                let locked = if pads
                    .iter()
                    .any(|(fp, _)| fp.locked || fp.component.lock_primitives)
                {
                    Editability::Locked
                } else {
                    Editability::Editable
                };
                let common = self.primitive_common(board, target, locked)?;
                PcbPrimitiveFacts::Pad(pad_facts(board.options.origin, common, &pads, i18n))
            }
            ItemRef::Zone { .. } => {
                let zones: Vec<&signex_types::pcb::Zone> = target
                    .items
                    .iter()
                    .filter_map(|i| match i {
                        ItemRef::Zone { zone, .. } => board.zones.iter().find(|z| z.uuid == *zone),
                        _ => None,
                    })
                    .collect();
                if zones.len() != target.items.len() {
                    return None;
                }
                let net = shared(zones.iter().map(|z| z.net));
                let common = self.primitive_common_on(board, target, Editability::Editable, net);
                PcbPrimitiveFacts::Zone(super::zone::zone_facts(board, common, &zones, i18n))
            }
            ItemRef::Footprint(_) => return None,
        })
    }

    /// The facts every kind shares. `None` when a selected item is gone.
    fn primitive_common(
        &self,
        board: &PcbBoard,
        target: PrimitiveTarget,
        editability: Editability,
    ) -> Option<PrimitiveCommon> {
        let nets: Option<Vec<u32>> = target
            .items
            .iter()
            .map(|i| item_on_board(board, *i).then(|| pcb_geom::item_net(board, *i).unwrap_or(0)))
            .collect();
        let shared_net = shared(nets?.into_iter());
        Some(self.primitive_common_on(board, target, editability, shared_net))
    }

    /// The facts every kind shares, for items on `shared_net` (`None`: they
    /// differ).
    pub(super) fn primitive_common_on(
        &self,
        board: &PcbBoard,
        target: PrimitiveTarget,
        editability: Editability,
        shared_net: Option<u32>,
    ) -> PrimitiveCommon {
        let i18n = &*self.ui_state.i18n;
        let name_of = |n: u32| {
            board
                .nets
                .iter()
                .find(|d| d.number == n)
                .map(|d| d.name.clone())
        };
        let name = shared_net.filter(|n| *n != 0).and_then(name_of);
        let class = name.as_deref().and_then(|name| {
            let classes = self.active_net_classes()?;
            first_matching_class(classes, name).map(|c| c.name.clone())
        });
        let on_net = shared_net
            .map(|n| pcb_geom::net_items(board, n))
            .unwrap_or_default();
        let selected_length_mm = match target.items.first() {
            Some(ItemRef::Track(_)) => pcb_geom::tracks_length_mm(
                board
                    .segments
                    .iter()
                    .filter(|s| target.items.contains(&ItemRef::Track(s.uuid))),
            ),
            _ => None,
        };
        let net_info = NetInfo {
            net: shared_net,
            name,
            class,
            selected_length_mm,
            total_length_mm: shared_net.and_then(|n| pcb_geom::net_routed_length_mm(board, n)),
            tracks: on_net
                .iter()
                .filter(|i| matches!(i, ItemRef::Track(_)))
                .count(),
            vias: on_net
                .iter()
                .filter(|i| matches!(i, ItemRef::Via(_)))
                .count(),
            pads: on_net
                .iter()
                .filter(|i| matches!(i, ItemRef::Pad { .. }))
                .count(),
        };
        let no_net = NetChoice {
            number: 0,
            label: t!(i18n, ids::PCB_PRIM_NO_NET).into_string(),
        };
        let mut nets: Vec<NetChoice> = board
            .nets
            .iter()
            .filter(|n| n.number != 0)
            .map(|n| NetChoice {
                number: n.number,
                label: n.name.clone(),
            })
            .collect();
        nets.sort_by_key(|n| n.number);
        nets.insert(0, no_net);
        let net = shared_net.and_then(|n| nets.iter().find(|c| c.number == n).cloned());
        let panel = &self.interaction_state.pcb_primitive;
        let editability = if board.unknown.newer_minor_source.is_some() {
            Editability::ReadOnly
        } else {
            editability
        };
        PrimitiveCommon {
            drafts: if panel.draft_target.as_ref() == Some(&target) {
                panel.drafts.clone()
            } else {
                Default::default()
            },
            picking_net: panel.net_pick.as_ref() == Some(&target),
            placement_paused: false,
            show_more: panel.show_more,
            search: panel.search.clone(),
            target,
            editability,
            net_info,
            nets,
            net,
        }
    }

    /// The active project's net classes, the table the schematic uses.
    pub(super) fn active_net_classes(&self) -> Option<&[signex_types::net_class::NetClass]> {
        let docs = &self.document_state;
        let id = docs.active_project?;
        let project = docs.projects.iter().find(|p| p.id == id)?;
        Some(&project.data.net_classes)
    }
}

fn item_on_board(board: &PcbBoard, item: ItemRef) -> bool {
    match item {
        ItemRef::Track(id) => board.segments.iter().any(|s| s.uuid == id),
        ItemRef::Via(id) => board.vias.iter().any(|v| v.uuid == id),
        ItemRef::Pad { footprint, pad } => board
            .footprints
            .iter()
            .any(|f| f.uuid == footprint && f.pads.iter().any(|p| p.uuid == pad)),
        ItemRef::Footprint(_) | ItemRef::Zone { .. } => false,
    }
}

pub(super) fn track_facts(
    board: &PcbBoard,
    common: PrimitiveCommon,
    tracks: &[&Segment],
    i18n: &I18n,
) -> TrackFacts {
    // Coordinates are shown from the board origin, Y up (#2384).
    let origin = board.options.origin;
    let layers: Vec<LayerChoice> = signex_renderer::pcb::board_copper_layers(&board.layers)
        .into_iter()
        .map(|layer| LayerChoice {
            layer,
            label: layer_style::copper_name(i18n, layer).into_string(),
        })
        .collect();
    let layer = shared(tracks.iter().map(|s| s.layer.as_str()))
        .and_then(parse_pcb_layer)
        .and_then(PcbLayer::copper)
        .map(|layer| LayerChoice {
            layer,
            label: layer_style::copper_name(i18n, layer).into_string(),
        });
    TrackFacts {
        location: track_location(tracks.iter().copied()).map(|p| {
            let shown = origin.to_user(p);
            (shown.x, shown.y)
        }),
        layer,
        layers,
        width: shared(tracks.iter().map(|s| s.width)),
        start: (
            shared(tracks.iter().map(|s| origin.user_x(s.start.x))),
            shared(tracks.iter().map(|s| origin.user_y(s.start.y))),
        ),
        end: (
            shared(tracks.iter().map(|s| origin.user_x(s.end.x))),
            shared(tracks.iter().map(|s| origin.user_y(s.end.y))),
        ),
        length_mm: pcb_geom::tracks_length_mm(tracks.iter().copied()),
        common,
    }
}

pub(super) fn via_facts(
    board: &PcbBoard,
    common: PrimitiveCommon,
    vias: &[&Via],
    i18n: &I18n,
) -> ViaFacts {
    let origin = board.options.origin;
    let stack = signex_renderer::pcb::board_copper_layers(&board.layers);
    let span = shared(
        vias.iter()
            .map(|v| (v.via_type, pcb_geom::via_span(&stack, v))),
    )
    .and_then(|(kind, span)| {
        let (from, to) = span?;
        let id = match kind {
            ViaType::Through => ids::PCB_PRIM_VIA_SPAN_THROUGH,
            ViaType::Blind => ids::PCB_PRIM_VIA_SPAN_BLIND,
            ViaType::Micro => ids::PCB_PRIM_VIA_SPAN_MICRO,
        };
        Some(t!(i18n, id, "from" => from, "to" => to).into_string())
    });
    ViaFacts {
        x: shared(vias.iter().map(|v| origin.user_x(v.position.x))),
        y: shared(vias.iter().map(|v| origin.user_y(v.position.y))),
        diameter: shared(vias.iter().map(|v| v.diameter)),
        hole: shared(vias.iter().map(|v| v.drill)),
        span,
        common,
    }
}

fn pad_facts(
    origin: signex_types::pcb_options::BoardOrigin,
    common: PrimitiveCommon,
    pads: &[(&signex_types::pcb::Footprint, &Pad)],
    i18n: &I18n,
) -> PadFacts {
    let centres: Vec<Option<signex_types::pcb::Point>> = pads
        .iter()
        .map(|(fp, pad)| pcb_geom::pad_world_center(fp, pad).ok().map(|p| p.to_mm()))
        .collect();
    let shape_label = |shape: PadShape| {
        let id = match shape {
            PadShape::Circle => ids::PCB_PRIM_SHAPE_ROUND,
            PadShape::Rect => ids::PCB_PRIM_SHAPE_RECT,
            PadShape::RoundRect => ids::PCB_PRIM_SHAPE_ROUNDRECT,
            PadShape::Oval => ids::PCB_PRIM_SHAPE_OVAL,
        };
        ShapeChoice {
            shape,
            label: t!(i18n, id).into_string(),
        }
    };
    let layer = shared(pads.iter().map(|(fp, pad)| pad_layer(fp, pad))).map(|layer| match layer {
        Some(layer) => (
            layer_style::layer_name(i18n, layer).into_string(),
            Some(layer),
        ),
        None => (t!(i18n, ids::PCB_LAYER_MULTI_LAYER).into_string(), None),
    });
    PadFacts {
        designator: shared(pads.iter().map(|(_, p)| p.number.clone())),
        layer,
        x: shared(centres.iter().map(|c| c.map(|p| origin.user_x(p.x)))).flatten(),
        y: shared(centres.iter().map(|c| c.map(|p| origin.user_y(p.y)))).flatten(),
        rotation: shared(pads.iter().map(|(fp, pad)| {
            pcb_geom::pad_world_rotation_deg(fp, pad)
                .ok()
                .map(|r| r.rem_euclid(360.0))
        }))
        .flatten(),
        size_x: shared(pads.iter().map(|(_, p)| p.size.x)),
        size_y: shared(pads.iter().map(|(_, p)| p.size.y)),
        hole: shared(
            pads.iter()
                .map(|(_, p)| p.drill.as_ref().map(|d| d.diameter)),
        )
        .flatten(),
        has_hole: pads.iter().all(|(_, p)| p.drill.is_some()),
        shape: shared(pads.iter().map(|(_, p)| p.shape)).map(shape_label),
        shapes: [
            PadShape::Circle,
            PadShape::Rect,
            PadShape::RoundRect,
            PadShape::Oval,
        ]
        .into_iter()
        .map(shape_label)
        .collect(),
        common,
    }
}

/// The copper layer a surface pad sits on; `None` for a through-hole pad,
/// which is on every copper layer.
fn pad_layer(fp: &signex_types::pcb::Footprint, pad: &Pad) -> Option<PcbLayer> {
    if pad.pad_type == PadType::Thru {
        return None;
    }
    let mask = pcb_geom::pad_layers(fp, pad);
    pcb_geom::copper_layers(mask)
        .first()
        .map(|c| PcbLayer::Copper(*c))
}
