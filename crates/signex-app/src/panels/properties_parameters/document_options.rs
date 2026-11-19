//! View state and messages of Document Options (#2531): the Parameters
//! list's filter, the typed zone counts, and the hover link between the
//! General tab's fields and its two illustrations.
//!
//! The state lives in `PanelContext::document_options`, kept across context
//! rebuilds like the open tab; it is view state only, never written to the
//! sheet. Edits of the sheet go through the existing Page Options messages.

use std::path::PathBuf;

use super::super::*;
use iced::Widget;
use signex_widgets::property_illustration::{
    FieldId, Illustration, IllustrationHover, IllustrationMsg, property_illustration,
};

use crate::styles::TypeScale;

const TS: TypeScale = TypeScale::DEFAULT;

/// Which rows the Parameters list shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ParametersFilter {
    /// Parameters and rules.
    #[default]
    All,
    Parameters,
    Rules,
}

/// A zone count of the sheet border.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoneAxis {
    /// Zones lettered A, B, C... down the sides.
    Vertical,
    /// Zones numbered 1, 2, 3... along the top and bottom.
    Horizontal,
}

impl ZoneAxis {
    /// The frame edit that sets this count to `n`.
    pub fn edit(self, n: u8) -> SheetFrameEdit {
        match self {
            Self::Vertical => SheetFrameEdit::VerticalZones(n),
            Self::Horizontal => SheetFrameEdit::HorizontalZones(n),
        }
    }
}

/// A half-typed zone count, tied to the sheet it was typed on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoneDraft {
    pub sheet: Option<PathBuf>,
    pub axis: ZoneAxis,
    pub text: String,
}

/// One event of Document Options that only changes how the panel shows.
#[derive(Debug, Clone, PartialEq)]
pub enum DocumentOptionsMsg {
    /// An All / Parameters / Rules chip was picked.
    ParametersFilterPicked(ParametersFilter),
    /// The Parameters list's filter text changed.
    ParametersQueryChanged(String),
    /// A keystroke in a zone count.
    ZoneCountTyped(ZoneAxis, String),
    /// Enter in a zone count: its draft is written to the sheet.
    ZoneCountSubmitted(ZoneAxis),
    /// The pointer moved over a field or an illustration part.
    Illustration(IllustrationMsg),
}

/// The view state of Document Options.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DocumentOptionsUi {
    pub parameters_filter: ParametersFilter,
    pub parameters_query: String,
    pub zone_draft: Option<ZoneDraft>,
    pub hover: IllustrationHover,
}

impl DocumentOptionsUi {
    /// Apply a view-only event. A submitted zone count is returned as the
    /// frame edit to run, `None` while the draft is not a count.
    pub fn apply(
        &mut self,
        msg: DocumentOptionsMsg,
        sheet: Option<PathBuf>,
    ) -> Option<SheetFrameEdit> {
        match msg {
            DocumentOptionsMsg::ParametersFilterPicked(filter) => self.parameters_filter = filter,
            DocumentOptionsMsg::ParametersQueryChanged(query) => self.parameters_query = query,
            DocumentOptionsMsg::ZoneCountTyped(axis, text) => {
                self.zone_draft = Some(ZoneDraft { sheet, axis, text });
            }
            DocumentOptionsMsg::ZoneCountSubmitted(axis) => {
                let draft = self
                    .zone_draft
                    .take_if(|d| d.axis == axis && d.sheet == sheet)?;
                return typed_zone_count(&draft.text).map(|n| axis.edit(n));
            }
            DocumentOptionsMsg::Illustration(hover) => self.hover.update(hover),
        }
        None
    }

    /// The text a zone field shows: its draft on `sheet`, else `stored`.
    pub fn zone_text(&self, sheet: Option<&std::path::Path>, axis: ZoneAxis, stored: u8) -> String {
        match &self.zone_draft {
            Some(d) if d.axis == axis && d.sheet.as_deref() == sheet => d.text.clone(),
            _ => stored.to_string(),
        }
    }
}

/// A typed zone count, capped at the most zones a border has; `None` for
/// text that is not a whole number.
pub fn typed_zone_count(text: &str) -> Option<u8> {
    let n: u32 = text.trim().parse().ok()?;
    let max = u32::from(signex_types::schematic::MAX_ZONES);
    u8::try_from(n.min(max)).ok()
}

pub(super) fn msg(m: DocumentOptionsMsg) -> PanelMsg {
    PanelMsg::DocumentOptions(m)
}

/// An illustration under a section header, its hover routed to the panel.
pub(super) fn illustration_view<'a>(
    illustration: Illustration,
    hover: &IllustrationHover,
    tokens: &ThemeTokens,
) -> Element<'a, PanelMsg> {
    let drawing = property_illustration(illustration, hover, tokens, TS.caption)
        .map(|m| msg(DocumentOptionsMsg::Illustration(m)));
    container(drawing)
        .padding([0.0, f32::from(PROPERTY_ROW_PAD_X)])
        .width(Length::Fill)
        .boxed()
}

/// A field row that reports its field to the illustrations on hover and is
/// highlighted while a linked part is hovered.
pub(super) fn hover_row<'a>(
    field: FieldId,
    hover: &IllustrationHover,
    tokens: &ThemeTokens,
    content: Element<'a, PanelMsg>,
) -> Element<'a, PanelMsg> {
    let lit = hover.is_field_highlighted(&field);
    let wash = crate::styles::tokens::resolve(tokens).accent_soft;
    let body = container(content)
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: lit.then_some(Background::Color(wash)),
            ..container::Style::default()
        });
    iced::widget::mouse_area(body)
        .on_enter(msg(DocumentOptionsMsg::Illustration(
            IllustrationMsg::FieldHovered(field.clone()),
        )))
        .on_exit(msg(DocumentOptionsMsg::Illustration(
            IllustrationMsg::FieldLeft(field),
        )))
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_zone_count_reads_whole_numbers_capped_at_the_border_limit() {
        assert_eq!(typed_zone_count(" 6 "), Some(6));
        assert_eq!(typed_zone_count("0"), Some(0));
        assert_eq!(typed_zone_count("x"), None);
        assert_eq!(typed_zone_count("-1"), None);
        let max = signex_types::schematic::MAX_ZONES;
        assert_eq!(typed_zone_count("9999"), Some(max));
    }

    #[test]
    fn a_submitted_zone_count_becomes_one_frame_edit_on_its_own_sheet() {
        let sheet = Some(PathBuf::from("a.snxsch"));
        let mut ui = DocumentOptionsUi::default();
        let typed = DocumentOptionsMsg::ZoneCountTyped(ZoneAxis::Horizontal, "5".into());
        assert_eq!(ui.apply(typed, sheet.clone()), None);
        assert_eq!(ui.zone_text(sheet.as_deref(), ZoneAxis::Horizontal, 6), "5");
        assert_eq!(ui.zone_text(sheet.as_deref(), ZoneAxis::Vertical, 4), "4");
        let other = Some(PathBuf::from("b.snxsch"));
        assert_eq!(ui.zone_text(other.as_deref(), ZoneAxis::Horizontal, 6), "6");
        let submitted = DocumentOptionsMsg::ZoneCountSubmitted(ZoneAxis::Horizontal);
        assert_eq!(
            ui.apply(submitted.clone(), sheet.clone()),
            Some(SheetFrameEdit::HorizontalZones(5))
        );
        assert_eq!(ui.zone_draft, None);
        assert_eq!(ui.apply(submitted, sheet), None, "nothing left to apply");
    }
}
