//! #1929: a headless run measures label text like the GUI does.
//!
//! The measurer and the font catalog are process-wide and set once, so each
//! path runs in a child process of this test binary: [`probe`] does the work
//! when `PROBE_ENV` names a path and is a no-op otherwise.

#![allow(
    clippy::disallowed_methods,
    reason = "the test re-runs its own binary as a child process"
)]

use signex_app::app::Signex;
use signex_app::fonts::catalog::{FontSet, FontsAlreadyInstalled, install_before_boot};
use signex_app::task_driver::TaskDriver;
use signex_types::schematic::{HAlign, SCHEMATIC_TEXT_MM, VAlign, text_aabb};

const PROBE_ENV: &str = "SIGNEX_FONT_PROBE_PATH";
const PROBE_LINE: &str = "LABEL_AABB ";
/// Not Iosevka, so a missing measurer falls back to the monospace estimate.
const FAMILY: &str = "Roboto";
const TEXT: &str = "SYSTEM_RESET_REQUEST_N";

/// The label box a child process measured on `path`.
fn measured_on(path: &str) -> String {
    let output = std::process::Command::new(std::env::current_exe().expect("test binary"))
        .args(["probe", "--exact", "--nocapture", "--test-threads=1"])
        .env(PROBE_ENV, path)
        .output()
        .expect("run the probe");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "probe {path} failed:\n{stdout}");
    stdout
        .lines()
        .find_map(|line| line.split_once(PROBE_LINE).map(|(_, aabb)| aabb))
        .unwrap_or_else(|| panic!("probe {path} printed no box:\n{stdout}"))
        .to_string()
}

#[test]
fn a_headless_run_measures_a_roboto_label_like_the_gui() {
    let gui = measured_on("gui");
    let headless = measured_on("headless");
    assert_eq!(headless, gui, "headless and GUI label boxes differ");
    assert_ne!(
        measured_on("none"),
        gui,
        "the probe must measure with real faces, not the monospace estimate"
    );
}

#[test]
fn probe() {
    let Ok(path) = std::env::var(PROBE_ENV) else {
        return;
    };
    match path.as_str() {
        // The GUI boots and lets the startup font scan land.
        "gui" => {
            let (mut app, boot) = Signex::boot(Vec::new());
            TaskDriver::new()
                .settle(&mut app, boot)
                .expect("boot settled");
        }
        // A headless run installs the fonts first and drives nothing.
        "headless" => {
            install_before_boot(FontSet::System).expect("first install");
            assert_eq!(
                install_before_boot(FontSet::BundledOnly),
                Err(FontsAlreadyInstalled),
                "a second set must not be kept silently"
            );
            let (_app, _boot) = Signex::new();
        }
        "none" => {
            let (_app, _boot) = Signex::new();
        }
        other => panic!("unknown probe path {other}"),
    }
    let aabb = text_aabb(
        TEXT,
        SCHEMATIC_TEXT_MM,
        (20.0, 30.0),
        0.0,
        HAlign::Left,
        VAlign::Bottom,
        Some(FAMILY),
    );
    println!("{PROBE_LINE}{aabb:?}");
}
