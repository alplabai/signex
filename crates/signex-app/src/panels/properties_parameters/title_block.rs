//! Title Block section of the sheet's Properties (#1080): edits the active
//! sheet's `title_block` map, which the canvas frame, the PDF title block
//! and every `${TOKEN}` substitution read.
//!
//! Each field commits on Enter, as one undoable edit; a half-typed value
//! is a draft that never reaches the document or undo.

use super::super::*;

/// One editable title-block field, with the key it is stored under in the
/// sheet's `title_block` map. The first four keys are the ones sheets have
/// always carried; the rest are new with #1080.
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
}

impl TitleBlockField {
    pub const ALL: [Self; 8] = [
        Self::Title,
        Self::DocumentNumber,
        Self::Revision,
        Self::Date,
        Self::Company,
        Self::DrawnBy,
        Self::CheckedBy,
        Self::ApprovedBy,
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
        }
    }
}

/// A half-typed title-block value, tied to the sheet it was typed on so a
/// tab switch can never commit it into another sheet.
#[derive(Debug, Clone, PartialEq)]
pub struct TitleBlockDraft {
    pub sheet: std::path::PathBuf,
    pub field: TitleBlockField,
    pub text: String,
}

/// The Title Block collapsible section for the active sheet.
pub fn view_title_block_section<'a>(
    ctx: &'a PanelContext,
    muted: Color,
    primary: Color,
    border_c: Color,
) -> Element<'a, PanelMsg> {
    collapsible_section(
        "prop_title_block",
        "Title Block",
        &ctx.collapsed_sections,
        primary,
        border_c,
        move || {
            let mut c = Column::new().spacing(0).width(Length::Fill);
            for field in TitleBlockField::ALL {
                let value = match &ctx.title_block_draft {
                    Some(draft) if draft.field == field => draft.text.clone(),
                    _ => ctx
                        .title_block
                        .get(field.key())
                        .cloned()
                        .unwrap_or_default(),
                };
                c = c.push(
                    container(
                        row![
                            property_label(field.label().to_string(), muted),
                            iced::widget::text_input("", &value)
                                .on_input(move |s| PanelMsg::TitleBlockTyping(field, s))
                                .on_submit(PanelMsg::CommitTitleBlockField(field))
                                .size(11)
                                .padding(4)
                                .width(Length::FillPortion(PROPERTY_CONTROL_PORTION)),
                        ]
                        .spacing(8.0)
                        .align_y(iced::Alignment::Center),
                    )
                    .padding([2, PROPERTY_ROW_PAD_X])
                    .width(Length::Fill),
                );
            }
            c = c.push(
                container(text("Press Enter to apply a field.").size(10).color(muted))
                    .padding([3, 8]),
            );
            c
        },
    )
    .into()
}
