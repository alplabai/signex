//! The bench's script, state machine and maths, headless: frame instants are
//! made up, so every run gives the same numbers.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use iced::time::Instant;

use super::*;

fn config(seconds: u32) -> BenchConfig {
    BenchConfig {
        board: PathBuf::from("board.snxpcb"),
        out: PathBuf::from("out.csv"),
        seconds,
    }
}

fn context(adapter: &str) -> RunContext {
    RunContext {
        adapter: adapter.to_string(),
        mount: "cpu",
        msaa: true,
        view: (1400, 900),
    }
}

fn vars(list: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> {
    let list: Vec<(String, String)> = list
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    move |key| {
        list.iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| OsString::from(v))
    }
}

/// Drive a whole run: `load_frames` frames before the board is ready, then
/// one frame every `frame_us` µs. Returns the actions and the report.
fn run(seconds: u32, load_frames: u32, frame_us: u64) -> (Vec<FrameAction>, BenchReport) {
    let mut bench = PanZoomBench::new(config(seconds));
    let t0 = Instant::now();
    let mut actions = Vec::new();
    let mut n: u64 = 0;
    loop {
        let at = t0 + Duration::from_micros(frame_us * n);
        let action = bench.on_frame(at, n >= u64::from(load_frames));
        if let FrameAction::Finish(report) = action {
            assert!(!bench.wants_frames());
            return (actions, report);
        }
        actions.push(action);
        n += 1;
    }
}

#[test]
fn unset_board_turns_nothing_on() {
    let cwd = Path::new("/work");
    assert_eq!(BenchConfig::from_vars(vars(&[]), cwd), None);
    assert_eq!(
        BenchConfig::from_vars(vars(&[(ENV_BOARD, ""), (ENV_SECONDS, "5")]), cwd),
        None
    );
}

#[test]
fn config_reads_defaults_and_overrides() {
    let cwd = Path::new("/work");
    let config = BenchConfig::from_vars(vars(&[(ENV_BOARD, "b/pcb-10k.snxpcb")]), cwd)
        .expect("on")
        .expect("valid");
    assert_eq!(config.board, cwd.join("b/pcb-10k.snxpcb"));
    assert_eq!(config.out, cwd.join("b/pcb-10k.pan-zoom.csv"));
    assert_eq!(config.seconds, DEFAULT_SECONDS);
    assert_eq!(
        config.summary_path(),
        cwd.join("b/pcb-10k.pan-zoom.summary.txt")
    );

    let config = BenchConfig::from_vars(
        vars(&[
            (ENV_BOARD, "x.snxpcb"),
            (ENV_OUT, "r/run.csv"),
            (ENV_SECONDS, " 7 "),
        ]),
        cwd,
    )
    .expect("on")
    .expect("valid");
    assert_eq!(config.out, cwd.join("r/run.csv"));
    assert_eq!(config.seconds, 7);
    assert_eq!(config.summary_path(), cwd.join("r/run.summary.txt"));
}

#[test]
fn a_bad_duration_is_refused() {
    for bad in ["0", "-3", "ten", "1.5"] {
        let got = BenchConfig::from_vars(
            vars(&[(ENV_BOARD, "x"), (ENV_SECONDS, bad)]),
            Path::new("/"),
        );
        assert!(matches!(got, Some(Err(_))), "{bad} accepted");
    }
}

#[test]
fn the_script_is_a_closed_loop() {
    let (mut x, mut y, mut zoom) = (0.0_f64, 0.0_f64, 1.0_f64);
    for index in 0..CYCLE_LEN {
        match step_at(index) {
            BenchStep::Pan { dx, dy } => {
                x += f64::from(dx);
                y += f64::from(dy);
            }
            BenchStep::Zoom { factor } => zoom *= f64::from(factor),
        }
    }
    assert!(x.abs() < 1e-4 && y.abs() < 1e-4, "pans end at ({x}, {y})");
    assert!((zoom - 1.0).abs() < 1e-4, "zoom ends at {zoom}");
    // It zooms in, pans every way and zooms out.
    let labels: std::collections::BTreeSet<_> =
        (0..CYCLE_LEN).map(|i| step_at(i).label()).collect();
    assert_eq!(
        labels.into_iter().collect::<Vec<_>>(),
        [
            "pan_down",
            "pan_left",
            "pan_right",
            "pan_up",
            "zoom_in",
            "zoom_out"
        ]
    );
    // And repeats.
    for index in 0..CYCLE_LEN {
        assert_eq!(step_at(index), step_at(index + 3 * CYCLE_LEN));
    }
}

#[test]
fn a_run_waits_fits_settles_then_times_every_frame() {
    let (actions, report) = run(2, 5, 16_000);
    // Five frames before the board opens, then one fit.
    assert!(actions[..5].iter().all(|a| *a == FrameAction::Wait));
    assert_eq!(actions[5], FrameAction::FitBoard);
    let settle = SETTLE_FRAMES as usize;
    assert!(
        actions[6..6 + settle]
            .iter()
            .all(|a| *a == FrameAction::Wait)
    );
    // From then on every frame moves the camera by the next script step.
    for (index, action) in actions[6 + settle..].iter().enumerate() {
        assert_eq!(*action, FrameAction::Apply(step_at(index)));
    }
    let BenchReport::Completed(samples) = report else {
        panic!("the board opened");
    };
    // 2 s at 16 ms: 125 timed frames, each showing the step before it.
    assert_eq!(samples.len(), 125);
    assert!(samples.iter().all(|s| (s.frame_ms - 16.0).abs() < 1e-9));
    assert_eq!(samples[0].action, step_at(0).label());
    assert!((samples[124].t_ms - 2000.0).abs() < 1e-9);
}

#[test]
fn the_same_frames_give_the_same_report() {
    assert_eq!(run(3, 2, 8_333).1, run(3, 2, 8_333).1);
}

#[test]
fn a_board_that_never_opens_gives_up() {
    let (actions, report) = run(1, u32::MAX, 100_000);
    assert_eq!(report, BenchReport::BoardNotOpened);
    let frames = actions.len() as u128;
    assert_eq!(frames, LOAD_TIMEOUT.as_millis() / 100);
}

#[test]
fn summary_maths() {
    // 100 frames: 1..=100 ms.
    let samples: Vec<FrameSample> = (1..=100)
        .map(|ms| FrameSample {
            t_ms: 0.0,
            frame_ms: f64::from(ms),
            action: "pan_right",
        })
        .rev()
        .collect();
    let summary = summarize(&samples, context("AMD Radeon 880M (Vulkan)"));
    assert_eq!(summary.frames, 100);
    assert_eq!(summary.p50_ms, 50.0);
    assert_eq!(summary.p95_ms, 95.0);
    assert_eq!(summary.p99_ms, 99.0);
    assert_eq!(summary.max_ms, 100.0);
    assert!((summary.seconds - 5.05).abs() < 1e-9);
    assert!((summary.fps - 100.0 / 5.05).abs() < 1e-9);
    assert!(!summary.within_budget());
    assert_eq!(
        summary.line(),
        "PAN-ZOOM-BENCH frames=100 seconds=5.05 p50_ms=50.00 p95_ms=95.00 p99_ms=99.00 \
         max_ms=100.00 fps=19.8 budget_60fps=OVER mount=cpu msaa=x4 view=1400x900 \
         adapter=\"AMD Radeon 880M (Vulkan)\""
    );

    let fast = vec![
        FrameSample {
            t_ms: 0.0,
            frame_ms: 8.0,
            action: "zoom_in",
        };
        10
    ];
    let summary = summarize(&fast, context("x"));
    assert!(summary.within_budget());
    assert_eq!(summary.fps, 125.0);

    let empty = summarize(&[], context("x"));
    assert_eq!((empty.frames, empty.p95_ms, empty.fps), (0, 0.0, 0.0));
    assert!(!empty.within_budget());
}

#[test]
fn csv_has_one_row_per_frame() {
    let samples = [
        FrameSample {
            t_ms: 0.0,
            frame_ms: 16.6667,
            action: "zoom_in",
        },
        FrameSample {
            t_ms: 16.6667,
            frame_ms: 8.25,
            action: "pan_up",
        },
    ];
    assert_eq!(
        csv_text(&samples),
        "frame,t_ms,frame_ms,action\n0,0.000,16.667,zoom_in\n1,16.667,8.250,pan_up\n"
    );
}

#[test]
fn adapter_label_reads_the_diagnostic() {
    assert_eq!(
        adapter_label("Graphics adapter selected: AMD Radeon 880M (Vulkan)").as_deref(),
        Some("AMD Radeon 880M (Vulkan)")
    );
    assert_eq!(adapter_label("Surface formats available: x"), None);
}
