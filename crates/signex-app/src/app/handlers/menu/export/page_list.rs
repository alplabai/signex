//! The text of the Export PDF Pages box: sheet numbers and ranges such as
//! `1-2, 4` (mockup row 111).

use std::collections::BTreeSet;

/// The most pages one list can name, so a typed `1-999999999` is refused
/// instead of being expanded on the UI thread.
const MAX_LISTED_PAGES: usize = 10_000;

/// The page numbers `text` names, in the order typed and each once. `None`
/// when any part of it does not read: an empty item, a number below 1, a
/// range that runs backwards or a list over [`MAX_LISTED_PAGES`].
pub(crate) fn parse_page_list(text: &str) -> Option<Vec<usize>> {
    let mut pages = Vec::new();
    let mut seen = BTreeSet::new();
    for item in text.split(',') {
        let (first, last) = page_span(item)?;
        if last - first >= MAX_LISTED_PAGES {
            return None;
        }
        for page in first..=last {
            if seen.insert(page) {
                pages.push(page);
            }
        }
        if pages.len() > MAX_LISTED_PAGES {
            return None;
        }
    }
    Some(pages)
}

/// The first and last page of one list item, `4` or `1-2`.
fn page_span(item: &str) -> Option<(usize, usize)> {
    match item.split_once('-') {
        None => {
            let page = page_number(item)?;
            Some((page, page))
        }
        Some((first, last)) => {
            let (first, last) = (page_number(first)?, page_number(last)?);
            (first <= last).then_some((first, last))
        }
    }
}

/// A page number: ASCII digits only (`parse` alone takes `+1`), 1 or more.
fn page_number(text: &str) -> Option<usize> {
    let text = text.trim();
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse::<usize>().ok().filter(|page| *page > 0)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use signex_output::PageRange;
    use signex_types::schematic::SchematicSheet;

    use super::{MAX_LISTED_PAGES, parse_page_list};
    use crate::app::Signex;

    #[test]
    fn a_list_takes_numbers_and_ranges_in_the_order_typed() {
        assert_eq!(parse_page_list("2"), Some(vec![2]));
        assert_eq!(parse_page_list("1-2, 4"), Some(vec![1, 2, 4]));
        assert_eq!(parse_page_list(" 4 , 1 - 2 "), Some(vec![4, 1, 2]));
        assert_eq!(parse_page_list("3-3"), Some(vec![3]));
    }

    #[test]
    fn a_page_named_twice_prints_once() {
        assert_eq!(parse_page_list("1-3, 2, 1"), Some(vec![1, 2, 3]));
    }

    #[test]
    fn text_that_does_not_read_is_refused() {
        for bad in [
            "",
            " ",
            "abc",
            "0",
            "-1",
            "1-",
            "-",
            "1,",
            ",1",
            "1,,2",
            "3-1",
            "0-2",
            "1-2-3",
            "+1",
            "1.5",
            "99999999999999999999999",
        ] {
            assert_eq!(parse_page_list(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn a_list_too_long_to_print_is_refused_without_expanding_it() {
        assert_eq!(parse_page_list("1-999999999"), None);
        let full = format!("1-{MAX_LISTED_PAGES}");
        assert_eq!(
            parse_page_list(&full).map(|pages| pages.len()),
            Some(MAX_LISTED_PAGES)
        );
        assert_eq!(parse_page_list(&format!("{full}, 20000")), None);
    }

    /// An app with one sheet and the Export PDF dialog open on Specific.
    fn app_on_specific() -> Signex {
        let path = PathBuf::from("page-list.snxsch");
        let sheet = SchematicSheet {
            uuid: uuid::Uuid::now_v7(),
            paper_size: "A4".to_string(),
            ..Default::default()
        };
        let engine = signex_engine::Engine::new(sheet).expect("engine");
        let (mut app, _boot) = Signex::new();
        app.document_state.engines.insert(path.clone(), engine);
        app.document_state.tabs.push(crate::app::TabInfo {
            title: "page-list".to_string(),
            path: path.clone(),
            project_id: None,
            kind: crate::app::TabKind::Schematic,
        });
        app.document_state.active_tab = 0;
        app.document_state.active_path = Some(path);
        let _ = app.handle_print_preview_requested();
        let _ = app.handle_print_preview_set_page_range_specific();
        app
    }

    /// The pages of a Specific range; `None` for any other.
    fn specific(app: &Signex) -> Option<Vec<usize>> {
        let preview = app.document_view.preview.as_ref().expect("preview");
        match &preview.pdf_options.page_range {
            PageRange::Specific(pages) => Some(pages.clone()),
            _ => None,
        }
    }

    #[test]
    fn going_back_to_specific_takes_the_whole_list_in_the_box() {
        let mut app = app_on_specific();
        let _ = app.handle_print_preview_set_specific_page_input("1-2, 4".to_string());
        let _ = app.handle_print_preview_set_page_range_all();
        let _ = app.handle_print_preview_set_page_range_specific();
        assert_eq!(specific(&app), Some(vec![1, 2, 4]));
    }

    #[test]
    fn bad_text_stays_in_the_box_keeps_the_range_and_export_is_refused() {
        let mut app = app_on_specific();
        let _ = app.handle_print_preview_set_specific_page_input("1".to_string());
        let _ = app.handle_print_preview_set_specific_page_input("1-".to_string());
        let preview = app.document_view.preview.as_ref().expect("preview");
        assert_eq!(preview.specific_page_input, "1-");
        assert_eq!(specific(&app), Some(vec![1]));

        assert!(app.document_view.error_notice.is_none());
        let _ = app.handle_print_preview_export();
        assert!(app.document_view.error_notice.is_some(), "refused");
    }
}
