//! Title-block rows of Document Options ▸ Parameters (#1080): edit the
//! active sheet's `title_block` map, which the canvas frame, the PDF title
//! block and every `${TOKEN}` substitution read.
//!
//! Each field commits on Enter, as one undoable edit; a half-typed value
//! is a draft that never reaches the document or undo.

use super::super::*;
use super::graphite_table::{ParameterRow, inline_value_input};
use crate::styles::{Spacing, TypeScale};
use iced::Widget;

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// One editable title-block field, with the key it is stored under in the
/// sheet's `title_block` map. The first four keys are the ones sheets have
/// always carried; DocumentNumber to ApprovedBy are new with #1080, Author
/// to Address4 with the Parameters list (maintainer decision 2026-11-18).
/// A key is written only when the sheet has a value for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitleBlockField {
    Title,
    DocumentNumber,
    Revision,
    Date,
    Company,
    DrawnBy,
    CheckedBy,
    ApprovedBy,
    Author,
    Engineer,
    Organization,
    Address1,
    Address2,
    Address3,
    Address4,
}

impl TitleBlockField {
    pub const ALL: [Self; 15] = [
        Self::Title,
        Self::DocumentNumber,
        Self::Revision,
        Self::Date,
        Self::Company,
        Self::DrawnBy,
        Self::CheckedBy,
        Self::ApprovedBy,
        Self::Author,
        Self::Engineer,
        Self::Organization,
        Self::Address1,
        Self::Address2,
        Self::Address3,
        Self::Address4,
    ];

    /// Key in `SchematicSheet::title_block`.
    pub fn key(self) -> &'static str {
        match self {
            Self::Title => "title",
            Self::DocumentNumber => "document_number",
            Self::Revision => "rev",
            Self::Date => "date",
            Self::Company => "company",
            Self::DrawnBy => "drawn_by",
            Self::CheckedBy => "checked_by",
            Self::ApprovedBy => "approved_by",
            Self::Author => "author",
            Self::Engineer => "engineer",
            Self::Organization => "organization",
            Self::Address1 => "address1",
            Self::Address2 => "address2",
            Self::Address3 => "address3",
            Self::Address4 => "address4",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Title => "Title",
            Self::DocumentNumber => "Document No.",
            Self::Revision => "Revision",
            Self::Date => "Date",
            Self::Company => "Company",
            Self::DrawnBy => "Drawn By",
            Self::CheckedBy => "Checked By",
            Self::ApprovedBy => "Approved By",
            Self::Author => "Author",
            Self::Engineer => "Engineer",
            Self::Organization => "Organization",
            Self::Address1 => "Address 1",
            Self::Address2 => "Address 2",
            Self::Address3 => "Address 3",
            Self::Address4 => "Address 4",
        }
    }

    /// The `${Name}` token the field is read with in any text.
    pub fn token(self) -> &'static str {
        match self {
            Self::Title => "Title",
            Self::DocumentNumber => "DocumentNumber",
            Self::Revision => "Revision",
            Self::Date => "Date",
            Self::Company => "CompanyName",
            Self::DrawnBy => "DrawnBy",
            Self::CheckedBy => "CheckedBy",
            Self::ApprovedBy => "ApprovedBy",
            Self::Author => "Author",
            Self::Engineer => "Engineer",
            Self::Organization => "Organization",
            Self::Address1 => "Address1",
            Self::Address2 => "Address2",
            Self::Address3 => "Address3",
            Self::Address4 => "Address4",
        }
    }

    /// The field's name in the catalog, as the Properties panel shows it.
    pub fn message(self) -> signex_i18n::MessageId {
        use signex_i18n::ids;
        match self {
            Self::Title => ids::PROPERTIES_TITLE_BLOCK_TITLE,
            Self::DocumentNumber => ids::PROPERTIES_TITLE_BLOCK_DOCUMENT_NUMBER,
            Self::Revision => ids::PROPERTIES_TITLE_BLOCK_REVISION,
            Self::Date => ids::PROPERTIES_TITLE_BLOCK_DATE,
            Self::Company => ids::PROPERTIES_TITLE_BLOCK_COMPANY,
            Self::DrawnBy => ids::PROPERTIES_TITLE_BLOCK_DRAWN_BY,
            Self::CheckedBy => ids::PROPERTIES_TITLE_BLOCK_CHECKED_BY,
            Self::ApprovedBy => ids::PROPERTIES_TITLE_BLOCK_APPROVED_BY,
            Self::Author => ids::PROPERTIES_TITLE_BLOCK_AUTHOR,
            Self::Engineer => ids::PROPERTIES_TITLE_BLOCK_ENGINEER,
            Self::Organization => ids::PROPERTIES_TITLE_BLOCK_ORGANIZATION,
            Self::Address1 => ids::PROPERTIES_TITLE_BLOCK_ADDRESS_1,
            Self::Address2 => ids::PROPERTIES_TITLE_BLOCK_ADDRESS_2,
            Self::Address3 => ids::PROPERTIES_TITLE_BLOCK_ADDRESS_3,
            Self::Address4 => ids::PROPERTIES_TITLE_BLOCK_ADDRESS_4,
        }
    }
}

/// A half-typed title-block value, tied to the sheet and field it was typed
/// in, so neither a tab switch nor moving to another field can commit it
/// anywhere else or drop it.
#[derive(Debug, Clone, PartialEq)]
pub struct TitleBlockDraft {
    pub sheet: std::path::PathBuf,
    pub field: TitleBlockField,
    pub text: String,
}

/// What a title-block row shows in its field: the text being typed, else
/// the sheet's own value; empty when the sheet has none.
pub fn shown_title_block_value(ctx: &PanelContext, field: TitleBlockField) -> String {
    ctx.title_block_sheet
        .as_deref()
        .and_then(|sheet| ctx.drafts.title_block_text(sheet, field))
        .or_else(|| ctx.title_block.get(field.key()).cloned())
        .unwrap_or_default()
}

/// Where a title-block row's value comes from (#2531).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitleBlockSource {
    /// The sheet has its own value.
    Sheet,
    /// The sheet leaves it empty and the project has a value: the row shows
    /// the project's value with a "project" pill, and typing overrides it
    /// for this sheet.
    Project,
    /// Neither has a value: the row reads "Not set".
    Unset,
}

/// Where `field`'s value comes from, given the sheet's title block and the
/// project's parameters.
pub fn title_block_source(
    field: TitleBlockField,
    sheet: &std::collections::BTreeMap<String, String>,
    project: &std::collections::BTreeMap<String, String>,
) -> TitleBlockSource {
    let has = |map: &std::collections::BTreeMap<String, String>| {
        map.get(field.key()).is_some_and(|v| !v.is_empty())
    };
    if has(sheet) {
        TitleBlockSource::Sheet
    } else if has(project) {
        TitleBlockSource::Project
    } else {
        TitleBlockSource::Unset
    }
}

/// The "project" pill of a row inherited from the project, with its hint.
fn project_pill<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    use signex_i18n::{ids, t};
    use signex_widgets::chips::{ChipMetrics, ChipTone, kind_chip};
    let r = crate::styles::tokens::resolve(&ctx.tokens);
    let tone = ChipTone {
        fill: r.sunken,
        ink: r.ink3,
    };
    let pill = kind_chip(
        t!(ctx.i18n, ids::PROPERTIES_PARAMETERS_FROM_PROJECT),
        tone,
        ChipMetrics::from_text_size(TS.body),
    );
    crate::panels::graphite::hint_tooltip(
        pill,
        t!(ctx.i18n, ids::PROPERTIES_PARAMETERS_FROM_PROJECT_TIP),
        &ctx.tokens,
        iced::widget::tooltip::Position::Left,
    )
}

/// The title-block rows of the Parameters list for the active sheet, for
/// `fields` in order. Each field commits on Enter through
/// `CommitTitleBlockField`, one undo step.
pub(super) fn title_block_rows<'a>(
    ctx: &'a PanelContext,
    fields: &[TitleBlockField],
) -> Vec<ParameterRow<'a>> {
    use signex_i18n::{RenderTranslatable, Translatable, ids, t};
    let i18n = &ctx.i18n;
    fields
        .iter()
        .map(|&field| {
            let source = title_block_source(field, &ctx.title_block, &ctx.project_parameters);
            // The project's value, shown greyed while the sheet leaves the
            // field empty (#1080).
            let project_value = match source {
                TitleBlockSource::Project => ctx.project_parameters.get(field.key()).cloned(),
                _ => None,
            };
            // An empty Date with no project value shows today, greyed, and
            // stores nothing (maintainer decision 2026-11-18).
            let today = match (field, source) {
                (TitleBlockField::Date, TitleBlockSource::Unset) => today_of(ctx),
                _ => None,
            };
            // Rendered without isolation marks, so the label reads as before.
            let name = if today.is_some() {
                Translatable::new(ids::PROPERTIES_TITLE_BLOCK_TODAY)
                    .arg("field", Translatable::new(field.message()))
                    .localized(i18n)
            } else {
                t!(i18n, field.message())
            };
            let placeholder: signex_widgets::Label<'a> = match project_value.or(today) {
                // The project's own value for the field (or today's date),
                // shown verbatim.
                Some(value) => signex_i18n::UserText::new(value).into(),
                None => t!(i18n, ids::PROPERTIES_PARAMETERS_NOT_SET).into(),
            };
            let shown = shown_title_block_value(ctx, field);
            let is_unset = source == TitleBlockSource::Unset;
            let input = inline_value_input(placeholder, shown.clone(), is_unset)
                .on_input(move |s| PanelMsg::TitleBlockTyping(field, s))
                .on_submit(PanelMsg::CommitTitleBlockField(field));
            let mut cell = row![input].spacing(SP.xs).align_y(iced::Alignment::Center);
            if shown.is_empty() && source == TitleBlockSource::Project {
                cell = cell.push(project_pill(ctx));
            }
            ParameterRow {
                name: name.into(),
                value: container(cell)
                    .padding([2.0, SP.xs])
                    .width(Length::Fill)
                    .boxed(),
            }
        })
        .collect()
}

/// Today's date as the CurrentDate row shows it; `None` with no sheet.
fn today_of(ctx: &PanelContext) -> Option<String> {
    ctx.builtin_parameters
        .iter()
        .find(|(parameter, _)| *parameter == super::BuiltinParameter::CurrentDate)
        .map(|(_, value)| value.clone())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mockup row 40: a title-block value, "Not set" included, reads inline
    /// and shows its box only on hover or focus.
    #[test]
    fn a_value_box_shows_only_on_hover_or_focus() {
        use crate::panels::form_look::inline_value_input_style as style;
        use iced::widget::text_input::Status;
        let theme = Theme::Dark;
        let none = Color::TRANSPARENT;
        assert_eq!(style(&theme, Status::Active).border.color, none);
        assert_ne!(style(&theme, Status::Hovered).border.color, none);
        let focused = Status::Focused { is_hovered: false };
        assert_ne!(style(&theme, focused).border.color, none);
    }

    /// Each field's catalog name reads as `TitleBlockField::label`, the
    /// English the panel showed before.
    #[test]
    fn each_field_keeps_its_english_name() {
        let en = signex_i18n::I18n::en_us();
        for field in TitleBlockField::ALL {
            assert_eq!(en.message_plain(field.message(), &[]), field.label());
        }
    }

    /// A row is the sheet's when the sheet has a value, the project's when
    /// only the project has one, and unset otherwise; an empty sheet value
    /// does not hide the project's.
    #[test]
    fn a_rows_value_comes_from_the_sheet_then_the_project() {
        use std::collections::BTreeMap;
        let mut sheet = BTreeMap::new();
        let mut project = BTreeMap::new();
        let source = |s: &BTreeMap<String, String>, p: &BTreeMap<String, String>| {
            title_block_source(TitleBlockField::Company, s, p)
        };
        assert_eq!(source(&sheet, &project), TitleBlockSource::Unset);
        project.insert("company".to_string(), "Alp Lab".to_string());
        assert_eq!(source(&sheet, &project), TitleBlockSource::Project);
        sheet.insert("company".to_string(), String::new());
        assert_eq!(source(&sheet, &project), TitleBlockSource::Project);
        sheet.insert("company".to_string(), "Other".to_string());
        assert_eq!(source(&sheet, &project), TitleBlockSource::Sheet);
    }
}
