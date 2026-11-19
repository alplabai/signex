//! Title-block rows of Document Options ▸ Parameters (#1080): edit the
//! active sheet's `title_block` map, which the canvas frame, the PDF title
//! block and every `${TOKEN}` substitution read.
//!
//! Each field commits on Enter, as one undoable edit; a half-typed value
//! is a draft that never reaches the document or undo.

use super::super::*;
use super::graphite_table::ParameterRow;
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

/// The title-block rows of the Parameters table for the active sheet. Each
/// field commits on Enter through `CommitTitleBlockField`.
pub(super) fn title_block_rows<'a>(ctx: &'a PanelContext) -> Vec<ParameterRow<'a>> {
    use signex_i18n::{RenderTranslatable, Translatable, ids, t};
    let i18n = &ctx.i18n;
    TitleBlockField::ALL
        .into_iter()
        .map(|field| {
            let own_is_empty = ctx
                .title_block
                .get(field.key())
                .is_none_or(String::is_empty);
            // The project's value, shown greyed while the sheet leaves the
            // field empty, and named in the label (#1080).
            let project_value = ctx
                .project_parameters
                .get(field.key())
                .filter(|v| !v.is_empty())
                .cloned();
            // An empty Date with no project value shows today, greyed, and
            // stores nothing (maintainer decision 2026-11-18).
            let today = match (field, &project_value) {
                (TitleBlockField::Date, None) => today_of(ctx),
                _ => None,
            };
            // Rendered without isolation marks, so the label reads as before.
            let around = |id| {
                Translatable::new(id)
                    .arg("field", Translatable::new(field.message()))
                    .localized(i18n)
            };
            let name = match (&project_value, own_is_empty, &today) {
                (Some(_), true, _) => around(ids::PROPERTIES_TITLE_BLOCK_FROM_PROJECT),
                (Some(_), false, _) => around(ids::PROPERTIES_TITLE_BLOCK_OVERRIDES_PROJECT),
                (None, true, Some(_)) => around(ids::PROPERTIES_TITLE_BLOCK_TODAY),
                (None, _, _) => t!(i18n, field.message()),
            };
            let input = signex_widgets::text_input(
                // The project's own value for the field (or today's date),
                // shown verbatim.
                signex_i18n::UserText::new(project_value.or(today).unwrap_or_default()),
                shown_title_block_value(ctx, field),
            )
            .on_input(move |s| PanelMsg::TitleBlockTyping(field, s))
            .on_submit(PanelMsg::CommitTitleBlockField(field))
            .size(TS.body)
            .padding(SP.xs)
            .style(crate::panels::form_look::text_input_style)
            .width(Length::Fill);
            ParameterRow {
                name: name.into(),
                value: container(input)
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

    /// Each field's catalog name reads as `TitleBlockField::label`, the
    /// English the panel showed before.
    #[test]
    fn each_field_keeps_its_english_name() {
        let en = signex_i18n::I18n::en_us();
        for field in TitleBlockField::ALL {
            assert_eq!(en.message_plain(field.message(), &[]), field.label());
        }
    }
}
