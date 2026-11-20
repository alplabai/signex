//! The shell's sub-tab strip: the pages of one document tab (Stack,
//! Materials, Finish, …) in a row that scrolls sideways with no visible
//! scrollbar. The active sub-tab reads in the ink colour at medium weight,
//! with an accent underline on the strip's bottom line.

use iced::alignment::Vertical;
use iced::widget::scrollable::{Direction, Scrollbar};
use iced::widget::{Row, Space, container, scrollable, stack};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding, Theme, Widget};

use super::{DocTabShellStyle, filled, medium};
use crate::theme_ext::HAIRLINE;
use crate::{Label, button, text};

/// One sub-tab: its label and an optional trailing tag (a "proposal" kind
/// chip, a count badge).
pub struct SubTab<'a, M> {
    pub label: Label<'a>,
    pub tag: Option<Element<'a, M>>,
}

impl<'a, M> SubTab<'a, M> {
    pub fn new(label: impl Into<Label<'a>>) -> Self {
        Self {
            label: label.into(),
            tag: None,
        }
    }

    pub fn tag(mut self, tag: Element<'a, M>) -> Self {
        self.tag = Some(tag);
        self
    }
}

/// How one sub-tab is drawn.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SubTabLook {
    pub ink: Color,
    pub is_medium: bool,
    pub has_underline: bool,
}

impl SubTabLook {
    fn of(is_active: bool, s: &DocTabShellStyle) -> Self {
        Self {
            ink: if is_active { s.ink } else { s.ink2 },
            is_medium: is_active,
            has_underline: is_active,
        }
    }
}

type OnSelect<'a, M> = Box<dyn Fn(usize) -> M + 'a>;

/// The strip. `selected` is the caller's value; a press on sub-tab `i` sends
/// `on_select(i)`. Without a handler the sub-tabs are not pressable.
pub struct SubTabs<'a, M> {
    tabs: Vec<SubTab<'a, M>>,
    selected: usize,
    on_select: Option<OnSelect<'a, M>>,
}

impl<'a, M: Clone + 'a> SubTabs<'a, M> {
    pub fn new(tabs: Vec<SubTab<'a, M>>, selected: usize) -> Self {
        Self {
            tabs,
            selected,
            on_select: None,
        }
    }

    /// Map a press on sub-tab `index` to the caller's message.
    pub fn on_select(mut self, on_select: impl Fn(usize) -> M + 'a) -> Self {
        self.on_select = Some(Box::new(on_select));
        self
    }

    pub fn len(&self) -> usize {
        self.tabs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    /// Whether sub-tab `index` is the selected one.
    pub fn is_active(&self, index: usize) -> bool {
        index == self.selected && index < self.tabs.len()
    }

    /// The message a press on sub-tab `index` sends; `None` without a
    /// handler or for an index past the end. Pressing the active sub-tab
    /// sends it too, so the caller decides whether that does anything.
    pub fn message_for(&self, index: usize) -> Option<M> {
        if index >= self.tabs.len() {
            return None;
        }
        self.on_select.as_ref().map(|f| f(index))
    }

    /// How sub-tab `index` is drawn.
    pub fn look(&self, index: usize, s: &DocTabShellStyle) -> SubTabLook {
        SubTabLook::of(self.is_active(index), s)
    }

    pub(super) fn view(self, s: &DocTabShellStyle) -> Element<'a, M> {
        let m = s.metrics;
        let looks: Vec<SubTabLook> = (0..self.tabs.len()).map(|i| self.look(i, s)).collect();
        let messages: Vec<Option<M>> = (0..self.tabs.len()).map(|i| self.message_for(i)).collect();
        let mut strip: Row<Element<'a, M>> = Row::new().spacing(m.sub_gap);
        for ((tab, look), message) in self.tabs.into_iter().zip(looks).zip(messages) {
            strip = strip.push(sub_tab(tab, look, message, s));
        }
        let row = scrollable(strip.padding(Padding::from([0.0, m.pad_x])))
            .direction(Direction::Horizontal(Scrollbar::hidden()))
            .width(Length::Fill);
        let line = s.border;
        let baseline = container(
            container(Space::new().width(Length::Fill).height(HAIRLINE))
                .width(Length::Fill)
                .style(move |_: &Theme| filled(line)),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_y(Vertical::Bottom);
        container(stack![baseline, row])
            .width(Length::Fill)
            .height(m.sub_height + HAIRLINE)
            .boxed()
    }
}

/// One sub-tab: the pressable face, and under the active one the accent
/// underline.
fn sub_tab<'a, M: Clone + 'a>(
    tab: SubTab<'a, M>,
    look: SubTabLook,
    message: Option<M>,
    s: &DocTabShellStyle,
) -> Element<'a, M> {
    let m = s.metrics;
    let mut label = text(tab.label)
        .size(m.text_size)
        .color(look.ink)
        .wrapping(iced::widget::text::Wrapping::None);
    if look.is_medium {
        label = label.font(medium());
    }
    let mut content: Row<Element<'a, M>> = Row::new()
        .spacing(m.sub_inner_gap)
        .align_y(Alignment::Center)
        .height(Length::Fill)
        .push(label.boxed());
    if let Some(tag) = tab.tag {
        content = content.push(tag);
    }
    let hover = s.hover;
    let ink = look.ink;
    let face = button(content)
        .height(m.sub_height)
        .padding(Padding::from([0.0, m.sub_pad_x]))
        .on_press_maybe(message)
        .style(move |_: &Theme, status: button::Status| button::Style {
            background: matches!(status, button::Status::Hovered | button::Status::Pressed)
                .then_some(Background::Color(hover)),
            text_color: ink,
            ..button::Style::default()
        });
    // The tab stands one hairline taller than its face, so the underline
    // sits on the strip's bottom line and hides it under the active tab.
    // The first layer of a stack sets its size, so the face goes in first.
    let base = container(face).height(m.sub_height + HAIRLINE);
    if !look.has_underline {
        return base.boxed();
    }
    let accent = s.accent;
    let radius = m.underline_radius;
    let underline = container(Space::new().width(Length::Fill).height(m.underline))
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(accent)),
            border: Border {
                radius: radius.into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
    let underline_layer = container(underline)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(Padding::from([0.0, m.underline_inset]))
        .align_y(Vertical::Bottom);
    stack![base, underline_layer].boxed()
}
