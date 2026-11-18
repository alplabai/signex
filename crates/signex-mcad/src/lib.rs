//! Mechanical CAD core (#2487): solids on an exact B-rep kernel.
//!
//! The kernel is the `solidcore` crate. This crate wraps it so the rest of
//! Signex never calls the kernel directly:
//!
//! - [`Solid`] is a body with its faces and edges. Faces carry persistent
//!   [`FaceName`]s that survive booleans, and edges are named by the faces
//!   they join ([`EdgeName`]), so a later feature can refer to "the wall of
//!   hole 3" instead of a kernel id that changes with every operation.
//! - Every kernel call is guarded: a failed operation, or a kernel panic, is a
//!   [`McadError`] value, never a crash.
//! - [`tessellate`] turns a solid into a triangle [`Mesh`] for the 3D view.
//!
//! The crate does no IO and starts no threads: data comes in as values and
//! leaves as values, so it builds for `wasm32-unknown-unknown`. See
//! docs/ROADMAP.md, "MCAD (P3)".

mod board;
mod error;
mod naming;
mod orient;
mod solid;
mod tessellate;
mod topology;

pub use board::{Hole, board_body};
pub use error::McadError;
pub use naming::{EdgeName, FaceName};
pub use solid::{FaceInfo, Solid};
pub use tessellate::{Mesh, MeshFace, TessellationTolerance, tessellate};
