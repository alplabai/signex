//! The pan/zoom frame bench in the app (#2472): boot with the board open,
//! listen to frames while the bench runs, move the board camera the way a
//! drag or a pinch does, write the report in a task and quit. The script and
//! the maths are in `crate::pan_zoom_bench`; `docs/PERFORMANCE.md` has the how.

use iced::{Subscription, Task};

use crate::app::{Message, Signex};
use crate::canvas::CanvasEvent;
use crate::pan_zoom_bench::{
    BenchConfig, BenchMsg, BenchReport, BenchStep, FrameAction, PanZoomBench, RunContext,
};

impl Signex {
    /// [`Signex::boot`] with the bench's board open and the bench armed.
    pub fn boot_pan_zoom_bench(config: BenchConfig) -> (Self, Task<Message>) {
        let (mut app, task) = Self::boot(vec![config.board.clone()]);
        app.pan_zoom_bench = Some(PanZoomBench::new(config));
        (app, task)
    }

    /// One message per frame of any window while the bench runs; none
    /// otherwise. Each handled message redraws, so the frames keep coming.
    pub(crate) fn pan_zoom_bench_subscription(&self) -> Subscription<Message> {
        if !self
            .pan_zoom_bench
            .as_ref()
            .is_some_and(PanZoomBench::wants_frames)
        {
            return Subscription::none();
        }
        iced::event::listen_raw(|event, _status, window| match event {
            iced::Event::Window(iced::window::Event::RedrawRequested(at)) => Some(
                Message::PanZoomBench(BenchMsg::FrameStarted { window, at }),
            ),
            _ => None,
        })
    }

    pub(crate) fn dispatch_pan_zoom_bench_message(&mut self, msg: BenchMsg) -> Task<Message> {
        match msg {
            BenchMsg::FrameStarted { window, at } => {
                if self.ui_state.main_window_id != Some(window) {
                    return Task::none();
                }
                let viewport = self.interaction_state.pcb_canvas.viewport_size();
                let board_ready =
                    self.has_active_pcb() && viewport.width > 0.0 && viewport.height > 0.0;
                let Some(bench) = self.pan_zoom_bench.as_mut() else {
                    return Task::none();
                };
                match bench.on_frame(at, board_ready) {
                    FrameAction::Wait => Task::none(),
                    FrameAction::FitBoard => {
                        self.interaction_state.pcb_canvas.fit_to_board();
                        Task::none()
                    }
                    FrameAction::Apply(step) => self.apply_bench_step(step),
                    FrameAction::Finish(report) => {
                        let config = bench.config.clone();
                        let canvas = &self.interaction_state.pcb_canvas;
                        let mount = if canvas.gpu_render { "gpu" } else { "cpu" };
                        let view = (viewport.width.round() as u32, viewport.height.round() as u32);
                        write_report_task(config, report, mount, view)
                    }
                }
            }
            BenchMsg::ReportWritten(result) => {
                if let Err(error) = result {
                    crate::diagnostics::log_error(
                        "pan/zoom bench",
                        &anyhow::anyhow!("report not written: {error}"),
                    );
                }
                self.flush_quit_writes_then(iced::exit())
            }
        }
    }

    /// Move the board camera, then report the move as the canvas does after
    /// a drag or a wheel zoom.
    fn apply_bench_step(&mut self, step: BenchStep) -> Task<Message> {
        let canvas = &self.interaction_state.pcb_canvas;
        let size = canvas.viewport_size();
        let moved = match step {
            BenchStep::Pan { dx, dy } => {
                canvas.pan_by(dx * size.width, dy * size.height);
                true
            }
            BenchStep::Zoom { factor } => canvas.zoom_by_factor_about_centre(factor),
        };
        if moved {
            self.update(Message::CanvasEvent(CanvasEvent::ViewMoved))
        } else {
            Task::none()
        }
    }
}

/// Write the CSV and the summary off the update thread, print the summary
/// line, and report back.
fn write_report_task(
    config: BenchConfig,
    report: BenchReport,
    mount: &'static str,
    view: (u32, u32),
) -> Task<Message> {
    Task::future(async move {
        let result =
            tokio::task::spawn_blocking(move || write_report(&config, &report, mount, view))
                .await
                .unwrap_or_else(|join| Err(format!("report writer stopped: {join}")));
        Message::PanZoomBench(BenchMsg::ReportWritten(result))
    })
}

fn write_report(
    config: &BenchConfig,
    report: &BenchReport,
    mount: &'static str,
    view: (u32, u32),
) -> Result<String, String> {
    let line = match report {
        BenchReport::BoardNotOpened => format!(
            "PAN-ZOOM-BENCH FAILED board not open after {} s: {}",
            crate::pan_zoom_bench::LOAD_TIMEOUT.as_secs(),
            config.board.display()
        ),
        BenchReport::Completed(samples) => {
            let csv = crate::pan_zoom_bench::csv_text(samples);
            write_file(&config.out, &csv)?;
            let context = RunContext {
                adapter: selected_adapter(),
                mount,
                // The setting the window was created with (`main.rs`).
                msaa: crate::fonts::read_antialiasing_pref(),
                view,
            };
            crate::pan_zoom_bench::summarize(samples, context).line()
        }
    };
    println!("{line}");
    write_file(&config.summary_path(), &format!("{line}\n"))?;
    Ok(line)
}

fn write_file(path: &std::path::Path, text: &str) -> Result<(), String> {
    signex_types::atomic_io::atomic_write(path, text.as_bytes())
        .map_err(|error| format!("{}: {error}", path.display()))
}

/// The adapter the `GPU-ADAPTER-SELECTED` diagnostic named, if the log still
/// holds it.
fn selected_adapter() -> String {
    crate::diagnostics::recent_entries()
        .iter()
        .rev()
        .filter(|entry| entry.code == "GPU-ADAPTER-SELECTED")
        .find_map(|entry| crate::pan_zoom_bench::adapter_label(&entry.message))
        .unwrap_or_else(|| "unknown".to_string())
}
