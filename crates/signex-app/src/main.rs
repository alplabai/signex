//! Signex EDA — AI-first electronics design automation.
//!
//! Thin binary entrypoint. The real implementation lives in the
//! library face (`lib.rs`) so integration tests can drive
//! dispatchers without spinning up the iced runtime.

use signex_app::app::Signex;
use signex_app::fonts::bundled;
use signex_app::startup_args::StartupAction;
use signex_app::{diagnostics, fonts, gpu_backend, notebook_cli, pan_zoom_bench, startup_args};

fn main() -> iced::Result {
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
        // `signex notebook …` (#2945): headless, no window.
        StartupAction::Notebook(command) => {
            std::process::exit(i32::from(notebook_cli::main(command)));
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
        move || match &bench {
            Some(config) => Signex::boot_pan_zoom_bench(config.clone()),
            None => Signex::boot(startup_paths.clone()),
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
