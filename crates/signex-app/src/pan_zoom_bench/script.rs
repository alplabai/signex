//! The pan/zoom script: one step per frame, the same on every run. A cycle
//! zooms in about the view centre, pans right two view widths, down one view
//! height, back left and up to where it started, and zooms out again, so it
//! ends where it began. Pans are fractions of the view, so the script does
//! the same thing at any window size.

/// One frame's camera move.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BenchStep {
    /// Scale the view by `factor` about its centre.
    Zoom { factor: f32 },
    /// Move the board on screen by these fractions of the view's width and
    /// height (negative `dx`: the view travels right).
    Pan { dx: f32, dy: f32 },
}

impl BenchStep {
    /// The CSV's name for the move.
    pub fn label(self) -> &'static str {
        match self {
            Self::Zoom { factor } if factor > 1.0 => "zoom_in",
            Self::Zoom { .. } => "zoom_out",
            Self::Pan { dx, .. } if dx < 0.0 => "pan_right",
            Self::Pan { dx, .. } if dx > 0.0 => "pan_left",
            Self::Pan { dy, .. } if dy < 0.0 => "pan_down",
            Self::Pan { .. } => "pan_up",
        }
    }
}

const ZOOM_FRAMES: usize = 40;
const ZOOM_FACTOR: f32 = 1.04;
/// Frames per horizontal leg; each moves the view by 1/120 of its width.
const PAN_X_FRAMES: usize = 240;
const PAN_X_STEP: f32 = 1.0 / 120.0;
/// Frames per vertical leg; each moves the view by 1/120 of its height.
const PAN_Y_FRAMES: usize = 120;
const PAN_Y_STEP: f32 = 1.0 / 120.0;

/// Frames in one cycle; the script repeats after it.
pub const CYCLE_LEN: usize = 2 * ZOOM_FRAMES + 2 * PAN_X_FRAMES + 2 * PAN_Y_FRAMES;

/// The move for frame `index` of the timed run.
pub fn step_at(index: usize) -> BenchStep {
    let legs = [
        (
            ZOOM_FRAMES,
            BenchStep::Zoom {
                factor: ZOOM_FACTOR,
            },
        ),
        (
            PAN_X_FRAMES,
            BenchStep::Pan {
                dx: -PAN_X_STEP,
                dy: 0.0,
            },
        ),
        (
            PAN_Y_FRAMES,
            BenchStep::Pan {
                dx: 0.0,
                dy: -PAN_Y_STEP,
            },
        ),
        (
            PAN_X_FRAMES,
            BenchStep::Pan {
                dx: PAN_X_STEP,
                dy: 0.0,
            },
        ),
        (
            PAN_Y_FRAMES,
            BenchStep::Pan {
                dx: 0.0,
                dy: PAN_Y_STEP,
            },
        ),
        (
            ZOOM_FRAMES,
            BenchStep::Zoom {
                factor: 1.0 / ZOOM_FACTOR,
            },
        ),
    ];
    let mut at = index % CYCLE_LEN;
    let mut chosen = legs[0].1;
    for (frames, step) in legs {
        chosen = step;
        if at < frames {
            break;
        }
        at -= frames;
    }
    chosen
}
