//! Where a primitive pick is written: one row, several rows (Re-bind,
//! #2572), a design part, a board footprint or a sketch editor.

use uuid::Uuid;

use super::EditorAddress;

/// Where the picker should write the picked `PrimitiveRef`.
#[derive(Debug, Clone)]
pub enum PrimitivePickerTarget {
    /// The user picks for an open Component Preview tab — apply +
    /// save the row.
    PreviewRow(EditorAddress),
    /// The user picks while editing a row in the Library Browser
    /// grid's Edit Component Details modal (Deliverable B).
    EditRowModal(EditorAddress),
    /// The user picks for the row currently selected in the Library
    /// Browser tab — apply directly through the adapter and refresh.
    /// F15 (2026-05-03): row binding lives next to the row, not in a
    /// separate Component Preview tab or modal.
    BrowserRow(EditorAddress),
    /// The in-use symbol Delete's Re-bind (#2572): the picked symbol takes
    /// the place of `replacing` in every row listed, as default and as
    /// alternate, each written as a [`Self::BrowserRow`] pick is.
    BrowserRows {
        rows: Vec<EditorAddress>,
        replacing: signex_library::PrimitiveRef,
    },
    /// The user picks a symbol to add as an alternate of the row open in a
    /// Component Preview tab (#1230).
    PreviewAlternate(EditorAddress),
    /// Update PCB's Choose footprint (#2371): the footprint of a part of the
    /// design, by its sheet placement and anchor unit, set on the schematic.
    DesignPart {
        sheet: signex_types::schematic::InstancePath,
        part: Uuid,
    },
    /// A board footprint swap (#2366): the footprints of `board` to replace,
    /// and the footprints their library parts offer, listed first.
    BoardFootprint {
        board: std::path::PathBuf,
        footprints: Vec<Uuid>,
        suggested: Vec<signex_library::PrimitiveRef>,
    },
    /// Import from another footprint (mockup row 189): the sketch editor at
    /// `editor` lists the picked footprint's parameters. Reads only.
    SketchParameters { editor: std::path::PathBuf },
}

impl PrimitivePickerTarget {
    /// The one row the pick is for; `None` for a pick that writes no row
    /// or several.
    pub fn address(&self) -> Option<&EditorAddress> {
        match self {
            Self::PreviewRow(a)
            | Self::EditRowModal(a)
            | Self::BrowserRow(a)
            | Self::PreviewAlternate(a) => Some(a),
            Self::BrowserRows { .. }
            | Self::DesignPart { .. }
            | Self::BoardFootprint { .. }
            | Self::SketchParameters { .. } => None,
        }
    }

    /// Every row the pick writes.
    pub fn rows(&self) -> &[EditorAddress] {
        match self {
            Self::PreviewRow(a)
            | Self::EditRowModal(a)
            | Self::BrowserRow(a)
            | Self::PreviewAlternate(a) => std::slice::from_ref(a),
            Self::BrowserRows { rows, .. } => rows,
            Self::DesignPart { .. }
            | Self::BoardFootprint { .. }
            | Self::SketchParameters { .. } => &[],
        }
    }

    /// Directory of the library whose rows this pick writes; `None` when
    /// it writes no row, or rows of several libraries.
    pub fn library_path(&self) -> Option<&std::path::Path> {
        let (first, rest) = self.rows().split_first()?;
        rest.iter()
            .all(|a| a.library_path == first.library_path)
            .then_some(first.library_path.as_path())
    }

    /// Every library this pick writes a row of, each once.
    pub fn library_paths(&self) -> Vec<&std::path::Path> {
        let mut paths: Vec<&std::path::Path> = self
            .rows()
            .iter()
            .map(|a| a.library_path.as_path())
            .collect();
        paths.sort();
        paths.dedup();
        paths
    }
}
