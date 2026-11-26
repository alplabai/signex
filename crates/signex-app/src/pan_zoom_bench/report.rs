//! The bench's numbers: per-frame samples, the CSV and the summary line.

/// The frame budget of the 60 fps target, in ms.
pub const BUDGET_MS: f64 = 1000.0 / 60.0;

/// One timed frame.
#[derive(Debug, Clone, PartialEq)]
pub struct FrameSample {
    /// Start of this frame, from the start of the timed run.
    pub t_ms: f64,
    /// Time since the frame before: what this frame cost on screen.
    pub frame_ms: f64,
    /// The script step this frame shows.
    pub action: &'static str,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Summary {
    pub frames: usize,
    pub seconds: f64,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub max_ms: f64,
    pub fps: f64,
    pub context: RunContext,
}

/// What the run drew with, for the summary line.
#[derive(Debug, Clone, PartialEq)]
pub struct RunContext {
    /// `name (backend)` of the graphics adapter.
    pub adapter: String,
    /// `cpu` or `gpu`: the board mount Preferences selected.
    pub mount: &'static str,
    /// Whether 4x multisampling was on.
    pub msaa: bool,
    /// The board view's size in logical pixels.
    pub view: (u32, u32),
}

impl Summary {
    /// Whether p95 fits the 60 fps budget.
    pub fn within_budget(&self) -> bool {
        self.frames > 0 && self.p95_ms <= BUDGET_MS
    }

    /// The one line printed and saved beside the CSV.
    pub fn line(&self) -> String {
        format!(
            "PAN-ZOOM-BENCH frames={} seconds={:.2} p50_ms={:.2} p95_ms={:.2} p99_ms={:.2} \
             max_ms={:.2} fps={:.1} budget_60fps={} mount={} msaa={} view={}x{} adapter=\"{}\"",
            self.frames,
            self.seconds,
            self.p50_ms,
            self.p95_ms,
            self.p99_ms,
            self.max_ms,
            self.fps,
            if self.within_budget() { "ok" } else { "OVER" },
            self.context.mount,
            if self.context.msaa { "x4" } else { "off" },
            self.context.view.0,
            self.context.view.1,
            self.context.adapter,
        )
    }
}

/// Percentiles are nearest-rank; fps is frames over the time they took.
pub fn summarize(samples: &[FrameSample], context: RunContext) -> Summary {
    let mut times: Vec<f64> = samples.iter().map(|s| s.frame_ms).collect();
    times.sort_by(f64::total_cmp);
    let total_ms: f64 = times.iter().sum();
    Summary {
        frames: times.len(),
        seconds: total_ms / 1000.0,
        p50_ms: percentile(&times, 50.0),
        p95_ms: percentile(&times, 95.0),
        p99_ms: percentile(&times, 99.0),
        max_ms: times.last().copied().unwrap_or(0.0),
        fps: if total_ms > 0.0 {
            times.len() as f64 * 1000.0 / total_ms
        } else {
            0.0
        },
        context,
    }
}

/// Nearest-rank percentile of ascending `sorted`; 0 when empty.
fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let rank = (p / 100.0 * sorted.len() as f64).ceil() as usize;
    sorted[rank.clamp(1, sorted.len()) - 1]
}

pub fn csv_text(samples: &[FrameSample]) -> String {
    let mut out = String::from("frame,t_ms,frame_ms,action\n");
    for (index, sample) in samples.iter().enumerate() {
        out.push_str(&format!(
            "{index},{:.3},{:.3},{}\n",
            sample.t_ms, sample.frame_ms, sample.action
        ));
    }
    out
}

/// `name (backend)` from the `GPU-ADAPTER-SELECTED` diagnostic's message.
pub fn adapter_label(message: &str) -> Option<String> {
    message
        .strip_prefix("Graphics adapter selected: ")
        .map(|rest| rest.trim().to_string())
        .filter(|rest| !rest.is_empty())
}
