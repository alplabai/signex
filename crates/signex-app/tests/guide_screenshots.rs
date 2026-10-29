//! Renders the screenshots in `docs/USER_GUIDE.md` (#322).
//!
//! Not part of the normal test run: it needs a GPU adapter and rewrites the
//! committed PNGs. Regenerate them with
//!
//! ```text
//! cargo test -p signex-app --test guide_screenshots -- --ignored
//! ```
//!
//! The app is driven through its own `update`, then `Signex::view` is laid out
//! and drawn by iced's wgpu renderer into an offscreen texture and read back.
//! A machine with no adapter prints a note and skips instead of failing.

#![expect(
    clippy::let_underscore_must_use,
    reason = "integration tests discard Results with `let _ = ...` routinely; see regression.rs"
)]

#[path = "regression/settle_support.rs"]
#[allow(dead_code)]
mod settle_support;

use iced::advanced::renderer::Style;
use iced::{Color, Font, Pixels, Size};
use iced_runtime::UserInterface;
use iced_runtime::user_interface::Cache;
use iced_wgpu::wgpu;
use signex_app::app::{ErcMsg, FileMsg, Message, Signex};
use signex_app::canvas::CanvasEvent;
use signex_types::schematic::{
    Graphic, HAlign, Label, LabelType, LibGraphic, LibPin, LibSymbol, Pin, PinDirection,
    PinShapeStyle, Point, SchematicSheet, Symbol, VAlign, Wire,
};
use std::path::PathBuf;

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 800;

fn gpu() -> Option<iced::Renderer> {
    let instance = wgpu::Instance::default();
    let adapter =
        futures::executor::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))
        .ok()?;
    let (device, queue) =
        futures::executor::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("guide_screenshots_device"),
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
        Font::with_name("Roboto"),
        Pixels(16.0),
    )))
}

/// Draw the app's main window and return its RGBA pixels.
fn draw(renderer: &mut iced::Renderer, app: &Signex) -> Vec<u8> {
    let window = app.ui_state.main_window_id.expect("main window");
    let theme = app.theme(window).expect("theme");
    let size = Size::new(WIDTH as f32, HEIGHT as f32);
    let mut ui = UserInterface::build(app.view(window), size, Cache::default(), renderer);
    // The canvas applies a pending fit and learns its size on a redraw event.
    let mut ignored = Vec::new();
    let _ = ui.update(
        &[iced::Event::Window(iced::window::Event::RedrawRequested(
            std::time::Instant::now(),
        ))],
        iced::mouse::Cursor::Unavailable,
        renderer,
        &mut iced::advanced::clipboard::Null,
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
    let viewport =
        iced::advanced::graphics::Viewport::with_physical_size(Size::new(WIDTH, HEIGHT), 1.0);
    let iced::Renderer::Primary(wgpu_renderer) = renderer else {
        unreachable!("built as Primary")
    };
    wgpu_renderer.screenshot(&viewport, Color::BLACK)
}

/// Draw the app and write it to `assets/screenshots/<name>`.
fn shoot(renderer: &mut iced::Renderer, app: &Signex, name: &str) {
    let rgba = draw(renderer, app);
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/screenshots");
    image::save_buffer(
        dir.join(name),
        &rgba,
        WIDTH,
        HEIGHT,
        image::ColorType::Rgba8,
    )
    .expect("write png");
}

/// A resistor (rectangle body, two pins) with a wire on pin 2 ending in a net label.
fn sheet() -> SchematicSheet {
    let pin = |y: f64, rotation: f64, number: &str| LibPin {
        unit: 0,
        body_style: 1,
        pin: Pin {
            direction: PinDirection::Passive,
            shape_style: PinShapeStyle::Plain,
            position: Point::new(0.0, y),
            rotation,
            length: 2.54,
            name: number.to_string(),
            number: number.to_string(),
            visible: true,
            name_visible: false,
            number_visible: true,
        },
    };
    let mut lib: LibSymbol =
        serde_json::from_value(serde_json::json!({ "id": "Device:R", "reference": "R" }))
            .expect("lib symbol");
    lib.graphics.push(LibGraphic {
        unit: 0,
        body_style: 1,
        graphic: Graphic::Rectangle {
            start: Point::new(-1.016, -2.54),
            end: Point::new(1.016, 2.54),
            width: 0.254,
            fill: signex_types::schematic::FillType::None,
        },
    });
    lib.pins.push(pin(5.08, 90.0, "1"));
    lib.pins.push(pin(-5.08, 270.0, "2"));
    let mut sheet = SchematicSheet {
        paper_size: "A4".into(),
        ..Default::default()
    };
    sheet.lib_symbols.insert(lib.id.clone(), lib);
    sheet.symbols.push(Symbol {
        uuid: uuid::Uuid::nil(),
        lib_id: "Device:R".into(),
        reference: "R1".into(),
        value: "10k".into(),
        position: Point::new(100.0, 90.0),
        ..Symbol::default()
    });
    sheet.wires.push(Wire {
        uuid: uuid::Uuid::nil(),
        start: Point::new(100.0, 95.08),
        end: Point::new(115.0, 95.08),
        stroke_width: 0.0,
        unknown_columns: Default::default(),
        locked: false,
    });
    sheet.labels.push(Label {
        uuid: uuid::Uuid::nil(),
        text: "SENSE".into(),
        position: Point::new(115.0, 95.08),
        rotation: 0.0,
        label_type: LabelType::Net,
        shape: String::new(),
        font_size: 1.27,
        font_family: None,
        justify: HAlign::Left,
        justify_v: VAlign::Bottom,
        unknown_columns: Default::default(),
        locked: false,
    });
    sheet
}

#[test]
#[ignore = "needs a GPU adapter; rewrites assets/screenshots/guide-*.png"]
fn render_guide_screenshots() {
    let Some(mut renderer) = gpu() else {
        eprintln!("no wgpu adapter: skipping guide screenshots");
        return;
    };
    let (mut app, _task) = Signex::new();
    shoot(&mut renderer, &app, "guide-first-run.png");

    app.ui_state.first_run_tour_open = false;
    let _ = app.update(Message::File(FileMsg::SchematicOpenFinished {
        path: PathBuf::from("guide.snxsch"),
        title: "guide".into(),
        result: Ok(Box::new(sheet())),
    }));
    // The canvas learns its size from a first frame; only then can it fit.
    let _ = draw(&mut renderer, &app);
    let _ = app.update(Message::CanvasEvent(CanvasEvent::FitAll));
    shoot(&mut renderer, &app, "guide-wired-symbol.png");

    settle_support::settle(&mut app, Message::Erc(ErcMsg::Run));
    shoot(&mut renderer, &app, "guide-erc.png");
}
