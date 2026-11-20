//! The custom net colour picker card (#72): 48 quick-pick swatches in 8
//! columns, an Old / New preview, an editable Hex field and R / G / B
//! fields, then Cancel / Use Color.

use iced::widget::{Space, Widget, column, container, row, stack};
use iced::{Alignment, Color, Element, Length, Theme};
use signex_i18n::{MessageId, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{text, text_input};

use super::{SWATCH, hatched, swatch, swatch_button};
use crate::app::contracts::Channel;
use crate::app::handlers::editing_commands::net_color_palette::{hex_of, parse_hex};
use crate::app::{Message, NetColorMsg, Signex};
use crate::styles::{
    DialogButtonKind, Radii, Spacing, TypeScale, chrome_px, dialog_body, dialog_button,
    dialog_footer, ti, tokens::resolve,
};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Width of the custom net-colour picker card in the main window, in px.
const NET_COLOR_CUSTOM_W: f32 = 520.0;
/// Quick Pick columns.
const QUICK_PICK_COLUMNS: usize = 8;
/// A quick-pick swatch.
const QUICK_W: f32 = chrome_px(24.0 * 11.0 / 12.0);
const QUICK_H: f32 = chrome_px(20.0 * 11.0 / 12.0);

/// The 48 quick-pick colours, row by row in 8 columns: bright hues, pinks
/// and purples, greens and teals, dark variants, night hues, pastels, muted
/// greys, then high-contrast schematic colours.
const QUICK_PICK: [(u8, u8, u8); 48] = [
    (0xEF, 0x44, 0x44),
    (0xF9, 0x73, 0x16),
    (0xEA, 0xB3, 0x08),
    (0x22, 0xC5, 0x5E),
    (0x06, 0xB6, 0xD4),
    (0x3B, 0x82, 0xF6),
    (0xF4, 0x72, 0xB6),
    (0xE1, 0x14, 0x8C),
    (0xD9, 0x46, 0xEF),
    (0xA8, 0x55, 0xF7),
    (0x8B, 0x5C, 0xF6),
    (0x6D, 0x28, 0xD9),
    (0x84, 0xCC, 0x16),
    (0x10, 0xB9, 0x81),
    (0x14, 0xB8, 0xA6),
    (0x0E, 0xA5, 0xE9),
    (0x60, 0xA5, 0xFA),
    (0x2D, 0xD4, 0xBF),
    (0x9F, 0x12, 0x39),
    (0xB4, 0x53, 0x09),
    (0xA1, 0x6A, 0x3C),
    (0x16, 0xA3, 0x4A),
    (0x15, 0x5E, 0x75),
    (0x1E, 0x40, 0xAF),
    (0x7F, 0x1D, 0x1D),
    (0x78, 0x35, 0x0F),
    (0x5B, 0x21, 0xB6),
    (0x3B, 0x0A, 0x45),
    (0x1E, 0x3A, 0x8A),
    (0x0F, 0x17, 0x2A),
    (0xFE, 0xCA, 0xCA),
    (0xFE, 0xD7, 0xAA),
    (0xFE, 0xF0, 0x8A),
    (0xBB, 0xF7, 0xD0),
    (0xA5, 0xF3, 0xFC),
    (0xBF, 0xDB, 0xFE),
    (0x64, 0x74, 0x8B),
    (0x78, 0x71, 0x6C),
    (0x4B, 0x55, 0x63),
    (0x9C, 0xA3, 0xAF),
    (0xD1, 0xD5, 0xDB),
    (0xFF, 0xFF, 0xFF),
    (0xFF, 0x00, 0xFF),
    (0x00, 0xFF, 0xFF),
    (0xFF, 0xFF, 0x00),
    (0x00, 0xFF, 0x00),
    (0xFF, 0xA5, 0x00),
    (0x1F, 0x23, 0x2A),
];

fn net_message(msg: NetColorMsg) -> Message {
    Message::NetColor(msg)
}

/// Whether `a` and `b` are the same 8-bit colour.
fn same_rgb8(a: Color, b: Color) -> bool {
    hex_of(a) == hex_of(b)
}

impl Signex {
    /// The colour of the net the picker colours, when it came from a palette
    /// row and the net has one: the Old side of the preview.
    fn custom_picker_old_colour(&self) -> Option<Color> {
        let net = self.ui_state.net_color_custom.net.as_deref()?;
        self.palette_net_color(net)
            .map(|c| Color::from_rgb8(c.r, c.g, c.b))
    }

    /// Header of the custom net colour card: the in-window strip with the
    /// net's current colour, the title, the net name and the close; the
    /// empty slot once the dialog has its own OS window, whose title bar
    /// carries the title and the net name.
    fn net_color_custom_header(&self) -> Element<'_, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        if self.modal_detached(crate::app::state::ModalId::NetColorCustom) {
            return crate::app::view::dialogs::os_title_bar_slot();
        }
        let mut line = row![].spacing(SP.m).align_y(Alignment::Center);
        if self.ui_state.net_color_custom.net.is_some() {
            line = line.push(swatch(self.custom_picker_old_colour(), SWATCH, tokens));
        }
        line = line.push_boxed(
            text(t!(self.ui_state.i18n, ids::DIALOG_TITLE_NET_COLOR_CUSTOM))
                .size(TS.heading)
                .color(ti(tokens.text)),
        );
        if let Some(net) = self.ui_state.net_color_custom.net.clone() {
            line = line.push_boxed(
                text(UserText::new(net))
                    .size(TS.label)
                    .font(crate::fonts::iced_font_for_family(
                        crate::fonts::CHROME_MONO_FONT,
                    ))
                    .color(resolve(tokens).ink3),
            );
        }
        line = line.push_boxed(Space::new().width(Length::Fill));
        line = line.push(crate::app::view::dialogs::neutral_close_button(
            tokens,
            net_message(NetColorMsg::CustomShow(false)),
        ));
        container(line)
            .padding([SP.m, SP.l])
            .width(Length::Fill)
            .style(crate::styles::toolbar_strip(tokens))
            .boxed()
    }

    /// Custom net-colour picker card. Unpositioned: the in-window overlay
    /// places it at [`Self::net_color_custom_anchor`].
    pub(in crate::app::view) fn view_net_color_custom_picker(&self) -> Element<'_, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = &self.ui_state.i18n;
        let custom = &self.ui_state.net_color_custom;
        let draft = custom.draft;
        let hex_invalid = custom
            .hex
            .as_deref()
            .is_some_and(|h| parse_hex(h).is_none());

        let mut grid = column![].spacing(SP.s);
        for chunk in QUICK_PICK.chunks(QUICK_PICK_COLUMNS) {
            let mut line = row![].spacing(SP.s);
            for &(r, g, b) in chunk {
                let c = Color::from_rgb8(r, g, b);
                line = line.push(swatch_button(
                    c,
                    QUICK_W,
                    QUICK_H,
                    same_rgb8(c, draft),
                    net_message(NetColorMsg::CustomDraft(c)),
                    tokens,
                ));
            }
            grid = grid.push_boxed(line);
        }
        let quick = column![
            section(i18n, ids::DIALOG_NET_COLOR_QUICK_PICK, tokens),
            grid
        ]
        .spacing(SP.s);

        let hex_value = custom.hex.clone().unwrap_or_else(|| hex_of(draft));
        let error = ti(tokens.error);
        let hex = text_input(t!(i18n, ids::DIALOG_NET_COLOR_HEX), hex_value)
            .on_input(|s| net_message(NetColorMsg::CustomHexChanged(s)))
            .style(move |theme: &Theme, status| {
                let mut style = crate::panels::form_look::text_input_style(theme, status);
                if hex_invalid {
                    style.border.color = error;
                }
                style
            })
            .padding([SP.s, SP.m])
            .size(TS.body);
        let rgb = column![
            section(i18n, ids::DIALOG_NET_COLOR_CUSTOM_RGB, tokens),
            self.old_new_preview(draft),
            field_row(i18n, ids::DIALOG_NET_COLOR_HEX, hex.boxed(), tokens),
            self.channel_row(ids::DIALOG_NET_COLOR_CHANNEL_R, draft.r, Channel::R),
            self.channel_row(ids::DIALOG_NET_COLOR_CHANNEL_G, draft.g, Channel::G),
            self.channel_row(ids::DIALOG_NET_COLOR_CHANNEL_B, draft.b, Channel::B),
        ]
        .spacing(SP.m)
        .width(Length::Fill);

        let body = row![quick, rgb].spacing(SP.xl);
        let footer = dialog_footer(vec![
            dialog_button(
                t!(i18n, ids::DIALOG_BUTTON_CANCEL),
                DialogButtonKind::Cancel,
                Some(net_message(NetColorMsg::CustomShow(false))),
                tokens,
            ),
            dialog_button(
                t!(i18n, ids::DIALOG_NET_COLOR_USE),
                DialogButtonKind::Primary,
                (!hex_invalid).then(|| net_message(NetColorMsg::CustomSubmit(draft))),
                tokens,
            ),
        ]);

        let windowed = self.modal_detached(crate::app::state::ModalId::NetColorCustom);
        let card = container(
            column![self.net_color_custom_header(), dialog_body(body), footer]
                .width(crate::library::card_len(windowed, NET_COLOR_CUSTOM_W)),
        );
        // In its own window the card is the dialog: flat surface, no anchor.
        if windowed {
            card.style(crate::styles::modal_card(tokens)).boxed()
        } else {
            card.style(crate::styles::context_menu(tokens)).boxed()
        }
    }

    /// The large preview: New (the pick) beside Old (the net's colour now,
    /// hatched when it has none). From the Active Bar there is no Old.
    fn old_new_preview(&self, draft: Color) -> Element<'_, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = &self.ui_state.i18n;
        let height = Length::Fixed(4.0 * TS.label * crate::styles::LINE_HEIGHT);
        let new_side = tagged(
            filled(draft, height, tokens),
            t!(i18n, ids::DIALOG_NET_COLOR_NEW),
            tokens,
        );
        let mut preview = row![container(new_side).width(Length::FillPortion(62))];
        if self.ui_state.net_color_custom.net.is_some() {
            let old = match self.custom_picker_old_colour() {
                Some(c) => filled(c, height, tokens),
                None => hatched(Length::Fill, height, tokens),
            };
            preview = preview.push_boxed(
                container(tagged(old, t!(i18n, ids::DIALOG_NET_COLOR_OLD), tokens))
                    .width(Length::FillPortion(38)),
            );
        }
        preview.boxed()
    }

    /// One R / G / B field: the letter, then the 0-255 value.
    fn channel_row(&self, label: MessageId, value: f32, chan: Channel) -> Element<'_, Message> {
        let i18n = &self.ui_state.i18n;
        let v255 = (value.clamp(0.0, 1.0) * 255.0).round() as u8;
        let input = text_input(t!(i18n, ids::PLACEHOLDER_COLOR_CHANNEL), v255.to_string())
            .style(crate::panels::form_look::text_input_style)
            .size(TS.body)
            .padding([SP.s, SP.m])
            .width(Length::Fixed(5.0 * TS.body))
            .on_input(move |s| net_message(NetColorMsg::CustomChannel(chan, s)));
        field_row(
            i18n,
            label,
            input.boxed(),
            &self.document_state.panel_ctx.tokens,
        )
    }

    /// Where the in-window net-colour picker card sits: centred under the
    /// Active Bar. The overlay places the unpositioned card here through the
    /// dismiss helper, so only a press outside the card closes it (#2377).
    pub(in crate::app::view) fn net_color_custom_anchor(&self) -> (f32, f32) {
        let (ww, _wh) = self.ui_state.window_size;
        let x = ((ww - NET_COLOR_CUSTOM_W) * 0.5).max(0.0);
        let y = crate::app::view::canvas_region::chrome_top(&self.document_state.panel_ctx.tokens)
            + 80.0;
        (x, y)
    }
}

/// A group caption over a column.
fn section<'a>(
    i18n: &signex_i18n::I18n,
    id: MessageId,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    text(t!(i18n, id))
        .size(TS.caption)
        .color(resolve(tokens).ink3)
        .boxed()
}

/// A labelled field row: the label in a fixed column, then the field.
fn field_row<'a>(
    i18n: &signex_i18n::I18n,
    label: MessageId,
    field: Element<'a, Message>,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    row![
        text(t!(i18n, label))
            .size(TS.body)
            .color(ti(tokens.text_secondary))
            .width(Length::Fixed(3.0 * TS.body)),
        field,
    ]
    .spacing(SP.m)
    .align_y(Alignment::Center)
    .boxed()
}

/// An area filled with `fill`, rounded like a button.
fn filled<'a>(fill: Color, height: Length, tokens: &ThemeTokens) -> Element<'a, Message> {
    let line = ti(tokens.border);
    container(Space::new())
        .width(Length::Fill)
        .height(height)
        .style(move |_: &Theme| container::Style {
            background: Some(fill.into()),
            border: iced::Border {
                width: 1.0,
                radius: Radii::GRAPHITE.button.into(),
                color: line,
                ..iced::Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// `area` with a small pill naming it in its lower-left corner.
fn tagged<'a>(
    area: Element<'a, Message>,
    tag: signex_i18n::Localized,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let r = resolve(tokens);
    let (pill_bg, pill_ink) = (r.raised, ti(tokens.text));
    let pill = container(text(tag).size(TS.caption).color(pill_ink))
        .padding([SP.xs, SP.s])
        .style(move |_: &Theme| container::Style {
            background: Some(pill_bg.into()),
            border: iced::Border {
                radius: Radii::GRAPHITE.pill.into(),
                ..iced::Border::default()
            },
            ..container::Style::default()
        });
    stack![
        area,
        container(pill)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(SP.s)
            .align_y(iced::alignment::Vertical::Bottom),
    ]
    .boxed()
}
