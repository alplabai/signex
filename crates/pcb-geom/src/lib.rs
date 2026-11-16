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
mod body;
mod copper;
mod flip;
mod graphic;
mod index;
mod measure;
mod mst;
mod net_length;
mod net_report;
mod pick_filter;
mod ratsnest;
mod rect;
mod region;
mod selection;
mod shape;
mod shape_gap;
mod shape_overlap;
mod staging;
mod stats;
mod text;
mod track_follow;
mod transform;
mod turned_marquee;
mod union_find;
mod units;
mod zone_fill;

pub use body::footprint_body;
pub use copper::{
    CopperItem, collect_copper, copper_layers, layer_bit, layer_mask, net_at, pad_layers,
    via_layers,
};
pub use flip::flip_footprint;
pub use graphic::{
    GraphicShape, PlacedGraphic, board_graphic_placement, board_text_placement,
    footprint_graphic_placement,
};
pub use index::{
    BoardIndex, IndexedItem, ItemRef, RectSelect, Rejected, item_by_uuid, track_shape, via_shape,
};
pub use measure::{Span, edge_gap};
pub use mst::{Edge, euclidean_mst, spanning_forest};
pub use net_length::{
    item_net, net_copper, net_items, net_members, net_routed_length_mm, track_length_mm,
    tracks_length_mm, via_span,
};
pub use net_report::{NetReport, net_named, net_reports};
pub use pick_filter::{PickFilter, PickKind};
pub use ratsnest::{RatLine, ratsnest, ratsnest_with_fills};
pub use rect::RectNm;
pub use region::{
    OFFSET_ARC_POINTS, Polygon, chord_sag, difference, intersection, offset, twice_signed_area,
    union,
};
pub use selection::{ClickMode, Selection, marquee_mode, marquee_select, pick, pick_filtered};
pub use shape::Shape;
pub use shape_gap::Separation;
pub use staging::{Staging, footprint_extent, stage, stage_up};
pub use stats::BoardStats;
pub use text::{TextPlacement, is_board_text_key, property_text_placement};
pub use track_follow::{TrackEnds, tracks_on_pads};
pub use transform::{Placement, Side, pad_shape, pad_world_center, pad_world_rotation_deg};
pub use turned_marquee::{turned_marquee_corners, turned_marquee_mode, turned_marquee_select};
pub use union_find::UnionFind;
pub use units::{GeomError, MAX_ABS_NM, NM_PER_MM, PointNm, mm_to_nm, nm_to_mm};
pub use zone_fill::{ZoneFill, ZoneFills, zone_area, zone_fills};
