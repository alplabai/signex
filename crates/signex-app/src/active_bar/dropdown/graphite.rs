//! Graphite paint of the Active Bar's irregular dropdowns: the Selection
//! Filter's toggle chips and All / None segmented control, and the
//! net-colour swatches. Legacy themes never reach this module.

use iced::widget::{button, container, row, text};
use iced::{Background, Border, Color, Element, Theme, Widget};
use signex_types::theme::ThemeTokens;

use crate::active_bar::ActiveBarMsg;
use crate::styles::{self, Radii, tokens};

const TS: styles::TypeScale = styles::TypeScale::DEFAULT;
const SP: styles::Spacing = styles::Spacing::DEFAULT;

/// Corner radius of the segmented control's frame.
const SEGMENT_FRAME_RADIUS: f32 = 8.0;
/// Inset of the segment cells inside their frame.
const SEGMENT_INSET: f32 = 2.0;
/// Side and corner radius of a net-colour swatch (the mock's swatch).
pub(super) const SWATCH_SIZE: f32 = 12.0;
const SWATCH_RADIUS: f32 = 3.0;

/// Colours of one filter chip.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct ChipPaint {
    pub fill: Option<Color>,
    pub ink: Color,
    pub border: Color,
}

impl ChipPaint {
    /// ON: soft accent fill, its ink and an accent hairline. OFF:
    /// transparent (the hover wash under the pointer), a line hairline and
    /// the secondary ink.
    pub(super) fn of(t: &ThemeTokens, on: bool, hovered: bool) -> Self {
        let r = tokens::resolve(t);
        if on {
            Self {
                fill: Some(r.accent_soft),
                ink: r.accent_soft_ink,
                border: tokens::accent(t),
            }
        } else {
            Self {
                fill: hovered.then(|| tokens::hover_overlay(t)),
                ink: tokens::muted_text(t),
                border: styles::ti(t.border),
            }
        }
    }
}

/// Which cell of the All / None control is lit for a filter set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FilterScope {
    All,
    None,
    /// Some categories on, some off: neither cell is lit.
    Mixed,
}

impl FilterScope {
    pub(super) fn of(on: usize, total: usize) -> Self {
        match on {
            0 => Self::None,
            n if n >= total => Self::All,
            _ => Self::Mixed,
        }
    }
}

fn chip_style(
    t: &ThemeTokens,
    on: bool,
) -> impl Fn(&Theme, button::Status) -> button::Style + use<> {
    let (rest, hover) = (ChipPaint::of(t, on, false), ChipPaint::of(t, on, true));
    move |_: &Theme, status: button::Status| {
        let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
        let p = if hovered { hover } else { rest };
        button::Style {
            background: p.fill.map(Background::Color),
            text_color: p.ink,
            border: Border {
                width: 1.0,
                radius: Radii::GRAPHITE.small.into(),
                color: p.border,
                ..Border::default()
            },
            ..button::Style::default()
        }
    }
}

/// One toggle chip (a filter category, or a preset shown as an OFF chip).
pub(super) fn chip(
    t: &ThemeTokens,
    label: String,
    on: bool,
    msg: ActiveBarMsg,
) -> Element<'static, ActiveBarMsg> {
    let ink = ChipPaint::of(t, on, false).ink;
    button(text(label).size(TS.body).color(ink))
        .padding([SP.s, 10.0])
        .on_press(msg)
        .style(chip_style(t, on))
        .boxed()
}

fn segment_style(
    t: &ThemeTokens,
    active: bool,
) -> impl Fn(&Theme, button::Status) -> button::Style + use<> {
    let r = tokens::resolve(t);
    let (raised, soft, ink, ink3) = (r.raised, r.accent_soft, styles::ti(t.text), r.ink3);
    move |_: &Theme, status: button::Status| {
        let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
        let fill = if active {
            Some(Background::Color(raised))
        } else if hovered {
            Some(Background::Color(soft))
        } else {
            None
        };
        button::Style {
            background: fill,
            text_color: if active { ink } else { ink3 },
            border: Border {
                radius: Radii::GRAPHITE.small.into(),
                ..Border::default()
            },
            ..button::Style::default()
        }
    }
}

/// The All / None segmented control that replaces the legacy "All - On"
/// toggle: All turns every category on, None turns every one off.
pub(super) fn all_none(t: &ThemeTokens, scope: FilterScope) -> Element<'static, ActiveBarMsg> {
    let r = tokens::resolve(t);
    let sunken = r.sunken;
    let (ink, ink3) = (styles::ti(t.text), r.ink3);
    let cell = |label: &'static str, active: bool, msg: ActiveBarMsg| {
        button(
            text(label)
                .size(TS.body)
                .color(if active { ink } else { ink3 }),
        )
        .padding([SP.xs, SP.l])
        .on_press(msg)
        .style(segment_style(t, active))
        .boxed()
    };
    let cells: iced::widget::Row<Element<'static, ActiveBarMsg>> = row![
        cell(
            "All",
            scope == FilterScope::All,
            ActiveBarMsg::AllFiltersPicked
        ),
        cell(
            "None",
            scope == FilterScope::None,
            ActiveBarMsg::NoFiltersPicked
        ),
    ]
    .spacing(SEGMENT_INSET);
    container(cells)
        .padding(SEGMENT_INSET)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(sunken)),
            border: Border {
                radius: SEGMENT_FRAME_RADIUS.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// A net-colour swatch: 12 px, radius 3, strong hairline.
pub(super) fn swatch_style(
    t: &ThemeTokens,
    color: Color,
) -> impl Fn(&Theme) -> container::Style + use<> {
    let edge = tokens::resolve(t).line_strong;
    move |_: &Theme| container::Style {
        background: Some(Background::Color(color)),
        border: Border {
            width: 1.0,
            radius: SWATCH_RADIUS.into(),
            color: edge,
            ..Border::default()
        },
        ..container::Style::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::theme::{ThemeId, theme_tokens};

    fn hex(c: Color) -> [u8; 3] {
        let b = |v: f32| (v * 255.0).round() as u8;
        [b(c.r), b(c.g), b(c.b)]
    }

    /// The maintainer's spec: ON = accent_soft fill, accent ink, accent
    /// hairline; OFF = transparent, line hairline, secondary ink.
    #[test]
    fn chip_paint_follows_the_graphite_tokens() {
        let t = theme_tokens(ThemeId::GraphiteLight);
        let on = ChipPaint::of(&t, true, false);
        assert_eq!(on.fill.map(hex), Some([0xF3, 0xE6, 0xDF]));
        assert_eq!(hex(on.ink), [0x7E, 0x32, 0x1D]);
        assert_eq!(hex(on.border), [0xC2, 0x55, 0x2D]);
        let off = ChipPaint::of(&t, false, false);
        assert_eq!(off.fill, None);
        assert_eq!(hex(off.border), [0xE4, 0xE2, 0xDC]);
        assert_eq!(hex(off.ink), [0x5B, 0x5E, 0x66]);
        assert!(
            ChipPaint::of(&t, false, true).fill.is_some(),
            "OFF hover wash"
        );
    }

    #[test]
    fn chip_paint_reads_in_graphite_dark() {
        let t = theme_tokens(ThemeId::GraphiteDark);
        let on = ChipPaint::of(&t, true, false);
        assert_eq!(on.fill.map(hex), Some([0x3A, 0x24, 0x18]));
        assert_eq!(hex(on.ink), [0xF0, 0xB0, 0x8E]);
    }

    /// Both Graphite filter and net-colour menus keep the legacy entry
    /// shape (one chip grid; seven swatches, then the Custom / Clear rows).
    #[test]
    fn graphite_menus_keep_their_entries() {
        use signex_widgets::active_bar_dropdown::DropdownEntry;
        for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
            let t = theme_tokens(id);
            let filters: std::collections::HashSet<_> = crate::active_bar::SelectionFilter::ALL
                .iter()
                .copied()
                .collect();
            let entry = super::super::custom::filter_entry(&t, &filters, &[]);
            assert!(matches!(entry, DropdownEntry::Custom(_)));
            let rows = super::super::custom::net_color_entries(&t, id, false, true);
            let swatches = rows
                .iter()
                .filter(|e| matches!(e, DropdownEntry::Custom(_)))
                .count();
            assert_eq!(swatches, 7);
            assert_eq!(rows.len(), 12);
        }
    }

    #[test]
    fn filter_scope_lights_all_none_or_neither() {
        assert_eq!(FilterScope::of(0, 12), FilterScope::None);
        assert_eq!(FilterScope::of(12, 12), FilterScope::All);
        assert_eq!(FilterScope::of(5, 12), FilterScope::Mixed);
    }
}
