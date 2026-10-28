//! Renders the screenshots `docs/USER_GUIDE.md` shows (#322).
//!
//! The real `Signex` state is driven through its own messages (new project,
//! place a resistor, draw wires, save) and the real
//! `Signex::view` is drawn by iced's wgpu renderer into an offscreen texture
//! at a fixed 1600 x 1000 window with the default theme and the bundled
//! fonts. Nothing is mocked, so the images show the UI as shipped.
//!
//! `#[ignore]`d, so CI pays nothing. Regenerate the images with
//!
//! ```text
//! cargo test -p signex-app --test guide_screenshots -- --ignored --nocapture
//! ```
//!
//! They land in `assets/screenshots/guide/`; set `SIGNEX_GUIDE_SCREENSHOTS_DIR`
//! to write elsewhere. A machine without a GPU needs Mesa lavapipe
//! (`mesa-vulkan-drivers`), the same adapter the pixel goldens use in CI.

use std::path::{Path, PathBuf};

use iced::advanced::renderer::Style;
use iced::{Font, Pixels, Size};
use iced_runtime::UserInterface;
use iced_runtime::user_interface::Cache;
use iced_wgpu::wgpu;
use signex_app::app::{FileMsg, Message, OverlayMsg, Signex, Tool};
use signex_app::canvas::CanvasEvent;
use signex_app::fonts::bundled;
use signex_app::library::LibraryMessage;
use signex_app::library::state::ComponentsMountSource;
use signex_app::menu_bar::MenuMessage;
use signex_app::toolbar::ToolMessage;
use signex_library::adapter::LibraryAdapter;
use signex_library::adapters::local_git::{LibraryInitOptions, LocalGitAdapter};
use signex_library::component::{ComponentRow, DatasheetRef, PlmReserved};
use signex_library::identity::{ComponentClass, InternalPn};
use signex_library::library_file::{FORMAT_TOKEN, LibrarySection, SnxlibManifest};
use signex_library::lifecycle::LifecycleState;
use signex_library::manifest::{LibraryMode, UsersConfig, WorkflowConfig};
use signex_library::manufacturer::ManufacturerPart;
use signex_library::param::ParamMap;
use signex_library::primitive::{PrimitiveRef, SymbolFile};
use signex_types::schematic::SymbolTransform;
use uuid::Uuid;

const WIDTH: u32 = 1600;
const HEIGHT: u32 = 1000;

/// Drive `task` and every task its messages spawn to completion, the way the
/// iced runtime would.
fn settle_task(app: &mut Signex, task: iced::Task<Message>) {
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let mut pending = vec![task];
    while let Some(task) = pending.pop() {
        let Some(mut stream) = iced_runtime::task::into_stream(task) else {
            continue;
        };
        let mut messages = Vec::new();
        runtime.block_on(async {
            while let Some(action) = futures::StreamExt::next(&mut stream).await {
                if let iced_runtime::Action::Output(next) = action {
                    messages.push(next);
                }
            }
        });
        pending.extend(messages.into_iter().map(|m| app.update(m)));
    }
}

fn settle(app: &mut Signex, message: Message) {
    let task = app.update(message);
    settle_task(app, task);
}

struct Capture {
    renderer: iced::Renderer,
    out_dir: PathBuf,
}

impl Capture {
    fn new() -> Self {
        let instance = wgpu::Instance::default();
        let adapter =
            futures::executor::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: None,
                force_fallback_adapter: false,
            }))
            .expect("no wgpu adapter: install Mesa lavapipe (mesa-vulkan-drivers)");
        let (device, queue) =
            futures::executor::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                label: Some("guide_screenshots_device"),
                required_features: wgpu::Features::empty(),
                required_limits: adapter.limits(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::Off,
            }))
            .expect("device");
        let engine = iced_wgpu::Engine::new(
            &adapter,
            device,
            queue,
            wgpu::TextureFormat::Rgba8Unorm,
            None,
            iced::advanced::graphics::Shell::headless(),
        );
        // The app registers its faces through the iced builder; so must we,
        // or every label falls back to whatever the host has installed.
        let mut fonts = iced::advanced::graphics::text::font_system()
            .write()
            .expect("font system");
        for face in bundled::BUNDLED_FONTS {
            fonts.load_font(std::borrow::Cow::Borrowed(face));
        }
        fonts.load_font(std::borrow::Cow::Borrowed(iced_aw::ICED_AW_FONT_BYTES));
        drop(fonts);
        let renderer = iced::Renderer::Primary(iced_wgpu::Renderer::new(
            engine,
            Font::with_name("Roboto"),
            Pixels(16.0),
        ));
        let out_dir = std::env::var_os("SIGNEX_GUIDE_SCREENSHOTS_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/screenshots/guide")
            });
        std::fs::create_dir_all(&out_dir).expect("create output dir");
        Self { renderer, out_dir }
    }

    /// Draw the main window of `app` and return RGBA pixels.
    fn draw(&mut self, app: &mut Signex) -> Vec<u8> {
        let window = app.ui_state.main_window_id.expect("main window id");
        let size = Size::new(WIDTH as f32, HEIGHT as f32);
        // The canvas applies a pending fit while handling an input event and
        // asks the app to redraw. Deliver one (a pointer move over the sheet)
        // and feed the app what it published, as the iced runtime would,
        // before the frame that is kept.
        let over_canvas = iced::Point::new(WIDTH as f32 / 2.0, HEIGHT as f32 / 2.0);
        let mut published = Vec::new();
        {
            let mut ui =
                UserInterface::build(app.view(window), size, Cache::default(), &mut self.renderer);
            let (_state, _statuses) = ui.update(
                &[iced::Event::Mouse(iced::mouse::Event::CursorMoved {
                    position: over_canvas,
                })],
                iced::mouse::Cursor::Available(over_canvas),
                &mut self.renderer,
                &mut iced::advanced::clipboard::Null,
                &mut published,
            );
        }
        for message in published {
            settle(app, message);
        }
        let theme = app.theme(window).expect("theme");
        let background = theme.palette().background;
        let mut ui =
            UserInterface::build(app.view(window), size, Cache::default(), &mut self.renderer);
        ui.draw(
            &mut self.renderer,
            &theme,
            &Style {
                text_color: theme.palette().text,
            },
            iced::mouse::Cursor::Unavailable,
        );
        let viewport = iced::advanced::graphics::Viewport::with_physical_size(size_u(), 1.0);
        let iced::Renderer::Primary(wgpu_renderer) = &mut self.renderer else {
            unreachable!("built as Primary above")
        };
        wgpu_renderer.screenshot(&viewport, background)
    }

    fn shoot(&mut self, app: &mut Signex, name: &str) {
        let pixels = self.draw(app);
        let path = self.out_dir.join(name);
        image::save_buffer(&path, &pixels, WIDTH, HEIGHT, image::ColorType::Rgba8)
            .expect("write png");
        println!("wrote {}", path.display());
    }
}

fn size_u() -> Size<u32> {
    Size::new(WIDTH, HEIGHT)
}

fn click(app: &mut Signex, at: (f64, f64)) {
    settle(
        app,
        Message::CanvasEvent(CanvasEvent::Clicked {
            world_x: at.0,
            world_y: at.1,
        }),
    );
}

fn double_click(app: &mut Signex, at: (f64, f64)) {
    settle(
        app,
        Message::CanvasEvent(CanvasEvent::DoubleClicked {
            world_x: at.0,
            world_y: at.1,
            screen_x: 0.0,
            screen_y: 0.0,
        }),
    );
}

/// A library of the reader's own with one resistor row. Its symbol is the
/// bundled stock `R.snxsym`, byte for byte, so the drawn part is the real
/// stock resistor. (The Stock library itself lists no placeable rows yet.)
fn parts_library(dir: &Path) -> PathBuf {
    signex_library::git_identity::test_identity::install();
    let root = dir.join("parts");
    let snxlib = root.join("parts.snxlib");
    let manifest = SnxlibManifest {
        format: FORMAT_TOKEN.into(),
        library_id: Uuid::now_v7(),
        library: LibrarySection {
            name: "My Parts".into(),
            description: None,
        },
        mode: LibraryMode::default(),
        workflow: WorkflowConfig::default(),
        users: UsersConfig::default(),
        classes: Vec::new(),
    };
    let adapter = LocalGitAdapter::init(
        &snxlib,
        manifest,
        LibraryInitOptions {
            enable_git: false,
            use_lfs: false,
        },
    )
    .expect("init library");
    let library_id = adapter.library_id();
    let stock_symbol =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/stock-library/symbols/R.snxsym");
    let bytes = std::fs::read(&stock_symbol).expect("read stock R.snxsym");
    let symbol_id = SymbolFile::from_bytes(&bytes)
        .expect("parse stock R.snxsym")
        .symbols[0]
        .uuid;
    std::fs::create_dir_all(root.join("symbols")).expect("symbols dir");
    std::fs::write(root.join("symbols").join("R.snxsym"), bytes).expect("copy symbol");
    let now = chrono::Utc::now();
    let row = ComponentRow {
        alternate_symbols: Vec::new(),
        row_id: Uuid::now_v7(),
        internal_pn: InternalPn::new("R-10K"),
        class: ComponentClass::new("resistor"),
        datasheet: DatasheetRef::default(),
        datasheet_label: String::new(),
        description: "10 kOhm resistor".into(),
        more_datasheets: Vec::new(),
        state: LifecycleState::Draft,
        symbol_ref: PrimitiveRef::new(library_id, symbol_id),
        footprint_ref: None,
        sim_ref: None,
        pin_map_overrides: Vec::new(),
        primary_mpn: ManufacturerPart::draft("Example", "10K-0603"),
        alternates: Vec::new(),
        supply: Vec::new(),
        parameters: ParamMap::new(),
        plm: PlmReserved::default(),
        version: "1.0.0".into(),
        released: false,
        symbol_version: String::new(),
        footprint_version: String::new(),
        sim_version: String::new(),
        created: now,
        updated: now,
        content_hash: [0u8; 32],
        server_revision: None,
    };
    adapter
        .insert_row("Resistors", row, "seed")
        .expect("insert row");
    drop(adapter);
    drop(LocalGitAdapter::recover_init(&snxlib).expect("recover_init"));
    snxlib
}

#[test]
#[ignore = "renders the user guide screenshots; run by hand, see the module docs"]
fn render_guide_screenshots() {
    let mut cap = Capture::new();
    let work = tempfile::tempdir().expect("tempdir");
    let (mut app, boot) = Signex::new();
    app.ui_state.window_size = (WIDTH as f32, HEIGHT as f32);
    settle_task(&mut app, boot);

    // 1. First launch: start screen and the Welcome card.
    assert!(app.ui_state.first_run_tour_open, "first-run card expected");
    cap.shoot(&mut app, "01-first-launch.png");
    settle(&mut app, Message::Overlay(OverlayMsg::DismissFirstRunTour));

    // 2. File > New Project.
    let project = work.path().join("First-Board.snxprj");
    settle(&mut app, Message::File(FileMsg::NewProject(Some(project))));
    assert!(
        app.document_state.active_project.is_some(),
        "project loaded"
    );
    let sheet_path = app.document_state.active_path.clone().expect("sheet open");
    cap.shoot(&mut app, "02-new-project.png");

    // 3. The Components panel, with a library holding a resistor mounted.
    settle(
        &mut app,
        Message::Library(LibraryMessage::ComponentsPanelAddLibraryAt {
            source: ComponentsMountSource::Installed,
            path: Some(parts_library(work.path())),
        }),
    );
    settle(&mut app, Message::Menu(MenuMessage::OpenComponentsPanel));
    cap.shoot(&mut app, "03-components-panel.png");

    // 4. Place the resistor, wire both pins, ready to save.
    let (library_path, summary) = app
        .library
        .all_components()
        .into_iter()
        .find(|(_, c)| c.internal_pn.as_str() == "R-10K")
        .expect("resistor row listed");
    let table = app
        .library
        .table_of_row(&library_path, summary.row_id)
        .expect("table");
    settle(
        &mut app,
        Message::Library(LibraryMessage::ComponentsPanelPlace {
            library_path,
            table,
            row_id: signex_library::RowId(summary.row_id),
        }),
    );
    let at = (147.32, 101.6);
    click(&mut app, at);
    let sheet = app
        .document_state
        .active_engine()
        .expect("engine")
        .document()
        .clone();
    assert_eq!(sheet.symbols.len(), 1, "resistor placed");
    let xform = SymbolTransform::from_symbol(&sheet.symbols[0]);
    let lib = &sheet.lib_symbols[&sheet.symbols[0].lib_id];
    let mut tips: Vec<(f64, f64)> = lib
        .pins
        .iter()
        .map(|p| {
            let t = xform.apply(p.pin.position);
            (t.x, t.y)
        })
        .collect();
    tips.sort_by(|a, b| a.1.total_cmp(&b.1));
    let (top, bottom) = (tips[0], tips[1]);
    settle(&mut app, Message::Tool(ToolMessage::SelectTool(Tool::Wire)));
    for (pin, dy) in [(top, -10.16), (bottom, 10.16)] {
        click(&mut app, pin);
        // Press once to commit the segment, then double-click on the same
        // spot to stop drawing, as a person would.
        let end = (pin.0, pin.1 + dy);
        click(&mut app, end);
        double_click(&mut app, end);
    }
    settle(
        &mut app,
        Message::Tool(ToolMessage::SelectTool(Tool::Select)),
    );
    let wires = app
        .document_state
        .active_engine()
        .expect("engine")
        .document()
        .wires
        .len();
    assert_eq!(wires, 2, "one wire on each pin");
    let (cx, cy) = (at.0 as f32, at.1 as f32);
    app.interaction_state
        .active_canvas_mut()
        .pending_fit
        .set(Some(iced::Rectangle::new(
            iced::Point::new(cx - 45.0, cy - 30.0),
            iced::Size::new(90.0, 60.0),
        )));
    cap.shoot(&mut app, "04-placed-and-wired.png");

    // 5. Save: the sheet, the project file and the managed .gitignore are
    // all that is in the project folder, and the sheet holds the work.
    settle(&mut app, Message::Menu(MenuMessage::Save));
    let saved = std::fs::read_to_string(&sheet_path).expect("read saved sheet");
    assert!(saved.contains("[sheets.components]") && saved.contains(" R1 "));
    let wire_rows = saved
        .split("[sheets.wires]")
        .nth(1)
        .and_then(|rest| rest.split("\"\"\"").nth(1))
        .map_or(0, |table| {
            table.lines().filter(|l| l.contains("147320000")).count()
        });
    assert_eq!(wire_rows, 2, "both wires are in the saved sheet");
    let mut names: Vec<String> = std::fs::read_dir(sheet_path.parent().expect("folder"))
        .expect("read project folder")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(
        names,
        [".gitignore", "First-Board.snxprj", "First-Board.snxsch"]
    );
    assert!(
        !app.document_state.tabs[app.document_state.active_tab].dirty,
        "saved sheet is clean"
    );
}
