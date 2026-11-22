//! A dropdown that also takes typing (mockup rows 40 and 51): a transparent
//! text box lies over a pick list's field and leaves its chevron free, so a
//! press on the chevron opens the list at the field's full width.

use iced::widget::{Space, row, stack};
use iced::{Alignment, Background, Color, Element, Length, Theme, Widget as _};
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::{Label, text, text_input};

use super::PanelMsg;
use super::form_look::{self, FormLook};
use crate::styles::{Spacing, TypeScale};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Room the pick list's chevron keeps free of the text box.
pub(in crate::panels) const CHEVRON_ROOM: f32 = TS.body + 2.0 * SP.s;

/// What the box shows.
pub(in crate::panels) struct TypedPick<'a> {
    pub placeholder: Label<'a>,
    pub shown: String,
    /// The error ink while the box holds text it refuses.
    pub error: Option<Color>,
    /// The unit tag at the box's right.
    pub unit: Option<Label<'a>>,
}

impl<'a> TypedPick<'a> {
    /// The control: typing reports `on_input`, Enter `on_submit`, a row of
    /// the list `on_pick`.
    pub(in crate::panels) fn view<T: PartialEq + Clone + 'a>(
        self,
        options: Vec<T>,
        option_text: impl Fn(&T) -> String + 'a,
        on_input: impl Fn(String) -> PanelMsg + 'a,
        on_submit: PanelMsg,
        on_pick: impl Fn(T) -> PanelMsg + 'a,
    ) -> Element<'a, PanelMsg> {
        let field = iced::widget::pick_list(None::<T>, options, option_text)
            .graphite()
            .on_select(on_pick)
            .text_size(TS.body)
            .padding(SP.s)
            .width(Length::Fill);
        let input = text_input(self.placeholder, self.shown)
            .on_input(on_input)
            .on_submit(on_submit)
            .size(TS.body)
            .padding(SP.s)
            .width(Length::Fill)
            .style(over_field_style(self.error));
        let mut over = row![input].align_y(Alignment::Center).width(Length::Fill);
        if let Some(unit) = self.unit {
            over = over.push(text(unit).size(TS.caption).style(|theme: &Theme| {
                iced::widget::text::Style {
                    color: Some(FormLook::of(theme).ink3),
                    ..Default::default()
                }
            }));
        }
        over = over.push(Space::new().width(CHEVRON_ROOM));
        stack![field, over].width(Length::Fill).boxed()
    }
}

/// The form field look on a clear fill: the pick list under it is the
/// field. Its edge shows only while focused or refusing.
fn over_field_style(
    error: Option<Color>,
) -> impl Fn(&Theme, iced::widget::text_input::Status) -> iced::widget::text_input::Style {
    move |theme, status| {
        let mut style = form_look::refusable_input_style(error)(theme, status);
        style.background = Background::Color(Color::TRANSPARENT);
        let is_focused = matches!(status, iced::widget::text_input::Status::Focused { .. });
        if error.is_none() && !is_focused {
            style.border.color = Color::TRANSPARENT;
        }
        style
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_i18n::UserText;

    fn control<'a>() -> Element<'a, PanelMsg> {
        TypedPick {
            placeholder: UserText::new(String::new()).into(),
            shown: "1.27".into(),
            error: None,
            unit: None,
        }
        .view(
            vec!["2.54".to_string()],
            String::clone,
            |_| PanelMsg::Noop,
            PanelMsg::Noop,
            |_| PanelMsg::Noop,
        )
    }

    /// The box stops short of the chevron, so a press there opens the list.
    #[test]
    fn the_text_box_leaves_the_chevron_free() {
        let inputs = crate::layout_probe::lay_out_inputs(control(), iced::Size::new(200.0, 40.0));
        assert_eq!(inputs.len(), 1, "{inputs:?}");
        let right = inputs[0].bounds.x + inputs[0].bounds.width;
        assert!(right <= 200.0 - CHEVRON_ROOM + 0.5, "{inputs:?}");
    }

    /// A refused box shows the error edge; an idle one shows the field's.
    #[test]
    fn only_a_focused_or_refusing_box_draws_an_edge() {
        let status = iced::widget::text_input::Status::Active;
        let idle = over_field_style(None)(&Theme::Dark, status);
        assert_eq!(idle.border.color, Color::TRANSPARENT);
        let refused = over_field_style(Some(Color::BLACK))(&Theme::Dark, status);
        assert_eq!(refused.border.color, Color::BLACK);
    }
}
