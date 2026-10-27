//! The one process-wide source of GPU scene generations.
//!
//! [`crate::scene_shader`] skips an upload when this frame's generation equals
//! the one resident in the pipeline slot. A slot is shared by every canvas
//! emitting the same surface (one per window), so two canvases counting on
//! their own would collide — two freshly opened documents both sit at 0 and the
//! second would draw the first's geometry. Every producer (the PCB canvas, the
//! schematic content cache) draws from this single counter, so no two scenes in
//! the process ever carry the same value.

use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

/// A scene generation nothing else in the process has used yet.
pub fn next_scene_generation() -> u64 {
    NEXT.fetch_add(1, Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generations_are_strictly_distinct() {
        let a = next_scene_generation();
        let b = next_scene_generation();
        assert_ne!(a, b);
        assert!(b > a);
    }
}
