//! Project thumbnails (mockup row 1): the snapshot of a board and of a root
//! sheet, the primitive cap, the cache file, the slot a tile shows, the ink
//! colours and the save that rebuilds a preview.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use iced::Color;
use signex_types::pcb::PcbBoard;
use signex_types::schematic::{Point, SchematicSheet, Symbol, Wire, power_port_lib_symbol};
use signex_types::theme::{ThemeId, canvas_colors, theme_tokens};

use super::cache::{self, CACHE_VERSION};
use super::snapshot::{self, MAX_SEGMENTS};
use super::tile::fit;
use super::*;
use crate::styles::board_palette::BoardPalette;

fn hex(c: Color) -> String {
    let [r, g, b, _] = c.into_rgba8();
    format!("#{r:02X}{g:02X}{b:02X}")
}

fn luminance(c: Color) -> f32 {
    0.2126 * c.r + 0.7152 * c.g + 0.0722 * c.b
}

/// A 40 x 30 mm board: its outline, a top and a bottom track, a via, a top
/// part (one surface pad, one silkscreen line), a bottom part (one surface
/// pad, one bottom silkscreen line) and a through-hole part.
fn small_board() -> PcbBoard {
    serde_json::from_str(
        r#"{
          "uuid": "0b7d6f52-7f5e-4d4f-9a7e-1a2b3c4d5e01",
          "outline": [
            { "x": 0.0, "y": 0.0 }, { "x": 40.0, "y": 0.0 },
            { "x": 40.0, "y": 30.0 }, { "x": 0.0, "y": 30.0 }
          ],
          "segments": [
            { "uuid": "0b7d6f52-7f5e-4d4f-9a7e-1a2b3c4d5e02",
              "start": { "x": 5.0, "y": 5.0 }, "end": { "x": 20.0, "y": 5.0 },
              "width": 0.25, "layer": "TopCopper", "net": 1 },
            { "uuid": "0b7d6f52-7f5e-4d4f-9a7e-1a2b3c4d5e03",
              "start": { "x": 5.0, "y": 25.0 }, "end": { "x": 20.0, "y": 25.0 },
              "width": 0.25, "layer": "BottomCopper", "net": 1 }
          ],
          "vias": [
            { "uuid": "0b7d6f52-7f5e-4d4f-9a7e-1a2b3c4d5e04",
              "position": { "x": 20.0, "y": 15.0 }, "diameter": 0.6, "drill": 0.3,
              "layers": ["TopCopper", "BottomCopper"], "net": 1 }
          ],
          "footprints": [
            { "uuid": "0b7d6f52-7f5e-4d4f-9a7e-1a2b3c4d5e05",
              "reference": "R1", "position": { "x": 10.0, "y": 10.0 }, "layer": "TopCopper",
              "pads": [
                { "uuid": "0b7d6f52-7f5e-4d4f-9a7e-1a2b3c4d5e06", "number": "1",
                  "pad_type": "smd", "shape": "rect",
                  "position": { "x": 1.0, "y": 0.0 }, "size": { "x": 1.0, "y": 0.8 },
                  "drill": null, "layers": ["TopCopper"], "net": null }
              ],
              "graphics": [
                { "graphic_type": "line", "layer": "TopSilkscreen", "width": 0.12,
                  "start": { "x": -2.0, "y": -1.0 }, "end": { "x": 2.0, "y": -1.0 } }
              ] },
            { "uuid": "0b7d6f52-7f5e-4d4f-9a7e-1a2b3c4d5e07",
              "reference": "C1", "position": { "x": 30.0, "y": 20.0 }, "layer": "BottomCopper",
              "pads": [
                { "uuid": "0b7d6f52-7f5e-4d4f-9a7e-1a2b3c4d5e08", "number": "1",
                  "pad_type": "smd", "shape": "rect",
                  "position": { "x": 1.0, "y": 0.0 }, "size": { "x": 1.0, "y": 0.8 },
                  "drill": null, "layers": ["BottomCopper"], "net": null }
              ],
              "graphics": [
                { "graphic_type": "line", "layer": "BottomSilkscreen", "width": 0.12,
                  "start": { "x": -2.0, "y": 1.0 }, "end": { "x": 2.0, "y": 1.0 } }
              ] },
            { "uuid": "0b7d6f52-7f5e-4d4f-9a7e-1a2b3c4d5e09",
              "reference": "J1", "position": { "x": 30.0, "y": 8.0 }, "layer": "TopCopper",
              "pads": [
                { "uuid": "0b7d6f52-7f5e-4d4f-9a7e-1a2b3c4d5e0a", "number": "1",
                  "pad_type": "thru", "shape": "circle",
                  "position": { "x": 0.0, "y": 0.0 }, "size": { "x": 1.7, "y": 1.7 },
                  "drill": { "diameter": 1.0 }, "layers": ["TopCopper", "BottomCopper"],
                  "net": null }
              ] }
          ]
        }"#,
    )
    .expect("valid board")
}

/// A root sheet: one wire, one part drawn with a body from the sheet's
/// symbol table, and a ground power port with no table entry.
pub(crate) fn small_sheet() -> SchematicSheet {
    let mut sheet = SchematicSheet::default();
    sheet.wires.push(Wire {
        start: Point::new(0.0, 0.0),
        end: Point::new(10.0, 0.0),
        ..Wire::default()
    });
    let mut body = power_port_lib_symbol("power:GND");
    body.id = "Test:Box".to_string();
    sheet.lib_symbols.insert("Test:Box".to_string(), body);
    sheet.symbols.push(Symbol {
        lib_id: "Test:Box".to_string(),
        position: Point::new(20.0, 10.0),
        ..Symbol::default()
    });
    sheet.symbols.push(Symbol {
        lib_id: "power:GND".to_string(),
        is_power: true,
        position: Point::new(40.0, 20.0),
        ..Symbol::default()
    });
    sheet
}

fn inks(thumbnail: &Thumbnail) -> Vec<Ink> {
    thumbnail.primitives.iter().map(|p| p.ink).collect()
}

fn count(thumbnail: &Thumbnail, ink: Ink) -> usize {
    thumbnail.primitives.iter().filter(|p| p.ink == ink).count()
}

// ─── Snapshot ─────────────────────────────────────────────────────

#[test]
fn a_board_snapshot_draws_outline_copper_by_face_and_silkscreen() {
    let thumbnail = snapshot::board(&small_board());
    assert_eq!(thumbnail.kind, ThumbnailKind::Board);
    assert_eq!(count(&thumbnail, Ink::BoardOutline), 1);
    // A track and a surface pad on each face.
    assert_eq!(count(&thumbnail, Ink::TopCopper), 2);
    assert_eq!(count(&thumbnail, Ink::BottomCopper), 2);
    // The via and the through-hole pad reach both faces.
    assert_eq!(count(&thumbnail, Ink::MultiLayer), 2);
    assert_eq!(count(&thumbnail, Ink::TopSilkscreen), 1);
    assert_eq!(count(&thumbnail, Ink::BottomSilkscreen), 1);
    // No sheet inks on a board.
    assert_eq!(count(&thumbnail, Ink::Wire), 0);
}

#[test]
fn pads_and_vias_are_filled_tracks_and_outlines_are_not() {
    let thumbnail = snapshot::board(&small_board());
    for primitive in &thumbnail.primitives {
        let is_area = matches!(primitive.ink, Ink::MultiLayer)
            || matches!(
                (&primitive.ink, &primitive.geometry),
                (
                    Ink::TopCopper | Ink::BottomCopper,
                    Geometry::Polyline { closed: true, .. }
                )
            );
        assert_eq!(primitive.filled, is_area, "{primitive:?}");
    }
}

#[test]
fn the_board_outline_is_a_closed_ring_in_a_view_box_of_the_board_aspect() {
    let thumbnail = snapshot::board(&small_board());
    // 40 x 30 mm: the longer side is the view box side.
    assert_eq!(thumbnail.view_box, [VIEW_BOX_SIDE, 750.0]);
    let outline = thumbnail
        .primitives
        .iter()
        .find(|p| p.ink == Ink::BoardOutline)
        .expect("outline");
    match &outline.geometry {
        Geometry::Polyline { points, closed } => {
            assert!(closed);
            assert_eq!(
                points,
                &[[0.0, 0.0], [1000.0, 0.0], [1000.0, 750.0], [0.0, 750.0]]
            );
        }
        other => panic!("outline drawn as {other:?}"),
    }
    for primitive in &thumbnail.primitives {
        if let Geometry::Polyline { points, .. } = &primitive.geometry {
            for [x, y] in points {
                assert!((0.0..=1000.0).contains(x) && (0.0..=750.0).contains(y));
            }
        }
    }
}

#[test]
fn the_top_face_paints_over_the_bottom_and_the_edge_paints_last() {
    let order = inks(&snapshot::board(&small_board()));
    let first = |ink: Ink| order.iter().position(|i| *i == ink).expect("drawn");
    let last = |ink: Ink| order.iter().rposition(|i| *i == ink).expect("drawn");
    assert!(last(Ink::BottomSilkscreen) < first(Ink::BottomCopper));
    assert!(last(Ink::BottomCopper) < first(Ink::TopCopper));
    assert!(last(Ink::TopCopper) < first(Ink::TopSilkscreen));
    assert_eq!(order.last(), Some(&Ink::BoardOutline));
}

#[test]
fn an_empty_board_has_no_art_and_a_square_view_box() {
    let board: PcbBoard =
        serde_json::from_str(r#"{ "uuid": "0b7d6f52-7f5e-4d4f-9a7e-1a2b3c4d5e0b" }"#)
            .expect("valid board");
    let thumbnail = snapshot::board(&board);
    assert!(thumbnail.primitives.is_empty());
    assert_eq!(thumbnail.view_box, [VIEW_BOX_SIDE, VIEW_BOX_SIDE]);
}

#[test]
fn a_sheet_snapshot_draws_wires_bodies_and_power_ports() {
    let thumbnail = snapshot::sheet(&small_sheet());
    assert_eq!(thumbnail.kind, ThumbnailKind::Sheet);
    assert_eq!(count(&thumbnail, Ink::Wire), 1);
    assert!(count(&thumbnail, Ink::SymbolBody) > 0, "the part's body");
    assert!(count(&thumbnail, Ink::PowerPort) > 0, "the ground port");
    assert_eq!(count(&thumbnail, Ink::TopCopper), 0);
}

#[test]
fn a_big_sheet_keeps_the_largest_primitives_up_to_the_cap() {
    // Wire n runs n mm along y = n, for n = 1..=4000.
    let mut sheet = SchematicSheet::default();
    for n in 1..=4000 {
        let n = f64::from(n);
        sheet.wires.push(Wire {
            start: Point::new(0.0, n),
            end: Point::new(n, n),
            ..Wire::default()
        });
    }
    let thumbnail = snapshot::sheet(&sheet);
    assert_eq!(thumbnail.primitives.len(), MAX_SEGMENTS);
    // 4000 mm wide maps to 1000 units: the 1000 shortest wires (1..=1000
    // mm, under 250.25 units) are the ones dropped.
    for primitive in &thumbnail.primitives {
        let Geometry::Polyline { points, .. } = &primitive.geometry else {
            panic!("a wire is a polyline");
        };
        let length = points[1][0] - points[0][0];
        assert!(length >= 250.0, "kept a dropped wire: {length}");
    }
}

// ─── Inks ─────────────────────────────────────────────────────────

#[test]
fn board_inks_are_the_board_palette_in_both_themes() {
    let board = BoardPalette::new();
    for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
        let sheet = canvas_colors(id);
        assert_eq!(hex(ink_colour(Ink::TopCopper, &board, &sheet)), "#D04040");
        assert_eq!(
            hex(ink_colour(Ink::BottomCopper, &board, &sheet)),
            "#386CD0"
        );
        assert_eq!(
            hex(ink_colour(Ink::BoardOutline, &board, &sheet)),
            "#E000E0"
        );
        assert_eq!(
            ink_colour(Ink::MultiLayer, &board, &sheet),
            board.multi_layer
        );
        assert_eq!(hex(board.canvas), "#111214");
    }
}

#[test]
fn bottom_silkscreen_is_the_top_hue_darker() {
    let board = BoardPalette::new();
    let sheet = canvas_colors(ThemeId::GraphiteLight);
    let top = ink_colour(Ink::TopSilkscreen, &board, &sheet);
    let bottom = ink_colour(Ink::BottomSilkscreen, &board, &sheet);
    assert!(luminance(bottom) < luminance(top));
    let ratio = |c: Color| [c.r / c.g.max(f32::EPSILON), c.b / c.g.max(f32::EPSILON)];
    let (t, b) = (ratio(top), ratio(bottom));
    assert!((t[0] - b[0]).abs() < 1e-4 && (t[1] - b[1]).abs() < 1e-4);
}

#[test]
fn power_ports_draw_in_the_power_red_of_each_theme() {
    let board = BoardPalette::new();
    let light = ink_colour(
        Ink::PowerPort,
        &board,
        &canvas_colors(ThemeId::GraphiteLight),
    );
    let dark = ink_colour(
        Ink::PowerPort,
        &board,
        &canvas_colors(ThemeId::GraphiteDark),
    );
    assert_eq!(hex(light), "#C0392B");
    assert_eq!(hex(dark), "#E06C5C");
}

#[test]
fn sheet_inks_follow_the_theme_the_tokens_belong_to() {
    for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
        assert_eq!(sheet_canvas(&theme_tokens(id)), canvas_colors(id));
    }
    let light = Inks::new(&theme_tokens(ThemeId::GraphiteLight));
    let dark = Inks::new(&theme_tokens(ThemeId::GraphiteDark));
    assert_ne!(light.colour(Ink::Wire), dark.colour(Ink::Wire));
    // The board palette is the same under both chromes.
    assert_eq!(light.colour(Ink::TopCopper), dark.colour(Ink::TopCopper));
}

#[test]
fn the_selected_option_look_does_not_change_the_sheet_theme() {
    let id = ThemeId::GraphiteDark;
    let accent = signex_types::theme::SelectedSegmentLook::Accent.apply(id, theme_tokens(id));
    assert_eq!(sheet_canvas(&accent), canvas_colors(id));
}

#[test]
fn the_art_is_fitted_centred_with_its_aspect() {
    let (scale, offset) = fit([1000.0, 500.0], [5.0, 5.0, 54.0, 36.0]);
    assert!((scale - 0.054).abs() < 1e-6);
    assert!((offset[0] - 5.0).abs() < 1e-4);
    assert!((offset[1] - 9.5).abs() < 1e-4);
}

#[test]
fn every_slot_builds_a_tile_in_both_themes() {
    let i18n = signex_i18n::I18n::en_us();
    let ready = Arc::new(snapshot::board(&small_board()));
    let sheet = Arc::new(snapshot::sheet(&small_sheet()));
    for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
        let tokens = theme_tokens(id);
        for slot in [
            ThumbnailSlot::NoPreview,
            ThumbnailSlot::MissingOnDisk,
            ThumbnailSlot::Ready(Arc::clone(&ready)),
            ThumbnailSlot::Ready(Arc::clone(&sheet)),
        ] {
            let element: iced::Element<'_, ()> = tile(&tokens, &i18n, slot);
            drop(element);
        }
    }
}

// ─── Slots ────────────────────────────────────────────────────────

#[test]
fn a_slot_is_missing_no_preview_or_ready() {
    let project = PathBuf::from("/w/thumb/a.snxprj");
    let other = PathBuf::from("/w/thumb/b.snxprj");
    let thumbnail = Arc::new(snapshot::sheet(&small_sheet()));
    let mut thumbnails = ProjectThumbnails::default();
    assert_eq!(thumbnails.slot(&project, false), ThumbnailSlot::NoPreview);
    thumbnails.insert(project.clone(), Arc::clone(&thumbnail));
    assert_eq!(
        thumbnails.slot(&project, false),
        ThumbnailSlot::Ready(Arc::clone(&thumbnail))
    );
    // Missing on disk never shows a stale picture.
    assert_eq!(
        thumbnails.slot(&project, true),
        ThumbnailSlot::MissingOnDisk
    );
    assert_eq!(thumbnails.slot(&other, true), ThumbnailSlot::MissingOnDisk);
    assert_eq!(thumbnails.slot(&other, false), ThumbnailSlot::NoPreview);
}

#[test]
fn a_cached_render_never_replaces_one_saved_this_session() {
    let project = PathBuf::from("/w/thumb/a.snxprj");
    let saved = Arc::new(snapshot::board(&small_board()));
    let cached = Arc::new(snapshot::sheet(&small_sheet()));
    let mut thumbnails = ProjectThumbnails::default();
    thumbnails.insert(project.clone(), Arc::clone(&saved));
    assert!(!thumbnails.insert_cached(project.clone(), cached));
    assert_eq!(thumbnails.get(&project), Some(&saved));
}

#[test]
fn a_render_moves_to_the_listed_spelling() {
    let opened = PathBuf::from("/w/link/a.snxprj");
    let listed = PathBuf::from("/w/real/a.snxprj");
    let thumbnail = Arc::new(snapshot::sheet(&small_sheet()));
    let mut thumbnails = ProjectThumbnails::default();
    thumbnails.insert(opened.clone(), Arc::clone(&thumbnail));
    thumbnails.rekey(&opened, listed.clone());
    assert_eq!(thumbnails.get(&opened), None);
    assert_eq!(thumbnails.get(&listed), Some(&thumbnail));
    assert_eq!(thumbnails.len(), 1);
}

// ─── Cache ────────────────────────────────────────────────────────

#[test]
fn fnv1a_64_matches_the_reference_vectors() {
    assert_eq!(cache::fnv1a_64(b""), 0xcbf2_9ce4_8422_2325);
    assert_eq!(cache::fnv1a_64(b"a"), 0xaf63_dc4c_8601_ec8c);
    assert_eq!(cache::fnv1a_64(b"foobar"), 0x8594_4171_f739_67e8);
    assert_eq!(cache::file_name(Path::new("a")), "af63dc4c8601ec8c.json");
}

#[test]
fn a_cache_file_round_trips() {
    let root = tempfile::tempdir().expect("temp dir");
    let project = PathBuf::from("/w/thumb/a.snxprj");
    let thumbnail = snapshot::board(&small_board());
    cache::write(root.path(), &project, &thumbnail).expect("written");
    assert!(cache::cache_path(root.path(), &project).is_file());
    let loaded = cache::read_all(root.path(), std::slice::from_ref(&project));
    assert!(loaded.unreadable.is_empty());
    assert_eq!(loaded.found.len(), 1);
    assert_eq!(loaded.found[0].0, project);
    assert_eq!(*loaded.found[0].1, thumbnail);
}

#[test]
fn another_version_another_project_or_junk_reads_as_no_preview() {
    let project = PathBuf::from("/w/thumb/a.snxprj");
    let thumbnail = snapshot::sheet(&small_sheet());
    let bytes = cache::encode(&project, &thumbnail).expect("encoded");
    assert_eq!(cache::decode(&project, &bytes), Some(thumbnail));
    let mut value: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
    value["version"] = serde_json::json!(CACHE_VERSION + 1);
    let newer = serde_json::to_vec(&value).expect("json");
    assert_eq!(cache::decode(&project, &newer), None);
    assert_eq!(cache::decode(Path::new("/w/thumb/b.snxprj"), &bytes), None);
    assert_eq!(cache::decode(&project, b"{ not json"), None);
    assert_eq!(cache::decode(&project, br#"{ "version": 1 }"#), None);
}

#[test]
fn a_missing_cache_file_is_no_preview_and_an_unreadable_one_is_reported() {
    let root = tempfile::tempdir().expect("temp dir");
    let missing = PathBuf::from("/w/thumb/never-saved.snxprj");
    let blocked = PathBuf::from("/w/thumb/blocked.snxprj");
    // A folder where the file should be cannot be read as one.
    std::fs::create_dir_all(cache::cache_path(root.path(), &blocked)).expect("folder");
    let loaded = cache::read_all(root.path(), &[missing, blocked.clone()]);
    assert!(loaded.found.is_empty());
    assert_eq!(loaded.unreadable.len(), 1);
    assert_eq!(loaded.unreadable[0].0, blocked);
}

// ─── Messages ─────────────────────────────────────────────────────

fn project_path() -> PathBuf {
    PathBuf::from("/w/thumb/ProjectA.snxprj")
}

#[test]
fn loaded_renders_are_kept_and_a_landed_write_moves_to_the_listed_spelling() {
    let (mut app, _boot) = crate::app::Signex::new();
    let cached = Arc::new(snapshot::sheet(&small_sheet()));
    let _task = app.update(message(ProjectThumbnailMsg::Loaded(Ok(LoadedThumbnails {
        found: vec![(project_path(), Arc::clone(&cached))],
        unreadable: vec![(PathBuf::from("/w/other.snxprj"), "denied".to_string())],
    }))));
    assert_eq!(
        app.ui_state.project_thumbnails.slot(&project_path(), false),
        ThumbnailSlot::Ready(Arc::clone(&cached))
    );
    let listed = PathBuf::from("/w/real/ProjectA.snxprj");
    let _task = app.update(message(ProjectThumbnailMsg::Written(Ok(
        WrittenThumbnail {
            project: project_path(),
            listed_as: listed.clone(),
        },
    ))));
    assert_eq!(app.ui_state.project_thumbnails.get(&listed), Some(&cached));
    // A failed write keeps the render shown this session.
    let _task = app.update(message(ProjectThumbnailMsg::Written(Err(
        ThumbnailWriteFailure {
            project: listed.clone(),
            listed_as: listed.clone(),
            reason: "disk full".to_string(),
        },
    ))));
    assert_eq!(app.ui_state.project_thumbnails.get(&listed), Some(&cached));
}

/// A project opened under another spelling than its listed one: a failed
/// cache write still moves the render to the listed spelling, so the Start
/// screen row shows it this session instead of 'No preview yet'.
#[test]
fn a_failed_write_still_moves_the_render_to_the_listed_spelling() {
    let (mut app, _boot) = crate::app::Signex::new();
    let cached = Arc::new(snapshot::sheet(&small_sheet()));
    let _task = app.update(message(ProjectThumbnailMsg::Loaded(Ok(LoadedThumbnails {
        found: vec![(project_path(), Arc::clone(&cached))],
        unreadable: Vec::new(),
    }))));
    let listed = PathBuf::from("/w/real/ProjectA.snxprj");
    let _task = app.update(message(ProjectThumbnailMsg::Written(Err(
        ThumbnailWriteFailure {
            project: project_path(),
            listed_as: listed.clone(),
            reason: "disk full".to_string(),
        },
    ))));
    assert_eq!(app.ui_state.project_thumbnails.get(&listed), Some(&cached));
    assert_eq!(app.ui_state.project_thumbnails.get(&project_path()), None);
}
