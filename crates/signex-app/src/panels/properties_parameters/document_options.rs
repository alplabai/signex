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
    /// Enter found no whole number: the draft stays, marked.
    pub invalid: bool,
}

/// A submitted zone count that was over the limit: the count used instead,
/// shown under the row until the next keystroke.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoneClamp {
    pub sheet: Option<PathBuf>,
    pub used: u8,
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
    pub zone_clamp: Option<ZoneClamp>,
    pub hover: IllustrationHover,
}

impl DocumentOptionsUi {
    /// Apply a view-only event. A submitted zone count is returned as the
    /// frame edit to run; text that is not a count keeps its draft, marked
    /// invalid, and runs nothing.
    pub fn apply(
        &mut self,
        msg: DocumentOptionsMsg,
        sheet: Option<PathBuf>,
    ) -> Option<SheetFrameEdit> {
        match msg {
            DocumentOptionsMsg::ParametersFilterPicked(filter) => self.parameters_filter = filter,
            DocumentOptionsMsg::ParametersQueryChanged(query) => self.parameters_query = query,
            DocumentOptionsMsg::ZoneCountTyped(axis, text) => {
                self.zone_clamp = None;
                self.zone_draft = Some(ZoneDraft {
                    sheet,
                    axis,
                    text,
                    invalid: false,
                });
            }
            DocumentOptionsMsg::ZoneCountSubmitted(axis) => {
                let draft =
                    (self.zone_draft.as_mut()).filter(|d| d.axis == axis && d.sheet == sheet)?;
                let Some((used, clamped)) = read_zone_count(&draft.text) else {
                    draft.invalid = true;
                    return None;
                };
                self.zone_draft = None;
                self.zone_clamp = clamped.then_some(ZoneClamp { sheet, used });
                return Some(axis.edit(used));
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

    /// Whether the zone field of `axis` on `sheet` holds text Enter could
    /// not read as a count.
    pub fn zone_invalid(&self, sheet: Option<&std::path::Path>, axis: ZoneAxis) -> bool {
        (self.zone_draft.as_ref())
            .is_some_and(|d| d.invalid && d.axis == axis && d.sheet.as_deref() == sheet)
    }

    /// The count a zone field on `sheet` was clamped to by its last Enter.
    pub fn zone_clamped_to(&self, sheet: Option<&std::path::Path>) -> Option<u8> {
        (self.zone_clamp.as_ref())
            .filter(|c| c.sheet.as_deref() == sheet)
            .map(|c| c.used)
    }
}

/// A typed zone count, capped at the most zones a border has; `None` for
/// text that is not a whole number.
pub fn typed_zone_count(text: &str) -> Option<u8> {
    read_zone_count(text).map(|(used, _)| used)
}

/// A typed zone count and whether it was clamped to the border limit.
fn read_zone_count(text: &str) -> Option<(u8, bool)> {
    let n: u64 = text.trim().parse().ok()?;
    let max = signex_types::schematic::MAX_ZONES;
    let used = u8::try_from(n.min(u64::from(max))).ok()?;
    Some((used, n > u64::from(max)))
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

    /// Enter on text that is not a count keeps the draft, marked invalid,
    /// and edits nothing; the next keystroke clears the mark.
    #[test]
    fn a_zone_count_that_is_not_a_number_keeps_its_draft_marked() {
        let sheet = Some(PathBuf::from("a.snxsch"));
        let mut ui = DocumentOptionsUi::default();
        let typed =
            |text: &str| DocumentOptionsMsg::ZoneCountTyped(ZoneAxis::Vertical, text.into());
        let _ = ui.apply(typed("4x"), sheet.clone());
        let submitted = DocumentOptionsMsg::ZoneCountSubmitted(ZoneAxis::Vertical);
        assert_eq!(ui.apply(submitted, sheet.clone()), None);
        assert_eq!(ui.zone_text(sheet.as_deref(), ZoneAxis::Vertical, 4), "4x");
        assert!(ui.zone_invalid(sheet.as_deref(), ZoneAxis::Vertical));
        assert!(!ui.zone_invalid(sheet.as_deref(), ZoneAxis::Horizontal));
        let _ = ui.apply(typed("4"), sheet.clone());
        assert!(!ui.zone_invalid(sheet.as_deref(), ZoneAxis::Vertical));
    }

    /// A count over the limit is clamped and says so, on its own sheet only.
    #[test]
    fn a_zone_count_over_the_limit_says_it_was_clamped() {
        let sheet = Some(PathBuf::from("a.snxsch"));
        let max = signex_types::schematic::MAX_ZONES;
        let mut ui = DocumentOptionsUi::default();
        let typed = DocumentOptionsMsg::ZoneCountTyped(ZoneAxis::Horizontal, "999".into());
        let _ = ui.apply(typed, sheet.clone());
        let submitted = DocumentOptionsMsg::ZoneCountSubmitted(ZoneAxis::Horizontal);
        assert_eq!(
            ui.apply(submitted, sheet.clone()),
            Some(SheetFrameEdit::HorizontalZones(max))
        );
        assert_eq!(ui.zone_clamped_to(sheet.as_deref()), Some(max));
        let other = PathBuf::from("b.snxsch");
        assert_eq!(ui.zone_clamped_to(Some(other.as_path())), None);
        let in_range = DocumentOptionsMsg::ZoneCountTyped(ZoneAxis::Horizontal, "5".into());
        let _ = ui.apply(in_range, sheet.clone());
        assert_eq!(ui.zone_clamped_to(sheet.as_deref()), None);
    }
}
