//! The documents a placed part offers (#1301): what its library row lists,
//! plus the symbol's own Datasheet field, as links and local files.

use std::path::{Path, PathBuf};

use signex_library::{ComponentRow, DatasheetRef, datasheet_asset_path};

/// Where a document lives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DatasheetLocation {
    Url(String),
    File(PathBuf),
}

/// One document to offer, with the label the library gives it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatasheetTarget {
    pub label: String,
    pub location: DatasheetLocation,
}

/// Where a placed part's library row stands.
pub enum PartSource<'a> {
    /// Placed without a library row.
    NoRow,
    /// Stamped with a row that is not readable: its library is closed or
    /// the row is gone.
    RowUnavailable,
    Row {
        row: &'a ComponentRow,
        /// The `.snxlib` file of the row's library.
        library_file: &'a Path,
    },
}

/// The documents to offer for `reference`, the row's primary first, or the
/// advice line to log when there is nothing to open.
pub fn datasheet_targets(
    reference: &str,
    source: PartSource<'_>,
    own_field: &str,
) -> Result<Vec<DatasheetTarget>, String> {
    let mut targets = Vec::new();
    let mut bad_link = false;
    let mut row_listed_none = false;
    match &source {
        PartSource::Row { row, library_file } => {
            for doc in row.documents().into_iter().filter(|d| !d.source.is_empty()) {
                let label = match doc.label.trim() {
                    "" => DEFAULT_LABEL.to_string(),
                    label => label.to_string(),
                };
                let location = match &doc.source {
                    DatasheetRef::Url { url } => web_link(url).map(DatasheetLocation::Url),
                    DatasheetRef::HashPinned { hash, .. } => {
                        datasheet_asset_path(library_file, hash).map(DatasheetLocation::File)
                    }
                };
                match location {
                    Some(location) => targets.push(DatasheetTarget { label, location }),
                    None => bad_link = true,
                }
            }
            row_listed_none = targets.is_empty() && !bad_link;
        }
        PartSource::NoRow | PartSource::RowUnavailable => {}
    }
    if let Some(target) = own_field_target(own_field, &targets) {
        targets.push(target);
    }
    if !targets.is_empty() {
        return Ok(targets);
    }
    Err(if bad_link {
        format!("Fix the datasheet link of {reference}: it must start with https:// or http://.")
    } else if row_listed_none {
        format!("Add a datasheet to the library row of {reference} to open it: the row lists none.")
    } else if matches!(source, PartSource::RowUnavailable) {
        format!("Open the library of {reference} to read its datasheet: its row is not available.")
    } else {
        format!(
            "Place {reference} from a library part to open its datasheet: it has no library row."
        )
    })
}

const DEFAULT_LABEL: &str = "Datasheet";

/// The symbol's own Datasheet field as a document, unless it is empty, the
/// `~` placeholder, already one of `listed`, or neither a web link nor an
/// absolute path.
fn own_field_target(field: &str, listed: &[DatasheetTarget]) -> Option<DatasheetTarget> {
    let field = field.trim();
    if field.is_empty() || field == "~" {
        return None;
    }
    let location = match web_link(field) {
        Some(link) => DatasheetLocation::Url(link),
        None if Path::new(field).is_absolute() => DatasheetLocation::File(PathBuf::from(field)),
        None => return None,
    };
    if listed.iter().any(|t| t.location == location) {
        return None;
    }
    Some(DatasheetTarget {
        label: DEFAULT_LABEL.to_string(),
        location,
    })
}

/// `link` trimmed, when it is an `http` or `https` address: the one kind of
/// link handed to the browser.
fn web_link(link: &str) -> Option<String> {
    let link = link.trim();
    let lower = link.to_ascii_lowercase();
    (lower.starts_with("https://") || lower.starts_with("http://")).then(|| link.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_library::{DatasheetDoc, DatasheetRef};

    fn row(docs: Vec<DatasheetDoc>) -> ComponentRow {
        let mut row = crate::library::component_preview::test_row();
        row.set_documents(docs);
        row
    }

    fn lib() -> PathBuf {
        PathBuf::from("/libs/main/main.snxlib")
    }

    fn plan(row: &ComponentRow, own: &str) -> Result<Vec<DatasheetTarget>, String> {
        let file = lib();
        datasheet_targets(
            "R1",
            PartSource::Row {
                row,
                library_file: &file,
            },
            own,
        )
    }

    fn url(label: &str, link: &str) -> DatasheetTarget {
        DatasheetTarget {
            label: label.into(),
            location: DatasheetLocation::Url(link.into()),
        }
    }

    #[test]
    fn a_row_with_one_link_offers_it() {
        let r = row(vec![DatasheetDoc::new(
            "",
            DatasheetRef::url(" https://ti.example/u1.pdf "),
        )]);
        assert_eq!(
            plan(&r, ""),
            Ok(vec![url("Datasheet", "https://ti.example/u1.pdf")])
        );
    }

    #[test]
    fn every_document_is_offered_primary_first() {
        let hash = "a".repeat(64);
        let r = row(vec![
            DatasheetDoc::new("Datasheet", DatasheetRef::url("https://a.example/d.pdf")),
            DatasheetDoc::new(
                "Errata",
                DatasheetRef::hash_pinned(hash.clone(), "errata.pdf"),
            ),
        ]);
        let targets = plan(&r, "").unwrap();
        assert_eq!(targets.len(), 2);
        assert_eq!(targets[0], url("Datasheet", "https://a.example/d.pdf"));
        assert_eq!(targets[1].label, "Errata");
        assert_eq!(
            targets[1].location,
            DatasheetLocation::File(
                PathBuf::from("/libs/main").join(format!("assets/datasheets/{hash}.pdf"))
            )
        );
    }

    #[test]
    fn empty_slots_are_skipped() {
        let r = row(vec![
            DatasheetDoc::new("", DatasheetRef::default()),
            DatasheetDoc::new("Spec", DatasheetRef::url("http://a.example/s.pdf")),
        ]);
        assert_eq!(
            plan(&r, ""),
            Ok(vec![url("Spec", "http://a.example/s.pdf")])
        );
    }

    #[test]
    fn a_row_without_a_datasheet_says_how_to_add_one() {
        let r = row(vec![]);
        let advice = plan(&r, "").unwrap_err();
        assert!(
            advice.starts_with("Add a datasheet to the library row of R1"),
            "{advice}"
        );
        assert!(advice.len() <= 157, "{advice}");
    }

    #[test]
    fn a_part_without_a_row_says_to_link_one() {
        let advice = datasheet_targets("R1", PartSource::NoRow, "").unwrap_err();
        assert!(
            advice.starts_with("Place R1 from a library part"),
            "{advice}"
        );
        assert!(advice.len() <= 157, "{advice}");
    }

    #[test]
    fn a_row_that_cannot_be_read_says_to_open_its_library() {
        let advice = datasheet_targets("R1", PartSource::RowUnavailable, "").unwrap_err();
        assert!(advice.starts_with("Open the library of R1"), "{advice}");
        assert!(advice.len() <= 157, "{advice}");
    }

    #[test]
    fn the_symbols_own_datasheet_field_is_a_fallback_without_a_row() {
        assert_eq!(
            datasheet_targets("R1", PartSource::NoRow, " https://x.example/r.pdf "),
            Ok(vec![url("Datasheet", "https://x.example/r.pdf")])
        );
    }

    #[test]
    fn a_field_equal_to_a_row_document_is_not_offered_twice() {
        let r = row(vec![DatasheetDoc::new(
            "",
            DatasheetRef::url("https://a.example/d.pdf"),
        )]);
        assert_eq!(plan(&r, "https://a.example/d.pdf").unwrap().len(), 1);
    }

    #[test]
    fn only_web_links_and_absolute_files_are_opened() {
        let r = row(vec![DatasheetDoc::new(
            "",
            DatasheetRef::url("javascript:alert(1)"),
        )]);
        let advice = plan(&r, "").unwrap_err();
        assert!(advice.contains("https://"), "{advice}");
        assert!(datasheet_targets("R1", PartSource::NoRow, "relative/d.pdf").is_err());
    }

    #[test]
    fn an_absolute_field_path_is_a_file_target() {
        let t = datasheet_targets("R1", PartSource::NoRow, "/docs/r1.pdf").unwrap();
        assert_eq!(
            t[0].location,
            DatasheetLocation::File(PathBuf::from("/docs/r1.pdf"))
        );
    }
}
