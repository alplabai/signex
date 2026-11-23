//! The layer dropdown of the status bar's Layer pill, under the Dropdown
//! layer switcher (mockup row 6): one card, the visible layers by group on
//! the left, each with its swatch and the active one filled, the layer sets
//! on the right. A layer row makes that layer active and closes the list.
//! The card takes its full height and scrolls only in a window too short
//! for it.

use iced::font::Weight;
use iced::widget::{Column, Widget, container, row, scrollable};
use iced::{Element, Length};
use signex_i18n::{I18n, MessageId, UserText, ids, t};
use signex_types::pcb_layer::{PcbLayer, TechnicalLayer};
use signex_types::theme::ThemeTokens;
use signex_widgets::active_bar_dropdown::MenuMetrics;
use signex_widgets::text;

use super::super::graphite::{SP, TS};
use super::counters::layer_swatch;
use super::kit::{PX, active_list_row, card, list_row, ui_font};
use super::{BoardPopoverEvent, LayerChoice};
use crate::styles::tokens::resolve;
use crate::styles::{section_label, ti};

/// The two-column card, in the mock's px.
pub(crate) const LAYER_DROPDOWN_WIDTH: f32 = 440.0 * PX;

/// A heading of the dropdown, in the order they are listed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LayerGroup {
    Copper,
    Silkscreen,
    Mask,
    Paste,
    Assembly,
    Mechanical,
    Documentation,
}

impl LayerGroup {
    const ALL: [Self; 7] = [
        Self::Copper,
        Self::Silkscreen,
        Self::Mask,
        Self::Paste,
        Self::Assembly,
        Self::Mechanical,
        Self::Documentation,
    ];

    /// The group `layer` is listed under.
    pub(crate) fn of(layer: PcbLayer) -> Self {
        use TechnicalLayer as T;
        match layer {
            PcbLayer::Copper(_) => Self::Copper,
            PcbLayer::Mechanical(_) => Self::Mechanical,
            PcbLayer::Technical(technical) => match technical {
                T::SilkscreenTop | T::SilkscreenBottom => Self::Silkscreen,
                T::SolderMaskTop | T::SolderMaskBottom => Self::Mask,
                T::PasteTop | T::PasteBottom => Self::Paste,
                T::AdhesiveTop
                | T::AdhesiveBottom
                | T::FabTop
                | T::FabBottom
                | T::AssemblyTop
                | T::AssemblyBottom
                | T::CourtyardTop
                | T::CourtyardBottom
                | T::Body3dTop
                | T::Body3dBottom
                | T::FiducialTop
                | T::FiducialBottom
                | T::DesignatorTop
                | T::DesignatorBottom => Self::Assembly,
                T::EdgeCuts
                | T::Margin
                | T::KeepoutTop
                | T::KeepoutBottom
                | T::Dimensions
                | T::DrillDrawing => Self::Mechanical,
                T::Notes | T::Eco1 | T::Eco2 => Self::Documentation,
            },
        }
    }

    fn title(self) -> MessageId {
        match self {
            Self::Copper => ids::BOARD_STATUS_LAYER_GROUP_COPPER,
            Self::Silkscreen => ids::BOARD_STATUS_LAYER_GROUP_SILKSCREEN,
            Self::Mask => ids::BOARD_STATUS_LAYER_GROUP_MASK,
            Self::Paste => ids::BOARD_STATUS_LAYER_GROUP_PASTE,
            Self::Assembly => ids::BOARD_STATUS_LAYER_GROUP_ASSEMBLY,
            Self::Mechanical => ids::BOARD_STATUS_LAYER_GROUP_MECHANICAL,
            Self::Documentation => ids::BOARD_STATUS_LAYER_GROUP_DOCUMENTATION,
        }
    }
}

/// One heading of the dropdown and its layers.
#[derive(Debug, Clone)]
pub(crate) struct LayerDropdownGroup {
    pub group: LayerGroup,
    pub rows: Vec<LayerChoice>,
}

/// `rows` under their headings, each keeping its order; an empty group is
/// left out.
pub(crate) fn group_layers(rows: Vec<LayerChoice>) -> Vec<LayerDropdownGroup> {
    LayerGroup::ALL
        .into_iter()
        .filter_map(|group| {
            let rows: Vec<LayerChoice> = rows
                .iter()
                .filter(|row| LayerGroup::of(row.layer) == group)
                .cloned()
                .collect();
            (!rows.is_empty()).then_some(LayerDropdownGroup { group, rows })
        })
        .collect()
}

/// The one card: `layers` (from [`layer_column`]) left, `sets` right.
pub(crate) fn two_column_card<'a, M: 'a>(
    layers: Element<'a, M>,
    sets: Element<'a, M>,
    tokens: &ThemeTokens,
) -> Element<'a, M> {
    let columns = row![
        container(layers).width(Length::Fill),
        container(sets).width(Length::Fill),
    ]
    .spacing(SP.s)
    .width(Length::Fill);
    // Shrink height: as tall as both columns, capped only by the window.
    let list = scrollable(columns)
        .width(Length::Fill)
        .direction(crate::styles::thin_vertical());
    card(LAYER_DROPDOWN_WIDTH, vec![list.boxed()], tokens)
}

/// The left column: the layers under their group headings.
pub(super) fn layer_column<'a>(
    groups: &[LayerDropdownGroup],
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, BoardPopoverEvent> {
    let mut lines: Vec<Element<'a, BoardPopoverEvent>> = Vec::new();
    for group in groups {
        lines.push(
            // The sets column's header height, so the two lists start level.
            container(group_heading(group.group, tokens, i18n))
                .padding([0.0, 6.0 * PX])
                .height(MenuMetrics::GRAPHITE.header_row_height())
                .align_y(iced::alignment::Vertical::Center)
                .boxed(),
        );
        lines.extend(group.rows.iter().map(|row| layer_row(row, tokens)));
    }
    Column::with_children(lines).width(Length::Fill).boxed()
}

/// A group heading as drawn: capitals at the section size in the muted ink.
fn group_heading<'a>(
    group: LayerGroup,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, BoardPopoverEvent> {
    let title = t!(i18n, group.title()).into_string();
    text(UserText::new(i18n.upper_for_display(&title)))
        .size(TS.section)
        .style(section_label(tokens))
        .wrapping(iced::widget::text::Wrapping::None)
        .boxed()
}

fn layer_row<'a>(row: &LayerChoice, tokens: &ThemeTokens) -> Element<'a, BoardPopoverEvent> {
    let name = text(row.name.clone())
        .size(TS.body)
        .wrapping(iced::widget::text::Wrapping::None);
    let swatch = layer_swatch(row.colour, tokens);
    let picked = BoardPopoverEvent::LayerPicked(row.layer);
    if row.active {
        let name = name
            .font(ui_font(Weight::Medium))
            .color(resolve(tokens).accent_soft_ink);
        active_list_row(swatch, name.boxed(), picked, tokens)
    } else {
        let name = name.color(ti(tokens.text));
        list_row(Some(swatch), name.boxed(), None, Some(picked), tokens)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::Size;
    use iced::widget::Space;
    use signex_types::pcb_layer::CopperLayer;
    use signex_types::theme::{ThemeId, theme_tokens};

    fn choice(layer: PcbLayer) -> LayerChoice {
        LayerChoice {
            layer,
            name: t!(I18n::en_us(), ids::BOARD_STATUS_LAYER_GROUP_COPPER),
            colour: iced::Color::WHITE,
            active: false,
        }
    }

    /// Rows land under their headings in the heading order, each group
    /// keeping its rows' order, and an empty group is left out.
    #[test]
    fn the_layers_are_listed_by_group() {
        let top = PcbLayer::Copper(CopperLayer::Top);
        let bottom = PcbLayer::Copper(CopperLayer::Bottom);
        let silk = PcbLayer::Technical(TechnicalLayer::SilkscreenTop);
        let mask = PcbLayer::Technical(TechnicalLayer::SolderMaskBottom);
        let paste = PcbLayer::Technical(TechnicalLayer::PasteTop);
        let rows = [paste, top, mask, silk, bottom].map(choice).to_vec();
        let groups: Vec<(LayerGroup, Vec<PcbLayer>)> = group_layers(rows)
            .into_iter()
            .map(|g| (g.group, g.rows.iter().map(|r| r.layer).collect()))
            .collect();
        assert_eq!(
            groups,
            [
                (LayerGroup::Copper, vec![top, bottom]),
                (LayerGroup::Silkscreen, vec![silk]),
                (LayerGroup::Mask, vec![mask]),
                (LayerGroup::Paste, vec![paste]),
            ]
        );
    }

    /// More lines than the old cap, across every group.
    fn layers() -> [PcbLayer; 18] {
        use TechnicalLayer as T;
        let t = PcbLayer::Technical;
        [
            PcbLayer::Copper(CopperLayer::Top),
            PcbLayer::Copper(CopperLayer::Bottom),
            t(T::SilkscreenTop),
            t(T::SilkscreenBottom),
            t(T::SolderMaskTop),
            t(T::SolderMaskBottom),
            t(T::PasteTop),
            t(T::PasteBottom),
            t(T::FabTop),
            t(T::FabBottom),
            t(T::CourtyardTop),
            t(T::CourtyardBottom),
            t(T::EdgeCuts),
            t(T::Margin),
            t(T::Dimensions),
            t(T::Notes),
            t(T::Eco1),
            t(T::Eco2),
        ]
    }

    fn tokens() -> ThemeTokens {
        theme_tokens(ThemeId::GraphiteDark)
    }

    fn dropdown<'a>(rows: Vec<LayerChoice>) -> Element<'a, BoardPopoverEvent> {
        let tokens = tokens();
        let column = layer_column(&group_layers(rows), &tokens, &I18n::en_us());
        two_column_card(column, Space::new().boxed(), &tokens)
    }

    fn all_rows() -> Vec<LayerChoice> {
        layers().map(choice).to_vec()
    }

    /// Mockup row 6: every group shows, with no line cap.
    #[test]
    fn a_tall_window_shows_every_group_without_scrolling() {
        let window = Size::new(600.0, 1000.0);
        let groups = group_layers(all_rows()).len() as f32;
        let height = crate::layout_probe::root_size(dropdown(all_rows()), window).height;
        let lines = groups * MenuMetrics::GRAPHITE.header_row_height()
            + layers().len() as f32 * crate::status_bar::popup_row_height();
        assert!(height >= lines, "{height} holds {lines}");
        let texts = crate::layout_probe::lay_out_texts(dropdown(all_rows()), window);
        let last = texts.last().expect("the last row").bounds;
        assert!(last.y + last.height <= height, "{last:?} in {height}");
    }

    /// A window too short for the card scrolls it: the last layer is
    /// reached and picked.
    #[test]
    fn a_short_window_scrolls_to_the_last_layer() {
        use iced::mouse::{Button, Event, ScrollDelta};
        let window = Size::new(600.0, 200.0);
        let height = crate::layout_probe::root_size(dropdown(all_rows()), window).height;
        assert!(height <= window.height, "the card fits: {height}");
        let row = crate::status_bar::popup_row_height();
        let at = iced::Point::new(60.0, height - 6.0 * PX - row / 2.0);
        let last = layers()[17];
        let picks_last = |messages: &[BoardPopoverEvent]| matches!(messages, [BoardPopoverEvent::LayerPicked(layer)] if *layer == last);
        let unscrolled =
            crate::layout_probe::messages_on_click_at(dropdown(all_rows()), window, None, at);
        assert!(!picks_last(&unscrolled), "the last row starts out of view");
        let events = [
            iced::Event::Mouse(Event::WheelScrolled {
                delta: ScrollDelta::Pixels {
                    x: 0.0,
                    y: -window.height * 10.0,
                },
            }),
            // The scroll moves the content on the next frame.
            iced::Event::Window(iced::window::Event::RedrawRequested(
                std::time::Instant::now() + std::time::Duration::from_secs(2),
            )),
            iced::Event::Mouse(Event::ButtonPressed(Button::Left)),
            iced::Event::Mouse(Event::ButtonReleased(Button::Left)),
        ];
        let messages =
            crate::layout_probe::messages_for_events(dropdown(all_rows()), window, at, &events);
        assert!(picks_last(&messages), "{messages:?}");
    }

    fn has_pixel(pixels: &[u8], colour: iced::Color) -> bool {
        let want = colour.into_rgba8();
        pixels
            .chunks_exact(4)
            .any(|p| (0..3).all(|i| p[i].abs_diff(want[i]) <= 1))
    }

    /// Mockup row 6: the active row has the accent-soft fill and no check.
    #[test]
    fn the_active_layer_is_filled_and_has_no_check() {
        let window = Size::new(600.0, 200.0);
        let tokens = tokens();
        let r = resolve(&tokens);
        let row = |active| LayerChoice {
            active,
            ..choice(layers()[0])
        };
        let active = crate::layout_probe::draw_pixels(dropdown(vec![row(true)]), window);
        let plain = crate::layout_probe::draw_pixels(dropdown(vec![row(false)]), window);
        assert!(has_pixel(&active, r.accent_soft), "the active fill");
        assert!(!has_pixel(&plain, r.accent_soft), "no fill on another row");
        assert!(!has_pixel(&active, r.accent_text), "no check mark");
    }

    /// Mockup row 6: a heading is the catalog text in capitals at the
    /// section size.
    #[test]
    fn a_group_heading_is_capitals_at_the_section_size() {
        let window = Size::new(600.0, 400.0);
        let texts = crate::layout_probe::lay_out_texts(dropdown(vec![choice(layers()[0])]), window);
        let heading = crate::layout_probe::find(&texts, "COPPER").bounds;
        let drawn: Element<'_, BoardPopoverEvent> = text(UserText::new("COPPER".to_string()))
            .size(TS.section)
            .boxed();
        let section = crate::layout_probe::natural(drawn, "COPPER");
        assert!((heading.width - section.width).abs() < 0.5, "{heading:?}");
    }
}
