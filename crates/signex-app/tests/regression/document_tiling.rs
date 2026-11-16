//! #2249 - Window ▸ Tile Horizontally / Tile Vertically show the main
//! window's documents in panes, and Window ▸ Untile, or closing down to one
//! document, goes back to one. Each entry point (menu row, palette, built-in
//! key) is driven through `Signex::update`.

use std::path::PathBuf;

use signex_app::app::{CommandPaletteMsg, FileMsg, Message, Signex, TileLayout, UiMsg, WindowMsg};
use signex_app::keymap::{BuiltInProfile, ShortcutProfileSet, ShortcutTrigger};
use signex_app::tab_bar::TabMessage;
use signex_types::format::SnxSchematic;
use signex_types::schematic::SchematicSheet;
use uuid::Uuid;

struct Fixture {
    app: Signex,
    main: iced::window::Id,
    sheets: [PathBuf; 2],
    _tmp: tempfile::TempDir,
}

fn send(app: &mut Signex, message: Message) {
    let task = app.update(message);
    crate::drive::drive(app, task);
}

/// The id of the built-in profile a first launch uses.
fn default_profile() -> String {
    ShortcutProfileSet::built_ins()
        .expect("built-in profiles")
        .active_profile()
        .id
        .clone()
}

/// Two sheets open in the main window under `profile`, the second active.
fn two_sheets(profile: &str) -> Fixture {
    let tmp = tempfile::tempdir().unwrap();
    let sheets = ["left.snxsch", "right.snxsch"].map(|name| tmp.path().join(name));
    for (n, path) in sheets.iter().enumerate() {
        let sheet = SchematicSheet {
            uuid: Uuid::from_u128(0x2249_0000 + n as u128),
            paper_size: "A4".to_string(),
            ..Default::default()
        };
        std::fs::write(path, SnxSchematic::new(sheet).write_string().unwrap()).unwrap();
    }
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    let main = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(main);
    app.ui_state
        .keymap_profiles
        .set_active_profile(profile)
        .expect("built-in profile");
    app.ui_state.active_keymap = app.ui_state.keymap_profiles.compile_active().into();
    for path in &sheets {
        send(&mut app, Message::File(FileMsg::Opened(Some(path.clone()))));
    }
    assert_eq!(app.document_state.tabs.len(), 2, "both sheets open");
    Fixture {
        app,
        main,
        sheets,
        _tmp: tmp,
    }
}

fn menu_row(id: &str) -> Message {
    crate::menu_support::menu_row(id)
}

/// Every stroke of `keys`, as the keyboard sends them.
fn press(app: &mut Signex, keys: &str) {
    let ShortcutTrigger::KeySequence(strokes) = ShortcutTrigger::parse(keys).unwrap() else {
        panic!("a key sequence");
    };
    for stroke in strokes {
        send(
            app,
            Message::Ui(UiMsg::KeymapStroke {
                window: None,
                stroke,
            }),
        );
    }
}

/// The panes' layout and tab indices, and the active tab.
fn tiling(app: &Signex) -> (Option<(TileLayout, Vec<usize>)>, usize) {
    (app.tiled_panes(), app.document_state.active_tab)
}

#[test]
fn the_menu_rows_tile_the_documents_and_untile_them() {
    let mut f = two_sheets(&default_profile());
    let active = f.app.document_state.active_tab;
    send(&mut f.app, menu_row("tile_windows_vertically"));
    assert_eq!(
        tiling(&f.app),
        (Some((TileLayout::SideBySide, vec![0, 1])), active)
    );
    send(&mut f.app, menu_row("tile_windows_horizontally"));
    assert_eq!(f.app.ui_state.document_tiling, Some(TileLayout::Stacked));
    send(&mut f.app, menu_row("untile_documents"));
    assert_eq!(
        tiling(&f.app),
        (None, active),
        "the same document stays active"
    );
    assert!(f.app.interaction_state.tile_panes.schematic.is_empty());
}

#[test]
fn the_palette_tiles_the_documents() {
    let mut f = two_sheets(&default_profile());
    send(&mut f.app, Message::CommandPalette(CommandPaletteMsg::Open));
    send(
        &mut f.app,
        Message::CommandPalette(CommandPaletteMsg::QueryChanged(
            "Tile windows one above the other".into(),
        )),
    );
    send(
        &mut f.app,
        Message::CommandPalette(CommandPaletteMsg::ExecuteSelected),
    );
    assert_eq!(f.app.ui_state.document_tiling, Some(TileLayout::Stacked));
    send(&mut f.app, Message::CommandPalette(CommandPaletteMsg::Open));
    send(
        &mut f.app,
        Message::CommandPalette(CommandPaletteMsg::QueryChanged("Untile documents".into())),
    );
    send(
        &mut f.app,
        Message::CommandPalette(CommandPaletteMsg::ExecuteSelected),
    );
    assert_eq!(f.app.ui_state.document_tiling, None);
}

#[test]
fn the_built_in_keys_tile_and_untile() {
    let mut f = two_sheets(&default_profile());
    press(&mut f.app, "W V");
    assert_eq!(f.app.ui_state.document_tiling, Some(TileLayout::SideBySide));
    press(&mut f.app, "W U");
    assert_eq!(f.app.ui_state.document_tiling, None);

    let mut f = two_sheets(BuiltInProfile::Classic.id());
    press(&mut f.app, "Shift+Alt+H");
    assert_eq!(f.app.ui_state.document_tiling, Some(TileLayout::Stacked));
    press(&mut f.app, "Shift+Alt+U");
    assert_eq!(f.app.ui_state.document_tiling, None);
}

/// A press on the other pane makes its document the active one; the pane
/// left behind draws on a canvas of its own.
#[test]
fn a_press_on_a_pane_focuses_its_document() {
    let mut f = two_sheets(&default_profile());
    send(&mut f.app, menu_row("tile_windows_vertically"));
    assert_eq!(f.app.document_state.active_tab, 1);
    let panes = &f.app.interaction_state.tile_panes.schematic;
    assert!(panes.contains_key(&f.sheets[0]) && !panes.contains_key(&f.sheets[1]));

    send(&mut f.app, Message::Window(WindowMsg::TilePanePressed(0)));
    assert_eq!(
        tiling(&f.app),
        (Some((TileLayout::SideBySide, vec![0, 1])), 0)
    );
    assert_eq!(
        f.app.document_state.active_path.as_ref(),
        Some(&f.sheets[0]),
        "the pressed sheet is the active document"
    );
    let panes = &f.app.interaction_state.tile_panes.schematic;
    assert!(panes.contains_key(&f.sheets[1]) && !panes.contains_key(&f.sheets[0]));
}

/// Closing down to one document ends the tiling and drops the pane canvases;
/// a second document opened later is not tiled again.
#[test]
fn closing_down_to_one_document_untiles() {
    let mut f = two_sheets(&default_profile());
    send(&mut f.app, menu_row("tile_windows_vertically"));
    assert!(f.app.tiled_panes().is_some());
    send(
        &mut f.app,
        Message::Tab {
            window_id: f.main,
            msg: TabMessage::CloseClicked(0),
        },
    );
    assert_eq!(f.app.document_state.tabs.len(), 1);
    assert_eq!(tiling(&f.app), (None, 0));
    assert_eq!(f.app.ui_state.document_tiling, None);
    assert!(f.app.interaction_state.tile_panes.schematic.is_empty());

    send(
        &mut f.app,
        Message::File(FileMsg::Opened(Some(f.sheets[0].clone()))),
    );
    assert_eq!(f.app.document_state.tabs.len(), 2);
    assert_eq!(f.app.ui_state.document_tiling, None);
}

#[test]
fn one_document_has_nothing_to_tile() {
    let mut f = two_sheets(&default_profile());
    send(
        &mut f.app,
        Message::Tab {
            window_id: f.main,
            msg: TabMessage::CloseClicked(0),
        },
    );
    send(&mut f.app, menu_row("tile_windows_vertically"));
    assert_eq!(f.app.ui_state.document_tiling, None);
}
