//! Renders the Sample Project with iced antialiasing off and on, at Fit All,
//! about 200 % and the zoom limit, in one dark and one light theme, and times
//! the draws (#2022 MSAA trial). A tool, not a test: it needs a GPU adapter.
//!
//! ```text
//! MSAA_OUT=/some/dir cargo run -p signex-app --example msaa_compare
//! MSAA_SUFFIX=-snapped MSAA_OUT=/some/dir cargo run -p signex-app --example msaa_compare
//! ```

#![allow(
    clippy::let_underscore_must_use,
    reason = "a tool: it discards Results with `let _ = ...` like the integration tests"
)]

#[path = "../tests/regression/settle_support.rs"]
#[allow(dead_code)]
mod settle_support;

use iced::advanced::graphics::Antialiasing;
use iced::advanced::renderer::Style;
use iced::{Color, Font, Pixels, Size};
use iced_runtime::UserInterface;
use iced_runtime::user_interface::Cache;
use iced_wgpu::wgpu;
use signex_app::app::{Message, PreferencesMsg, Signex};
use signex_app::canvas::CanvasEvent;
use signex_app::menu_bar::MenuMessage;
use signex_app::preferences::PrefMsg;
use signex_types::theme::ThemeId;
use std::path::PathBuf;

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 800;

fn gpu(antialiasing: Option<Antialiasing>) -> Option<(iced::Renderer, String)> {
    let instance = wgpu::Instance::default();
    let adapter =
        futures::executor::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: None,
            force_fallback_adapter: false,
            apply_limit_buckets: false,
        }))
        .ok()?;
    let name = adapter.get_info().name;
    let (device, queue) =
        futures::executor::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("msaa_compare_device"),
            required_features: wgpu::Features::empty(),
            required_limits: adapter.limits(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
        }))
        .ok()?;
    let engine = iced_wgpu::Engine::new(
        &adapter,
        device,
        queue,
        wgpu::TextureFormat::Rgba8Unorm,
        antialiasing,
        iced::advanced::graphics::Shell::headless(),
    );
    let mut fonts = iced::advanced::graphics::text::font_system()
        .write()
        .expect("font system");
    for face in signex_app::fonts::bundled::BUNDLED_FONTS {
        fonts.load_font(std::borrow::Cow::Borrowed(face));
    }
    drop(fonts);
    Some((
        iced::Renderer::Primary(iced_wgpu::Renderer::new(
            engine,
            iced::advanced::renderer::Settings {
                font: Font::new("Roboto"),
                text_size: Pixels(16.0),
                ..Default::default()
            },
        )),
        name,
    ))
}

fn draw(renderer: &mut iced::Renderer, app: &Signex) -> Vec<u8> {
    let window = app.ui_state.main_window_id.expect("main window");
    let theme = app.theme(window).expect("theme");
    let size = Size::new(WIDTH as f32, HEIGHT as f32);
    let mut ui = UserInterface::build(app.view(window), size, Cache::default(), renderer);
    let mut ignored = iced::advanced::shell::Bus::new();
    let waker = iced::advanced::shell::Waker::noop();
    let _ = ui.update(
        &iced::window::Headless,
        &waker,
        &[iced::Event::Window(iced::window::Event::RedrawRequested(
            std::time::Instant::now(),
        ))],
        iced::mouse::Cursor::Unavailable,
        renderer,
        &mut ignored,
    );
    ui.draw(
        renderer,
        &theme,
        &Style {
            text_color: Color::WHITE,
        },
        iced::mouse::Cursor::Unavailable,
    );
    let viewport = iced::advanced::graphics::Viewport::with_physical_size(
        Size::new(WIDTH, HEIGHT),
        iced::advanced::renderer::Scale::default(),
    );
    let iced::Renderer::Primary(wgpu_renderer) = renderer else {
        unreachable!("built as Primary")
    };
    wgpu_renderer.screenshot(&viewport, Color::BLACK)
}

fn sample_app(theme: ThemeId, renderer: &mut iced::Renderer) -> Signex {
    let (mut app, _t) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    settle_support::settle(&mut app, Message::Menu(MenuMessage::OpenSampleProject));
    let _ = app.update(Message::Preferences(PreferencesMsg::Inner(
        PrefMsg::DraftTheme(theme),
    )));
    let _ = app.update(Message::Preferences(PreferencesMsg::Inner(PrefMsg::Save)));
    let _ = draw(renderer, &app);
    let _ = app.update(Message::CanvasEvent(CanvasEvent::FitAll));
    app
}

/// Zoom in about the centre until the zoom reaches `percent` (or the limit).
fn zoom_to(app: &mut Signex, percent: f64) {
    let canvas = app.interaction_state.active_canvas_mut();
    canvas.set_viewport(iced::Rectangle::new(
        iced::Point::ORIGIN,
        Size::new(WIDTH as f32 * 0.7, HEIGHT as f32 * 0.9),
    ));
    while canvas.zoom_percent() < percent && canvas.zoom_about_centre(true, 1.25) {}
}

fn main() {
    let out = PathBuf::from(std::env::var("MSAA_OUT").expect("MSAA_OUT directory"));
    std::fs::create_dir_all(&out).expect("out dir");
    // `MSAA_SUFFIX=-snapped` writes a second set next to an earlier one.
    let suffix = std::env::var("MSAA_SUFFIX").unwrap_or_default();
    for (label, aa) in [("off", None), ("msaa4", Some(Antialiasing::MSAAx4))] {
        let Some((mut renderer, adapter)) = gpu(aa) else {
            eprintln!("no wgpu adapter: skipping");
            return;
        };
        eprintln!("adapter: {adapter}");
        for (theme, theme_name) in [
            (ThemeId::Signex, "dark"),
            (ThemeId::SolarizedLight, "light"),
        ] {
            let mut app = sample_app(theme, &mut renderer);
            for (zoom, percent) in [("fit", 0.0), ("200pct", 200.0), ("max", 1.0e9)] {
                zoom_to(&mut app, percent);
                let _ = draw(&mut renderer, &app);
                let mut rgba = Vec::new();
                let mut per_frame = std::time::Duration::MAX;
                for _ in 0..20 {
                    let start = std::time::Instant::now();
                    rgba = draw(&mut renderer, &app);
                    per_frame = per_frame.min(start.elapsed());
                }
                let shown = app.interaction_state.active_canvas_mut().zoom_percent();
                eprintln!(
                    "{label} {theme_name} {zoom} ({shown:.0}%): {per_frame:?} best of 20 (build+draw+readback)"
                );
                image::save_buffer(
                    out.join(format!("{label}{suffix}-{theme_name}-{zoom}.png")),
                    &rgba,
                    WIDTH,
                    HEIGHT,
                    image::ColorType::Rgba8,
                )
                .expect("write png");
            }
        }
    }
    // Pixel difference inside the canvas area, and a 4x montage (off | on) of
    // a patch around the circuit, for the eye. Off is compared from this run
    // and, when it exists, from the earlier run without the suffix.
    let differing = |a: &image::RgbaImage, b: &image::RgbaImage| {
        let mut count = 0_u32;
        for y in 67..550 {
            for x in 246..1055 {
                let (p, q) = (a.get_pixel(x, y), b.get_pixel(x, y));
                if (0..3).any(|c| p[c].abs_diff(q[c]) > 8) {
                    count += 1;
                }
            }
        }
        count
    };
    for theme in ["dark", "light"] {
        for zoom in ["fit", "200pct", "max"] {
            let load = |name: String| image::open(out.join(name)).map(|i| i.to_rgba8());
            let off = load(format!("off{suffix}-{theme}-{zoom}.png")).expect("off png");
            let on = load(format!("msaa4{suffix}-{theme}-{zoom}.png")).expect("on png");
            let total = 483 * 809;
            eprintln!(
                "{theme} {zoom}: msaa vs off (same run) {} of {total} canvas pixels differ by more than 8",
                differing(&off, &on)
            );
            if !suffix.is_empty()
                && let Ok(original) = load(format!("off-{theme}-{zoom}.png"))
            {
                eprintln!(
                    "{theme} {zoom}: msaa vs the earlier unsnapped off {} differ",
                    differing(&original, &on)
                );
            }
            let (x0, y0, w, h, k) = (400_u32, 300_u32, 120_u32, 80_u32, 4_u32);
            let mut montage = image::RgbaImage::new(w * k * 2 + 10, h * k);
            for (side, img) in [(0, &off), (1, &on)] {
                for y in 0..h * k {
                    for x in 0..w * k {
                        montage.put_pixel(
                            side * (w * k + 10) + x,
                            y,
                            *img.get_pixel(x0 + x / k, y0 + y / k),
                        );
                    }
                }
            }
            montage
                .save(out.join(format!("compare{suffix}-{theme}-{zoom}.png")))
                .expect("montage");
        }
    }
}
