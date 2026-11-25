//! #2058 / #2430 / #1298: the one-screen Export PDF dialog. Its text comes
//! from the catalog; the folded sections summarise themselves; Select All
//! and Clear move the count, the thumbnails and the footer; and every
//! dependent of a switched-off option takes no input.

use std::path::{Path, PathBuf};

use iced::{Point, Size};
use signex_i18n::{I18n, LanguagePref, ids};
use signex_output::{PdfLayer, PdfOptions};
use signex_types::schematic::InstancePath;

use super::rail::rail_entries;
use super::sheets::sheets_summary;
use super::viewer::paper_caption;
use crate::app::state::{PdfSection, PreviewState};
use crate::app::{Message, PdfOptionMsg, PrintPreviewMsg, Signex, UiMsg};
use crate::layout_probe::{lay_out_texts, messages_on_click_at};

/// The settings column laid out with room for every section.
const COLUMN: Size = Size::new(320.0, 4000.0);

/// Every laid-out text, without the isolation marks around placeables.
fn texts_of(element: iced::Element<'_, Message>, window: Size) -> Vec<String> {
    lay_out_texts(element, window)
        .into_iter()
        .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect()
}

/// A preview of four sheets, all ticked, every section open.
fn four_sheets() -> PreviewState {
    let mut preview = PreviewState::blank_for_tests();
    preview.sheet_files = (1..=4)
        .map(|n| {
            (
                (
                    PathBuf::from(format!("/w/s{n}.snxsch")),
                    InstancePath::root(),
                ),
                format!("s{n}"),
            )
        })
        .collect();
    preview.selected_files = preview.sheet_files.iter().map(|(k, _)| k.clone()).collect();
    for folded in [
        PdfSection::Structure,
        PdfSection::BookmarksAndNets,
        PdfSection::SchematicsInclude,
    ] {
        preview.dialog.sections.toggle(folded);
    }
    preview
}

/// Whether `messages` holds one that `wanted` picks out.
fn sent(messages: &[Message], wanted: impl Fn(&PrintPreviewMsg) -> bool) -> bool {
    messages
        .iter()
        .any(|m| matches!(m, Message::PrintPreview(msg) if wanted(msg)))
}

/// The centre of the last text `label` in `preview`'s settings column.
fn centre_of(app: &Signex, preview: &PreviewState, label: &str) -> Point {
    lay_out_texts(app.view_pdf_settings_column(preview), COLUMN)
        .into_iter()
        .rfind(|t| t.text == label)
        .unwrap_or_else(|| panic!("no {label:?} in the settings column"))
        .bounds
        .center()
}

/// What a click where `label` lays out in the `on` column sends, in the
/// `on` column and in the `off` column. The veil keeps the layout, so the
/// point is the same control in both.
fn click_on_and_off(
    app: &Signex,
    on: &PreviewState,
    off: &PreviewState,
    label: &str,
) -> (Vec<Message>, Vec<Message>) {
    let at = centre_of(app, on, label);
    (
        messages_on_click_at(app.view_pdf_settings_column(on), COLUMN, None, at),
        messages_on_click_at(app.view_pdf_settings_column(off), COLUMN, None, at),
    )
}

/// #2060: every page-size and scale name keeps the English its domain
/// `Display` gives, comes from the catalog, and follows the language. The
/// ISO sizes drop the "ISO " prefix, as mockup row 111 names them ("A4").
#[test]
fn the_page_setup_names_keep_their_english() {
    use super::page_setup::{page_size_label, scale_mode_label};
    use signex_output::{PageSizeKind, PdfScaleMode};

    let en = I18n::en_us();
    let xa = I18n::new(&LanguagePref::Locale("en-XA".to_owned()));
    let check = |id, english: String| {
        assert_eq!(en.message_plain(id, &[]), english);
        // The pseudo-locale only changes letters, so `1:1` reads the same.
        if english.chars().any(char::is_alphabetic) {
            assert_ne!(xa.message_plain(id, &[]), english);
        }
    };
    for kind in PageSizeKind::ALL {
        let display = kind.to_string();
        let english = display.strip_prefix("ISO ").unwrap_or(&display);
        check(page_size_label(kind), english.to_owned());
    }
    assert_eq!(
        en.message_plain(page_size_label(PageSizeKind::IsoA4), &[]),
        "A4"
    );
    assert_eq!(
        en.message_plain(page_size_label(PageSizeKind::AnsiA), &[]),
        "ANSI A"
    );
    for mode in PdfScaleMode::ALL {
        check(scale_mode_label(mode), mode.to_string());
    }
}

/// A section title follows the language in force.
#[test]
fn the_pdf_text_follows_the_language() {
    let (mut app, _task) = Signex::new();
    let switch = |app: &mut Signex, tag: &str| {
        let pref = LanguagePref::Locale(tag.to_owned());
        let _ = app.update(Message::Ui(UiMsg::LanguageChanged(pref)));
    };
    // The text the section header lays out.
    let shown = |app: &Signex| {
        let window = Size::new(400.0, 100.0);
        texts_of(
            app.pdf_section_header(PdfSection::Sheets, ids::PDF_SECTION_SHEETS, None, true),
            window,
        )
        .join("|")
    };
    switch(&mut app, "en-US");
    assert_eq!(shown(&app), "SHEETS");
    switch(&mut app, "en-XA");
    assert_ne!(shown(&app), "SHEETS");
    let i18n = &app.ui_state.i18n;
    assert_eq!(
        shown(&app),
        i18n.upper_for_display(&i18n.message_plain(ids::PDF_SECTION_SHEETS, &[]))
    );
    let default = PdfOptions::default();
    assert_ne!(
        paper_caption(&default, &app.ui_state.i18n).into_string(),
        paper_caption(&default, &I18n::en_us()).into_string()
    );
    switch(&mut app, "en-US");
    assert_eq!(shown(&app), "SHEETS");
}

/// Folded sections say what they hold: "4 of 7" boxes of Schematics
/// include, "3 on" of Bookmarks and Nets, the logical structure, and the
/// ticked sheets.
#[test]
fn folded_sections_show_their_summaries() {
    let (app, _task) = Signex::new();
    let mut preview = PreviewState::blank_for_tests();
    preview.sheet_files = four_sheets().sheet_files;
    preview.selected_files = preview.sheet_files[..3]
        .iter()
        .map(|(k, _)| k.clone())
        .collect();
    let options = &mut preview.pdf_options;
    // Schematics include: No-ERC, No-Connect, Notes and Collapsed notes.
    options.include_parameter_sets = false;
    options.include_probes = false;
    options.include_blankets = false;
    options.include_collapsed_notes = true;
    // Bookmarks and Nets: Pins, Net Labels and Ports.
    options.bookmark_ports = true;
    options.global_bookmarks = false;
    options.use_physical_structure = false;

    let texts = texts_of(app.view_pdf_settings_column(&preview), COLUMN);
    for summary in ["3 of 4", "4 of 7", "3 on", "Logical"] {
        assert!(texts.iter().any(|t| t == summary), "{summary}: {texts:#?}");
    }
    // Folded: their boxes are not drawn.
    assert!(!texts.iter().any(|t| t == "Probes"), "{texts:#?}");
    assert_eq!(sheets_summary(&preview), (3, 4));
}

/// Bookmarks and Nets has no "Include Component Parameters" box: part
/// bookmarks follow "One bookmark per part" in Bookmarks (Caner, 2026-11-20).
#[test]
fn bookmarks_and_nets_has_no_component_parameters_box() {
    let (app, _task) = Signex::new();
    // `four_sheets` already opens every section, Bookmarks and Nets too.
    let preview = four_sheets();
    let texts = texts_of(app.view_pdf_settings_column(&preview), COLUMN);
    assert!(
        texts
            .iter()
            .any(|t| t == "Global Bookmarks for Components and Nets"),
        "the section is open: {texts:#?}"
    );
    assert!(
        !texts.iter().any(|t| t.contains("Component Parameters")),
        "{texts:#?}"
    );
}

/// Clear and Select All re-render the preview: the "N of M" count, the
/// thumbnails (dimmed, not removed) and the footer follow.
#[test]
fn clear_and_select_all_move_the_count_the_thumbnails_and_the_footer() {
    use crate::app::handlers::menu::export::settle;
    use crate::app::handlers::menu::export::tests::{app_workspace, open_with, sheet_with_net};
    let dir = "/w/pdf-sheets";
    let mut app = app_workspace(dir, &["top.snxsch", "b.snxsch"]);
    let top = Path::new(dir).join("top.snxsch");
    let b = Path::new(dir).join("b.snxsch");
    open_with(
        &mut app.document_state,
        &top,
        sheet_with_net("R1", "N1", &[]),
    );
    open_with(&mut app.document_state, &b, sheet_with_net("R2", "N2", &[]));
    app.document_state.active_path = Some(top);
    let task = app.handle_print_preview_requested();
    settle(&mut app, task);

    let footer = |app: &Signex| {
        let preview = app.document_state.preview.as_ref().expect("preview");
        app.pdf_footer_caption(preview)
            .as_str()
            .replace(['\u{2068}', '\u{2069}'], "")
    };
    let count = |app: &Signex| {
        let preview = app.document_state.preview.as_ref().expect("preview");
        let texts = texts_of(app.view_pdf_settings_column(preview), COLUMN);
        (sheets_summary(preview), texts)
    };
    let dimmed = |app: &Signex| {
        let preview = app.document_state.preview.as_ref().expect("preview");
        rail_entries(preview)
            .iter()
            .map(|e| !e.ticked)
            .collect::<Vec<_>>()
    };

    let all = footer(&app);
    assert!(all.starts_with("2 pages · "), "{all}");
    assert!(all.ends_with(" · Medium (300 dpi)"), "{all}");

    let task = app.update(Message::PrintPreview(PrintPreviewMsg::ClearAllFiles));
    settle(&mut app, task);
    let (summary, texts) = count(&app);
    assert_eq!(summary, (0, 2));
    assert!(texts.iter().any(|t| t == "0 of 2"), "{texts:#?}");
    assert_eq!(dimmed(&app), [true, true], "kept, dimmed");
    assert_eq!(footer(&app), "No files selected for export");

    let task = app.update(Message::PrintPreview(PrintPreviewMsg::SelectAllFiles));
    settle(&mut app, task);
    let (summary, texts) = count(&app);
    assert_eq!(summary, (2, 2));
    assert!(texts.iter().any(|t| t == "2 of 2"), "{texts:#?}");
    assert_eq!(dimmed(&app), [false, false]);
    assert_eq!(footer(&app), all);
}

/// Mockup row 111: away from Specific the page field is empty, so it reads
/// its placeholder, and the typed list comes back with Specific.
#[test]
fn the_page_field_shows_its_list_only_for_specific() {
    use super::sheets::pages_field_text;
    use signex_output::PageRange;
    assert_eq!(pages_field_text(&PageRange::All, "1-2, 4"), "");
    assert_eq!(pages_field_text(&PageRange::Current, "1-2, 4"), "");
    assert_eq!(
        pages_field_text(&PageRange::Specific(vec![1, 2, 4]), "1-2, 4"),
        "1-2, 4"
    );
}

/// Mockup row 111, both Graphite themes: a field that takes no input (the
/// page list, the Scale percent under Fit to Page) sits on the sunken fill in
/// faint ink; a live field keeps the field look.
#[test]
fn an_inert_field_is_greyed_in_both_themes() {
    use crate::panels::form_look::{greyable_input_style, text_input_style};
    use iced::widget::text_input::Status;
    use signex_types::theme::{ThemeId, theme_tokens};
    for &id in ThemeId::BUILTINS {
        let theme = Signex::id_to_iced_theme(id);
        let r = crate::styles::tokens::resolve(&theme_tokens(id));
        let inert = greyable_input_style(&theme, Status::Disabled);
        assert_eq!(
            inert.background,
            iced::Background::Color(r.sunken),
            "{id:?}"
        );
        assert_eq!(inert.value, r.ink4, "{id:?}");
        let live = greyable_input_style(&theme, Status::Active);
        let plain = text_input_style(&theme, Status::Active);
        assert_eq!(live.background, plain.background, "{id:?}");
        assert_eq!(live.value, plain.value, "{id:?}");
    }
}

/// The expanded names take input only while Use Physical Structure is
/// ticked.
#[test]
fn physical_options_are_disabled_until_use_physical_structure() {
    let (app, _task) = Signex::new();
    let mut on = four_sheets();
    on.pdf_options.use_physical_structure = true;
    let mut off = four_sheets();
    off.pdf_options.use_physical_structure = false;
    let (live, veiled) = click_on_and_off(&app, &on, &off, "Designators");
    let designators = |m: &PrintPreviewMsg| matches!(m, PrintPreviewMsg::SetPhysicalDesignators(_));
    assert!(sent(&live, designators), "{live:?}");
    assert!(!sent(&veiled, designators), "{veiled:?}");
}

/// The watermark fields take input only while "Print a watermark" is on.
#[test]
fn watermark_fields_are_disabled_while_the_watermark_is_off() {
    let (app, _task) = Signex::new();
    let mut on = four_sheets();
    on.dialog.watermark.is_on = true;
    let off = four_sheets();
    assert!(!off.dialog.watermark.is_on, "off by default");
    let (live, veiled) = click_on_and_off(&app, &on, &off, "CONFIDENTIAL");
    let preset = |m: &PrintPreviewMsg| {
        matches!(
            m,
            PrintPreviewMsg::PdfOption(PdfOptionMsg::WatermarkPresetPicked(_))
        )
    };
    assert!(sent(&live, preset), "{live:?}");
    assert!(!sent(&veiled, preset), "{veiled:?}");
}

/// The layer rows take input only while "Write PDF layers" is on; off, the
/// section says the PDF is flat.
#[test]
fn layer_rows_are_disabled_while_pdf_layers_are_off() {
    let (app, _task) = Signex::new();
    let mut on = four_sheets();
    on.dialog.layers.is_on = true;
    let off = four_sheets();
    assert!(!off.dialog.layers.is_on, "off by default");
    let (live, veiled) = click_on_and_off(&app, &on, &off, "Parts");
    let parts = |m: &PrintPreviewMsg| {
        matches!(
            m,
            PrintPreviewMsg::PdfOption(PdfOptionMsg::PdfLayerVisibilityToggled(
                PdfLayer::Parts,
                false
            ))
        )
    };
    assert!(sent(&live, parts), "{live:?}");
    assert!(!sent(&veiled, parts), "{veiled:?}");
    let texts = texts_of(app.view_pdf_settings_column(&off), COLUMN);
    assert!(
        texts
            .iter()
            .any(|t| t == "Off: the PDF is flat; every group prints as drawn."),
        "{texts:#?}"
    );
}

/// The Watermark layer row takes input only while a watermark prints.
#[test]
fn the_watermark_layer_row_is_disabled_while_the_watermark_is_off() {
    let (app, _task) = Signex::new();
    let mut on = four_sheets();
    on.dialog.watermark.is_on = true;
    on.dialog.layers.is_on = true;
    let mut off = four_sheets();
    off.dialog.layers.is_on = true;
    let texts = texts_of(app.view_pdf_settings_column(&on), COLUMN);
    assert!(
        texts.iter().any(|t| t == "locked on in print"),
        "{texts:#?}"
    );
    let (live, veiled) = click_on_and_off(&app, &on, &off, "Watermark");
    let row = |m: &PrintPreviewMsg| {
        matches!(
            m,
            PrintPreviewMsg::PdfOption(PdfOptionMsg::PdfLayerVisibilityToggled(
                PdfLayer::Watermark,
                _
            ))
        )
    };
    assert!(sent(&live, row), "{live:?}");
    assert!(!sent(&veiled, row), "{veiled:?}");
}

/// #2690: Signal harnesses is a layer row of its own, right under Wires and
/// buses, named from the catalog; unticking it hides that group on open.
#[test]
fn signal_harnesses_have_their_own_layer_row() {
    let (app, _task) = Signex::new();
    let mut on = four_sheets();
    on.dialog.layers.is_on = true;
    let texts = texts_of(app.view_pdf_settings_column(&on), COLUMN);
    let wires = texts.iter().position(|t| t == "Wires and buses");
    let harnesses = texts.iter().position(|t| t == "Signal harnesses");
    assert!(
        wires.is_some() && harnesses == wires.map(|at| at + 1),
        "{texts:#?}"
    );
    let off = four_sheets();
    let (live, veiled) = click_on_and_off(&app, &on, &off, "Signal harnesses");
    let row = |m: &PrintPreviewMsg| {
        matches!(
            m,
            PrintPreviewMsg::PdfOption(PdfOptionMsg::PdfLayerVisibilityToggled(
                PdfLayer::SignalHarnesses,
                false
            ))
        )
    };
    assert!(sent(&live, row), "{live:?}");
    assert!(!sent(&veiled, row), "{veiled:?}");
}

/// The part bookmarks' Group switch takes input only while "One bookmark
/// per part" is ticked.
#[test]
fn part_bookmark_grouping_is_disabled_until_part_bookmarks() {
    let (app, _task) = Signex::new();
    let mut on = four_sheets();
    on.pdf_options.bookmark_parts = true;
    let mut off = four_sheets();
    off.pdf_options.bookmark_parts = false;
    let (live, veiled) = click_on_and_off(&app, &on, &off, "A\u{2013}Z");
    let order = |m: &PrintPreviewMsg| {
        matches!(
            m,
            PrintPreviewMsg::PdfOption(PdfOptionMsg::PartBookmarkOrderPicked(_))
        )
    };
    assert!(sent(&live, order), "{live:?}");
    assert!(!sent(&veiled, order), "{veiled:?}");
}

/// "One bookmark per net" is the nets-information setting, shown once.
#[test]
fn one_bookmark_per_net_is_the_nets_information_setting() {
    let (app, _task) = Signex::new();
    let preview = four_sheets();
    let texts = texts_of(app.view_pdf_settings_column(&preview), COLUMN);
    assert!(!texts.iter().any(|t| t == "Generate nets information"));
    let at = centre_of(&app, &preview, "One bookmark per net");
    let clicked = messages_on_click_at(app.view_pdf_settings_column(&preview), COLUMN, None, at);
    assert!(
        sent(&clicked, |m| matches!(
            m,
            PrintPreviewMsg::SetGenerateNetsInfo(_)
        )),
        "{clicked:?}"
    );
}

/// Mockup row 168 and DECISIONS 2026-11-19 (#2606): Variant is its own
/// section between Output and Structure, not a row inside the folded
/// Structure section that mockup row 111 still draws.
#[test]
fn the_sections_run_in_the_decided_order() {
    let (app, _task) = Signex::new();
    let preview = four_sheets();
    let texts = texts_of(app.view_pdf_settings_column(&preview), COLUMN);
    let titles = [
        "SHEETS",
        "PAGES",
        "PAGE SETUP",
        "OUTPUT",
        "VARIANT",
        "STRUCTURE",
        "WATERMARK",
        "PDF LAYERS",
        "BOOKMARKS",
        "BOOKMARKS AND NETS",
        "SCHEMATICS INCLUDE",
    ];
    let at: Vec<usize> = titles
        .iter()
        .map(|title| {
            texts
                .iter()
                .position(|t| t == title)
                .unwrap_or_else(|| panic!("no {title:?} heading: {texts:#?}"))
        })
        .collect();
    assert!(at.windows(2).all(|w| w[0] < w[1]), "{titles:?} at {at:?}");
}

/// Mockup row 168: the Watermark, PDF Layers and Bookmarks headings carry a
/// NEW pill; the sections that were already there do not.
#[test]
fn the_new_sections_carry_a_new_pill() {
    let (app, _task) = Signex::new();
    let window = Size::new(COLUMN.width, 100.0);
    let header =
        |section, title| texts_of(app.pdf_section_header(section, title, None, true), window);
    for (section, title) in [
        (PdfSection::Watermark, ids::PDF_SECTION_WATERMARK),
        (PdfSection::PdfLayers, ids::PDF_SECTION_LAYERS),
        (PdfSection::Bookmarks, ids::PDF_SECTION_BOOKMARKS),
    ] {
        let texts = header(section, title);
        assert!(texts.iter().any(|t| t == "NEW"), "{section:?}: {texts:?}");
    }
    for (section, title) in [
        (PdfSection::Sheets, ids::PDF_SECTION_SHEETS),
        (PdfSection::Variant, ids::PDF_SECTION_VARIANT),
        (
            PdfSection::BookmarksAndNets,
            ids::PDF_SECTION_BOOKMARKS_NETS,
        ),
    ] {
        let texts = header(section, title);
        assert!(!texts.iter().any(|t| t == "NEW"), "{section:?}: {texts:?}");
    }
}

/// A heading of `body` laid out for `off` (its switch off) is a toggle
/// switch: the label sits right of the switch track (a checkbox's label
/// sits right of a body-sized box), and a press on the label or on the
/// track sends `toggled`.
fn assert_heading_switch<'a>(
    off: &'a PreviewState,
    label: &str,
    body: impl Fn(&'a PreviewState) -> iced::Element<'a, Message>,
    toggled: impl Fn(&PrintPreviewMsg) -> bool,
) {
    use signex_widgets::toggle_switch::TRACK_WIDTH;
    let window = Size::new(COLUMN.width, 1000.0);
    let texts = lay_out_texts(body(off), window);
    let bounds = crate::layout_probe::find(&texts, label).bounds;
    assert!(
        bounds.x >= TRACK_WIDTH,
        "{label}: the label starts at {}, inside the switch track",
        bounds.x
    );
    let on_label = messages_on_click_at(body(off), window, None, bounds.center());
    assert!(sent(&on_label, &toggled), "{label}: {on_label:?}");
    let track = Point::new(TRACK_WIDTH / 2.0, bounds.center().y);
    let on_track = messages_on_click_at(body(off), window, None, track);
    assert!(sent(&on_track, &toggled), "{label}: {on_track:?}");
}

/// Mockup row 168: "Print a watermark" and "Write PDF layers" are toggle
/// switches heading their sections.
#[test]
fn the_watermark_and_layers_headings_are_switches() {
    let (app, _task) = Signex::new();
    let off = four_sheets();
    assert!(!off.dialog.watermark.is_on && !off.dialog.layers.is_on);
    assert_heading_switch(
        &off,
        "Print a watermark",
        |p| app.view_pdf_watermark(p),
        |m| {
            matches!(
                m,
                PrintPreviewMsg::PdfOption(PdfOptionMsg::WatermarkToggled(true))
            )
        },
    );
    assert_heading_switch(
        &off,
        "Write PDF layers",
        |p| app.view_pdf_layers(p),
        |m| {
            matches!(
                m,
                PrintPreviewMsg::PdfOption(PdfOptionMsg::PdfLayersToggled(true))
            )
        },
    );
}

/// Mockup row 111: the header names the project (or document) file and its
/// sheet count, in a dialog window's own title bar as in the in-window
/// header.
#[test]
fn the_dialog_window_names_the_file_and_its_sheets() {
    use crate::app::state::ModalId;
    let (mut app, _task) = Signex::new();
    assert_eq!(app.detached_modal_subtitle(ModalId::PrintPreview), None);
    let mut preview = four_sheets();
    preview.source = PathBuf::from("/w/dcdc.snxsch");
    app.document_state.preview = Some(preview);
    let subtitle = app
        .detached_modal_subtitle(ModalId::PrintPreview)
        .map(|s| s.replace(['\u{2068}', '\u{2069}'], ""));
    assert_eq!(subtitle.as_deref(), Some("dcdc.snxsch · 4 sheets"));
}
