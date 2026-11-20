//! Compact group: a collapsible group header over member rows, each with
//! its Δ from the group's target and a small [`target_band`] bar. The
//! length-matching tab and its dock panel list their groups with it; the
//! polygon list reuses the header.
//!
//! Stateless: the caller owns whether a group is open and which member is
//! selected, and maps a header click to `on_toggle(group)` and a row click
//! to `on_select(member)`. Every text arrives as a [`Label`]; lengths are mm
//! shown through [`LengthFormat`]. Every length is an em of the style's text
//! size, authored at 12 px.
//!
//! [`target_band`]: crate::target_band

mod header;
mod member_row;
#[cfg(test)]
mod tests;

use iced::widget::{Column, container};
use iced::{Background, Color, Element, Length, Theme, Widget};
use signex_types::theme::ThemeTokens;

use crate::chips::{ChipMetrics, ChipTone};
use crate::target_band::{TargetBandStyle, band_geometry};
use crate::theme_ext::{HAIRLINE, TypeScale, to_color};
use crate::{Label, LengthFormat};

/// How a group's status pill reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusTone {
    /// Every member in band.
    Ok,
    /// At least one member out of band.
    Err,
}

/// The pill at the end of a group header.
#[derive(Debug, Clone)]
pub struct StatusPill<'a> {
    pub label: Label<'a>,
    pub tone: StatusTone,
}

/// The header of one group: its name, caption fields (member count,
/// target ± tolerance, …) and an optional status pill.
#[derive(Debug, Clone)]
pub struct GroupHeader<'a> {
    pub name: Label<'a>,
    pub captions: Vec<Label<'a>>,
    pub status: Option<StatusPill<'a>>,
}

/// One member: a net, a pair half or an xSignal, with its length against
/// the group's target and tolerance (all mm).
#[derive(Debug, Clone)]
pub struct Member<'a, K> {
    pub key: K,
    pub name: Label<'a>,
    /// A short note after the name ("pair USB_D", "xSignal").
    pub caption: Option<Label<'a>>,
    pub length: f64,
    pub target: f64,
    pub tolerance: f64,
    pub is_selected: bool,
}

impl<K> Member<'_, K> {
    /// Signed distance from the target, in mm.
    pub fn delta(&self) -> f64 {
        self.length - self.target
    }

    /// Whether the member is outside the target band.
    pub fn is_out_of_band(&self) -> bool {
        band_geometry(self.length, self.target, self.tolerance).is_out_of_band
    }
}

/// How many members are out of band, for the header pill.
pub fn out_of_band_count<K>(members: &[Member<'_, K>]) -> usize {
    members.iter().filter(|m| m.is_out_of_band()).count()
}

/// Lengths of the header and the rows, in px at the style's text size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GroupMetrics {
    pub header_height: f32,
    /// Header padding (vertical, horizontal).
    pub header_pad: (f32, f32),
    /// Gap between the header's fields.
    pub header_gap: f32,
    pub row_height: f32,
    pub row_pad: f32,
    /// Width of the selected row's accent bar.
    pub accent_bar: f32,
    /// Width of the Δ column.
    pub delta_width: f32,
    /// Width of the mini bar.
    pub band_width: f32,
    /// Gap between a row's columns.
    pub column_gap: f32,
}

impl GroupMetrics {
    pub fn new(text_size: f32) -> Self {
        let em = |px: f32| px * text_size / 12.0;
        Self {
            header_height: em(34.0),
            header_pad: (em(4.0), em(10.0)),
            header_gap: em(12.0),
            row_height: em(28.0),
            row_pad: em(10.0),
            accent_bar: em(2.0),
            delta_width: em(48.0),
            band_width: em(92.0),
            column_gap: em(6.0),
        }
    }
}

/// Colours of the header and rows, the bar style and the text sizes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CompactGroupStyle {
    pub text_size: f32,
    pub caption_size: f32,
    pub sunken: Color,
    pub line_soft: Color,
    pub hover: Color,
    pub accent: Color,
    pub accent_soft: Color,
    pub ink: Color,
    pub ink3: Color,
    pub ink4: Color,
    pub err: Color,
    pub ok_pill: ChipTone,
    pub err_pill: ChipTone,
    pub band: TargetBandStyle,
}

impl CompactGroupStyle {
    /// Graphite tokens; an unset extended token falls back to the nearest
    /// base token.
    pub fn from_tokens(tokens: &ThemeTokens, text_size: f32) -> Self {
        let ext = &tokens.extended;
        let or = |c: Option<signex_types::theme::Color>, base| to_color(&c.unwrap_or(base));
        Self {
            text_size,
            caption_size: TypeScale::from_base(text_size).caption,
            sunken: or(ext.sunken, tokens.panel_bg),
            line_soft: or(ext.line_soft, tokens.border),
            hover: to_color(&tokens.hover),
            accent: to_color(&tokens.accent),
            accent_soft: or(ext.accent_soft, tokens.selection),
            ink: to_color(&tokens.text),
            ink3: or(ext.ink3, tokens.text_secondary),
            ink4: or(ext.ink4, tokens.text_secondary),
            err: to_color(&tokens.error),
            ok_pill: ChipTone {
                fill: or(ext.ok_soft, tokens.hover),
                ink: to_color(&tokens.success),
            },
            err_pill: ChipTone {
                fill: or(ext.err_soft, tokens.hover),
                ink: to_color(&tokens.error),
            },
            band: TargetBandStyle::from_tokens(tokens, text_size),
        }
    }

    pub fn metrics(&self) -> GroupMetrics {
        GroupMetrics::new(self.text_size)
    }

    fn chip_metrics(&self) -> ChipMetrics {
        ChipMetrics::from_text_size(self.text_size)
    }

    fn pill_tone(&self, tone: StatusTone) -> ChipTone {
        match tone {
            StatusTone::Ok => self.ok_pill,
            StatusTone::Err => self.err_pill,
        }
    }
}

/// One group: its header and, while open, its member rows. Build with
/// [`CompactGroup::new`], then call [`CompactGroup::view`].
pub struct CompactGroup<'a, G, K, M> {
    group: G,
    header: GroupHeader<'a>,
    members: Vec<Member<'a, K>>,
    is_open: bool,
    on_toggle: Box<dyn Fn(G) -> M + 'a>,
    on_select: Box<dyn Fn(K) -> M + 'a>,
}

impl<'a, G, K, M> CompactGroup<'a, G, K, M>
where
    G: Clone + 'a,
    K: Clone + 'a,
    M: Clone + 'a,
{
    /// `is_open` is the caller's value; a header click sends
    /// `on_toggle(group)` and a row click `on_select(member key)`.
    pub fn new(
        group: G,
        header: GroupHeader<'a>,
        members: Vec<Member<'a, K>>,
        is_open: bool,
        on_toggle: impl Fn(G) -> M + 'a,
        on_select: impl Fn(K) -> M + 'a,
    ) -> Self {
        Self {
            group,
            header,
            members,
            is_open,
            on_toggle: Box::new(on_toggle),
            on_select: Box::new(on_select),
        }
    }

    /// What a click on the header sends.
    pub fn toggle_message(&self) -> M {
        (self.on_toggle)(self.group.clone())
    }

    /// What a click on member `index` sends; `None` past the last member.
    pub fn select_message(&self, index: usize) -> Option<M> {
        self.members
            .get(index)
            .map(|m| (self.on_select)(m.key.clone()))
    }

    /// The header, then the rows when the group is open.
    pub fn view(self, format: &LengthFormat, style: CompactGroupStyle) -> Element<'a, M> {
        let toggle = self.toggle_message();
        let mut column: Column<Element<'a, M>> = Column::new().width(Length::Fill);
        column = column.push(header::view(&self.header, self.is_open, toggle, style));
        if self.is_open {
            for member in &self.members {
                let select = (self.on_select)(member.key.clone());
                column = column.push(member_row::view(member, format, select, style));
            }
        }
        column.boxed()
    }
}

/// A full-width `line-soft` rule under a header or row.
fn bottom_rule<'a, M: 'a>(color: Color) -> Element<'a, M> {
    container(iced::widget::Space::new())
        .width(Length::Fill)
        .height(HAIRLINE)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(color)),
            ..container::Style::default()
        })
        .boxed()
}
