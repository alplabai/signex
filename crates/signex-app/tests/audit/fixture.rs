//! The documents a surface opens, copied into a temp dir so nothing in the
//! repo is written, and the app set-up steps most surfaces share.

use crate::{render, settle_support};
use signex_app::app::{FileMsg, Message, PreferencesMsg, Signex};
use signex_app::dock::{DockMessage, PanelPosition};
use signex_app::panels::PanelKind;
use signex_app::preferences::PrefMsg;
use signex_types::theme::ThemeId;
use std::path::{Path, PathBuf};

/// Copies of the sample project, a real board and the stock library.
pub struct Fixture {
    _tmp: tempfile::TempDir,
    /// `linear-regulator.snxprj` of the sample project.
    pub sample: PathBuf,
    /// A board with an outline, holes and a keepout.
    pub pcb: PathBuf,
    /// `Stock.snxlib` of the stock library.
    pub stock: PathBuf,
    /// The first footprint file of the stock library.
    pub footprint: PathBuf,
    /// The first symbol file of the stock library.
    pub symbol: PathBuf,
}

fn manifest(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel)
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let dest = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &dest)?;
        } else {
            std::fs::copy(entry.path(), &dest)?;
        }
    }
    Ok(())
}

fn first_file(dir: &Path) -> Result<PathBuf, String> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))? {
        files.push(entry.map_err(|e| e.to_string())?.path());
    }
    files.sort();
    files
        .into_iter()
        .next()
        .ok_or_else(|| format!("{} is empty", dir.display()))
}

/// A fresh set of document copies.
pub fn fixture() -> Result<Fixture, String> {
    let io = |what: &str, e: std::io::Error| format!("{what}: {e}");
    let tmp = tempfile::TempDir::new().map_err(|e| io("tempdir", e))?;
    let sample = tmp.path().join("sample");
    copy_dir(&manifest("assets/sample-project"), &sample).map_err(|e| io("sample", e))?;
    let pcb = tmp.path().join("mixed-tht-smd.snxpcb");
    std::fs::copy(
        manifest("../signex-types/tests/corpus/pcb/real/mixed-tht-smd.snxpcb"),
        &pcb,
    )
    .map_err(|e| io("pcb", e))?;
    let stock_dir = tmp.path().join("stock");
    copy_dir(&manifest("../../assets/stock-library"), &stock_dir).map_err(|e| io("stock", e))?;
    Ok(Fixture {
        footprint: first_file(&stock_dir.join("footprints"))?,
        symbol: first_file(&stock_dir.join("symbols"))?,
        stock: stock_dir.join("Stock.snxlib"),
        sample: sample.join("linear-regulator.snxprj"),
        pcb,
        _tmp: tmp,
    })
}

/// A fresh app in `theme`, picked the way Preferences does but never saved.
pub fn fresh(theme: ThemeId) -> Signex {
    let (mut app, _task) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state.window_size = (render::WIDTH as f32, render::HEIGHT as f32);
    // Applied without a Save, so the user's prefs file is never written; the
    // update after it refreshes the panel tokens.
    app.ui_state.theme_id = theme;
    let _ = app.update(Message::Preferences(PreferencesMsg::Inner(
        PrefMsg::DraftTheme(theme),
    )));
    app
}

/// Mount the stock library copy as the app's editable stock library.
pub fn mount_stock(app: &mut Signex, fx: &Fixture) -> Result<(), String> {
    app.library.pending_mounts.clear();
    app.library.stock_library_root = Some(fx.stock.clone());
    app.library.stock_editable = true;
    app.library
        .open_library(fx.stock.clone())
        .map_err(|e| format!("mount stock: {e:?}"))
}

/// Open the sample project and its root sheet, fitted to the sheet paper.
pub fn open_sample(app: &mut Signex, r: &mut iced::Renderer, fx: &Fixture) {
    settle_support::settle(app, Message::File(FileMsg::Opened(Some(fx.sample.clone()))));
    settle_support::settle(
        app,
        Message::File(FileMsg::Opened(Some(fx.sample.with_extension("snxsch")))),
    );
    let _ = render::draw_main(r, app);
    // Fit the whole sheet paper, so its frame and title block are in view.
    app.interaction_state.active_canvas().fit_to_sheet();
    let _ = render::draw_main(r, app);
}

/// Open the board copy, fitted to the canvas.
pub fn open_board(app: &mut Signex, r: &mut iced::Renderer, fx: &Fixture) {
    settle_support::settle(app, Message::File(FileMsg::Opened(Some(fx.pcb.clone()))));
    let _ = render::draw_main(r, app);
    // The canvas knows its size after the first draw; fit the board to it.
    app.interaction_state.pcb_canvas.fit_to_board();
    let _ = render::draw_main(r, app);
}

/// Bring the dock tab of `kind` to the front, in whichever dock holds it.
pub fn select_panel(app: &mut Signex, kind: PanelKind) -> Result<(), String> {
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
            .position(|k| *k == kind)
        {
            let _ = app.update(Message::Dock(DockMessage::SelectTab(pos, i)));
            return Ok(());
        }
    }
    Err(format!("no {kind:?} tab in any dock"))
}
