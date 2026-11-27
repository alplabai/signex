//! Rows, chips and section counts of the Components panel (#2535): part
//! counts per source, Compact and Previews rows, the hover Place and the
//! "shown of total" header count.

use super::rows::{RowColors, RowContext, RowFlags, RowLook, component_row_label, view_row_button};
use super::sections::section_count;
use super::tests::{fixture_row, library_with, thumb_look};
use super::*;
use crate::library::state::{ComponentsPanelRowKey, ComponentsRowStyle};
use signex_i18n::I18n;

/// Two libraries, one stock (3 parts) and one global (1 part), the second
/// mounted first.
fn stock_and_global() -> LibraryState {
    let mut state = LibraryState::default();
    for (name, parts) in [
        ("g", vec!["CAP-1"]),
        ("s", vec!["STOCK-C", "STOCK-CP", "STOCK-R"]),
    ] {
        let mut one = library_with(parts.iter().map(|pn| fixture_row(pn, "", "Mfr")).collect());
        let mut lib = one.open_libraries.remove(0);
        lib.root = PathBuf::from(format!("/tmp/{name}.snxlib"));
        lib.display_name = name.into();
        state.open_libraries.push(lib);
    }
    state.stock_library_root = Some(PathBuf::from("/tmp/s.snxlib"));
    state.global_libraries = vec![global_prefs::GlobalLibraryEntry {
        path: PathBuf::from("/tmp/g.snxlib"),
        remote: None,
        auto_pull: None,
    }];
    state.touch_data();
    state.sync_view_caches();
    state
}

/// The chips count parts, not libraries, and follow the filter; a source
/// whose libraries hold no part has no chip.
#[test]
fn chips_count_parts_per_source_and_follow_the_filter() {
    use ComponentsMountSource::{Global, Installed, Stock};
    let mut state = stock_and_global();
    let counts = SourceCounts::of(&listed_libraries(&state, &[]), &state.components_panel.rows);
    assert_eq!(counts.all, 4);
    assert_eq!(counts.of_source(Stock), 3);
    assert_eq!(counts.of_source(Global), 1);
    assert_eq!(counts.chips(), [None, Some(Stock), Some(Global)]);
    assert_eq!(counts.libraries(None), 2, "the caption counts libraries");
    assert_eq!(counts.libraries(Some(Stock)), 1);

    state.components_panel.filter = "stock-c".into();
    state.sync_view_caches();
    let counts = SourceCounts::of(&listed_libraries(&state, &[]), &state.components_panel.rows);
    assert_eq!(counts.all, 2, "STOCK-C and STOCK-CP");
    assert_eq!(counts.of_source(Stock), 2);
    assert_eq!(counts.of_source(Global), 0);
    assert_eq!(
        counts.chips(),
        [None, Some(Stock), Some(Global)],
        "a source keeps its chip while the filter hides its parts"
    );

    // A library with no part at all gives its source no chip.
    let mut empty = library_with(Vec::new());
    let mut lib = empty.open_libraries.remove(0);
    lib.root = PathBuf::from("/tmp/i.snxlib");
    state.open_libraries.push(lib);
    state.installed_libraries = vec![PathBuf::from("/tmp/i.snxlib")];
    state.touch_data();
    state.sync_view_caches();
    let counts = SourceCounts::of(&listed_libraries(&state, &[]), &state.components_panel.rows);
    assert!(!counts.chips().contains(&Some(Installed)));
    assert_eq!(counts.effective(Some(Installed)), None, "falls back to All");
}

/// "1 library" / "2 libraries" at the right of the chips.
#[test]
fn the_library_caption_is_singular_for_one() {
    let en = I18n::en_us();
    let one = t!(en, ids::PANELS_COMPONENTS_LIBRARY_COUNT, "count" => 1usize).into_string();
    let two = t!(en, ids::PANELS_COMPONENTS_LIBRARY_COUNT, "count" => 2usize).into_string();
    assert!(
        one.contains("library") && !one.contains("libraries"),
        "{one}"
    );
    assert!(two.contains("libraries"), "{two}");
}

/// While filtering, a library header says "shown of total"; else the total.
#[test]
fn the_header_count_reads_shown_of_total_while_filtering() {
    let en = I18n::en_us();
    let filtering = section_count(2, 34, true, &en).into_string();
    assert!(filtering.contains('2') && filtering.contains("of") && filtering.contains("34"));
    let all = section_count(2, 34, false, &en).into_string();
    assert!(all.contains("34") && !all.contains("of"), "{all}");
}

/// Compact is the default; the row height follows the style.
#[test]
fn compact_rows_are_the_default_and_previews_rows_are_taller() {
    assert_eq!(ComponentsRowStyle::default(), ComponentsRowStyle::Compact);
    assert!(!ComponentsRowStyle::default().shows_previews());
    assert_eq!(
        ComponentsRowStyle::from_previews(true),
        ComponentsRowStyle::Previews
    );
    let look = RowLook::graphite();
    assert_eq!(
        look.height_for(ComponentsRowStyle::Compact),
        look::ROW_HEIGHT
    );
    assert_eq!(
        look.height_for(ComponentsRowStyle::Previews),
        look::PREVIEW_ROW_HEIGHT
    );
    const { assert!(look::PREVIEW_ROW_HEIGHT > look::ROW_HEIGHT) };
}

/// The open menu's row wins over the selection, the selection over hover.
#[test]
fn the_row_whose_menu_is_open_stays_highlighted() {
    let colors = RowColors {
        text: iced::Color::BLACK,
        muted: iced::Color::BLACK,
        tint: iced::Color::from_rgb(0.1, 0.0, 0.0),
        hover: iced::Color::from_rgb(0.0, 0.1, 0.0),
        menu: iced::Color::from_rgb(0.0, 0.0, 0.1),
    };
    let flags = |selected, hovered, menu_open| RowFlags {
        selected,
        hovered,
        menu_open,
    };
    assert_eq!(flags(true, true, true).fill(colors), Some(colors.menu));
    assert_eq!(flags(true, true, false).fill(colors), Some(colors.tint));
    assert_eq!(flags(false, true, false).fill(colors), Some(colors.hover));
    assert_eq!(flags(false, false, false).fill(colors), None);
}

#[test]
fn a_row_names_the_part_then_its_mpn() {
    assert_eq!(component_row_label("PN-1", " "), "PN-1");
    assert_eq!(component_row_label("PN-1", "GRM155"), "PN-1 — GRM155");
}

/// Lays out the stock library's block in `style`, with `hovered` hovered.
fn stock_texts(
    style: ComponentsRowStyle,
    hovered: Option<usize>,
) -> Vec<crate::layout_probe::LaidOutText> {
    let mut state = stock_and_global();
    state.open_libraries[1].tables.get_mut("t").expect("table")[0].description =
        "Ceramic capacitor".into();
    state.touch_data();
    state.sync_view_caches();
    state.components_panel.row_style = style;
    let lib = &state.open_libraries[1];
    state.components_panel.hovered = hovered.map(|i| ComponentsPanelRowKey {
        library_path: lib.root.clone(),
        table: "t".into(),
        row_id: signex_library::RowId::from_uuid(lib.tables["t"][i].row_id),
    });
    let tokens = signex_types::theme::theme_tokens(signex_types::theme::ThemeId::GraphiteDark);
    crate::layout_probe::lay_out_texts(
        view_library_block(
            &state.open_libraries[1],
            ComponentsMountSource::Stock,
            &state.components_panel,
            "",
            thumb_look(),
            &tokens,
            &I18n::en_us(),
        ),
        iced::Size::new(320.0, 400.0),
    )
}

/// Previews rows carry a one-line description and sit a Previews row apart;
/// Compact rows show the name only.
#[test]
fn previews_rows_show_the_description_and_compact_rows_do_not() {
    let compact = stock_texts(ComponentsRowStyle::Compact, None);
    assert!(compact.iter().all(|t| t.text != "Ceramic capacitor"));
    let a = crate::layout_probe::find(&compact, "STOCK-C").bounds;
    let b = crate::layout_probe::find(&compact, "STOCK-CP").bounds;
    assert!((b.y - a.y - look::ROW_HEIGHT).abs() < 0.5, "{}", b.y - a.y);

    let previews = stock_texts(ComponentsRowStyle::Previews, None);
    crate::layout_probe::find(&previews, "Ceramic capacitor");
    let a = crate::layout_probe::find(&previews, "STOCK-C").bounds;
    let b = crate::layout_probe::find(&previews, "STOCK-CP").bounds;
    assert!(
        (b.y - a.y - look::PREVIEW_ROW_HEIGHT).abs() < 0.5,
        "{}",
        b.y - a.y
    );
}

/// Only the hovered row offers the inline Place.
#[test]
fn a_hovered_row_offers_place() {
    let idle = stock_texts(ComponentsRowStyle::Compact, None);
    assert!(idle.iter().all(|t| t.text != "Place"));
    let hovered = stock_texts(ComponentsRowStyle::Compact, Some(0));
    assert_eq!(hovered.iter().filter(|t| t.text == "Place").count(), 1);
}

/// The messages a mouse `button` press (and release) on the text `label`
/// of a hovered Compact row publishes, hover enter/exit left out.
fn row_press(button: iced::mouse::Button, label: &str) -> Vec<LibraryMessage> {
    use iced::mouse::Event;
    let row = fixture_row("R0805_10k", "ERA-3AED103V", "Panasonic");
    let key = ComponentsPanelRowKey {
        library_path: PathBuf::from("lib.snxlib"),
        table: "Passives".to_owned(),
        row_id: signex_library::RowId::from_uuid(row.row_id),
    };
    let tokens = signex_types::theme::theme_tokens(signex_types::theme::ThemeId::GraphiteDark);
    let i18n = I18n::en_us();
    let cx = RowContext {
        style: ComponentsRowStyle::Compact,
        colors: RowColors {
            text: iced::Color::WHITE,
            muted: iced::Color::WHITE,
            tint: iced::Color::BLACK,
            hover: iced::Color::BLACK,
            menu: iced::Color::BLACK,
        },
        look: RowLook::graphite(),
        tokens: &tokens,
        i18n: &i18n,
    };
    let flags = RowFlags {
        hovered: true,
        ..RowFlags::default()
    };
    let window = iced::Size::new(600.0, 200.0);
    let build = || view_row_button(key.clone(), &row, flags, None, &cx);
    let texts = crate::layout_probe::lay_out_texts(build(), window);
    let at = crate::layout_probe::find(&texts, label).bounds.center();
    let mut published = crate::layout_probe::messages_for_events(
        build(),
        window,
        at,
        &[Event::ButtonPressed(button), Event::ButtonReleased(button)].map(iced::Event::Mouse),
    );
    published.retain(|m| {
        !matches!(
            m,
            LibraryMessage::ComponentsPanelRowEntered(_)
                | LibraryMessage::ComponentsPanelRowExited(_)
        )
    });
    published
}

/// A press on a row's label selects it, a press on its hover Place places
/// it, a right press opens its menu: each publishes that one message only
/// (#2631; restores the row-press coverage #2620 dropped).
#[test]
fn a_row_publishes_press_place_and_right_click_apart() {
    use iced::mouse::Button;
    let label = "R0805_10k — ERA-3AED103V";

    let pressed = row_press(Button::Left, label);
    assert!(
        matches!(
            pressed.as_slice(),
            [LibraryMessage::ComponentsPanelRowPressed(key)]
                if key.table == "Passives" && key.library_path == PathBuf::from("lib.snxlib")
        ),
        "{pressed:?}"
    );

    let placed = row_press(Button::Left, "Place");
    assert!(
        matches!(
            placed.as_slice(),
            [LibraryMessage::ComponentsPanelPlace { table, .. }] if table == "Passives"
        ),
        "{placed:?}"
    );

    let right = row_press(Button::Right, label);
    assert!(
        matches!(
            right.as_slice(),
            [LibraryMessage::ComponentsPanelRowRightClicked(_)]
        ),
        "{right:?}"
    );
}
