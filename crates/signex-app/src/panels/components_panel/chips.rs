//! Source chips of the Components panel: All, then each source that holds
//! parts, each with the count of parts it shows (#2535). The library count
//! is a caption at the right of the chip row.

use iced::Element;
use signex_i18n::{I18n, Localized, ids, t};
use signex_types::theme::ThemeTokens;

use super::look;
use crate::library::messages::LibraryMessage;
use crate::library::state::{ComponentsMountSource, OpenLibrary, PanelRows};

/// Part counts per source, for the chips.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SourceCounts {
    /// Parts the list shows across every source (filter applied).
    pub all: usize,
    /// Parts each source shows (filter applied), in `ORDER` slots.
    shown: [usize; 4],
    /// Parts each source holds, filter or not.
    held: [usize; 4],
    /// Libraries each source mounts.
    libraries: [usize; 4],
}

impl SourceCounts {
    pub(super) fn of(
        libraries: &[(ComponentsMountSource, &OpenLibrary)],
        rows: &PanelRows,
    ) -> Self {
        let mut counts = Self {
            all: 0,
            shown: [0; 4],
            held: [0; 4],
            libraries: [0; 4],
        };
        for (source, lib) in libraries {
            let slot = Self::slot(*source);
            let shown = rows.visible_count(&lib.root);
            counts.shown[slot] += shown;
            counts.held[slot] += lib.total_rows();
            counts.libraries[slot] += 1;
            counts.all += shown;
        }
        counts
    }

    fn slot(source: ComponentsMountSource) -> usize {
        ComponentsMountSource::ORDER
            .iter()
            .position(|s| *s == source)
            .unwrap_or(0)
    }

    /// Parts `source` shows.
    pub(super) fn of_source(&self, source: ComponentsMountSource) -> usize {
        self.shown[Self::slot(source)]
    }

    /// Whether `source` holds any part.
    fn holds_parts(&self, source: ComponentsMountSource) -> bool {
        self.held[Self::slot(source)] > 0
    }

    /// The chip choice that applies: a source with no part left falls back
    /// to All.
    pub(super) fn effective(
        &self,
        chosen: Option<ComponentsMountSource>,
    ) -> Option<ComponentsMountSource> {
        chosen.filter(|s| self.holds_parts(*s))
    }

    /// Chips to draw: All always, then each source that holds parts.
    pub(super) fn chips(&self) -> Vec<Option<ComponentsMountSource>> {
        std::iter::once(None)
            .chain(
                ComponentsMountSource::ORDER
                    .iter()
                    .copied()
                    .filter(|s| self.holds_parts(*s))
                    .map(Some),
            )
            .collect()
    }

    /// Libraries the list shows under the chip `selected`.
    pub(super) fn libraries(&self, selected: Option<ComponentsMountSource>) -> usize {
        match selected {
            Some(source) => self.libraries[Self::slot(source)],
            None => self.libraries.iter().sum(),
        }
    }
}

/// The chip row: the chips, then "N libraries" at the right.
pub(super) fn chip_row<'a>(
    counts: &SourceCounts,
    selected: Option<ComponentsMountSource>,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let chips = counts
        .chips()
        .into_iter()
        .map(|chip| {
            let (label, count) = match chip {
                None => (ids::PANELS_COMPONENTS_SOURCE_ALL, counts.all),
                Some(source) => (source.message(), counts.of_source(source)),
            };
            look::source_chip(
                t!(i18n, label),
                count,
                chip == selected,
                LibraryMessage::ComponentsPanelSourceFilterChanged(chip),
                tokens,
                i18n,
            )
        })
        .collect();
    look::chip_row(
        chips,
        library_caption(counts.libraries(selected), i18n),
        tokens,
    )
}

/// "1 library" / "N libraries".
fn library_caption(count: usize, i18n: &I18n) -> Localized {
    t!(i18n, ids::PANELS_COMPONENTS_LIBRARY_COUNT, "count" => count)
}
