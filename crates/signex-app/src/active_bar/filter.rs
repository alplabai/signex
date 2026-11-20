//! The schematic selection-filter categories.

/// Selection filter categories — each can be independently toggled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SelectionFilter {
    Components,
    Wires,
    Buses,
    SheetSymbols,
    SheetEntries,
    NetLabels,
    Parameters,
    Ports,
    PowerPorts,
    Texts,
    DrawingObjects,
    Other,
}

impl SelectionFilter {
    pub const ALL: &'static [SelectionFilter] = &[
        Self::Components,
        Self::Wires,
        Self::Buses,
        Self::SheetSymbols,
        Self::SheetEntries,
        Self::NetLabels,
        Self::Parameters,
        Self::Ports,
        Self::PowerPorts,
        Self::Texts,
        Self::DrawingObjects,
        Self::Other,
    ];

    /// The catalog message of [`Self::label`].
    pub fn message(self) -> signex_i18n::MessageId {
        use signex_i18n::ids;
        match self {
            Self::Components => ids::PANELS_FILTER_COMPONENTS,
            Self::Wires => ids::PANELS_FILTER_WIRES,
            Self::Buses => ids::PANELS_FILTER_BUSES,
            Self::SheetSymbols => ids::PANELS_FILTER_SHEET_SYMBOLS,
            Self::SheetEntries => ids::PANELS_FILTER_SHEET_ENTRIES,
            Self::NetLabels => ids::PANELS_FILTER_NET_LABELS,
            Self::Parameters => ids::PANELS_FILTER_PARAMETERS,
            Self::Ports => ids::PANELS_FILTER_PORTS,
            Self::PowerPorts => ids::PANELS_FILTER_POWER_PORTS,
            Self::Texts => ids::PANELS_FILTER_TEXTS,
            Self::DrawingObjects => ids::PANELS_FILTER_DRAWING_OBJECTS,
            Self::Other => ids::PANELS_FILTER_OTHER,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Components => "Parts",
            Self::Wires => "Wires",
            Self::Buses => "Buses",
            Self::SheetSymbols => "Sheet Symbols",
            Self::SheetEntries => "Sheet Entries",
            Self::NetLabels => "Net Labels",
            Self::Parameters => "Parameters",
            Self::Ports => "Ports",
            Self::PowerPorts => "Power Ports",
            Self::Texts => "Texts",
            Self::DrawingObjects => "Drawing Objects",
            Self::Other => "Other",
        }
    }

    /// The category as a command operand spells it (`sheet_symbols`).
    pub fn token(self) -> &'static str {
        match self {
            Self::Components => "components",
            Self::Wires => "wires",
            Self::Buses => "buses",
            Self::SheetSymbols => "sheet_symbols",
            Self::SheetEntries => "sheet_entries",
            Self::NetLabels => "net_labels",
            Self::Parameters => "parameters",
            Self::Ports => "ports",
            Self::PowerPorts => "power_ports",
            Self::Texts => "texts",
            Self::DrawingObjects => "drawing_objects",
            Self::Other => "other",
        }
    }

    /// The category an operand names, by [`Self::token`].
    pub fn from_token(token: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|f| f.token() == token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `label` is the en-US catalog text, so a chip and the Properties
    /// panel name a category the same way; Components reads "Parts".
    #[test]
    fn every_label_is_the_en_us_catalog_text() {
        let i18n = signex_i18n::I18n::en_us_ref();
        for filter in SelectionFilter::ALL {
            assert_eq!(
                filter.label(),
                i18n.message_plain(filter.message(), &[]),
                "{filter:?}"
            );
        }
        assert_eq!(SelectionFilter::Components.label(), "Parts");
    }
}
