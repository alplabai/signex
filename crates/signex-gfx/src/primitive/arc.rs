//! Arc primitive type.
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: IPC-2612-1, IEEE 315, IEC 60617, wgpu/WGSL public docs.

/// Circular arc with start/end angles in radians.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Arc {
    pub center: [f32; 2],
    pub radius: f32,
    pub start_angle: f32,
    pub end_angle: f32,
    pub width: f32,
    pub color: [f32; 4],
    pub _pad: [f32; 3],
}

/// The single Rust-side authority for this codebase's arc-sweep
/// convention: `start_angle..end_angle` sweeps counter-clockwise
/// (increasing angle) from `start_angle`, wrapping through a full
/// turn when `end_angle < start_angle` — never the signed, unwrapped
/// `end - start` difference. Returns the sweep in `[0, TAU)` radians:
/// the angular distance travelled going CCW from `start_angle` to
/// reach `end_angle`.
///
/// This is the exact Rust equivalent of `normalize_angle(end_angle -
/// start_angle)` in `crates/signex-gfx/src/shader/arc.wgsl`'s
/// `sdf_arc` (the GPU arc renderer) — same formula, same convention.
/// `crates/signex-app/src/library/editor/symbol/state/hit_test.rs`'s
/// `Arc` hit-test arm and `rotation.rs`'s Arc rotate arm independently
/// implement the same wraparound rule against `SymbolGraphicKind::
/// Arc`'s degree-valued `start_deg`/`end_deg` (via `rem_euclid(360.0)`
/// combined with an `if s <= e { .. } else { .. }` branch) rather than
/// calling this function directly, since they operate in degrees on a
/// different (signex-library) type — but the rule they implement is
/// this one. Any Rust code that needs the CCW-wraparound sweep of a
/// radian-valued arc (in particular the CPU canvas draw path, which
/// used to hand iced's arc builder a raw unnormalized `end - start`
/// and silently draw the wrong complement for any wrapped arc) must
/// call this function rather than re-deriving the formula.
pub fn ccw_wrapped_sweep_rad(start_angle: f32, end_angle: f32) -> f32 {
    const TAU: f32 = std::f32::consts::TAU;
    (end_angle - start_angle).rem_euclid(TAU)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The bug this whole normalization pass exists to fix, pinned as
    /// a unit test: a wrapped arc stored as `330° -> 30°` sweeps 60°
    /// counter-clockwise through the 0°/360° seam — not the 300°
    /// complement a naive signed `end - start` would suggest.
    #[test]
    fn pins_330_to_30_as_60_degrees_through_zero() {
        let sweep = ccw_wrapped_sweep_rad(330f32.to_radians(), 30f32.to_radians());
        assert!(
            (sweep - 60f32.to_radians()).abs() < 1e-4,
            "expected 60°, got {}°",
            sweep.to_degrees()
        );
    }

    /// A non-wrapped arc (`start <= end`, no seam crossing) sweeps
    /// exactly the raw positive difference — the case that was
    /// already correct before this fix, unchanged by it.
    #[test]
    fn non_wrapped_arc_sweeps_the_raw_difference() {
        let sweep = ccw_wrapped_sweep_rad(10f32.to_radians(), 100f32.to_radians());
        assert!((sweep - 90f32.to_radians()).abs() < 1e-4);
    }

    /// A degenerate zero-sweep arc (start == end) stays a point, not
    /// a full circle — matches `arc.wgsl`'s `normalize_angle(0) == 0`
    /// and the zero-sweep-degenerate handling in `hit_test.rs`.
    #[test]
    fn equal_start_and_end_is_zero_sweep_not_a_full_circle() {
        let sweep = ccw_wrapped_sweep_rad(45f32.to_radians(), 45f32.to_radians());
        assert!(sweep.abs() < 1e-4);
    }

    /// Sign/direction sanity: swapping start and end complements the
    /// sweep to `360° - sweep` (mirrors the placement-commit swap in
    /// `symbol/updates/mod.rs`, which relies on exactly this
    /// relationship to preserve the user's intended short arc).
    #[test]
    fn swapping_endpoints_complements_the_sweep() {
        let a = 30f32.to_radians();
        let b = 300f32.to_radians();
        let sweep_ab = ccw_wrapped_sweep_rad(a, b);
        let sweep_ba = ccw_wrapped_sweep_rad(b, a);
        let tau = std::f32::consts::TAU;
        assert!((sweep_ab + sweep_ba - tau).abs() < 1e-4);
    }
}
