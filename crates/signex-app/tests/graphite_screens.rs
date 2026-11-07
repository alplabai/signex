//! Renders the Graphite screens at 1440x900 in both Graphite themes, for a
//! visual check against the design mock-ups.
//!
//! Not part of the normal run: it needs a GPU adapter. Set `GRAPHITE_SHOTS_DIR`
//! to the output directory (and optionally `GRAPHITE_ONLY` to one screen name)
//! and run
//!
//! ```text
//! GRAPHITE_SHOTS_DIR=out cargo test -p signex-app --test graphite_screens -- --ignored
//! ```
//!
//! Files are `<screen>-<light|dark>.png`. A screen that cannot be set up is
//! reported and skipped.

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
use signex_app::app::{AnnotateMsg, CommandPaletteMsg, FileMsg, Message, PreferencesMsg, Signex};
use signex_app::canvas::CanvasEvent;
use signex_app::dock::{DockMessage, PanelPosition};
use signex_app::library::LibraryMessage;
use signex_app::panels::PanelKind;
use signex_app::preferences::{PrefMsg, PrefNav};
use signex_types::theme::ThemeId;
use std::path::{Path, PathBuf};

const WIDTH: u32 = 1440;
const HEIGHT: u32 = 900;

fn gpu() -> Option<iced::Renderer> {
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
        iced::advanced::renderer::Settings {
            font: Font::new("Roboto"),
            text_size: Pixels(16.0),
            ..Default::default()
        },
    )))
}

/// Draw the app's main window and return its RGBA pixels.
fn draw(renderer: &mut iced::Renderer, app: &Signex) -> Vec<u8> {
    let window = app.ui_state.main_window_id.expect("main window");
    draw_window(renderer, app, window, (WIDTH, HEIGHT))
}

/// Draw window `window` at `(w, h)` physical pixels.
fn draw_window(
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

fn save_png(rgba: &[u8], dir: &Path, name: &str, (w, h): (u32, u32)) {
    image::save_buffer(dir.join(name), rgba, w, h, image::ColorType::Rgba8).expect("write png");
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("mkdir");
    for entry in std::fs::read_dir(from).expect("read_dir") {
        let entry = entry.expect("entry");
        let dest = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &dest);
        } else {
            std::fs::copy(entry.path(), &dest).expect("copy");
        }
    }
}

fn manifest(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel)
}

/// A fresh app in `theme`, picked the way Preferences does but never saved.
fn fresh(theme: ThemeId) -> Signex {
    let (mut app, _task) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state.window_size = (WIDTH as f32, HEIGHT as f32);
    // Applied without a Save, so the user's prefs file is never written; the
    // update after it refreshes the panel tokens.
    app.ui_state.theme_id = theme;
    let _ = app.update(Message::Preferences(PreferencesMsg::Inner(
        PrefMsg::DraftTheme(theme),
    )));
    app
}

struct Fixture {
    _tmp: tempfile::TempDir,
    sample: PathBuf,
    pcb: PathBuf,
    stock: PathBuf,
    footprint: PathBuf,
    symbol: PathBuf,
}

fn first_file(dir: &Path) -> PathBuf {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .expect("read_dir")
        .map(|e| e.expect("entry").path())
        .collect();
    files.sort();
    files.remove(0)
}

fn fixture() -> Fixture {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let sample = tmp.path().join("sample");
    copy_dir(&manifest("assets/sample-project"), &sample);
    let pcb = tmp.path().join("routed_board.snxpcb");
    std::fs::copy(
        manifest("../signex-types/tests/corpus/pcb/routed_board.snxpcb"),
        &pcb,
    )
    .expect("copy pcb");
    let stock_dir = tmp.path().join("stock");
    copy_dir(&manifest("../../assets/stock-library"), &stock_dir);
    Fixture {
        footprint: first_file(&stock_dir.join("footprints")),
        symbol: first_file(&stock_dir.join("symbols")),
        stock: stock_dir.join("Stock.snxlib"),
        sample: sample.join("linear-regulator.snxprj"),
        pcb,
        _tmp: tmp,
    }
}

fn mount_stock(app: &mut Signex, fx: &Fixture) {
    app.library.pending_mounts.clear();
    app.library.stock_library_root = Some(fx.stock.clone());
    app.library.stock_editable = true;
    app.library.open_library(fx.stock.clone()).expect("mount");
}

fn open_sample(app: &mut Signex, r: &mut iced::Renderer, fx: &Fixture) {
    settle_support::settle(app, Message::File(FileMsg::Opened(Some(fx.sample.clone()))));
    settle_support::settle(
        app,
        Message::File(FileMsg::Opened(Some(fx.sample.with_extension("snxsch")))),
    );
    let _ = draw(r, app);
    let _ = app.update(Message::CanvasEvent(CanvasEvent::FitAll));
    let _ = draw(r, app);
}

fn select_signal(app: &mut Signex) {
    for pos in [
        PanelPosition::Left,
        PanelPosition::Right,
        PanelPosition::Bottom,
    ] {
        if let Some(i) = app
            .document_state
            .dock
            .panel_kinds(pos)
            .iter()
            .position(|k| *k == PanelKind::Signal)
        {
            let _ = app.update(Message::Dock(DockMessage::SelectTab(pos, i)));
            return;
        }
    }
    eprintln!("no Signal tab in any dock");
}

type Build = fn(&mut Signex, &mut iced::Renderer, &Fixture) -> Result<(), String>;

fn screens() -> Vec<(&'static str, Build)> {
    vec![
        ("start", |_, _, _| Ok(())),
        ("schematic", |app, r, fx| {
            open_sample(app, r, fx);
            Ok(())
        }),
        ("schematic-signal", |app, r, fx| {
            open_sample(app, r, fx);
            select_signal(app);
            Ok(())
        }),
        ("pcb", |app, r, fx| {
            settle_support::settle(app, Message::File(FileMsg::Opened(Some(fx.pcb.clone()))));
            let _ = draw(r, app);
            Ok(())
        }),
        ("library", |app, _, fx| {
            mount_stock(app, fx);
            settle_support::settle(
                app,
                Message::Library(LibraryMessage::OpenLibraryBrowser(fx.stock.clone())),
            );
            Ok(())
        }),
        ("component", |app, _, fx| {
            mount_stock(app, fx);
            let (_, row) = app
                .library
                .all_components()
                .into_iter()
                .next()
                .ok_or("stock lists no component")?;
            let table = app
                .library
                .table_of_row(&fx.stock, row.row_id)
                .ok_or("no table for the row")?;
            settle_support::settle(
                app,
                Message::Library(LibraryMessage::OpenComponentRow {
                    library_path: fx.stock.clone(),
                    table,
                    row_id: signex_library::RowId(row.row_id),
                }),
            );
            Ok(())
        }),
        ("footprint", |app, r, fx| {
            mount_stock(app, fx);
            settle_support::settle(
                app,
                Message::File(FileMsg::Opened(Some(fx.footprint.clone()))),
            );
            let _ = draw(r, app);
            Ok(())
        }),
        ("symbol", |app, r, fx| {
            mount_stock(app, fx);
            settle_support::settle(app, Message::File(FileMsg::Opened(Some(fx.symbol.clone()))));
            let _ = draw(r, app);
            Ok(())
        }),
        ("preferences", |app, _, _| {
            let _ = app.update(Message::Preferences(PreferencesMsg::Open));
            let _ = app.update(Message::Preferences(PreferencesMsg::Nav(
                PrefNav::Appearance,
            )));
            Ok(())
        }),
        ("annotate", |app, r, fx| {
            open_sample(app, r, fx);
            settle_support::settle(app, Message::Annotate(AnnotateMsg::OpenDialog));
            Ok(())
        }),
        ("palette", |app, r, fx| {
            open_sample(app, r, fx);
            let _ = app.update(Message::CommandPalette(CommandPaletteMsg::Open));
            Ok(())
        }),
    ]
}

#[test]
#[ignore = "needs a GPU adapter; writes PNGs into $GRAPHITE_SHOTS_DIR"]
fn render_graphite_screens() {
    let Some(mut renderer) = gpu() else {
        eprintln!("no wgpu adapter: skipping Graphite screens");
        return;
    };
    let dir = PathBuf::from(std::env::var("GRAPHITE_SHOTS_DIR").expect("GRAPHITE_SHOTS_DIR"));
    std::fs::create_dir_all(&dir).expect("out dir");
    let only = std::env::var("GRAPHITE_ONLY").ok();
    for (suffix, theme) in [
        ("light", ThemeId::GraphiteLight),
        ("dark", ThemeId::GraphiteDark),
    ] {
        for (name, build) in screens() {
            if only.as_deref().is_some_and(|o| o != name) {
                continue;
            }
            let fx = fixture();
            let mut app = fresh(theme);
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                build(&mut app, &mut renderer, &fx)?;
                let _ = draw(&mut renderer, &app);
                // A dialog that opens as its own window is drawn as that window.
                let detached = app.ui_state.windows.iter().find_map(|(id, kind)| {
                    format!("{kind:?}")
                        .starts_with("DetachedModal")
                        .then_some(*id)
                });
                Ok::<_, String>(match detached {
                    Some(id) => {
                        let size = (1000, 720);
                        (draw_window(&mut renderer, &app, id, size), size)
                    }
                    None => (draw(&mut renderer, &app), (WIDTH, HEIGHT)),
                })
            }));
            match result {
                Ok(Ok((rgba, size))) => {
                    save_png(&rgba, &dir, &format!("{name}-{suffix}.png"), size)
                }
                Ok(Err(why)) => eprintln!("SKIP {name}-{suffix}: {why}"),
                Err(_) => eprintln!("SKIP {name}-{suffix}: panicked"),
            }
        }
    }
}
