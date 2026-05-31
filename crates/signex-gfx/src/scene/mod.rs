//! Scene module namespace.
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: IPC-2612-1, IEEE 315, IEC 60617, wgpu/WGSL public docs.

pub mod dirty;
pub mod frame;
pub mod upload;

pub use dirty::DirtyFlags;
pub use frame::Scene;
pub use upload::{
	apply_dirty_uploads, apply_dirty_uploads_with_culling, SceneUploadTarget, TextUploadParams,
	UploadCounters, UploadCulling, ViewportAabbMm,
};
