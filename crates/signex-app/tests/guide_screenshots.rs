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
use signex_app::app::{
    ErcMsg, FileMsg, LoadedProject, Message, PreferencesMsg, Signex, TabInfo, TabKind,
};
use signex_app::canvas::{BoxDirection, CanvasEvent};
use signex_app::library::LibraryMessage;
use signex_app::preferences::PrefMsg;
use signex_types::project::{ProjectData, SheetEntry};
use signex_types::schematic::{
    Graphic, HAlign, Label, LabelType, LibGraphic, LibPin, LibSymbol, Pin, PinDirection,
    PinShapeStyle, Point, SchematicSheet, Symbol, SymbolTransform, VAlign, Wire,
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

/// The Stock library that ships with the app, mounted the way first run does.
fn stock_file() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/stock-library/Stock.snxlib")
}

/// Place Stock part `internal_pn` at `at` through the Components panel's own
/// message and a canvas click, so the symbol, library id, footprint and field
/// placement are what a user gets.
fn place_stock(app: &mut Signex, internal_pn: &str, at: (f64, f64)) {
    let (_, row) = app
        .library
        .all_components()
        .into_iter()
        .find(|(_, c)| c.internal_pn.as_str() == internal_pn)
        .unwrap_or_else(|| panic!("Stock lists {internal_pn}"));
    let table = app
        .library
        .table_of_row(&stock_file(), row.row_id)
        .expect("row table");
    settle_support::settle(
        app,
        Message::Library(LibraryMessage::ComponentsPanelPlace {
            library_path: stock_file(),
            table,
            row_id: signex_library::RowId(row.row_id),
        }),
    );
    let _ = app.update(Message::CanvasEvent(CanvasEvent::Clicked {
        world_x: at.0,
        world_y: at.1,
    }));
}

/// Place the sensor-node parts from Stock on a scratch sheet and return it.
fn placed_parts() -> SchematicSheet {
    let path = PathBuf::from("scratch.snxsch");
    let sheet = SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    let (mut app, _task) = Signex::new();
    app.document_state.engines.insert(
        path.clone(),
        signex_engine::Engine::new(sheet).expect("engine"),
    );
    app.document_state.tabs.push(TabInfo {
        title: "scratch".into(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path);
    app.library.stock_library_root = Some(stock_file());
    app.library
        .open_library_read_only(stock_file())
        .expect("mount stock");
    for (pn, at, _, _) in PARTS {
        place_stock(&mut app, pn, *at);
    }
    app.document_state
        .active_engine()
        .expect("engine")
        .document()
        .clone()
}

/// (Stock part, position in mm, value, footprint).
type Part = (&'static str, (f64, f64), &'static str, &'static str);

const PARTS: &[Part] = &[
    (
        "STOCK-Conn_1x04",
        (60.96, 101.6),
        "I2C",
        "Header-1x04-2.54mm",
    ),
    ("STOCK-R", (88.9, 71.12), "10k", "Chip-0805"),
    ("STOCK-R", (104.14, 71.12), "10k", "Chip-0805"),
    ("STOCK-IC_Generic", (127.0, 101.6), "MCU", "SOIC-8"),
    ("STOCK-C", (88.9, 134.62), "100n", "Chip-0805"),
    ("STOCK-C", (104.14, 134.62), "10u", "Chip-0805"),
];

/// Where a symbol's designator and value sit, as `(dx, dy)` from its origin:
/// beside the body for the two-pin parts, above and below it otherwise.
fn field_offsets(reference: &str) -> ((f64, f64), (f64, f64)) {
    match reference.chars().next() {
        Some('R' | 'C') => ((2.8, -1.4), (2.8, 1.6)),
        Some('J') => ((-1.5, -6.5), (-1.5, 7.0)),
        _ => ((-7.6, -8.5), (-7.6, 9.0)),
    }
}

fn field_text(position: Point) -> signex_types::schematic::TextProp {
    signex_types::schematic::TextProp {
        position,
        rotation: 0.0,
        font_size: 1.7,
        font_family: None,
        justify_h: HAlign::Left,
        justify_v: VAlign::Center,
        hidden: false,
    }
}

/// The Stock part name behind a designator.
fn part_name(reference: &str) -> &'static str {
    match reference.chars().next() {
        Some('R') => "R",
        Some('C') => "C",
        Some('J') => "Conn_1x04",
        _ => "IC_Generic",
    }
}

/// The sensor node: an I2C header, two pull-ups, a small MCU and decoupling,
/// placed from Stock and wired with net labels.
fn readme_sheet() -> SchematicSheet {
    let mut sheet = placed_parts();
    let mut tips = std::collections::HashMap::new();
    for (sym, (_, _, value, footprint)) in sheet.symbols.iter_mut().zip(PARTS) {
        // Stock lists parts by library uuid; show the readable `Stock:<part>` id.
        let readable = format!("Stock:{}", part_name(&sym.reference));
        if let Some(mut lib) = sheet.lib_symbols.remove(&sym.lib_id) {
            lib.id = readable.clone();
            sheet.lib_symbols.insert(readable.clone(), lib);
        }
        sym.lib_id = readable;
        sym.value = (*value).into();
        sym.footprint = (*footprint).into();
        sym.fields_user_placed = true;
        let (r, v) = field_offsets(&sym.reference);
        let at = sym.position;
        sym.ref_text = Some(field_text(Point::new(at.x + r.0, at.y + r.1)));
        sym.val_text = Some(field_text(Point::new(at.x + v.0, at.y + v.1)));
    }
    for sym in &sheet.symbols {
        let xform = SymbolTransform::from_symbol(sym);
        let lib = sheet.lib_symbols.get(&sym.lib_id).expect("lib");
        for p in &lib.pins {
            let tip = xform.apply(p.pin.position);
            tips.insert(
                (sym.reference.clone(), p.pin.number.clone()),
                (tip.x, tip.y),
            );
        }
    }
    let tip = |reference: &str, pin: &str| tips[&(reference.to_string(), pin.to_string())];
    let mut wire = |a: (f64, f64), b: (f64, f64)| {
        sheet.wires.push(Wire {
            uuid: uuid::Uuid::now_v7(),
            start: Point::new(a.0, a.1),
            end: Point::new(b.0, b.1),
            stroke_width: 0.0,
            unknown_columns: Default::default(),
            locked: false,
        });
    };
    // Header and MCU left pins run out to a net label.
    let mut labels: Vec<(&str, (f64, f64), HAlign)> = Vec::new();
    for (pin, net) in [("1", "3V3"), ("2", "SDA"), ("3", "SCL"), ("4", "GND")] {
        let t = tip("J1", pin);
        wire(t, (t.0 - 10.16, t.1));
        labels.push((net, (t.0 - 10.16, t.1), HAlign::Right));
    }
    for (pin, net) in [("1", "3V3"), ("2", "SDA"), ("3", "SCL"), ("4", "GND")] {
        let t = tip("U1", pin);
        wire(t, (t.0 - 10.16, t.1));
        labels.push((net, (t.0 - 10.16, t.1), HAlign::Right));
    }
    for (pin, net) in [
        ("8", "GPIO0"),
        ("7", "GPIO1"),
        ("6", "GPIO2"),
        ("5", "GPIO3"),
    ] {
        let t = tip("U1", pin);
        wire(t, (t.0 + 10.16, t.1));
        labels.push((net, (t.0 + 10.16, t.1), HAlign::Left));
    }
    // 3V3 rail over the pull-ups, SDA / SCL out of their lower pins.
    let (r1_top, r2_top) = (tip("R1", "1"), tip("R2", "1"));
    wire(r1_top, (r1_top.0, 58.42));
    wire((r1_top.0, 58.42), (r2_top.0, 58.42));
    wire(r2_top, (r2_top.0, 58.42));
    labels.push(("3V3", (96.52, 58.42), HAlign::Center));
    for (r, net) in [("R1", "SDA"), ("R2", "SCL")] {
        let t = tip(r, "2");
        wire(t, (t.0, t.1 + 7.62));
        labels.push((net, (t.0, t.1 + 7.62), HAlign::Left));
    }
    // Decoupling between 3V3 and GND.
    for c in ["C1", "C2"] {
        let (top, bottom) = (tip(c, "1"), tip(c, "2"));
        wire(top, (top.0, top.1 - 7.62));
        labels.push(("3V3", (top.0, top.1 - 7.62), HAlign::Left));
        wire(bottom, (bottom.0, bottom.1 + 7.62));
        labels.push(("GND", (bottom.0, bottom.1 + 7.62), HAlign::Left));
    }
    for (text, at, justify) in labels {
        sheet.labels.push(Label {
            uuid: uuid::Uuid::now_v7(),
            text: text.into(),
            position: Point::new(at.0, at.1),
            rotation: 0.0,
            label_type: LabelType::Net,
            shape: String::new(),
            font_size: 1.7,
            font_family: None,
            justify,
            justify_v: VAlign::Bottom,
            unknown_columns: Default::default(),
            locked: false,
        });
    }
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
    // Frame the circuit rather than the whole A4 page: select it, fit the
    // selection, then click empty sheet to deselect.
    let _ = app.update(Message::CanvasEvent(CanvasEvent::BoxSelect {
        x1: 30.0,
        y1: 45.0,
        x2: 165.0,
        y2: 155.0,
        direction: BoxDirection::LeftToRight,
    }));
    let _ = app.update(Message::CanvasEvent(CanvasEvent::FitSelection));
    let _ = draw(&mut renderer, &app);
    let _ = app.update(Message::CanvasEvent(CanvasEvent::Clicked {
        world_x: 20.0,
        world_y: 20.0,
    }));

    shoot(&mut renderer, &app, "hero.png");
    // Select R1 so the Properties panel shows a component, as `editor.png` does.
    let _ = app.update(Message::CanvasEvent(CanvasEvent::Clicked {
        world_x: 88.9,
        world_y: 71.12,
    }));
    for (theme, name) in [
        (ThemeId::Signex, "editor.png"),
        (ThemeId::CatppuccinMocha, "catppuccin-theme.png"),
        (ThemeId::GitHubDark, "github-dark-theme.png"),
        (ThemeId::SolarizedLight, "solarized-theme.png"),
    ] {
        // Pick the theme the way Preferences does: draft it, then save.
        let _ = app.update(Message::Preferences(PreferencesMsg::Inner(
            PrefMsg::DraftTheme(theme),
        )));
        let _ = app.update(Message::Preferences(PreferencesMsg::Inner(PrefMsg::Save)));
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
