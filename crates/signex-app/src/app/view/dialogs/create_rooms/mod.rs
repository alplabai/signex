//! Create Rooms from Sheets (#2457): the rooms to create (sheets, then
//! channels), the options, the live preview and Create. The plan it shows
//! is the plan Create runs (`CreateRoomsDialog::plan_with`). State and
//! handlers: `app/rooms/`.

mod preview;

use iced::widget::{Column, Space, column, container, row, scrollable};
use iced::{Alignment, Background, Border, Color, Element, Font, Length, Widget as _};
use signex_engine::pcb::rooms::PlanShape;
use signex_i18n::{I18n, Localized, MessageId, UserText, ids, t};
use signex_types::pcb::RoomLayer;
use signex_types::theme::ThemeTokens;
use signex_widgets::{
    CheckboxLabel as _, Label, SegmentedStyle, checkbox, segmented, text, text_input,
};

use crate::app::rooms::{CreateRoomsDialog, CreateRoomsMsg, RoomRow};
use crate::app::{Message, Signex};
use crate::panels::form_look;
use crate::panels::pcb_room_properties::room_colour;
use crate::styles::tokens::resolve;
use crate::styles::{
    DialogButtonKind, Radii, Spacing, TypeScale, dialog_button, dialog_footer_split, ti,
};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;
/// The dialog: 920 by 560, as the mock draws it.
const SIZE: (f32, f32) = (920.0, 560.0);
/// The list and options column.
const LEFT_W: f32 = 380.0;
/// The options' label column.
const LABEL_W: f32 = 96.0;

fn cr(msg: CreateRoomsMsg) -> Message {
    Message::from(msg)
}

fn mono() -> Font {
    Font::MONOSPACE
}

impl Signex {
    pub(in crate::app::view) fn view_create_rooms_dialog(&self) -> Element<'_, Message> {
        self.wrap_centred_dialog(self.view_create_rooms_card(), SIZE)
    }

    fn view_create_rooms_card(&self) -> Element<'_, Message> {
        let Some(dialog) = self.ui_state.rooms.create.as_ref() else {
            return container(Space::new()).boxed();
        };
        let Some(board) = self.document_state.pcb_board(&dialog.board) else {
            return container(Space::new()).boxed();
        };
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = self.ui_state.i18n.as_ref();
        let plan = dialog.preview(board);
        let (sheets, channels) = dialog.counts();
        let channels = if channels == 0 {
            t!(i18n, ids::ROOMS_DIALOG_NO_CHANNELS).into_string()
        } else {
            t!(i18n, ids::ROOMS_DIALOG_CHANNELS, "count" => channels).into_string()
        };
        let subtitle = t!(
            i18n, ids::ROOMS_DIALOG_SUBTITLE,
            "project" => dialog.project_file.clone(), "sheets" => sheets, "channels" => channels
        )
        .into_string();
        let cancel = Some(cr(CreateRoomsMsg::Cancelled));
        let header = super::header::graphite_header_closing(
            t!(i18n, ids::ROOMS_DIALOG_TITLE).into_string(),
            Some(subtitle),
            None,
            cancel.clone(),
            super::header::DIALOG_HEADER_H,
            tokens,
        );
        let left = scrollable(
            container(left_column(dialog, &dialog.rows(board), i18n, tokens))
                .padding([SP.l, SP.l])
                .width(Length::Fixed(LEFT_W)),
        )
        .height(Length::Fill);
        let right = right_column(preview::Preview::of(board, &plan), i18n, tokens);
        let body = row![left, right].height(Length::Fill);
        let rooms = plan.rooms.len();
        let footer = dialog_footer_split(
            vec![
                text(t!(
                    i18n, ids::ROOMS_FOOTER,
                    "rooms" => rooms, "parts" => plan.moves.len()
                ))
                .size(TS.caption)
                .color(resolve(tokens).ink3)
                .boxed(),
            ],
            vec![
                dialog_button(
                    t!(i18n, ids::ROOMS_CANCEL),
                    DialogButtonKind::Cancel,
                    cancel,
                    tokens,
                ),
                dialog_button(
                    t!(i18n, ids::ROOMS_CREATE, "count" => rooms),
                    DialogButtonKind::Primary,
                    (rooms > 0).then(|| cr(CreateRoomsMsg::Confirmed)),
                    tokens,
                ),
            ],
        );
        container(column![header, body, footer].width(Length::Fixed(
            super::widgets::modal_card_width(self.ui_state.window_size, SIZE.0),
        )))
        .height(Length::Fixed(SIZE.1))
        .style(crate::styles::modal_card(tokens))
        .clip(true)
        .boxed()
    }
}

/// A section heading (mockup row 163): upper case, cased for the language
/// in force at display time.
fn small_label<'a>(id: MessageId, i18n: &I18n, tokens: &ThemeTokens) -> Element<'a, Message> {
    text(UserText::new(
        i18n.upper_for_display(&t!(i18n, id).into_string()),
    ))
    .size(TS.section)
    .color(resolve(tokens).ink3)
    .boxed()
}

fn caption<'a>(line: Localized, tokens: &ThemeTokens) -> Element<'a, Message> {
    text(line)
        .size(TS.caption)
        .color(resolve(tokens).ink3)
        .boxed()
}

/// A group row of the list: sunken, upper-case title.
fn group<'a>(id: MessageId, i18n: &I18n, tokens: &ThemeTokens) -> Element<'a, Message> {
    let r = resolve(tokens);
    container(
        text(UserText::new(
            i18n.upper_for_display(&t!(i18n, id).into_string()),
        ))
        .size(TS.section)
        .color(r.ink3),
    )
    .padding([SP.xs, SP.m])
    .width(Length::Fill)
    .style(move |_| container::Style {
        background: Some(Background::Color(r.sunken)),
        ..Default::default()
    })
    .boxed()
}

/// A room row: checkbox, dashed swatch, mono name, sheet file, part count.
fn room_row<'a>(row_: &RoomRow, i18n: &I18n, tokens: &ThemeTokens) -> Element<'a, Message> {
    let r = resolve(tokens);
    let colour = room_colour(&row_.colour);
    let name = row_.name.clone();
    let tick = checkbox(row_.ticked)
        .size(TS.body)
        .style(form_look::checkbox_style)
        .on_toggle(move |_| cr(CreateRoomsMsg::RowToggled(name.clone())));
    let swatch = container(Space::new())
        .width(Length::Fixed(TS.body))
        .height(Length::Fixed(TS.body))
        .style(move |_| container::Style {
            background: Some(Background::Color(Color { a: 0.2, ..colour })),
            border: Border {
                color: colour,
                width: 1.5,
                radius: (TS.body / 4.0).into(),
                ..Border::default()
            },
            ..Default::default()
        });
    let pill = container(
        text(t!(i18n, ids::ROOMS_PARTS_PILL, "count" => row_.parts))
            .size(TS.caption)
            .color(r.ink3),
    )
    .padding([0.0, SP.s])
    .style(move |_| container::Style {
        background: Some(Background::Color(r.sunken)),
        border: Border {
            radius: Radii::GRAPHITE.pill.into(),
            ..Default::default()
        },
        ..Default::default()
    });
    container(
        row![
            tick,
            swatch,
            text(UserText::new(row_.name.clone()))
                .size(TS.body)
                .font(mono())
                .color(ti(tokens.text))
                .width(Length::Fill),
            text(UserText::new(row_.file.clone()))
                .size(TS.caption)
                .color(r.ink3),
            pill,
        ]
        .spacing(SP.s)
        .align_y(Alignment::Center),
    )
    .padding([SP.xs, SP.m])
    .width(Length::Fill)
    .boxed()
}

fn segmented_style(tokens: &ThemeTokens) -> SegmentedStyle {
    let r = resolve(tokens);
    r.segmented(TS.body)
}

fn option_row<'a>(
    id: MessageId,
    i18n: &I18n,
    tokens: &ThemeTokens,
    control: Element<'a, Message>,
) -> Element<'a, Message> {
    row![
        container(
            text(t!(i18n, id))
                .size(TS.body)
                .color(ti(tokens.text_secondary))
        )
        .width(Length::Fixed(LABEL_W)),
        control,
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center)
    .boxed()
}

/// A checkbox with its label and, under it, an optional description.
fn check<'a>(
    on: bool,
    label: MessageId,
    desc: Option<MessageId>,
    i18n: &I18n,
    tokens: &ThemeTokens,
    msg: fn(bool) -> CreateRoomsMsg,
) -> Element<'a, Message> {
    let tick = checkbox(on)
        .size(TS.body)
        .labelled(t!(i18n, label))
        .text_size(TS.body)
        .style(form_look::checkbox_style)
        .on_toggle(move |on| cr(msg(on)));
    let mut col: Column<Element<'a, Message>> = column![tick].spacing(SP.xs);
    if let Some(desc) = desc {
        col = col.push(caption(t!(i18n, desc), tokens));
    }
    col.boxed()
}

fn left_column<'a>(
    dialog: &CreateRoomsDialog,
    rows: &[RoomRow],
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let r = resolve(tokens);
    let mut list: Column<Element<'a, Message>> =
        column![group(ids::ROOMS_GROUP_SHEETS, i18n, tokens)];
    for row_ in rows.iter().filter(|r| !r.channel) {
        list = list.push(room_row(row_, i18n, tokens));
    }
    list = list.push(group(ids::ROOMS_GROUP_CHANNELS, i18n, tokens));
    let channels: Vec<&RoomRow> = rows.iter().filter(|r| r.channel).collect();
    if channels.is_empty() {
        list = list.push(
            container(caption(t!(i18n, ids::ROOMS_CHANNELS_NONE), tokens))
                .padding([SP.s, SP.m])
                .boxed(),
        );
    }
    for row_ in channels {
        list = list.push(room_row(row_, i18n, tokens));
    }
    let list = container(list).style(move |_| container::Style {
        border: Border {
            color: r.line_strong,
            width: 1.0,
            radius: Radii::GRAPHITE.button.into(),
            ..Border::default()
        },
        ..Default::default()
    });
    let options = dialog.options;
    let shapes = [
        (
            PlanShape::AroundParts,
            Label::from(t!(i18n, ids::ROOMS_AROUND_PARTS)),
        ),
        (
            PlanShape::EvenGrid,
            Label::from(t!(i18n, ids::ROOMS_EVEN_GRID)),
        ),
    ];
    let shape = segmented(shapes, options.shape, segmented_style(tokens), |s| {
        cr(CreateRoomsMsg::ShapePicked(s))
    });
    let layers = [
        (RoomLayer::Top, Label::from(t!(i18n, ids::ROOMS_LAYER_TOP))),
        (
            RoomLayer::Bottom,
            Label::from(t!(i18n, ids::ROOMS_LAYER_BOTTOM)),
        ),
        (
            RoomLayer::Both,
            Label::from(t!(i18n, ids::ROOMS_LAYER_BOTH)),
        ),
    ];
    let layer = segmented(layers, options.layer, segmented_style(tokens), |l| {
        cr(CreateRoomsMsg::LayerPicked(l))
    });
    let margin = row![
        text_input(t!(i18n, ids::ROOMS_UNIT_MM), dialog.margin_text.clone())
            .size(TS.body)
            .font(mono())
            .padding([SP.xs, SP.s])
            .on_input(|text| cr(CreateRoomsMsg::MarginEdited(text)))
            .style(form_look::text_input_style)
            .width(Length::Fixed(LABEL_W)),
        caption(t!(i18n, ids::ROOMS_UNIT_MM), tokens),
    ]
    .spacing(SP.xs)
    .align_y(Alignment::Center);
    column![
        small_label(ids::ROOMS_DIALOG_TO_CREATE, i18n, tokens),
        list,
        small_label(ids::ROOMS_DIALOG_OPTIONS, i18n, tokens),
        option_row(ids::ROOMS_OPTION_SHAPE, i18n, tokens, shape),
        option_row(ids::ROOMS_OPTION_MARGIN, i18n, tokens, margin.boxed()),
        option_row(ids::ROOMS_OPTION_LAYER, i18n, tokens, layer),
        check(
            options.move_parts,
            ids::ROOMS_MOVE_PARTS,
            Some(ids::ROOMS_MOVE_PARTS_DESC),
            i18n,
            tokens,
            CreateRoomsMsg::MovePartsToggled,
        ),
        check(
            options.keep_rule,
            ids::ROOMS_ADD_KEEP,
            None,
            i18n,
            tokens,
            CreateRoomsMsg::KeepRuleToggled,
        ),
        check(
            options.replace_same_name,
            ids::ROOMS_REPLACE_SAME,
            None,
            i18n,
            tokens,
            CreateRoomsMsg::ReplaceToggled,
        ),
    ]
    .spacing(SP.s)
    .boxed()
}

fn right_column<'a>(
    preview: preview::Preview,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let r = resolve(tokens);
    let canvas = container(preview.view())
        .width(Length::Fill)
        .height(Length::Fill)
        .clip(true)
        .style(move |_| container::Style {
            border: Border {
                color: r.line_strong,
                width: 1.0,
                radius: Radii::GRAPHITE.card.into(),
                ..Border::default()
            },
            ..Default::default()
        });
    column![
        row![
            container(small_label(ids::ROOMS_PREVIEW, i18n, tokens)).width(Length::Fill),
            caption(t!(i18n, ids::ROOMS_PREVIEW_LEGEND), tokens),
        ]
        .align_y(Alignment::Center),
        caption(t!(i18n, ids::ROOMS_PREVIEW_CAPTION), tokens),
        canvas,
    ]
    .spacing(SP.s)
    .padding([SP.l, SP.l])
    .width(Length::Fill)
    .height(Length::Fill)
    .boxed()
}

#[cfg(test)]
mod tests {
    use iced::Size;
    use signex_i18n::{I18n, LanguagePref, MessageId, ids};
    use signex_types::theme::{ThemeId, theme_tokens};

    use super::{SIZE, small_label};
    use crate::layout_probe::lay_out_texts;

    const HEADINGS: [MessageId; 3] = [
        ids::ROOMS_DIALOG_TO_CREATE,
        ids::ROOMS_DIALOG_OPTIONS,
        ids::ROOMS_PREVIEW,
    ];

    /// The section headings as laid out, in the language `tag`.
    fn headings(tag: &str) -> (I18n, Vec<String>) {
        let i18n = I18n::new(&LanguagePref::Locale(tag.to_owned()));
        let tokens = theme_tokens(ThemeId::GraphiteLight);
        let window = Size::new(SIZE.0, SIZE.1);
        let shown = HEADINGS
            .into_iter()
            .map(|id| {
                lay_out_texts(small_label(id, &i18n, &tokens), window)
                    .into_iter()
                    .map(|t| t.text)
                    .collect::<Vec<_>>()
                    .join("|")
            })
            .collect();
        (i18n, shown)
    }

    /// Mockup row 163: the section headings read in upper case, cased at
    /// display time in the language in force; the catalog keeps its case.
    #[test]
    fn the_section_headings_are_upper_case_in_the_language_in_force() {
        let (en, shown) = headings("en-US");
        assert_eq!(shown, ["ROOMS TO CREATE", "OPTIONS", "PREVIEW"]);
        assert_eq!(
            en.message_plain(ids::ROOMS_DIALOG_TO_CREATE, &[]),
            "Rooms to create"
        );
        let (xa, shown) = headings("en-XA");
        let expected: Vec<String> = HEADINGS
            .into_iter()
            .map(|id| xa.upper_for_display(&xa.message_plain(id, &[])))
            .collect();
        assert_eq!(shown, expected);
        assert_ne!(shown[0], "ROOMS TO CREATE");
    }
}
