//! Pan/zoom frame bench (#2472): a dev/QA mode behind
//! `SIGNEX_BENCH_PAN_ZOOM=<board.snxpcb>`. The app opens the board, drives a
//! fixed pan/zoom script one step per frame, records each frame's time, writes
//! a CSV and a summary line, and quits. Unset, nothing here runs. How to run
//! it and read the numbers: `docs/PERFORMANCE.md`.
//!
//! Everything in this module is pure: the app feeds it frame instants and
//! applies what it returns (`app/handlers/pan_zoom_bench.rs`).

mod report;
mod script;
#[cfg(test)]
mod tests;

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use iced::time::Instant;

pub use report::{FrameSample, RunContext, Summary, adapter_label, csv_text, summarize};
pub use script::{BenchStep, CYCLE_LEN, step_at};

/// The board to open; setting it turns the bench on.
pub const ENV_BOARD: &str = "SIGNEX_BENCH_PAN_ZOOM";
/// Where the CSV goes (default: beside the board, `<board>.pan-zoom.csv`).
pub const ENV_OUT: &str = "SIGNEX_BENCH_OUT";
/// How long the timed run lasts, in whole seconds.
pub const ENV_SECONDS: &str = "SIGNEX_BENCH_SECONDS";
pub const DEFAULT_SECONDS: u32 = 20;

/// Untimed frames between the board fitting and the first timed frame, so
/// the first upload and tessellation are not in the numbers.
pub const SETTLE_FRAMES: u32 = 60;
/// How long the board may take to open before the bench gives up.
pub const LOAD_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BenchConfig {
    pub board: PathBuf,
    pub out: PathBuf,
    pub seconds: u32,
}

impl BenchConfig {
    /// The bench asked for by the process environment, relative paths
    /// resolved against the working directory.
    pub fn from_env() -> Option<Result<Self, String>> {
        let cwd = std::env::current_dir().unwrap_or_default();
        Self::from_vars(|key| std::env::var_os(key), &cwd)
    }

    /// `None` when [`ENV_BOARD`] is unset or empty; `Err` names a bad value.
    pub fn from_vars(
        get: impl Fn(&str) -> Option<OsString>,
        cwd: &Path,
    ) -> Option<Result<Self, String>> {
        let board = get(ENV_BOARD).filter(|v| !v.is_empty())?;
        let board = cwd.join(PathBuf::from(board));
        let seconds = match get(ENV_SECONDS).filter(|v| !v.is_empty()) {
            None => DEFAULT_SECONDS,
            Some(raw) => match raw.to_str().and_then(|s| s.trim().parse::<u32>().ok()) {
                Some(n) if n > 0 => n,
                _ => {
                    return Some(Err(format!(
                        "{ENV_SECONDS} must be a whole number of seconds above 0, got {raw:?}"
                    )));
                }
            },
        };
        let out = match get(ENV_OUT).filter(|v| !v.is_empty()) {
            Some(out) => cwd.join(PathBuf::from(out)),
            None => board.with_extension("pan-zoom.csv"),
        };
        Some(Ok(Self {
            board,
            out,
            seconds,
        }))
    }

    /// The summary file written beside the CSV.
    pub fn summary_path(&self) -> PathBuf {
        self.out.with_extension("summary.txt")
    }
}

/// What the bench reports to the app.
#[derive(Debug, Clone)]
pub enum BenchMsg {
    /// A window started a frame at `at` (iced's frame clock).
    FrameStarted {
        window: iced::window::Id,
        at: Instant,
    },
    /// The CSV and summary were written (`Ok`: the summary line).
    ReportWritten(Result<String, String>),
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    /// Waiting for the board to open; `since` is the first frame seen.
    Loading { since: Option<Instant> },
    /// The board is fitted; these many untimed frames are left.
    Settling { frames_left: u32 },
    /// Timed: `step` is the script step shown by the next frame.
    Running {
        started: Instant,
        last: Instant,
        step: usize,
    },
    /// The report is out; frames are no longer looked at.
    Done,
}

/// What the app does for one frame.
#[derive(Debug, Clone, PartialEq)]
pub enum FrameAction {
    Wait,
    /// Fit the board to the view (once, when it has opened).
    FitBoard,
    /// Move the camera by this script step.
    Apply(BenchStep),
    /// Write this report and quit.
    Finish(BenchReport),
}

#[derive(Debug, Clone, PartialEq)]
pub enum BenchReport {
    Completed(Vec<FrameSample>),
    BoardNotOpened,
}

/// A bench run: the config plus where the script is.
#[derive(Debug, Clone)]
pub struct PanZoomBench {
    pub config: BenchConfig,
    phase: Phase,
    samples: Vec<FrameSample>,
}

impl PanZoomBench {
    pub fn new(config: BenchConfig) -> Self {
        Self {
            config,
            phase: Phase::Loading { since: None },
            samples: Vec::new(),
        }
    }

    /// Whether frames still matter; after the report they do not.
    pub fn wants_frames(&self) -> bool {
        self.phase != Phase::Done
    }

    /// Advance on the frame started at `at`. `board_ready`: the board is the
    /// active document and its canvas has been laid out.
    pub fn on_frame(&mut self, at: Instant, board_ready: bool) -> FrameAction {
        match self.phase {
            Phase::Loading { since } => {
                let since = since.unwrap_or(at);
                if board_ready {
                    self.phase = Phase::Settling {
                        frames_left: SETTLE_FRAMES,
                    };
                    return FrameAction::FitBoard;
                }
                if at.saturating_duration_since(since) >= LOAD_TIMEOUT {
                    self.phase = Phase::Done;
                    return FrameAction::Finish(BenchReport::BoardNotOpened);
                }
                self.phase = Phase::Loading { since: Some(since) };
                FrameAction::Wait
            }
            Phase::Settling { frames_left } if frames_left > 0 => {
                self.phase = Phase::Settling {
                    frames_left: frames_left - 1,
                };
                FrameAction::Wait
            }
            Phase::Settling { .. } => {
                self.phase = Phase::Running {
                    started: at,
                    last: at,
                    step: 1,
                };
                FrameAction::Apply(step_at(0))
            }
            Phase::Running {
                started,
                last,
                step,
            } => {
                self.samples.push(FrameSample {
                    t_ms: millis(at.saturating_duration_since(started)),
                    frame_ms: millis(at.saturating_duration_since(last)),
                    action: step_at(step - 1).label(),
                });
                let run = Duration::from_secs(u64::from(self.config.seconds));
                if at.saturating_duration_since(started) >= run {
                    self.phase = Phase::Done;
                    return FrameAction::Finish(BenchReport::Completed(std::mem::take(
                        &mut self.samples,
                    )));
                }
                self.phase = Phase::Running {
                    started,
                    last: at,
                    step: step + 1,
                };
                FrameAction::Apply(step_at(step))
            }
            Phase::Done => FrameAction::Wait,
        }
    }
}

fn millis(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}
