//! One part row of the Components panel (#2535). Compact rows (the default)
//! are one line, the part name only, with the description as a tooltip;
//! Previews rows add a symbol thumbnail and a one-line description. A
//! hovered row shows an inline Place, the row menu's Place.

use iced::widget::{Column, column, container, row};
use iced::{Background, Color, Element, Length, Theme, Widget};
use signex_i18n::I18n;
use signex_library::ComponentRow;
use signex_types::theme::ThemeTokens;
use signex_widgets::text;

use super::{HINT_TEXT_SIZE, ROW_TEXT_SIZE, SP, look};
use crate::library::messages::LibraryMessage;
use crate::library::state::{ComponentsPanelRowKey, ComponentsRowStyle};

/// Height, inset and part-number face of a Components panel row.
#[derive(Debug, Clone, Copy)]
pub(super) struct RowLook {
    /// Height of a Compact row.
    pub height: f32,
    /// Left inset.
    pub inset: f32,
    /// Right inset.
    pub right: f32,
    /// Face of the part number in a Compact row.
    pub part_font: iced::Font,
}

impl RowLook {
    pub(super) fn graphite() -> Self {
        Self {
            height: look::ROW_HEIGHT,
            inset: look::ROW_INSET,
            right: look::EDGE,
            part_font: look::mono(),
        }
    }

    /// Height of a row drawn in `style`.
    pub(super) fn height_for(&self, style: ComponentsRowStyle) -> f32 {
        match style {
            ComponentsRowStyle::Compact => self.height,
            ComponentsRowStyle::Previews => look::PREVIEW_ROW_HEIGHT,
        }
    }
}

/// The theme colours a Components panel row is drawn with.
#[derive(Debug, Clone, Copy)]
pub(super) struct RowColors {
    pub text: Color,
    pub muted: Color,
    /// Selection fill.
    pub tint: Color,
    /// Hover fill.
    pub hover: Color,
    /// Fill of the row whose menu is open.
    pub menu: Color,
}

/// What the row's state asks of its look.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct RowFlags {
    pub selected: bool,
    pub hovered: bool,
    /// The row's context menu is open: it stays highlighted.
    pub menu_open: bool,
}

impl RowFlags {
    /// The fill the row is drawn with, if any: an open menu wins, then the
    /// selection, then the hover.
    pub(super) fn fill(self, colors: RowColors) -> Option<Color> {
        if self.menu_open {
            Some(colors.menu)
        } else if self.selected {
            Some(colors.tint)
        } else {
            self.hovered.then_some(colors.hover)
        }
    }
}

/// Everything a row reads that is the same for every row of the list.
pub(super) struct RowContext<'b> {
    pub style: ComponentsRowStyle,
    pub colors: RowColors,
    pub look: RowLook,
    pub tokens: &'b ThemeTokens,
    pub i18n: &'b I18n,
}

/// The name of a component row: part number, then the MPN when set.
pub(super) fn component_row_label(internal_pn: &str, mpn: &str) -> String {
    match mpn.trim() {
        "" => internal_pn.to_owned(),
        mpn => format!("{internal_pn} — {mpn}"),
    }
}

/// One part row. A press selects the row and arms a drag onto the canvas; a
/// right-click selects it and opens its menu; the inline Place of a hovered
/// row places it.
pub(super) fn view_row_button<'a>(
    key: ComponentsPanelRowKey,
    row_data: &'a ComponentRow,
    flags: RowFlags,
    thumb: Option<Element<'a, LibraryMessage>>,
    cx: &RowContext<'_>,
) -> Element<'a, LibraryMessage> {
    let previews = cx.style.shows_previews();
    let label = component_row_label(row_data.internal_pn.as_str(), &row_data.primary_mpn.mpn);
    let description = row_data.description.trim();
    let name = text(signex_i18n::UserText::new(label))
        .size(ROW_TEXT_SIZE)
        .color(cx.colors.text)
        .width(Length::Fill)
        .wrapping(iced::widget::text::Wrapping::None);
    // Compact keeps today's mono part number; Previews sets it in the UI face.
    let name = if previews {
        name
    } else {
        name.font(cx.look.part_font)
    };
    let body: Element<'a, LibraryMessage> = if previews {
        let lines: Column<Element<'a, LibraryMessage>> = column![
            name.boxed(),
            text(signex_i18n::UserText::new(description))
                .size(HINT_TEXT_SIZE)
                .color(cx.colors.muted)
                .width(Length::Fill)
                .wrapping(iced::widget::text::Wrapping::None)
                .boxed(),
        ]
        .width(Length::Fill);
        lines.boxed()
    } else {
        name.boxed()
    };

    let mut line = row![].spacing(SP.s).align_y(iced::Alignment::Center);
    if previews {
        line = line.push(thumb.unwrap_or_else(look::thumb_placeholder));
    }
    line = line.push(body);
    if flags.hovered {
        line = line.push(look::place_button(
            LibraryMessage::ComponentsPanelPlace {
                library_path: key.library_path.clone(),
                table: key.table.clone(),
                row_id: key.row_id,
            },
            cx.tokens,
            cx.i18n,
        ));
    }
    let height = cx.look.height_for(cx.style);
    let fill = flags.fill(cx.colors);
    let content = container(line)
        .width(Length::Fill)
        .padding(iced::Padding {
            left: cx.look.inset,
            right: cx.look.right,
            ..iced::Padding::ZERO
        })
        .height(height)
        .center_y(height)
        .style(move |_: &Theme| container::Style {
            background: fill.map(Background::Color),
            ..container::Style::default()
        });
    // Compact rows carry the description as a tooltip.
    let content: Element<'a, LibraryMessage> = if !previews && !description.is_empty() {
        let tip = container(
            text(signex_i18n::UserText::new(description.to_owned()))
                .size(HINT_TEXT_SIZE)
                .color(cx.colors.text),
        )
        .padding([SP.xs, SP.s])
        .style(crate::styles::context_menu(cx.tokens));
        signex_widgets::tooltip(
            content.boxed(),
            tip,
            iced::widget::tooltip::Position::Bottom,
        )
        .gap(SP.xs)
        .boxed()
    } else {
        content.boxed()
    };

    iced::widget::mouse_area(content)
        .on_press(LibraryMessage::ComponentsPanelRowPressed(key.clone()))
        .on_right_press(LibraryMessage::ComponentsPanelRowRightClicked(key.clone()))
        .on_enter(LibraryMessage::ComponentsPanelRowEntered(key.clone()))
        .on_exit(LibraryMessage::ComponentsPanelRowExited(key))
        .boxed()
}
