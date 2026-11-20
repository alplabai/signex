//! The shell's footer: a status line on the left ("4 copper layers ·
//! 13 rows") and an optional note at the right edge ("Changes apply when you
//! save the board."), in caption text on the chrome band.

use iced::widget::{Column, Row, Space, container};
use iced::{Alignment, Element, Length, Padding, Theme, Widget};

use super::{DocTabShellStyle, filled, rule};
use crate::{Label, text};

/// The footer's text. An empty status or note is not drawn.
#[derive(Debug, Clone, Default)]
pub struct Footer<'a> {
    status: Option<Label<'a>>,
    note: Option<Label<'a>>,
}

impl<'a> Footer<'a> {
    pub fn new(status: impl Into<Label<'a>>) -> Self {
        Self {
            status: Some(status.into()),
            note: None,
        }
    }

    /// The note at the right edge.
    pub fn note(mut self, note: impl Into<Label<'a>>) -> Self {
        self.note = Some(note.into());
        self
    }

    pub fn has_status(&self) -> bool {
        self.status.as_ref().is_some_and(|l| !l.as_str().is_empty())
    }

    pub fn has_note(&self) -> bool {
        self.note.as_ref().is_some_and(|l| !l.as_str().is_empty())
    }

    pub(super) fn view<M: 'a>(self, s: &DocTabShellStyle) -> Element<'a, M> {
        let m = s.metrics;
        let has_status = self.has_status();
        let has_note = self.has_note();
        let caption = |label: Label<'a>| text(label).size(m.caption_size).color(s.ink3).boxed();
        // A zero-width strut holds the footer at its smallest height; the
        // text grows it when a long note wraps.
        let strut = Space::new()
            .width(0.0)
            .height((m.footer_min_height - 2.0 * m.footer_pad_y).max(0.0));
        let mut line: Row<Element<'a, M>> = Row::new()
            .spacing(m.footer_gap)
            .align_y(Alignment::Center)
            .width(Length::Fill)
            .push(strut.boxed());
        match self.status.filter(|_| has_status) {
            Some(status) => {
                line = line.push(container(caption(status)).width(Length::Fill).boxed());
            }
            None => line = line.push(Space::new().width(Length::Fill).boxed()),
        }
        if let Some(note) = self.note.filter(|_| has_note) {
            line = line.push(caption(note));
        }
        let chrome = s.chrome;
        let band = container(line)
            .width(Length::Fill)
            .padding(Padding::from([m.footer_pad_y, m.pad_x]))
            .style(move |_: &Theme| filled(chrome));
        Column::new()
            .width(Length::Fill)
            .push(rule(s.border))
            .push(band.boxed())
            .boxed()
    }
}
