//! The header of a Properties panel: the kind of the object as the title, a
//! muted line the caller supplies (`net /SPI_CLK`, `covers 6 objects`, a
//! part's description) beside it (epic #2523). A kind's own banner (a part's
//! library state, a sheet symbol's missing file) is built by that kind.
//!
//! The single selection builds it through [`selection_header`]: a placed
//! part reads "Component" with its description; every other object shows
//! the engine's type name.

use super::super::*;
use crate::panels::form_look::FormLook;
use crate::styles::{Spacing, TypeScale};
use iced::{Alignment, Padding};
use signex_i18n::{UserText, ids, t};
use signex_widgets::Label;

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Header padding at a 12 px body: 8 px above, 6 px below.
const HEADER_PAD_TOP_EM: f32 = 8.0 / 12.0;
const HEADER_PAD_BOTTOM_EM: f32 = 6.0 / 12.0;
/// Smallest header height (36 px at 12 px).
const HEADER_MIN_HEIGHT_EM: f32 = 36.0 / 12.0;

/// The title a placed part's header shows.
pub(super) const COMPONENT_TITLE: &str = "Component";

/// What a Properties header shows.
pub(in crate::panels) struct ObjectHeader<'a> {
    /// The kind of the object (Wire, Net Label, Component).
    pub title: Label<'a>,
    /// The muted line beside the title, worded by the caller.
    pub line: Option<Label<'a>>,
}

impl<'a> ObjectHeader<'a> {
    /// A header with `title` and nothing else.
    pub fn new(title: impl Into<Label<'a>>) -> Self {
        Self {
            title: title.into(),
            line: None,
        }
    }

    /// Sets the muted line; an empty line shows nothing.
    pub fn line(mut self, line: impl Into<Label<'a>>) -> Self {
        let line = line.into();
        self.line = (!line.as_str().trim().is_empty()).then_some(line);
        self
    }
}

/// The title row: the bold kind, the muted line filling the rest.
fn title_row<'a>(header: &mut ObjectHeader<'a>) -> Element<'a, PanelMsg> {
    let title = std::mem::replace(&mut header.title, Label::from(UserText::new(String::new())));
    let mut row = iced::widget::Row::<Element<'a, PanelMsg>>::new()
        .spacing(SP.s + SP.xs)
        .align_y(Alignment::Center)
        .push(
            text(title)
                .size(TS.label)
                .font(iced::Font {
                    weight: iced::font::Weight::Semibold,
                    ..iced::Font::DEFAULT
                })
                .style(|theme: &Theme| iced::widget::text::Style {
                    color: Some(FormLook::of(theme).ink),
                    ..Default::default()
                })
                .wrapping(iced::widget::text::Wrapping::None)
                .boxed(),
        );
    row = match header.line.take() {
        Some(line) => row.push(
            text(line)
                .size(TS.caption)
                .style(|theme: &Theme| iced::widget::text::Style {
                    color: Some(FormLook::of(theme).ink4),
                    ..Default::default()
                })
                .wrapping(iced::widget::text::Wrapping::None)
                .ellipsis(iced::widget::text::Ellipsis::End)
                .width(Length::Fill)
                .boxed(),
        ),
        None => row.push(Space::new().width(Length::Fill).boxed()),
    };
    container(row)
        .padding(Padding {
            top: TS.body * HEADER_PAD_TOP_EM,
            right: SP.s,
            bottom: TS.body * HEADER_PAD_BOTTOM_EM,
            left: SP.m,
        })
        .height(Length::Shrink)
        .width(Length::Fill)
        .align_y(iced::alignment::Vertical::Center)
        .boxed()
}

/// A Properties header: the title row, never taller than the content needs
/// but at least the mockup's 36 px row.
pub(in crate::panels) fn object_header<'a>(mut header: ObjectHeader<'a>) -> Element<'a, PanelMsg> {
    let title = container(title_row(&mut header))
        .height(Length::Shrink)
        .width(Length::Fill);
    Column::<Element<'a, PanelMsg>>::new()
        .width(Length::Fill)
        .push(
            iced::widget::row![Space::new().height(TS.body * HEADER_MIN_HEIGHT_EM), title,]
                .align_y(Alignment::Center)
                .width(Length::Fill)
                .boxed(),
        )
        .boxed()
}

/// Title and the muted line beside it. A placed part is a "Component" with
/// its description; anything else keeps `elem_type` alone.
pub(super) fn header_text<'s>(
    elem_type: &'s str,
    is_placed_part: bool,
    description: &'s str,
) -> (&'s str, Option<&'s str>) {
    if is_placed_part {
        let line = description.trim();
        (COMPONENT_TITLE, (!line.is_empty()).then_some(line))
    } else {
        (elem_type, None)
    }
}

/// The header of the single selection. No "1 of N" counter: this view shows
/// one object, and a count is shown only where the panel steps through a
/// real list. `_primary` stays for the callers; the look comes from the form.
pub(super) fn selection_header<'a>(
    ctx: &PanelContext,
    elem_type: &str,
    is_placed_part: bool,
    description: &str,
    _primary: Color,
) -> Element<'a, PanelMsg> {
    let (title, line) = header_text(elem_type, is_placed_part, description);
    // The engine's type name has no catalog message yet (#2060).
    let title: Label<'a> = if is_placed_part {
        t!(ctx.i18n, ids::PANELS_PROPERTIES_COMPONENT).into()
    } else {
        raw(title.to_owned())
    };
    let mut header = ObjectHeader::new(title);
    if let Some(line) = line {
        header = header.line(UserText::new(line.to_owned()));
    }
    object_header(header)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_placed_part_is_a_component_with_its_description() {
        assert_eq!(
            header_text("Symbol", true, "Resistor (IEC rectangle). "),
            ("Component", Some("Resistor (IEC rectangle)."))
        );
    }

    #[test]
    fn a_part_without_a_description_shows_only_the_title() {
        assert_eq!(header_text("Symbol", true, "  "), ("Component", None));
    }

    #[test]
    fn other_objects_keep_the_type_name() {
        assert_eq!(header_text("Wire", false, ""), ("Wire", None));
    }

    fn texts(element: Element<'_, PanelMsg>) -> Vec<String> {
        crate::layout_probe::lay_out_texts(element, iced::Size::new(320.0, 400.0))
            .into_iter()
            .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
            .collect()
    }

    /// The muted line the caller words sits beside the title (mockup row
    /// 43, a wire: `net /SPI_CLK`); an empty line draws nothing.
    #[test]
    fn the_header_shows_the_callers_muted_line_beside_the_title() {
        let header = ObjectHeader::new(UserText::new("Wire".to_owned()))
            .line(UserText::new("net /SPI_CLK".to_owned()));
        assert_eq!(texts(object_header(header)), ["Wire", "net /SPI_CLK"]);
        let bare = ObjectHeader::new(UserText::new("Wire".to_owned()))
            .line(UserText::new("  ".to_owned()));
        assert_eq!(texts(object_header(bare)), ["Wire"]);
    }
}
