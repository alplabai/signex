//! The F5 net colour palette body (#71, #2528): a filter, one row per net
//! label (the swatch in a fixed column, the name in mono, coloured nets
//! first), a chooser under the clicked row, and the Clear all footer. The
//! window's title bar carries the title and the net count.

use iced::widget::{Space, Widget, column, container, row, scrollable};
use iced::{Alignment, Color, Element, Length, Theme};
use signex_i18n::{Arg, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, text, text_input};

use super::{SWATCH, swatch, swatch_button};
use crate::app::handlers::editing_commands::net_color_palette::{CHOOSER_COLORS, PaletteRow};
use crate::app::{Message, NetColorMsg, Signex};
use crate::styles::{
    ChromeMetrics, DialogButtonKind, Spacing, TypeScale, dialog_button, dialog_footer_split, ti,
    tokens::resolve,
};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

fn net_message(msg: NetColorMsg) -> Message {
    Message::NetColor(msg)
}

fn to_iced(c: signex_types::theme::Color) -> Color {
    Color::from_rgb8(c.r, c.g, c.b)
}

impl Signex {
    /// The palette body; see the module doc.
    pub(in crate::app::view) fn view_net_color_palette_body(&self) -> Element<'_, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = &self.ui_state.i18n;
        let state = &self.ui_state.net_color_palette;

        let filter = container(
            text_input(
                t!(i18n, ids::DIALOG_NET_COLOR_FILTER),
                state.filter.as_str(),
            )
            .on_input(|s| net_message(NetColorMsg::FilterChanged(s)))
            .style(crate::panels::form_look::text_input_style)
            .padding([SP.s, SP.m])
            .size(TS.body),
        )
        .padding(iced::Padding {
            top: SP.m,
            right: SP.l,
            bottom: SP.s,
            left: SP.l,
        });

        let list: Element<'_, Message> = if self.palette_net_count() == 0 {
            container(crate::panels::empty_state_card(
                tokens,
                t!(i18n, ids::DIALOG_TITLE_NET_COLORS),
                t!(i18n, ids::DIALOG_NET_COLOR_NO_NETS),
                None,
            ))
            .padding([SP.s, SP.l])
            .boxed()
        } else {
            let mut rows = column![].width(Length::Fill);
            for net_row in self.palette_rows() {
                let open = state.chooser.as_deref() == Some(net_row.net.as_str());
                rows = rows.push(palette_row(&net_row, open, tokens));
                if open {
                    rows = rows.push(self.chooser(&net_row));
                }
            }
            scrollable(container(rows).padding([SP.s, SP.m]))
                .direction(crate::styles::thin_vertical())
                .style(crate::styles::scrollbar_style(tokens))
                .height(Length::Fill)
                .boxed()
        };

        let has_colours = self
            .document_state
            .active_engine()
            .is_some_and(|engine| !engine.document().net_colors.is_empty());
        let footer = dialog_footer_split(
            vec![dialog_button(
                t!(i18n, ids::DIALOG_NET_COLOR_CLEAR_ALL),
                DialogButtonKind::Ghost,
                has_colours.then(|| net_message(NetColorMsg::ClearedAll)),
                tokens,
            )],
            vec![
                text(t!(i18n, ids::DIALOG_NET_COLOR_ROW_HINT))
                    .size(TS.caption)
                    .color(resolve(tokens).ink3)
                    .boxed(),
            ],
        );

        container(column![filter, list, footer].height(Length::Fill))
            .style(crate::styles::modal_card(tokens))
            .clip(true)
            .boxed()
    }

    /// The header subtitle: how many nets the active sheet labels.
    pub(crate) fn net_color_palette_subtitle(&self) -> String {
        self.ui_state.i18n.message_plain(
            ids::DIALOG_NET_COLOR_COUNT,
            &[("count", Arg::from(self.palette_net_count()))],
        )
    }

    /// The chooser under an open row: the ten colours, Custom… and Clear.
    fn chooser(&self, net_row: &PaletteRow) -> Element<'_, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = &self.ui_state.i18n;
        let net = &net_row.net;
        let mut swatches = row![].spacing(SP.s).align_y(Alignment::Center);
        for (r, g, b) in CHOOSER_COLORS {
            let color = signex_types::theme::Color::new(r, g, b, 255);
            swatches = swatches.push(swatch_button(
                Color::from_rgb8(r, g, b),
                SWATCH,
                SWATCH,
                net_row.color == Some(color),
                net_message(NetColorMsg::Set {
                    net: net.clone(),
                    color: Some(color),
                }),
                tokens,
            ));
        }
        let actions = row![
            dialog_button(
                t!(i18n, ids::DIALOG_NET_COLOR_CUSTOM),
                DialogButtonKind::Secondary,
                Some(net_message(NetColorMsg::CustomOpenedFor(net.clone()))),
                tokens,
            ),
            Space::new().width(Length::Fill),
            dialog_button(
                t!(i18n, ids::DIALOG_NET_COLOR_CLEAR),
                DialogButtonKind::Ghost,
                net_row.color.is_some().then(|| {
                    net_message(NetColorMsg::Set {
                        net: net.clone(),
                        color: None,
                    })
                }),
                tokens,
            ),
        ]
        .align_y(Alignment::Center);
        let card = container(
            column![
                text(UserText::new(net.clone()))
                    .size(TS.caption)
                    .font(mono())
                    .color(resolve(tokens).ink3),
                swatches,
                actions,
            ]
            .spacing(SP.m),
        )
        .padding(SP.m)
        .width(Length::Fill)
        .style(crate::styles::card(tokens));
        // Indented past the swatch column, so it reads as the row's.
        container(card)
            .padding(iced::Padding {
                top: SP.xs,
                right: 0.0,
                bottom: SP.s,
                left: SWATCH + SP.m,
            })
            .boxed()
    }
}

fn mono() -> iced::Font {
    crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT)
}

/// One net row: the swatch in a fixed column, then the name in mono. The
/// row whose chooser is open keeps the selection fill.
fn palette_row<'a>(net_row: &PaletteRow, open: bool, tokens: &ThemeTokens) -> Element<'a, Message> {
    let ghost = crate::styles::ghost_button(tokens);
    let open_fill = crate::panels::graphite::selection_fill(tokens);
    let face = row![
        swatch(net_row.color.map(to_iced), SWATCH, tokens),
        text(UserText::new(net_row.net.clone()))
            .size(TS.dense)
            .font(mono())
            .color(ti(tokens.text)),
    ]
    .spacing(SP.m)
    .align_y(Alignment::Center);
    button(face)
        .width(Length::Fill)
        .height(Length::Fixed(ChromeMetrics::DEFAULT.row))
        .padding([0.0, SP.m])
        .on_press(net_message(NetColorMsg::RowOpened(net_row.net.clone())))
        .style(move |theme: &Theme, status| {
            let mut style = ghost(theme, status);
            if open {
                style.background = Some(open_fill.into());
            }
            style
        })
        .boxed()
}
