//! Board geometry in integer nanometres (#2174): the base that board
//! connectivity, ratsnest, hit-test and the design rule check
//! (`signex-drc`) are built on.
//!
//! The board model stays `f64` mm. Every value enters through
//! [`mm_to_nm`] / [`PointNm::from_mm`], which refuse a length the
//! geometry cannot hold, and leaves through [`nm_to_mm`]. Inside the crate
//! every coordinate is an `i64` nm and every product an `i128`. Why, and
//! how it relates to the `Distance` type of #1058: see
//! `docs/architecture/adr-0005-integer-nm-board-geometry.md`.
//!
//! Hit-testing reads the model, never the render output (#662).

mod area_touch;
mod arrange;
mod body;
mod closed_path;
mod connections;
mod copper;
mod drawn_copper;
mod flip;
mod free;
#[cfg(test)]
mod free_tests;
mod gloss;
mod graphic;
mod graphic_index;
mod index;
mod measure;
mod mst;
mod net_length;
mod net_report;
mod obstacle;
pub mod panel;
mod pick_filter;
mod pour_edit;
mod ratsnest;
mod rect;
mod region;
pub mod route;
mod route_rats;
mod selection;
mod shape;
mod shape_gap;
mod shape_overlap;
mod shape_polygon;
mod staging;
mod stats;
mod stitch;
mod stroke_font;
mod teardrop;
mod text;
mod text_frame;
#[cfg(test)]
mod tool_fixture;
mod track_follow;
mod transform;
mod turned_marquee;
mod union_find;
mod units;
mod zone_fill;

pub use arrange::{arrange_in_box, pack_in_box, placed_extent};
pub use body::{checked_body, courtyard_outline, footprint_body, is_courtyard};
pub use closed_path::{PathRefusal, closed_path};
pub use connections::{ConnectionCounts, NetUnrouted, PadName, connection_counts};
pub use copper::{
    CopperItem, collect_copper, copper_layers, layer_bit, layer_mask, net_at, pad_layers,
    pad_pieces, pieces_on, via_layers, via_pieces,
};
pub use drawn_copper::drawn_copper;
pub use flip::flip_footprint;
pub use free::{
    ARC_CHORD_ERROR_MM, arc_is_copper, arc_shapes, dimension_shapes, free_copper_on,
    free_pad_shape, text_shape,
};
pub use gloss::{Glossed, gloss};
pub use graphic::{
    GraphicShape, PlacedGraphic, board_graphic_placement, board_text_placement,
    footprint_graphic_placement,
};
pub use graphic_index::graphic_strokes;
pub use index::{
    BoardIndex, IndexedItem, ItemRef, ROOM_LABEL_HEIGHT_MM, ROOM_LABEL_INSET_MM, RectSelect,
    Rejected, item_by_uuid, room_label_box, track_shape, via_shape,
};
pub use measure::{Span, edge_gap};
pub use mst::{Edge, euclidean_mst, spanning_forest};
pub use net_length::{
    item_net, net_copper, net_items, net_members, net_routed_length_mm, track_length_mm,
    tracks_length_mm, via_span,
};
pub use net_report::{NetReport, net_named, net_reports};
pub use pick_filter::{PickFilter, PickKind};
pub use pour_edit::{AreaShape, cut_area, slice_area};
pub use ratsnest::{RatLine, ratsnest, ratsnest_with_fills};
pub use rect::RectNm;
pub use region::{
    OFFSET_ARC_POINTS, Polygon, chord_sag, difference, intersection, offset, twice_signed_area,
    union,
};
pub use route_rats::{RouteRats, route_rats};
pub use selection::{
    ClickMode, Selection, marquee_mode, marquee_select, pick, pick_filtered, pick_under_texts,
};
pub use shape::Shape;
pub use shape_gap::Separation;
pub use staging::{Staging, footprint_extent, stage, stage_up};
pub use stats::BoardStats;
pub use stitch::{StitchSpec, stitch_points, stitch_spec_mm};
pub use stroke_font::{
    TEXT_STROKE_EM, justify_start_mm, stroke_text_width_mm, text_stroke_width_mm, text_strokes,
    text_strokes_within,
};
pub use teardrop::{Teardrop, TeardropPlan, teardrops};
pub use text::{
    BoardTextField, FootprintText, TextPlacement, footprint_text, footprint_texts,
    is_board_text_key, justified_centre, local_text_position, local_text_rotation, property_index,
    property_text_layer, property_text_placement, property_text_size_mm, shown_footprint_texts,
    text_box,
};
pub use text_frame::{
    FrameLine, TEXT_FRAME_ADVANCE_EM, TEXT_FRAME_LINE_PITCH_EM, TEXT_FRAME_MARGIN_EM,
    text_frame_lines, text_frame_size_mm, wrap as wrap_text_frame,
};
pub use track_follow::{
    ARC_FOLLOW_MAX_GROWTH, ArcEnds, ArcFollowFault, TrackEnds, arcs_on_vias, tracks_on_pads,
    tracks_on_vias,
};
pub use transform::{Placement, Side, pad_shape, pad_world_center, pad_world_rotation_deg};
pub use turned_marquee::{turned_marquee_corners, turned_marquee_mode, turned_marquee_select};
pub use union_find::UnionFind;
pub use units::{GeomError, MAX_ABS_NM, NM_PER_MM, PointNm, mm_to_nm, nm_to_mm};
pub use zone_fill::{Unfilled, ZoneFill, ZoneFills, pour_fill_order, zone_area, zone_fills};
