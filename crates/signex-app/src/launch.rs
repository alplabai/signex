//! Running the application: the body of the Community `main`, and the entry
//! point a binary built on this crate calls with its extensions (#2495,
//! `docs/EXTENSION_API.md`).

use std::cell::Cell;

use crate::app::Signex;
use crate::extension::{Extension, ExtensionHost};
use crate::fonts::bundled;
use crate::startup_args::StartupAction;
use crate::{diagnostics, fonts, gpu_backend, pan_zoom_bench, startup_args};

/// Run the application with `extensions` installed. The Community binary
/// passes none. Returns when the last window closes; exits the process
/// (status 2) on a refused launch, an invalid extension set included.
pub fn run(extensions: Vec<Box<dyn Extension>>) -> iced::Result {
    // `--version` / `--help` must answer before logging, fonts, iced or
    // wgpu are touched, so they work on a machine with no display or GPU.
    let startup_paths = match startup_args::startup_action_from_env() {
        StartupAction::PrintVersion => {
            println!("{}", startup_args::version_text());
            return Ok(());
        }
        StartupAction::PrintHelp => {
            println!("{}", startup_args::help_text());
            return Ok(());
        }
        // File associations launch `signex <path>...` (#317). Read once;
        // the boot closure hands each path to the normal open flow.
        StartupAction::Launch(paths) => paths,
        // A relative path with no readable working directory (#2619).
        StartupAction::Refused(why) => {
            eprintln!("[signex] {why}");
            std::process::exit(2);
        }
    };

    // Installed before any window, so a bad set refuses the launch the way
    // a bad argument does. Taken by the boot closure, which iced calls once.
    let extensions = match ExtensionHost::install(extensions) {
        Ok(mut host) => {
            // Read once, before any window, like the UI font below. The
            // Community binary installs none and reads nothing.
            if !host.is_empty() {
                host.load_preferences(&fonts::read_extension_prefs());
            }
            Cell::new(Some(host))
        }
        Err(error) => {
            eprintln!("[signex] {error}");
            std::process::exit(2);
        }
    };

    gpu_backend::prefer_dx12_on_windows();

    // The pan/zoom frame bench, a dev/QA mode (#2472, docs/PERFORMANCE.md):
    // it opens its own board instead of the command-line documents.
    let bench = match pan_zoom_bench::BenchConfig::from_env() {
        None => None,
        Some(Ok(config)) if config.board.is_file() => Some(config),
        Some(Ok(config)) => {
            eprintln!(
                "[signex] {}: no such board file: {}",
                pan_zoom_bench::ENV_BOARD,
                config.board.display()
            );
            std::process::exit(2);
        }
        Some(Err(error)) => {
            eprintln!("[signex] {error}");
            std::process::exit(2);
        }
    };

    if let Err(error) = diagnostics::init_logging() {
        eprintln!("[signex] failed to initialize logging: {error:#}");
    }

    // Read the persisted UI font preference (defaults to "Inter").
    let ui_font = signex_widgets::ui_font::set_launch_family(&fonts::read_ui_font_pref());

    iced::daemon(
        move || {
            let (app, task) = match &bench {
                Some(config) => Signex::boot_pan_zoom_bench(config.clone()),
                None => Signex::boot(startup_paths.clone()),
            };
            (
                app.with_extensions(extensions.take().unwrap_or_default()),
                task,
            )
        },
        Signex::update,
        Signex::view,
    )
    // macOS: the app installs its own menu bar (`.menu` below), so winit's
    // default one is off, and Command+Q / Dock Quit / logout reach
    // `system::quit_requests` for the unsaved-changes guard instead of
    // quitting at once. See docs/MENU_BAR.md, "Native macOS menu".
    .settings(iced::Settings {
        default_menu: false,
        exit_on_quit_request: false,
        ..iced::Settings::default()
    })
    .menu(Signex::native_menu)
    .title(Signex::title)
    .theme(Signex::theme)
    .subscription(Signex::subscription)
    .scale_factor(Signex::scale_factor)
    // 4x multisampling for canvas meshes (vector outlines of oversize text,
    // polygon fills). Only iced's mesh pass is multisampled, and it resolves
    // into the single-sample frame, so signex-gfx's own pipelines are unaffected.
    .antialiasing(fonts::read_antialiasing_pref())
    // Iosevka (canvas text), Roboto and Inter (UI chrome), JetBrains Mono
    // (chrome-only mono text) and iced_aw's icon font; the list is shared
    // with the test layout probe.
    .fonts(bundled::registered())
    // Default UI font resolves through the Preferences-panel pick. When
    // it matches a bundled family ("Inter" / "Roboto" / "Iosevka") iced uses the
    // embedded TTF directly; otherwise it falls back to a system font.
    .default_font(ui_font)
    .run()
}
