//! Built-in sheet templates — ISO A0-A5 + ANSI A-E, portrait + landscape
//! where standard practice allows. Constructed from a single
//! `standard_title_block()` helper so all 17 templates share identical
//! title-block layout (width, field positions, fonts) — only the page
//! size and orientation differ.
//!
//! When users want to customise, they clone a built-in via the future
//! `.snxsht` parser (`format.rs`).

use super::{FontStyle, Frame, Template, TemplateId, TitleBlock, TitleBlockField};
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

/// Shared title-block layout used by every built-in. 180 x 40 mm rectangle
/// anchored at the page's bottom-right: title, revision, date and company
/// down the left column; sheet number, file name and document number down
/// the right; the Drawn / Checked / Approved sign-off row along the bottom
/// (#1080). Extends the sketch in OUTPUT_PLAN.md §4.
///
/// Also the title block the schematic canvas and the PDF export draw for a
/// sheet with no `.snxsht` template, so what is on screen is what prints.
pub fn standard_title_block() -> TitleBlock {
    TitleBlock {
        width_mm: 180.0,
        height_mm: 40.0,
        fields: vec![
            TitleBlockField {
                name: "title".into(),
                x_mm: 4.0,
                y_mm: 4.0,
                font_family: "Roboto".into(),
                font_size_mm: 4.5,
                font_style: FontStyle::Bold,
                default_text: "${TITLE}".into(),
            },
            TitleBlockField {
                name: "revision".into(),
                x_mm: 4.0,
                y_mm: 12.0,
                font_family: "Roboto".into(),
                font_size_mm: 3.0,
                font_style: FontStyle::Normal,
                default_text: "Rev: ${REV}".into(),
            },
            TitleBlockField {
                name: "date".into(),
                x_mm: 4.0,
                y_mm: 18.0,
                font_family: "Roboto".into(),
                font_size_mm: 3.0,
                font_style: FontStyle::Normal,
                default_text: "${DATE}".into(),
            },
            TitleBlockField {
                name: "company".into(),
                x_mm: 4.0,
                y_mm: 24.0,
                font_family: "Roboto".into(),
                font_size_mm: 3.0,
                font_style: FontStyle::Normal,
                default_text: "${COMPANY}".into(),
            },
            TitleBlockField {
                name: "sheet".into(),
                x_mm: 100.0,
                y_mm: 4.0,
                font_family: "Roboto".into(),
                font_size_mm: 3.0,
                font_style: FontStyle::Normal,
                default_text: "Sheet ${SHEETNUMBER} of ${SHEETCOUNT}".into(),
            },
            TitleBlockField {
                name: "filename".into(),
                x_mm: 100.0,
                y_mm: 12.0,
                font_family: "Iosevka".into(),
                font_size_mm: 2.5,
                font_style: FontStyle::Italic,
                default_text: "${FILENAME}".into(),
            },
            TitleBlockField {
                name: "document_number".into(),
                x_mm: 100.0,
                y_mm: 18.0,
                font_family: "Roboto".into(),
                font_size_mm: 2.5,
                font_style: FontStyle::Normal,
                default_text: "Doc No: ${DOCUMENTNUMBER}".into(),
            },
            // Sign-off row (#1080).
            TitleBlockField {
                name: "drawn_by".into(),
                x_mm: 4.0,
                y_mm: 31.0,
                font_family: "Roboto".into(),
                font_size_mm: 2.5,
                font_style: FontStyle::Normal,
                default_text: "Drawn: ${DRAWNBY}".into(),
            },
            TitleBlockField {
                name: "checked_by".into(),
                x_mm: 64.0,
                y_mm: 31.0,
                font_family: "Roboto".into(),
                font_size_mm: 2.5,
                font_style: FontStyle::Normal,
                default_text: "Checked: ${CHECKEDBY}".into(),
            },
            TitleBlockField {
                name: "approved_by".into(),
                x_mm: 124.0,
                y_mm: 31.0,
                font_family: "Roboto".into(),
                font_size_mm: 2.5,
                font_style: FontStyle::Normal,
                default_text: "Approved: ${APPROVEDBY}".into(),
            },
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
            assert_eq!(t.title_block.fields.len(), 10);
        }
    }

    /// #1080 — the document number and the sign-off fields are drawn, and
    /// every field starts inside the block with its line inside it too.
    #[test]
    fn the_standard_block_carries_the_sign_off_fields_inside_its_outline() {
        let block = standard_title_block();
        for token in [
            "${DOCUMENTNUMBER}",
            "${DRAWNBY}",
            "${CHECKEDBY}",
            "${APPROVEDBY}",
        ] {
            assert!(
                block.fields.iter().any(|f| f.default_text.contains(token)),
                "{token} is not in the standard title block"
            );
        }
        for field in &block.fields {
            assert!(
                field.x_mm >= 0.0 && field.x_mm < block.width_mm,
                "{}",
                field.name
            );
            assert!(
                field.y_mm >= 0.0 && field.y_mm + field.font_size_mm <= block.height_mm,
                "{}",
                field.name
            );
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
                .fields
                .iter()
                .map(|f| f.default_text.as_str())
                .collect();
            assert!(concat.contains("${TITLE}"), "{id:?} missing TITLE");
            assert!(concat.contains("${REV}"), "{id:?} missing REV");
            assert!(concat.contains("${DATE}"), "{id:?} missing DATE");
            assert!(concat.contains("${FILENAME}"), "{id:?} missing FILENAME");
        }
    }
}
