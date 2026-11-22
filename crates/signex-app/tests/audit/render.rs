//! Headless wgpu rendering of the app's windows to RGBA pixels and PNG files.
//!
//! Shared by the `audit` and `graphite_screens` test binaries. Build one
//! renderer per process with [`gpu`] and reuse it: the font system and the
//! wgpu device are process-wide.

use iced::advanced::renderer::Style;
use iced::{Color, Font, Pixels, Size};
use iced_runtime::UserInterface;
use iced_runtime::user_interface::Cache;
use iced_wgpu::wgpu;
use signex_app::app::Signex;
use std::path::Path;

/// The main window size every surface is laid out at.
pub const WIDTH: u32 = 1440;
pub const HEIGHT: u32 = 900;
/// The size a dialog opened as its own window is drawn at.
pub const DETACHED_SIZE: (u32, u32) = (1000, 720);

/// A headless wgpu renderer with the app's bundled fonts loaded, or `None`
/// on a machine with no adapter.
pub fn gpu() -> Option<iced::Renderer> {
    let instance = wgpu::Instance::default();
    let adapter =
        futures::executor::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: None,
            force_fallback_adapter: false,
            apply_limit_buckets: false,
        }))
        .ok()?;
    let (device, queue) =
        futures::executor::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("headless_render_device"),
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
        None,
        iced::advanced::graphics::Shell::headless(),
    );
    let mut fonts = iced::advanced::graphics::text::font_system()
        .write()
        .expect("font system");
    for face in signex_app::fonts::bundled::BUNDLED_FONTS {
        fonts.load_font(std::borrow::Cow::Borrowed(face));
    }
    drop(fonts);
    Some(iced::Renderer::Primary(iced_wgpu::Renderer::new(
        engine,
        iced::advanced::renderer::Settings {
            font: Font::new("Roboto"),
            text_size: Pixels(16.0),
            ..Default::default()
        },
    )))
}

/// Draw the app's main window at [`WIDTH`] x [`HEIGHT`].
pub fn draw_main(renderer: &mut iced::Renderer, app: &Signex) -> Vec<u8> {
    let window = app.ui_state.main_window_id.expect("main window");
    draw_window(renderer, app, window, (WIDTH, HEIGHT))
}

/// Draw window `window` at `(w, h)` physical pixels and return its RGBA.
pub fn draw_window(
    renderer: &mut iced::Renderer,
    app: &Signex,
    window: iced::window::Id,
    (w, h): (u32, u32),
) -> Vec<u8> {
    let theme = app.theme(window).expect("theme");
    let size = Size::new(w as f32, h as f32);
    let mut ui = UserInterface::build(app.view(window), size, Cache::default(), renderer);
    // The canvas applies a pending fit and learns its size on a redraw event.
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
        Size::new(w, h),
        iced::advanced::renderer::Scale::default(),
    );
    let iced::Renderer::Primary(wgpu_renderer) = renderer else {
        unreachable!("built as Primary")
    };
    wgpu_renderer.screenshot(&viewport, theme.palette().background.base.color)
}

/// The newest dialog the app opened as its own window, if any: a confirm
/// opened over a dialog is the one on top. Window ids only ever grow, while
/// the map's order changes from run to run.
pub fn detached_modal(app: &Signex) -> Option<iced::window::Id> {
    app.ui_state
        .windows
        .iter()
        .filter(|(_, kind)| format!("{kind:?}").starts_with("DetachedModal"))
        .map(|(id, _)| *id)
        .max()
}

/// A rectangle of a drawn window, in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Crop {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

/// Cut `rect` out of an RGBA image of `(w, h)`, clamped to the image.
pub fn crop(rgba: &[u8], (w, h): (u32, u32), rect: Crop) -> (Vec<u8>, (u32, u32)) {
    let x0 = rect.x.min(w);
    let y0 = rect.y.min(h);
    let cw = rect.w.min(w - x0);
    let ch = rect.h.min(h - y0);
    let stride = w as usize * 4;
    let mut out = Vec::with_capacity(cw as usize * ch as usize * 4);
    for row in y0..y0 + ch {
        let start = row as usize * stride + x0 as usize * 4;
        out.extend_from_slice(&rgba[start..start + cw as usize * 4]);
    }
    (out, (cw, ch))
}

/// Write `rgba` of `(w, h)` as `dir/name`.
pub fn save_png(rgba: &[u8], dir: &Path, name: &str, (w, h): (u32, u32)) -> Result<(), String> {
    image::save_buffer(dir.join(name), rgba, w, h, image::ColorType::Rgba8)
        .map_err(|e| format!("write {name}: {e}"))
}
