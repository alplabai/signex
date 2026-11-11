//! A zoom the canvas owes the model: the zoom pill's `-` / `+` buttons.
//!
//! The camera lives in the canvas widget, so `update` cannot zoom it. The
//! pill records a numbered request here instead; the canvas applies each
//! number once (it remembers the last one it applied) and publishes the new
//! scale. A number, not a flag, because a zoom is not idempotent: two canvas
//! events before the model hears back must not zoom twice.

/// The latest zoom request. `seq` 0 is "none yet".
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZoomRequest {
    pub seq: u64,
    /// Scale factor about the canvas centre; above 1 zooms in.
    pub factor: f32,
}

impl Default for ZoomRequest {
    fn default() -> Self {
        Self {
            seq: 0,
            factor: 1.0,
        }
    }
}

impl ZoomRequest {
    /// Record a new request for `factor`. A non-finite or non-positive
    /// factor is refused.
    pub fn step(&mut self, factor: f32) {
        if !factor.is_finite() || factor <= 0.0 {
            return;
        }
        self.seq = self.seq.wrapping_add(1).max(1);
        self.factor = factor;
    }
}

#[cfg(test)]
mod tests {
    use super::ZoomRequest;

    #[test]
    fn each_step_gets_a_new_number() {
        let mut request = ZoomRequest::default();
        request.step(1.25);
        assert_eq!(
            request,
            ZoomRequest {
                seq: 1,
                factor: 1.25
            }
        );
        request.step(0.8);
        assert_eq!(
            request,
            ZoomRequest {
                seq: 2,
                factor: 0.8
            }
        );
    }

    #[test]
    fn a_bad_factor_is_refused() {
        let mut request = ZoomRequest::default();
        request.step(f32::NAN);
        request.step(0.0);
        assert_eq!(request, ZoomRequest::default());
    }
}
