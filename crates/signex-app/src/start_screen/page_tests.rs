//! The Start screen's pages (mock m1), its time column and the project
//! hint (m2), on the fixture of `tests.rs`.

use std::path::PathBuf;

use chrono::{TimeZone, Utc};

use super::tests::{BASE, Fixture, MOCK_WINDOW, paths, plain, sent};
use super::*;
use crate::layout_probe::{find, lay_out_texts};
use crate::library::state::ComponentsMountSource;

/// m1: the Projects and Libraries rows and the Libraries card open their
/// pages; the page on screen is lit and its row sends nothing; each page
/// draws its own title and line.
#[test]
fn the_page_rows_and_the_libraries_card_switch_pages() {
    let home = Fixture::new(&[]);
    let projects = home.click(MOCK_WINDOW, "Projects");
    assert!(sent(
        &projects,
        &StartScreenMsg::PageSelected(StartPage::Projects)
    ));
    let libraries = home.click(MOCK_WINDOW, "Libraries");
    assert!(sent(
        &libraries,
        &StartScreenMsg::PageSelected(StartPage::Libraries)
    ));
    // The card's summary line sits on the card, not on the nav row.
    let summary = home.says(
        signex_i18n::ids::START_LIBRARIES_SUMMARY,
        &[("libraries", 0usize.into()), ("components", 0usize.into())],
    );
    let card = home.click(MOCK_WINDOW, &summary);
    assert!(sent(
        &card,
        &StartScreenMsg::PageSelected(StartPage::Libraries)
    ));
    assert!(
        home.click(MOCK_WINDOW, "Home").is_empty(),
        "Home is on screen"
    );

    let fx = Fixture::new(&[]).on(StartPage::Projects);
    let texts = fx.texts(MOCK_WINDOW);
    let body = fx.says(signex_i18n::ids::START_PROJECTS_BODY, &[]);
    assert!(texts.iter().any(|t| t.text == body));
    assert!(texts.iter().all(|t| t.text != "What are you building?"));
    let back = fx.click(MOCK_WINDOW, "Home");
    assert!(sent(&back, &StartScreenMsg::PageSelected(StartPage::Home)));

    let fx = Fixture::new(&[]).on(StartPage::Libraries);
    let texts = fx.texts(MOCK_WINDOW);
    let body = fx.says(signex_i18n::ids::START_LIBRARIES_BODY, &[]);
    assert!(texts.iter().any(|t| t.text == body));
}

/// m1: Home lists at most five recent projects; "Show all N ›" shows only
/// when there are more, names how many the Projects page lists and opens it.
#[test]
fn show_all_appears_only_when_recent_has_more_than_five() {
    let show_all =
        |fx: &Fixture, n: usize| fx.says(signex_i18n::ids::START_SHOW_ALL, &[("count", n.into())]);
    let five = Fixture::new(&paths(MAX_START_RECENT));
    let texts = five.texts(MOCK_WINDOW);
    assert!(
        texts
            .iter()
            .all(|t| !plain(&t.text).starts_with("Show all")),
        "{texts:#?}"
    );

    let mut seven = Fixture::new(&paths(MAX_START_RECENT + 2));
    seven.pinned.pin(&PathBuf::from("pinned.snxprj"));
    let label = show_all(&seven, MAX_START_RECENT + 3);
    let texts = seven.texts(MOCK_WINDOW);
    assert!(texts.iter().any(|t| t.text == label), "{texts:#?}");
    let header = find(&texts, "RECENT PROJECTS").bounds;
    let link = find(&texts, &label).bounds;
    assert!((link.center_y() - header.center_y()).abs() < BASE);
    assert!(link.x > header.x + header.width);
    let names = texts
        .iter()
        .filter(|t| t.text.starts_with("proj"))
        .map(|t| t.text.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(names.len(), MAX_START_RECENT, "Home lists five: {names:?}");
    let clicked = seven.click(MOCK_WINDOW, &label);
    assert!(sent(
        &clicked,
        &StartScreenMsg::PageSelected(StartPage::Projects)
    ));
}

/// m1: the Projects page lists pinned projects first, then every recent
/// one; the filter matches name or folder, case-insensitively; the switch
/// picks All, Pinned or Missing.
#[test]
fn the_projects_page_filters_by_text_and_scope() {
    let i18n = I18n::en_us();
    let recent = [
        PathBuf::from("/work/hw/dcdc/dcdc.snxprj"),
        PathBuf::from("/work/hw/Motor-Ctrl/motor.snxprj"),
        PathBuf::from("/work/hw/carrier/carrier.snxprj"),
    ];
    let mut pinned = PinnedProjects::default();
    pinned.pin(&recent[2]);
    pinned.pin(&PathBuf::from("/archive/gone.snxprj"));
    pinned.set_missing(&PathBuf::from("/archive/gone.snxprj"), true);
    let listed = |filter: &str, scope| -> Vec<PathBuf> {
        project_listing(&i18n, &recent, &pinned, filter, scope)
            .into_iter()
            .map(|p| p.path)
            .collect()
    };
    assert_eq!(
        listed("", ProjectScope::All),
        [
            recent[2].clone(),
            PathBuf::from("/archive/gone.snxprj"),
            recent[0].clone(),
            recent[1].clone(),
        ]
    );
    assert_eq!(listed("motor-ctrl", ProjectScope::All), [recent[1].clone()]);
    assert_eq!(listed("  DCDC ", ProjectScope::All), [recent[0].clone()]);
    assert_eq!(
        listed("", ProjectScope::Pinned),
        [recent[2].clone(), PathBuf::from("/archive/gone.snxprj")]
    );
    assert_eq!(
        listed("", ProjectScope::Missing),
        [PathBuf::from("/archive/gone.snxprj")]
    );
    assert!(listed("dcdc", ProjectScope::Pinned).is_empty());
    let counts = projects_page::scope_counts(&recent, &pinned);
    assert_eq!((counts.all, counts.pinned, counts.missing), (4, 2, 1));
}

/// m1: the Projects page draws its switch with the counts, lists every
/// project (no cap of five), and a filter matching nothing shows the empty
/// state whose Clear filter clears it.
#[test]
fn the_projects_page_lists_everything_and_says_when_nothing_matches() {
    let mut fx = Fixture::new(&paths(MAX_START_RECENT + 2)).on(StartPage::Projects);
    let texts = fx.texts(MOCK_WINDOW);
    let all = fx.says(
        signex_i18n::ids::START_SCOPE_ALL,
        &[("count", (MAX_START_RECENT + 2).into())],
    );
    assert!(texts.iter().any(|t| t.text == all), "{texts:#?}");
    let names = texts
        .iter()
        .filter(|t| t.text.starts_with("proj"))
        .map(|t| t.text.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(names.len(), MAX_START_RECENT + 2);
    let missing = fx.says(
        signex_i18n::ids::START_SCOPE_MISSING,
        &[("count", 0usize.into())],
    );
    let picked = fx.click(MOCK_WINDOW, &missing);
    assert!(sent(
        &picked,
        &StartScreenMsg::ProjectScopePicked(ProjectScope::Missing)
    ));

    fx.state.project_filter = "zzz".to_owned();
    let texts = fx.texts(MOCK_WINDOW);
    let none = fx.says(
        signex_i18n::ids::START_PROJECTS_NO_MATCH,
        &[("query", "zzz".into())],
    );
    assert!(texts.iter().any(|t| t.text == none), "{texts:#?}");
    assert!(texts.iter().all(|t| !t.text.starts_with("proj")));
    let cleared = fx.click(MOCK_WINDOW, "Clear filter");
    assert!(sent(&cleared, &StartScreenMsg::FilterCleared));

    // An empty switch position without a filter says what it would list.
    fx.state.project_filter.clear();
    fx.state.project_scope = ProjectScope::Missing;
    let texts = fx.texts(MOCK_WINDOW);
    assert!(texts.iter().any(|t| t.text == "No missing projects"));
    assert!(texts.iter().all(|t| t.text != "Clear filter"));
}

/// m1, GG-10: the Libraries page lists Stock with its read-only caption and
/// every library's scope and count; Open opens that library's browser, the
/// toolbar adds a global library or opens the browser; a filter matching
/// nothing says so.
#[test]
fn the_libraries_page_lists_the_loaded_libraries() {
    let mut fx = Fixture::new(&[]).on(StartPage::Libraries);
    fx.libraries = vec![
        libraries_page::sample("Stock", ComponentsMountSource::Stock, 34),
        libraries_page::sample("alp-passives", ComponentsMountSource::Global, 1108),
    ];
    let texts = fx.texts(MOCK_WINDOW);
    for label in [
        "LIBRARY",
        "SCOPE",
        "COMPONENTS",
        "Stock",
        "Global",
        "ships with Signex · read-only",
        "alp-passives.snxlib",
    ] {
        assert!(
            texts.iter().any(|t| plain(&t.text) == label),
            "{label}: {texts:#?}"
        );
    }
    assert!(texts.iter().any(|t| plain(&t.text) == "1,108"));
    let open = fx.click(MOCK_WINDOW, "Open");
    assert!(
        open.iter().any(|m| matches!(
            m,
            Message::Library(crate::library::messages::LibraryMessage::OpenLibraryBrowser(p))
                if p.as_path() == std::path::Path::new("Stock.snxlib")
        )),
        "Stock opens view-only: {open:?}"
    );
    let add = fx.click(MOCK_WINDOW, "Add library\u{2026}");
    assert!(add.iter().any(|m| matches!(
        m,
        Message::Library(
            crate::library::messages::LibraryMessage::ComponentsPanelAddLibrary(
                ComponentsMountSource::Global
            )
        )
    )));
    let browser = fx.click(MOCK_WINDOW, "Open Library Browser");
    assert!(browser.iter().any(|m| matches!(
        m,
        Message::Menu(MenuMessage::Command(id)) if id.as_str() == "open_library_browser"
    )));

    fx.state.library_filter = "PASSIVE".to_owned();
    let texts = fx.texts(MOCK_WINDOW);
    assert!(texts.iter().any(|t| t.text == "alp-passives"));
    assert!(
        texts
            .iter()
            .all(|t| t.text != "ships with Signex · read-only")
    );
    fx.state.library_filter = "nothing".to_owned();
    let texts = fx.texts(MOCK_WINDOW);
    let none = fx.says(
        signex_i18n::ids::START_LIBRARIES_NO_MATCH,
        &[("query", "nothing".into())],
    );
    assert!(texts.iter().any(|t| t.text == none), "{texts:#?}");
    assert!(sent(
        &fx.click(MOCK_WINDOW, "Clear filter"),
        &StartScreenMsg::FilterCleared
    ));
}

/// m1: the last-opened column reads "today HH:MM", "yesterday", an
/// earlier day in the locale's date, and "—" when the time is unknown.
#[test]
fn the_last_opened_time_reads_today_yesterday_a_date_or_a_dash() {
    let i18n = I18n::en_us();
    let now = Utc.with_ymd_and_hms(2026, 11, 19, 15, 30, 0).unwrap();
    let shown = |at: Option<chrono::DateTime<Utc>>| {
        let label = projects::opened_label_in(&i18n, at.map(|t| t.timestamp()), now, &Utc);
        plain(&label.into_string())
    };
    let today = Utc.with_ymd_and_hms(2026, 11, 19, 9, 12, 0).unwrap();
    assert_eq!(shown(Some(today)), "today 09:12");
    let yesterday = Utc.with_ymd_and_hms(2026, 11, 18, 23, 59, 0).unwrap();
    assert_eq!(shown(Some(yesterday)), "yesterday");
    let earlier = Utc.with_ymd_and_hms(2026, 11, 14, 8, 0, 0).unwrap();
    assert_eq!(
        shown(Some(earlier)),
        plain(&i18n.date(earlier, signex_i18n::DateLength::Medium))
    );
    assert_eq!(shown(None), "—");

    // The row shows it in its own column, right of the path.
    let recent = paths(1);
    let mut fx = Fixture::new(&recent);
    fx.recent
        .restore_opened([(recent[0].clone(), Utc::now().timestamp())]);
    let texts = fx.texts(MOCK_WINDOW);
    let when = texts
        .iter()
        .find(|t| plain(&t.text).starts_with("today "))
        .expect("the time column");
    let path = texts
        .iter()
        .filter(|t| t.text == "proj0.snxprj")
        .nth(1)
        .expect("the path line")
        .bounds;
    assert!(when.bounds.x >= path.x + path.width - 0.5);
}

/// m2: the hint names the state (a project with documents, or an empty
/// one) and offers New sheet, New PCB and Project Options… through the
/// command catalog.
#[test]
fn the_project_hint_says_when_the_project_is_empty_and_offers_its_commands() {
    let tokens = signex_types::theme::theme_tokens(signex_types::theme::ThemeId::GraphiteLight);
    let i18n = I18n::en_us();
    let window = iced::Size::new(900.0, 500.0);
    let has = HintFacts {
        has_documents: true,
    };
    let empty = HintFacts {
        has_documents: false,
    };
    let texts = lay_out_texts(hint_view(&tokens, &i18n, has), window);
    assert!(texts.iter().any(|t| t.text == "No document selected"));
    let empty_texts = lay_out_texts(hint_view(&tokens, &i18n, empty), window);
    assert!(
        empty_texts
            .iter()
            .any(|t| t.text == "This project has no documents yet")
    );
    assert!(empty_texts.iter().all(|t| t.text != "No document selected"));
    for (label, command) in [
        ("New sheet", "new_schematic_sheet"),
        ("New PCB", "new_pcb_document"),
        ("Project Options\u{2026}", "open_project_options"),
    ] {
        let sent =
            crate::layout_probe::messages_on_click(hint_view(&tokens, &i18n, empty), window, label);
        assert!(
            sent.iter().any(|m| matches!(
                m,
                Message::Menu(MenuMessage::Command(id)) if id.as_str() == command
            )),
            "{label}: {sent:?}"
        );
    }
    // The column is never wider than the window minus its edges.
    let narrow = iced::Size::new(300.0, 400.0);
    let texts = lay_out_texts(hint_view(&tokens, &i18n, has), narrow);
    for t in &texts {
        assert!(
            t.bounds.x >= hint::EDGE - 0.5
                && t.bounds.x + t.bounds.width <= narrow.width - hint::EDGE + 0.5,
            "{t:?} leaves the column"
        );
    }
}

/// The time column reads the clock from the Model, never in `view`: boot
/// stamps it, the minute tick moves it, it is dropped while a document is
/// on screen and stamped again when the screen comes back.
#[test]
fn the_start_screen_clock_lives_in_the_model() {
    let (mut app, _boot) = crate::app::Signex::new();
    assert!(
        app.ui_state.start_screen.now.is_some(),
        "boot stamps the clock"
    );
    let tick = Utc.with_ymd_and_hms(2026, 11, 19, 15, 31, 0).unwrap();
    let _ = app.update(message(StartScreenMsg::ClockTicked(tick)));
    assert_eq!(app.ui_state.start_screen.now, Some(tick));

    app.document_state.tabs.push(crate::app::TabInfo {
        title: "a.snxsch".to_string(),
        path: PathBuf::from("a.snxsch"),
        cached_document: None,
        project_id: None,
        kind: crate::app::TabKind::Schematic,
    });
    app.drop_start_row_menu_off_screen();
    assert_eq!(app.ui_state.start_screen.now, None, "off screen: no clock");

    app.document_state.tabs.clear();
    app.drop_start_row_menu_off_screen();
    assert!(
        app.ui_state.start_screen.now.is_some(),
        "back on screen: stamped again"
    );
}
