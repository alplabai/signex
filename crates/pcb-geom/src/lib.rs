//! Board geometry in integer nanometres (#2174): the base that board
//! connectivity, ratsnest, hit-test and later DRC are built on.
//!
//! The board model stays `f64` mm. Every value enters through
//! [`mm_to_nm`] / [`PointNm::from_mm`], which refuse a length the
//! geometry cannot hold, and leaves through [`nm_to_mm`]. Inside the crate
//! every coordinate is an `i64` nm and every product an `i128`. Why, and
//! how it relates to the `Distance` type of #1058: see
//! `docs/architecture/adr-0005-integer-nm-board-geometry.md`.
//!
//! Hit-testing reads the model, never the render output (#662).

mod flip;
mod index;
mod mst;
mod rect;
mod selection;
mod shape;
mod transform;
mod union_find;
mod units;

pub use flip::flip_footprint;
pub use index::{BoardIndex, IndexedItem, ItemRef, RectSelect, Rejected};
pub use mst::{Edge, euclidean_mst, spanning_forest};
pub use rect::RectNm;
pub use selection::{ClickMode, Selection, marquee_mode, marquee_select, pick};
pub use shape::Shape;
pub use transform::{Placement, Side, pad_shape, pad_world_center};
pub use union_find::UnionFind;
pub use units::{GeomError, MAX_ABS_NM, NM_PER_MM, PointNm, mm_to_nm, nm_to_mm};
