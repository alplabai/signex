//! What the table lists and the ticked bar offers (Graphite #80): the All /
//! Changed / No alternates chips, the filter field, the ticked bar's shared
//! footprints, and the reason pill of a part that cannot change. Pure data.

use signex_i18n::{MessageId, ids};

use super::state::{AssignFootprintsState, FootprintOffer, PartAccess};

/// The filter chip that is on.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AssignFilter {
    #[default]
    All,
    /// Parts Apply changes.
    Changed,
    /// Parts whose row offers one footprint only.
    NoAlternates,
}

impl AssignFilter {
    /// Every chip, in the order the bar shows them.
    pub const ALL: [Self; 3] = [Self::All, Self::Changed, Self::NoAlternates];

    /// The chip's label.
    pub(crate) fn label(self) -> MessageId {
        match self {
            Self::All => ids::DIALOG_ASSIGN_FOOTPRINTS_CHIP_ALL,
            Self::Changed => ids::DIALOG_ASSIGN_FOOTPRINTS_CHIP_CHANGED,
            Self::NoAlternates => ids::DIALOG_ASSIGN_FOOTPRINTS_CHIP_NO_ALTERNATES,
        }
    }
}

/// The bar shown while parts are ticked: how many, and the footprints every
/// ticked part's row offers (empty when they share none).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TickedBar {
    pub count: usize,
    pub offers: Vec<FootprintOffer>,
}

impl PartAccess {
    /// The reason pill of a part that cannot change here; `None` for an
    /// editable part.
    pub fn reason(self) -> Option<MessageId> {
        Some(match self {
            Self::Editable => return None,
            Self::SheetClosed => ids::DIALOG_ASSIGN_FOOTPRINTS_SHEET_CLOSED,
            Self::ReadOnly => ids::DIALOG_ASSIGN_FOOTPRINTS_READ_ONLY,
            Self::RowNotOpen => ids::DIALOG_ASSIGN_FOOTPRINTS_ROW_NOT_OPEN,
            Self::NoLink => ids::DIALOG_ASSIGN_FOOTPRINTS_NO_LINK,
            Self::NoAlternates => ids::DIALOG_ASSIGN_FOOTPRINTS_NO_ALTERNATES,
        })
    }
}

impl AssignFootprintsState {
    /// Whether row `index` changes on Apply.
    pub fn is_changed(&self, index: usize) -> bool {
        self.picks.get(index).is_some_and(Option::is_some)
    }

    /// Whether row `index` belongs under chip `filter`.
    fn in_chip(&self, index: usize, filter: AssignFilter) -> bool {
        match filter {
            AssignFilter::All => true,
            AssignFilter::Changed => self.is_changed(index),
            AssignFilter::NoAlternates => self
                .parts()
                .get(index)
                .is_some_and(|p| p.access == PartAccess::NoAlternates),
        }
    }

    /// How many parts chip `filter` lists, before the filter field.
    pub fn filter_count(&self, filter: AssignFilter) -> usize {
        (0..self.parts().len())
            .filter(|&index| self.in_chip(index, filter))
            .count()
    }

    /// Whether row `index` matches the filter field: its designator, value
    /// or library row holds the text, ignoring case.
    fn matches_query(&self, index: usize) -> bool {
        let query = self.query.trim().to_lowercase();
        if query.is_empty() {
            return true;
        }
        let Some(part) = self.parts().get(index) else {
            return false;
        };
        [
            part.designator.as_str(),
            part.value.as_str(),
            part.row_label.as_deref().unwrap_or(""),
        ]
        .iter()
        .any(|field| field.to_lowercase().contains(&query))
    }

    /// The rows the table lists: the chip that is on and the filter field.
    pub fn visible_rows(&self) -> Vec<usize> {
        (0..self.parts().len())
            .filter(|&index| self.in_chip(index, self.filter) && self.matches_query(index))
            .collect()
    }

    /// The ticked bar; `None` with nothing ticked.
    pub fn ticked_bar(&self) -> Option<TickedBar> {
        (!self.selected.is_empty()).then(|| TickedBar {
            count: self.selected.len(),
            offers: self.shared_offers(),
        })
    }
}
