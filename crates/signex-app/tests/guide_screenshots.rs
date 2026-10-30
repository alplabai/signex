//! Renders the screenshots in `docs/USER_GUIDE.md` (#322) and the README (#1724).
//!
//! Not part of the normal test run: it needs a GPU adapter and rewrites the
//! committed PNGs. Regenerate them with
//!
//! ```text
//! cargo test -p signex-app --test guide_screenshots -- --ignored
//! ```
//!
//! `render_readme_screenshots` writes `hero.png`, `editor.png` and the three
//! `*-theme.png` files from a small `sensor-node` project; `render_guide_screenshots`
//! writes the `guide-*.png` files.
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
use signex_app::app::{ErcMsg, FileMsg, LoadedProject, Message, Signex, UiMsg};
use signex_app::canvas::CanvasEvent;
use signex_types::project::{ProjectData, SheetEntry};
use signex_types::schematic::{
    Graphic, HAlign, Junction, Label, LabelType, LibGraphic, LibPin, LibSymbol, Pin, PinDirection,
    PinShapeStyle, Point, SchematicSheet, Symbol, VAlign, Wire,
};
use signex_types::theme::ThemeId;
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

/// A two-pin box symbol (`Device:<name>`), the stand-in for R and C.
fn box_symbol(name: &str, reference: &str) -> LibSymbol {
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
            number_visible: false,
        },
    };
    let mut lib: LibSymbol = serde_json::from_value(
        serde_json::json!({ "id": format!("Device:{name}"), "reference": reference }),
    )
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
    lib.pins.push(pin(5.08, 270.0, "1"));
    lib.pins.push(pin(-5.08, 90.0, "2"));
    lib
}

/// An RC divider: VIN -> R1 -> VOUT -> R2 -> GND, with C1 from VOUT to GND.
fn readme_sheet() -> SchematicSheet {
    let mut sheet = SchematicSheet {
        paper_size: "A4".into(),
        ..Default::default()
    };
    for (name, reference) in [("R", "R"), ("C", "C")] {
        let lib = box_symbol(name, reference);
        sheet.lib_symbols.insert(lib.id.clone(), lib);
    }
    let part = |lib_id: &str, reference: &str, value: &str, x: f64, y: f64| Symbol {
        uuid: uuid::Uuid::nil(),
        lib_id: lib_id.into(),
        reference: reference.into(),
        value: value.into(),
        position: Point::new(x, y),
        ..Symbol::default()
    };
    sheet
        .symbols
        .push(part("Device:R", "R1", "10k", 100.0, 80.0));
    sheet
        .symbols
        .push(part("Device:R", "R2", "4.7k", 100.0, 110.0));
    sheet
        .symbols
        .push(part("Device:C", "C1", "100n", 130.0, 110.0));
    let wire = |x1: f64, y1: f64, x2: f64, y2: f64| Wire {
        uuid: uuid::Uuid::nil(),
        start: Point::new(x1, y1),
        end: Point::new(x2, y2),
        stroke_width: 0.0,
        unknown_columns: Default::default(),
        locked: false,
    };
    // R1 spans y 74.92..85.08, R2 and C1 span y 104.92..115.08.
    for (x1, y1, x2, y2) in [
        (80.0, 74.92, 100.0, 74.92),
        (100.0, 85.08, 100.0, 104.92),
        (100.0, 95.0, 130.0, 95.0),
        (130.0, 95.0, 130.0, 104.92),
        (150.0, 95.0, 130.0, 95.0),
        (100.0, 115.08, 100.0, 130.0),
        (130.0, 115.08, 130.0, 130.0),
        (100.0, 130.0, 130.0, 130.0),
        (100.0, 130.0, 80.0, 130.0),
    ] {
        sheet.wires.push(wire(x1, y1, x2, y2));
    }
    for (x, y) in [(100.0, 95.0), (130.0, 95.0), (100.0, 130.0)] {
        sheet.junctions.push(Junction {
            uuid: uuid::Uuid::nil(),
            position: Point::new(x, y),
            diameter: 0.0,
            minted: false,
            unknown_columns: Default::default(),
        });
    }
    let label = |text: &str, x: f64, y: f64, justify: HAlign| Label {
        uuid: uuid::Uuid::nil(),
        text: text.into(),
        position: Point::new(x, y),
        rotation: 0.0,
        label_type: LabelType::Net,
        shape: String::new(),
        font_size: 1.27,
        font_family: None,
        justify,
        justify_v: VAlign::Bottom,
        unknown_columns: Default::default(),
        locked: false,
    };
    sheet.labels.push(label("VIN", 80.0, 74.92, HAlign::Right));
    sheet.labels.push(label("VOUT", 150.0, 95.0, HAlign::Left));
    sheet.labels.push(label("GND", 80.0, 130.0, HAlign::Right));
    sheet
}

/// Open a small `Sensor Node` project with native file names, backed by real
/// (empty) files so the tree shows no `(missing)` marks.
fn open_readme_project(app: &mut Signex, dir: &std::path::Path) -> PathBuf {
    for name in [
        "sensor-node.snxprj",
        "sensor-node.snxsch",
        "power.snxsch",
        "sensor-node.snxpcb",
    ] {
        std::fs::write(dir.join(name), b"").expect("write placeholder");
    }
    let sheets = [
        ("Sensor Node", "sensor-node.snxsch"),
        ("Power", "power.snxsch"),
    ];
    let data = ProjectData {
        description: None,
        name: "sensor-node".into(),
        dir: dir.to_string_lossy().into_owned(),
        schematic_root: Some("sensor-node.snxsch".into()),
        pcb_file: Some("sensor-node.snxpcb".into()),
        sheets: sheets
            .iter()
            .map(|(name, filename)| SheetEntry {
                name: (*name).into(),
                filename: (*filename).into(),
                symbols_count: 0,
                wires_count: 0,
                labels_count: 0,
                unknown: Default::default(),
            })
            .collect(),
        variant_definitions: Vec::new(),
        active_variant: None,
        libraries: Vec::new(),
        enable_git: false,
        net_classes: Vec::new(),
        rules: None,
        annotation: None,
        parameters: Default::default(),
        unknown: Default::default(),
    };
    let id = app.document_state.mint_project_id();
    let prj = dir.join("sensor-node.snxprj");
    app.document_state.projects.push(LoadedProject {
        id,
        path: prj,
        data,
        pending_libraries: std::collections::HashMap::new(),
        case_fold: Default::default(),
        case_probed: false,
        case_decided: false,
    });
    app.document_state.active_project = Some(id);
    dir.join("sensor-node.snxsch")
}

#[test]
#[ignore = "needs a GPU adapter; rewrites the README screenshots in assets/screenshots"]
fn render_readme_screenshots() {
    let Some(mut renderer) = gpu() else {
        eprintln!("no wgpu adapter: skipping README screenshots");
        return;
    };
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let (mut app, _task) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    let sheet_path = open_readme_project(&mut app, tmp.path());
    let _ = app.update(Message::File(FileMsg::SchematicOpenFinished {
        path: sheet_path,
        title: "sensor-node".into(),
        result: Ok(Box::new(readme_sheet())),
    }));
    let _ = draw(&mut renderer, &app);
    let _ = app.update(Message::CanvasEvent(CanvasEvent::FitAll));

    shoot(&mut renderer, &app, "hero.png");
    // Select R1 so the Properties panel shows a component, as `editor.png` does.
    let _ = app.update(Message::CanvasEvent(CanvasEvent::Clicked {
        world_x: 100.0,
        world_y: 80.0,
    }));
    for (theme, name) in [
        (ThemeId::Signex, "editor.png"),
        (ThemeId::CatppuccinMocha, "catppuccin-theme.png"),
        (ThemeId::GitHubDark, "github-dark-theme.png"),
        (ThemeId::SolarizedLight, "solarized-theme.png"),
    ] {
        let _ = app.update(Message::Ui(UiMsg::ThemeChanged(theme)));
        // The chrome reads its tokens from the panel context and the canvas
        // its colours from the draft theme; the Preferences dialog moves both
        // on a theme pick, so mirror that here.
        app.ui_state.preferences_draft_theme = theme;
        // Toggling the grid twice drops the cached background layer (paper
        // and grid), which a theme pick through Preferences also repaints.
        let _ = app.update(Message::Ui(UiMsg::GridToggle));
        let _ = app.update(Message::Ui(UiMsg::GridToggle));
        app.document_state.panel_ctx.tokens = signex_types::theme::theme_tokens(theme);
        let _ = draw(&mut renderer, &app);
        shoot(&mut renderer, &app, name);
    }
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
