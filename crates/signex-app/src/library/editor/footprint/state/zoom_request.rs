//! A zoom the canvas owes the model: the zoom pill's `-` / `+` buttons.
//!
//! The camera lives in the canvas widget, so `update` cannot zoom it. The
//! pill records a numbered request here instead; the canvas remembers the
//! request it last applied and zooms by everything recorded since. A
//! running total, not a flag or a last factor, because a zoom is not
//! idempotent: two canvas events before the model hears back must not zoom
//! twice, and two clicks before the canvas runs must both count.

/// Every zoom the pill asked of one document. `seq` 0 is "none yet".
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ZoomRequest {
    pub seq: u64,
    /// The natural log of the product of every factor recorded so far.
    log_scale: f64,
}

impl ZoomRequest {
    /// Record a request for `factor` about the canvas centre; above 1 zooms
    /// in. A non-finite or non-positive factor is refused.
    pub fn step(&mut self, factor: f32) {
        if !factor.is_finite() || factor <= 0.0 {
            return;
        }
        self.seq = self.seq.wrapping_add(1).max(1);
        self.log_scale += f64::from(factor).ln();
    }

    /// The scale factor that takes a camera from `applied` to `self`: the
    /// product of every factor recorded after `applied`.
    pub fn factor_since(&self, applied: &ZoomRequest) -> f32 {
        (self.log_scale - applied.log_scale).exp() as f32
    }
}

#[cfg(test)]
mod tests {
    use super::ZoomRequest;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    #[test]
    fn each_step_gets_a_new_number() {
        let mut request = ZoomRequest::default();
        request.step(1.25);
        assert_eq!(request.seq, 1);
        let first = request;
        request.step(0.8);
        assert_eq!(request.seq, 2);
        assert!(close(request.factor_since(&first), 0.8));
    }

    /// Two clicks before the canvas applies either zoom by both.
    #[test]
    fn pending_steps_multiply() {
        let applied = ZoomRequest::default();
        let mut request = applied;
        request.step(1.25);
        request.step(1.25);
        assert!(close(request.factor_since(&applied), 1.5625));
        assert!(close(applied.factor_since(&applied), 1.0));
    }

    #[test]
    fn a_bad_factor_is_refused() {
        let mut request = ZoomRequest::default();
        request.step(f32::NAN);
        request.step(0.0);
        request.step(-2.0);
        assert_eq!(request, ZoomRequest::default());
    }
}
