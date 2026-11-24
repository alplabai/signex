//! Mockup rows 193 and 193f: the Gerber Viewer tab, its open paths, the
//! layer list, the board view and the empty state.

use std::io::Write as _;
use std::path::PathBuf;
use std::sync::Arc;

use iced::{Point, Size};
use pcb_geom::PointNm;
use signex_gerber::{Package, ReadFile, assemble, read_file};

use super::canvas::BoardFit;
use super::read::{PackageRead, read_paths, read_zip_from};
use super::state::{Camera, GerberViewerState, OpenPackage, default_colour};
use super::view_update::is_fabrication_name;
use super::{GerberViewerMsg, OpenKind};
use crate::app::document::DocumentKey;
use crate::app::{Message, Signex, TabKind};

/// A Gerber copper image with flashes at `xs` mm (y 0) on `net`.
fn copper(function: &str, net: &str, xs: &[i64]) -> String {
    let mut text = format!(
        "%FSLAX46Y46*%\n%MOMM*%\n%TF.FileFunction,{function}*%\n%ADD10C,1.000000*%\n%TO.N,{net}*%\nD10*\n"
    );
    for x in xs {
        text.push_str(&format!("X{}Y0D03*\n", x * 1_000_000));
    }
    text.push_str("%TD*%\nM02*\n");
    text
}

fn two_layer_files() -> Vec<(&'static str, String)> {
    vec![
        ("b-L1.gbr", copper("Copper,L1,Top", "VIN", &[0, 5])),
        ("b-L2.gbr", copper("Copper,L2,Bot", "GND", &[0])),
    ]
}

fn package() -> Arc<Package> {
    let files: Vec<ReadFile> = two_layer_files()
        .iter()
        .map(|(name, text)| read_file(name, text.as_bytes()))
        .collect();
    Arc::new(assemble(files))
}

fn opened() -> OpenPackage {
    OpenPackage::new(package(), vec![PathBuf::from("b")])
}

fn layer_named(open: &OpenPackage, file: &str) -> usize {
    open.package
        .layers
        .iter()
        .position(|layer| layer.image.file == file)
        .expect("layer")
}

#[test]
fn a_package_opens_in_its_default_order_with_colours_from_its_functions() {
    let open = opened();
    assert_eq!(open.order, open.package.default_order());
    for (layer, look) in open.package.layers.iter().zip(&open.looks) {
        assert!(look.visible);
        assert_eq!(look.colour, default_colour(layer.function.as_ref()));
    }
    let top = layer_named(&open, "b-L1.gbr");
    let bottom = layer_named(&open, "b-L2.gbr");
    assert_ne!(open.looks[top].colour, open.looks[bottom].colour);
}

/// mockup row 193: the top of the list draws on top; a grip drag moves a
/// row; Flip swaps each pair's drawing order.
#[test]
fn the_list_order_flip_and_drills_decide_the_drawing_order() {
    let mut open = opened();
    let top = layer_named(&open, "b-L1.gbr");
    let bottom = layer_named(&open, "b-L2.gbr");
    assert_eq!(open.order, vec![top, bottom]);
    assert_eq!(open.draw_order(), vec![bottom, top], "bottom first");
    open.flipped = true;
    assert_eq!(open.draw_order(), vec![top, bottom], "the pair swapped");
    open.flipped = false;
    open.move_layer(1, 0);
    assert_eq!(open.order, vec![bottom, top]);
    open.move_layer(0, 9);
    assert_eq!(
        open.order,
        vec![bottom, top],
        "a place past the list is ignored"
    );
}

#[test]
fn the_far_face_is_known_from_the_function() {
    let mut open = opened();
    let top = layer_named(&open, "b-L1.gbr");
    let bottom = layer_named(&open, "b-L2.gbr");
    assert!(open.is_near_side(top));
    assert!(!open.is_near_side(bottom));
    open.flipped = true;
    assert!(!open.is_near_side(top));
    assert!(open.is_near_side(bottom));
}

/// mockup row 193: a click selects the topmost visible object; a hidden
/// layer is passed over.
#[test]
fn a_click_selects_the_topmost_visible_object() {
    let mut open = opened();
    let top = layer_named(&open, "b-L1.gbr");
    let bottom = layer_named(&open, "b-L2.gbr");
    let at = PointNm::new(0, 0);
    let hit = open.object_at(at, 0).expect("an object");
    assert_eq!(hit.layer, top);
    open.toggle_visible(top);
    assert_eq!(open.object_at(at, 0).map(|o| o.layer), Some(bottom));
    assert_eq!(open.object_at(PointNm::new(50_000_000, 0), 0), None);
}

/// mockup row 193: Highlight net keeps the selected object's X2 net.
#[test]
fn highlight_net_takes_the_selected_objects_net_and_lets_it_go() {
    let (mut app, _task) = Signex::new();
    app.document_state.gerber_viewer.open = Some(opened());
    let clicked = GerberViewerMsg::BoardClicked {
        at: PointNm::new(5_000_000, 0),
        tolerance: 0,
    };
    let _ = app.update(Message::from(clicked));
    let _ = app.update(Message::from(GerberViewerMsg::NetHighlightToggled));
    let open = app.document_state.gerber_viewer.open.as_ref().unwrap();
    assert_eq!(open.net.as_deref(), Some("VIN"));
    assert!(open.package.has_net_attributes());
    let _ = app.update(Message::from(GerberViewerMsg::NetHighlightToggled));
    let open = app.document_state.gerber_viewer.open.as_ref().unwrap();
    assert_eq!(open.net, None);
}

#[test]
fn a_swatch_opens_its_colours_and_a_pick_closes_them() {
    let (mut app, _task) = Signex::new();
    app.document_state.gerber_viewer.open = Some(opened());
    let _ = app.update(Message::from(GerberViewerMsg::SwatchPressed(0)));
    let open = app.document_state.gerber_viewer.open.as_ref().unwrap();
    assert_eq!(open.colour_open, Some(0));
    let colour = [1, 2, 3];
    let _ = app.update(Message::from(GerberViewerMsg::ColourPicked {
        layer: 0,
        colour,
    }));
    let open = app.document_state.gerber_viewer.open.as_ref().unwrap();
    assert_eq!(open.looks[0].colour, colour);
    assert_eq!(open.colour_open, None);
}

#[test]
fn a_grip_drag_moves_the_row_on_release() {
    let (mut app, _task) = Signex::new();
    app.document_state.gerber_viewer.open = Some(opened());
    let before = app
        .document_state
        .gerber_viewer
        .open
        .as_ref()
        .unwrap()
        .order
        .clone();
    for msg in [
        GerberViewerMsg::GripPressed(0),
        GerberViewerMsg::RowEntered(1),
        GerberViewerMsg::GripReleased,
    ] {
        let _ = app.update(Message::from(msg));
    }
    let open = app.document_state.gerber_viewer.open.as_ref().unwrap();
    assert_eq!(open.order, vec![before[1], before[0]]);
    assert_eq!(open.grip, None);
}

/// A file point comes back from the screen where it went, and zooming at
/// the pointer keeps the point under it.
#[test]
fn the_board_fit_maps_both_ways_and_zooms_at_the_pointer() {
    let bounds = (PointNm::new(0, 0), PointNm::new(32_000_000, 24_000_000));
    let size = Size::new(640.0, 480.0);
    for flipped in [false, true] {
        let fit = BoardFit::of(bounds, Camera::default(), size, flipped);
        let p = PointNm::new(8_000_000, 6_000_000);
        let (x, y) = fit.to_world(fit.to_screen(p));
        assert!((x - 8.0e6).abs() < 100.0 && (y - 6.0e6).abs() < 100.0);
        let at = Point::new(100.0, 120.0);
        let under = fit.to_world(at);
        let camera = fit.zoomed_at(Camera::default(), at, 2.0);
        let zoomed = BoardFit::of(bounds, camera, size, flipped);
        let after = zoomed.to_world(at);
        assert!((under.0 - after.0).abs() < 100.0 && (under.1 - after.1).abs() < 100.0);
        assert!((camera.zoom - 2.0).abs() < 1e-9);
    }
    let fit = BoardFit::of(bounds, Camera::default(), size, false);
    let top = fit.to_screen(PointNm::new(0, 24_000_000));
    let low = fit.to_screen(PointNm::new(0, 0));
    assert!(top.y < low.y, "y up in the file is up on the screen");
}

/// mockup row 193f: a drop of several files opens once, when the last
/// hovered file lands.
#[test]
fn a_drop_opens_once_every_hovered_file_has_landed() {
    let mut viewer = GerberViewerState {
        hovered: vec![PathBuf::from("a.gbr"), PathBuf::from("b.drl")],
        ..Default::default()
    };
    assert_eq!(viewer.note_drop(PathBuf::from("a.gbr")), None);
    let opened = viewer.note_drop(PathBuf::from("b.drl")).expect("the drop");
    assert_eq!(opened.len(), 2);
    assert!(viewer.hovered.is_empty() && viewer.dropped.is_empty());
}

#[test]
fn a_drop_with_no_hover_settles_after_the_batch() {
    let (mut app, _task) = Signex::new();
    let _ = app.update(Message::from(GerberViewerMsg::FileDropped(PathBuf::from(
        "readme.txt",
    ))));
    assert_eq!(app.document_state.gerber_viewer.dropped.len(), 1);
    let _ = app.update(Message::from(GerberViewerMsg::DropSettled));
    assert!(app.document_state.gerber_viewer.dropped.is_empty());
    assert_eq!(
        app.gerber_viewer_tab(),
        None,
        "a drop of no fabrication file opens nothing"
    );
}

#[test]
fn fabrication_files_and_zips_are_told_by_their_names() {
    for name in ["a.gbr", "a.GTL", "a.drl", "a.gbrjob", "fab.ZIP"] {
        assert!(is_fabrication_name(&PathBuf::from(name)), "{name}");
    }
    for name in ["readme.txt", "board.snxpcb", "noext"] {
        assert!(!is_fabrication_name(&PathBuf::from(name)), "{name}");
    }
}

/// mockup row 193f: the zip is read in place; a file in it that is not
/// fabrication data is listed as Not read.
#[test]
fn a_zip_is_read_in_memory_and_lists_what_it_cannot_read() {
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    for (name, text) in two_layer_files() {
        writer
            .start_file(format!("gerber/{name}"), options)
            .unwrap();
        writer.write_all(text.as_bytes()).unwrap();
    }
    writer.start_file("readme.txt", options).unwrap();
    writer.write_all(b"hello").unwrap();
    let bytes = writer.finish().unwrap();
    let files = read_zip_from(bytes, &PathBuf::from("p.zip")).unwrap();
    let package = assemble(files);
    assert_eq!(package.layers.len(), 2);
    assert_eq!(package.not_read.len(), 1);
    assert_eq!(package.not_read[0].at.file, "readme.txt");
    assert!(
        package
            .layers
            .iter()
            .any(|layer| layer.image.file == "b-L1.gbr")
    );
}

#[test]
fn a_folder_is_read_file_by_file() {
    let dir = tempfile::tempdir().unwrap();
    for (name, text) in two_layer_files() {
        std::fs::write(dir.path().join(name), text).unwrap();
    }
    std::fs::write(dir.path().join("notes.txt"), "x").unwrap();
    let read = read_paths(vec![dir.path().to_path_buf()]).unwrap();
    assert_eq!(read.package.layers.len(), 2);
    assert_eq!(read.package.not_read.len(), 1);
    assert!(read_paths(vec![dir.path().join("gone.zip")]).is_err());
}

fn read_of(paths: Vec<PathBuf>) -> Arc<PackageRead> {
    Arc::new(PackageRead {
        paths,
        package: Arc::unwrap_or_clone(package()),
    })
}

/// mockup row 193: opening paths opens one Gerber Viewer tab; the read
/// lands on it, names it and enters Recent.
#[test]
fn opening_paths_opens_one_tab_and_the_read_lands_on_it() {
    let (mut app, _task) = Signex::new();
    let paths = vec![PathBuf::from("/fab/dcdc-buck")];
    let _ = app.update(Message::from(GerberViewerMsg::PathsOpened(paths.clone())));
    let _ = app.update(Message::from(GerberViewerMsg::PathsOpened(paths.clone())));
    let tabs: Vec<_> = app
        .document_state
        .tabs
        .iter()
        .filter(|t| t.kind.is_gerber_viewer())
        .collect();
    assert_eq!(tabs.len(), 1);
    let job = app.document_state.gerber_viewer.reading.expect("a read");
    let stale = GerberViewerMsg::PackageRead {
        job: job - 1,
        result: Ok(read_of(paths.clone())),
    };
    let _ = app.update(Message::from(stale));
    assert!(
        app.document_state.gerber_viewer.open.is_none(),
        "a superseded read is dropped"
    );
    let landed = GerberViewerMsg::PackageRead {
        job,
        result: Ok(read_of(paths.clone())),
    };
    let _ = app.update(Message::from(landed));
    let viewer = &app.document_state.gerber_viewer;
    assert!(viewer.open.is_some() && viewer.reading.is_none());
    assert_eq!(viewer.recent.entries()[0].paths, paths);
    assert_eq!(viewer.recent.entries()[0].layers, 2);
    let idx = app.gerber_viewer_tab().unwrap();
    assert_eq!(app.document_state.tabs[idx].title, "dcdc-buck");
    assert_eq!(app.document_state.active_tab, idx);
}

/// A Gerber Viewer tab is a page: no document, nothing to save or undo.
#[test]
fn the_tab_holds_no_document_and_closing_it_keeps_recent() {
    let (mut app, _task) = Signex::new();
    let paths = vec![PathBuf::from("/fab/p.zip")];
    let _ = app.update(Message::from(GerberViewerMsg::PathsOpened(paths.clone())));
    let job = app.document_state.gerber_viewer.reading.unwrap();
    let landed = GerberViewerMsg::PackageRead {
        job,
        result: Ok(read_of(paths)),
    };
    let _ = app.update(Message::from(landed));
    let idx = app.gerber_viewer_tab().unwrap();
    let tab = &app.document_state.tabs[idx];
    assert!(DocumentKey::of(tab).is_none());
    assert!(matches!(tab.kind, TabKind::GerberViewer));
    assert_eq!(crate::app::page_tab::page_name(&tab.kind), None);
    assert_eq!(app.page_tab_title(&tab.kind), None, "titled by its package");
    let windows = app.ui_state.windows.len();
    let _ = app.handle_undock_tab(idx);
    assert_eq!(app.ui_state.windows.len(), windows, "it stays docked");
    let _ = app.close_tab_now(idx);
    assert_eq!(app.gerber_viewer_tab(), None);
    let viewer = &app.document_state.gerber_viewer;
    assert!(viewer.open.is_none());
    assert_eq!(viewer.recent.entries().len(), 1);
}

/// mockup row 193f: a read that fails leaves the empty state, names the
/// failure in Messages and checks the recent packages.
#[test]
fn a_failed_read_keeps_the_empty_state() {
    let (mut app, _task) = Signex::new();
    let _ = app.update(Message::from(GerberViewerMsg::PathsOpened(vec![
        PathBuf::from("/x"),
    ])));
    let job = app.document_state.gerber_viewer.reading.unwrap();
    let failed = GerberViewerMsg::PackageRead {
        job,
        result: Err("gone".into()),
    };
    let _ = app.update(Message::from(failed));
    let viewer = &app.document_state.gerber_viewer;
    assert!(viewer.open.is_none() && viewer.reading.is_none());
    assert!(!viewer.has_files(), "tools that need files stay greyed");
}

/// mockup row 193f: Ctrl+O on the tab opens the Open menu.
#[test]
fn ctrl_o_on_the_tab_opens_the_open_menu() {
    use iced::keyboard::{self, Key, Modifiers};
    let (mut app, _task) = Signex::new();
    let window = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(window);
    app.open_gerber_viewer_tab();
    let press = |modifiers| keyboard::Event::KeyPressed {
        key: Key::Character("o".into()),
        modified_key: Key::Character("o".into()),
        physical_key: keyboard::key::Physical::Unidentified(
            keyboard::key::NativeCode::Unidentified,
        ),
        location: keyboard::Location::Standard,
        modifiers,
        text: None,
        repeat: false,
    };
    let message = app.gerber_viewer_key(window, &press(Modifiers::COMMAND));
    assert!(matches!(
        message,
        Some(Message::GerberViewer(GerberViewerMsg::OpenMenuToggled))
    ));
    assert!(
        app.gerber_viewer_key(window, &press(Modifiers::empty()))
            .is_none()
    );
    let _ = app.update(message.unwrap());
    assert!(app.document_state.gerber_viewer.open_menu);
    let _ = app.update(Message::from(GerberViewerMsg::OpenPressed(
        OpenKind::Folder,
    )));
    assert!(!app.document_state.gerber_viewer.open_menu);
}
