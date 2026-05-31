//! Iced bridge skeleton for shader integration.
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: IPC-2612-1, IEEE 315, IEC 60617, wgpu/WGSL public docs.

use std::sync::{Arc, RwLock};

use signex_gfx::scene::{DirtyFlags, Scene};

// Phase-0 keeps this bridge dependency-light so the crate compiles
// independently while the full iced shader integration is implemented.

/// Program state holder for phase-0 integration.
pub struct SchematicProgram {
    pub scene: Arc<RwLock<Scene>>,
    pub dirty: DirtyFlags,
}

impl SchematicProgram {
    pub fn new(scene: Arc<RwLock<Scene>>) -> Self {
        Self {
            scene,
            dirty: DirtyFlags::ALL,
        }
    }

    pub fn set_dirty(&mut self, dirty: DirtyFlags) {
        self.dirty = dirty;
    }
}
