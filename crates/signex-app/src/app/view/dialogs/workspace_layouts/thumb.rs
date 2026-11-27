//! The arrangement thumbnail of a workspace layout (mockup row 166): a sunken frame
//! with one block per dock region in proportion to its width, and the
//! document area as a panel-coloured block with a border. A region's tabs
//! share its block, as they share the dock.

use iced::widget::{Column, Row, Space, container};
use iced::{Background, Border, Element, Length, Theme, Widget as _};
use signex_types::theme::ThemeTokens;

use crate::app::Message;
use crate::dock::{DockLayoutSnapshot, RegionLayout};
use crate::styles::{Radii, ti, tokens};

/// Gap between the blocks and around them, as in the mock.
const GAP: f32 = 3.0;
/// Corner radius of a block.
const BLOCK_RADIUS: f32 = 2.0;
/// The document area's share against a side dock's width in px.
const DOCUMENT_PORTION: u16 = 560;
/// A collapsed side dock's share: a thin strip.
const COLLAPSED_PORTION: u16 = 40;
/// Document area over bottom dock, about three to one (74 % / 24 % in the mock).
const DOCUMENT_ROWS: u16 = 3;
const BOTTOM_ROWS: u16 = 1;

/// A dock region block: the strong line colour.
fn region_block<'a>(tokens: &ThemeTokens, width: Length, height: Length) -> Element<'a, Message> {
    let fill = tokens::resolve(tokens).line_strong;
    container(Space::new())
        .width(width)
        .height(height)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                radius: BLOCK_RADIUS.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// The document area: panel fill with a strong-line border.
fn document_block<'a>(tokens: &ThemeTokens, height: Length) -> Element<'a, Message> {
    let fill = ti(tokens.panel_bg);
    let line = tokens::resolve(tokens).line_strong;
    container(Space::new())
        .width(Length::Fill)
        .height(height)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                width: 1.0,
                color: line,
                radius: BLOCK_RADIUS.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// A side dock's share of the width, or `None` when it holds no panel.
fn side_portion(region: &RegionLayout) -> Option<u16> {
    if region.panels.is_empty() {
        return None;
    }
    if region.collapsed {
        return Some(COLLAPSED_PORTION);
    }
    let width = region
        .width
        .unwrap_or(crate::dock::DEFAULT_LEFT_DOCK_WIDTH as u32);
    Some(width.clamp(1, u32::from(u16::MAX)) as u16)
}

/// The thumbnail of `dock` (`None`: a layout that stores no panels, only the
/// document area), `width` wide and `height` px tall.
pub(super) fn thumbnail<'a>(
    dock: Option<&DockLayoutSnapshot>,
    width: Length,
    height: f32,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let empty = RegionLayout {
        panels: Vec::new(),
        collapsed: false,
        width: None,
        kept: Vec::new(),
    };
    let (left, right, bottom) = match dock {
        Some(d) => (&d.left, &d.right, &d.bottom),
        None => (&empty, &empty, &empty),
    };
    let mut centre: Column<Element<'a, Message>> = Column::new().spacing(GAP).width(Length::Fill);
    if bottom.panels.is_empty() {
        centre = centre.push(document_block(tokens, Length::Fill));
    } else {
        // A collapsed bottom dock is a thin strip under the documents.
        let bottom_rows = if bottom.collapsed {
            BOTTOM_ROWS
        } else {
            BOTTOM_ROWS * 3
        };
        let document_rows = if bottom.collapsed {
            DOCUMENT_ROWS * 6
        } else {
            DOCUMENT_ROWS * 3
        };
        centre = centre
            .push(document_block(tokens, Length::FillPortion(document_rows)))
            .push(region_block(
                tokens,
                Length::Fill,
                Length::FillPortion(bottom_rows),
            ));
    }
    let mut line: Row<Element<'a, Message>> = Row::new().spacing(GAP).height(Length::Fill);
    if let Some(portion) = side_portion(left) {
        line = line.push(region_block(
            tokens,
            Length::FillPortion(portion),
            Length::Fill,
        ));
    }
    line = line.push(
        container(centre)
            .width(Length::FillPortion(DOCUMENT_PORTION))
            .height(Length::Fill)
            .boxed(),
    );
    if let Some(portion) = side_portion(right) {
        line = line.push(region_block(
            tokens,
            Length::FillPortion(portion),
            Length::Fill,
        ));
    }
    let r = tokens::resolve(tokens);
    let (sunken, frame) = (r.sunken, r.line_strong);
    container(line)
        .padding(GAP)
        .width(width)
        .height(Length::Fixed(height))
        .clip(true)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(sunken)),
            border: Border {
                width: 1.0,
                color: frame,
                radius: Radii::GRAPHITE.small.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}
