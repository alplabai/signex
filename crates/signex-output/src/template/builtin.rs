//! Built-in sheet templates — ISO A0-A5 + ANSI A-E, portrait + landscape
//! where standard practice allows. Constructed from a single
//! `standard_title_block()` helper so all 17 templates share identical
//! title-block layout (width, field positions, fonts) — only the page
//! size and orientation differ.
//!
//! When users want to customise, they clone a built-in via the future
//! `.snxsht` parser (`format.rs`).

use super::{FontStyle, Frame, Template, TemplateId, TitleBlock, TitleBlockCell};
use crate::pdf::{Orientation, PageSize};

/// Every built-in template's IDs, in display order.
pub fn all_builtin_ids() -> Vec<TemplateId> {
    BUILTIN_TEMPLATES
        .iter()
        .map(|(id, ..)| TemplateId::from(*id))
        .collect()
}

/// Load a built-in template by ID. Returns `None` for unknown IDs.
pub fn load_builtin(id: &TemplateId) -> Option<Template> {
    BUILTIN_TEMPLATES
        .iter()
        .find(|(builtin_id, ..)| *builtin_id == id.0.as_str())
        .map(|&(id, display, size, orientation)| Template {
            id: TemplateId::from(id),
            display_name: display.to_string(),
            page_size: size,
            orientation,
            frame: Frame::default(),
            title_block: standard_title_block(),
        })
}

/// Registry of built-ins. Order matters for display — ISO first (most
/// common in European/Asian use), ANSI second (US), portrait before
/// landscape within each pair.
const BUILTIN_TEMPLATES: &[(&str, &str, PageSize, Orientation)] = &[
    (
        "iso_a0_portrait",
        "ISO A0 portrait",
        PageSize::IsoA0,
        Orientation::Portrait,
    ),
    (
        "iso_a0_landscape",
        "ISO A0 landscape",
        PageSize::IsoA0,
        Orientation::Landscape,
    ),
    (
        "iso_a1_portrait",
        "ISO A1 portrait",
        PageSize::IsoA1,
        Orientation::Portrait,
    ),
    (
        "iso_a1_landscape",
        "ISO A1 landscape",
        PageSize::IsoA1,
        Orientation::Landscape,
    ),
    (
        "iso_a2_portrait",
        "ISO A2 portrait",
        PageSize::IsoA2,
        Orientation::Portrait,
    ),
    (
        "iso_a2_landscape",
        "ISO A2 landscape",
        PageSize::IsoA2,
        Orientation::Landscape,
    ),
    (
        "iso_a3_portrait",
        "ISO A3 portrait",
        PageSize::IsoA3,
        Orientation::Portrait,
    ),
    (
        "iso_a3_landscape",
        "ISO A3 landscape",
        PageSize::IsoA3,
        Orientation::Landscape,
    ),
    (
        "iso_a4_portrait",
        "ISO A4 portrait",
        PageSize::IsoA4,
        Orientation::Portrait,
    ),
    (
        "iso_a4_landscape",
        "ISO A4 landscape",
        PageSize::IsoA4,
        Orientation::Landscape,
    ),
    (
        "iso_a5_portrait",
        "ISO A5 portrait",
        PageSize::IsoA5,
        Orientation::Portrait,
    ),
    (
        "iso_a5_landscape",
        "ISO A5 landscape",
        PageSize::IsoA5,
        Orientation::Landscape,
    ),
    (
        "ansi_a_portrait",
        "ANSI A (Letter) portrait",
        PageSize::AnsiA,
        Orientation::Portrait,
    ),
    (
        "ansi_a_landscape",
        "ANSI A (Letter) landscape",
        PageSize::AnsiA,
        Orientation::Landscape,
    ),
    (
        "ansi_b_portrait",
        "ANSI B (Tabloid) portrait",
        PageSize::AnsiB,
        Orientation::Portrait,
    ),
    (
        "ansi_b_landscape",
        "ANSI B (Tabloid) landscape",
        PageSize::AnsiB,
        Orientation::Landscape,
    ),
    (
        "ansi_c_landscape",
        "ANSI C landscape",
        PageSize::AnsiC,
        Orientation::Landscape,
    ),
    (
        "ansi_d_landscape",
        "ANSI D landscape",
        PageSize::AnsiD,
        Orientation::Landscape,
    ),
    (
        "ansi_e_landscape",
        "ANSI E landscape",
        PageSize::AnsiE,
        Orientation::Landscape,
    ),
];

/// Width of the standard title block, in mm: about a third of an A4
/// landscape sheet (#1201).
pub const STANDARD_TITLE_BLOCK_W_MM: f64 = 110.0;
/// Height of the standard title block, in mm: four rows.
pub const STANDARD_TITLE_BLOCK_H_MM: f64 = 28.0;

/// Shared title-block layout used by every built-in: a compact table of
/// labelled cells anchored at the drawing area's bottom-right (#1201).
///
/// | Title (full width)                        |
/// | Doc No     | Rev  | Sheet   | Date         |
/// | Company            | File                  |
/// | Drawn       | Checked      | Approved      |
///
/// Also the title block the schematic canvas and the PDF export draw for a
/// sheet with no `.snxsht` template, so what is on screen is what prints.
pub fn standard_title_block() -> TitleBlock {
    /// A cell with a 2.5 mm regular value.
    fn cell([x_mm, y_mm, w_mm, h_mm]: [f64; 4], caption: &str, value: &str) -> TitleBlockCell {
        TitleBlockCell {
            x_mm,
            y_mm,
            w_mm,
            h_mm,
            caption: caption.into(),
            value: value.into(),
            font_family: "Roboto".into(),
            value_size_mm: 2.5,
            value_style: FontStyle::Normal,
        }
    }
    TitleBlock {
        width_mm: STANDARD_TITLE_BLOCK_W_MM,
        height_mm: STANDARD_TITLE_BLOCK_H_MM,
        fields: Vec::new(),
        cells: vec![
            TitleBlockCell {
                value_size_mm: 3.5,
                value_style: FontStyle::Bold,
                ..cell([0.0, 0.0, 110.0, 9.0], "Title", "${TITLE}")
            },
            cell([0.0, 9.0, 40.0, 7.0], "Doc No", "${DOCUMENTNUMBER}"),
            cell([40.0, 9.0, 16.0, 7.0], "Rev", "${REV}"),
            cell(
                [56.0, 9.0, 26.0, 7.0],
                "Sheet",
                "${SHEETNUMBER} of ${SHEETCOUNT}",
            ),
            cell([82.0, 9.0, 28.0, 7.0], "Date", "${DATE}"),
            cell([0.0, 16.0, 60.0, 6.0], "Company", "${COMPANY}"),
            TitleBlockCell {
                value_style: FontStyle::Italic,
                ..cell([60.0, 16.0, 50.0, 6.0], "File", "${FILENAME}")
            },
            // Sign-off row (#1080).
            cell([0.0, 22.0, 37.0, 6.0], "Drawn", "${DRAWNBY}"),
            cell([37.0, 22.0, 37.0, 6.0], "Checked", "${CHECKEDBY}"),
            cell([74.0, 22.0, 36.0, 6.0], "Approved", "${APPROVEDBY}"),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn has_expected_builtin_count() {
        // 6 ISO × portrait+landscape = 12
        // ANSI A / B: portrait + landscape each = 4
        // ANSI C / D / E: landscape only = 3
        // Total = 19. (OUTPUT_PLAN.md table sums to 19 but typoed "17" in
        // the total line; doc gets corrected in a follow-up commit.)
        assert_eq!(all_builtin_ids().len(), 19);
    }

    #[test]
    fn every_id_loads() {
        for id in all_builtin_ids() {
            let t = load_builtin(&id).unwrap_or_else(|| panic!("missing: {id:?}"));
            assert_eq!(t.id, id);
            assert!(!t.display_name.is_empty());
            assert_eq!(t.title_block.cells.len(), 10);
        }
    }

    #[test]
    fn unknown_id_returns_none() {
        assert!(load_builtin(&TemplateId::from("bogus")).is_none());
        assert!(load_builtin(&TemplateId::from("")).is_none());
    }

    #[test]
    fn default_template_id_loads() {
        let t = load_builtin(&TemplateId::default()).unwrap();
        assert_eq!(t.page_size, PageSize::IsoA4);
        assert_eq!(t.orientation, Orientation::Landscape);
    }

    #[test]
    fn all_title_blocks_carry_substitution_tokens() {
        for id in all_builtin_ids() {
            let t = load_builtin(&id).unwrap();
            let concat: String = t
                .title_block
                .cells
                .iter()
                .map(|c| c.value.as_str())
                .collect();
            assert!(concat.contains("${TITLE}"), "{id:?} missing TITLE");
            assert!(concat.contains("${REV}"), "{id:?} missing REV");
            assert!(concat.contains("${DATE}"), "{id:?} missing DATE");
            assert!(concat.contains("${FILENAME}"), "{id:?} missing FILENAME");
        }
    }
}
