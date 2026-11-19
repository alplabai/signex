//! Room Properties (#2457): the header (swatch, title, name pill, lock), the
//! line-art illustration (`illustration`), then the Room, Position, Parts and
//! Rule sections. Every field is one engine command, one undo step; the
//! facts are built from the board in `Signex::room_facts`.

mod illustration;

use std::collections::BTreeMap;

use iced::widget::{Column, Row, Space, column, container, pick_list, row};
use iced::{Alignment, Background, Border, Color, Element, Font, Length, Widget as _};
use signex_i18n::{I18n, Localized, MessageId, UserText, ids, t};
use signex_types::pcb::{Room, RoomLayer, RoomShape};
use signex_types::schematic::InstancePath;
use signex_types::theme::ThemeTokens;
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::enable_section::EnableSection;
use signex_widgets::icon_button::icon_button;
use signex_widgets::property_illustration::{FieldId, IllustrationHover, IllustrationMsg};
use signex_widgets::{Label, SegmentedStyle, button, segmented, text, text_input};

use super::{PanelContext, PanelMsg, form_look};
use crate::app::rooms::{RoomField, RoomMsg};
use crate::styles::tokens::resolve;
use crate::styles::{Radii, Spacing, TypeScale, ti};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;
/// The label column of the forms, as the mock's 96 px grid at body size.
const LABEL_W: f32 = TS.body * 8.0;
/// Lock and unlock glyphs of the header button.
const LOCKED: &str = "\u{1F512}";
const UNLOCKED: &str = "\u{1F513}";

/// One member part: its designator and how far outside the room it is, nm
/// (0: inside), and its body box, mm.
#[derive(Debug, Clone, PartialEq)]
pub struct RoomPart {
    pub designator: String,
    pub outside_nm: i64,
    pub body: [f64; 4],
}

/// A From sheet choice: a sheet placement of the board's parts, or none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetChoice {
    pub sheet: Option<InstancePath>,
    pub label: String,
}

impl std::fmt::Display for SheetChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}

/// What Room Properties shows.
#[derive(Debug, Clone, PartialEq)]
pub struct RoomFacts {
    pub room: Room,
    pub parts: Vec<RoomPart>,
    pub sheets: Vec<SheetChoice>,
    pub drafts: BTreeMap<RoomField, String>,
    pub hover: IllustrationHover,
}

impl RoomFacts {
    /// The parts outside, worst first.
    pub fn outside(&self) -> Vec<&RoomPart> {
        let mut out: Vec<&RoomPart> = self.parts.iter().filter(|p| p.outside_nm > 0).collect();
        out.sort_by_key(|p| std::cmp::Reverse(p.outside_nm));
        out
    }

    /// The text of `field`: the draft being typed, else the room's value.
    pub fn field_text(&self, field: RoomField) -> String {
        if let Some(draft) = self.drafts.get(&field) {
            return draft.clone();
        }
        let b = self.room.outline.bounds();
        let mm = |v: Option<f64>| v.map_or_else(String::new, |v| format!("{v:.3}"));
        match field {
            RoomField::Name => self.room.name.clone(),
            RoomField::X => mm(b.map(|b| b.x_mm)),
            RoomField::Y => mm(b.map(|b| b.y_mm)),
            RoomField::Width => mm(b.map(|b| b.w_mm)),
            RoomField::Height => mm(b.map(|b| b.h_mm)),
            RoomField::Margin => mm(Some(self.room.margin_mm)),
        }
    }
}

fn msg(m: RoomMsg) -> PanelMsg {
    PanelMsg::Room(m)
}

/// The room's colour; the first palette colour when it does not read.
pub(crate) fn room_colour(hex: &str) -> Color {
    signex_types::theme::Color::parse_hex(hex)
        .or_else(|| signex_types::theme::Color::parse_hex(signex_types::pcb::room::ROOM_PALETTE[0]))
        .map_or(Color::WHITE, |c| ti(c))
}

fn semibold() -> Font {
    Font {
        weight: iced::font::Weight::Semibold,
        ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
    }
}

fn mono() -> Font {
    Font::MONOSPACE
}

/// The panel body for `facts`.
pub fn view<'a>(ctx: &'a PanelContext, facts: &'a RoomFacts) -> Element<'a, PanelMsg> {
    let i18n: &I18n = &ctx.i18n;
    let tokens = &ctx.tokens;
    let illustration = illustration::view(facts, i18n, tokens)
        .map(|m: IllustrationMsg| msg(RoomMsg::Illustration(m)));
    let body = column![
        section(
            i18n,
            tokens,
            ids::ROOMS_SECTION_ROOM,
            room_section(facts, i18n, tokens)
        ),
        section(
            i18n,
            tokens,
            ids::ROOMS_SECTION_POSITION,
            position_section(facts, i18n, tokens)
        ),
        parts_section(facts, i18n, tokens),
        section(
            i18n,
            tokens,
            ids::ROOMS_SECTION_RULE,
            rule_section(facts, i18n, tokens)
        ),
    ]
    .spacing(SP.s)
    .padding([0.0, SP.m]);
    column![header(facts, i18n, tokens), illustration, body]
        .spacing(SP.xs)
        .width(Length::Fill)
        .boxed()
}

/// Swatch, title, name pill and the lock button.
fn header<'a>(facts: &'a RoomFacts, i18n: &I18n, tokens: &ThemeTokens) -> Element<'a, PanelMsg> {
    let r = resolve(tokens);
    let colour = room_colour(&facts.room.colour);
    let swatch = container(Space::new())
        .width(Length::Fixed(TS.body))
        .height(Length::Fixed(TS.body))
        .style(move |_| container::Style {
            background: Some(Background::Color(Color { a: 0.18, ..colour })),
            border: Border {
                color: colour,
                width: 1.5,
                radius: (TS.body / 4.0).into(),
            },
            ..Default::default()
        });
    let pill = container(
        text(UserText::new(facts.room.name.clone()))
            .size(TS.caption)
            .color(r.accent_soft_ink),
    )
    .padding([0.0, SP.s])
    .style(move |_| container::Style {
        background: Some(Background::Color(r.accent_soft)),
        border: Border {
            radius: Radii::GRAPHITE.pill.into(),
            ..Default::default()
        },
        ..Default::default()
    });
    let room = facts.room.uuid;
    let (glyph, tip) = if facts.room.locked {
        (LOCKED, ids::ROOMS_PROPS_UNLOCK)
    } else {
        (UNLOCKED, ids::ROOMS_PROPS_LOCK)
    };
    let lock = icon_button(
        glyph,
        &t!(i18n, tip).into_string(),
        msg(RoomMsg::LockToggled { room }),
        facts.room.locked,
        tokens,
    );
    row![
        swatch,
        text(t!(i18n, ids::ROOMS_PROPS_TITLE))
            .size(TS.title)
            .font(semibold())
            .color(ti(tokens.text)),
        pill,
        Space::new().width(Length::Fill),
        lock,
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center)
    .padding([SP.m, SP.m])
    .boxed()
}

/// A section: its header line, then `body`.
fn section<'a>(
    i18n: &I18n,
    tokens: &ThemeTokens,
    title: MessageId,
    body: Element<'a, PanelMsg>,
) -> Element<'a, PanelMsg> {
    let r = resolve(tokens);
    column![
        text(t!(i18n, title))
            .size(TS.section)
            .font(semibold())
            .color(r.ink3),
        body,
    ]
    .spacing(SP.s)
    .boxed()
}

/// A form row: the label column, then `control`; hovering the row lights
/// its callout.
fn form_row<'a>(
    label: Localized,
    tokens: &ThemeTokens,
    control: Element<'a, PanelMsg>,
) -> Element<'a, PanelMsg> {
    row![
        container(text(label).size(TS.body).color(ti(tokens.text_secondary)))
            .width(Length::Fixed(LABEL_W)),
        container(control).width(Length::Fill),
    ]
    .align_y(Alignment::Center)
    .spacing(SP.s)
    .boxed()
}

/// A text field of `field`, sending what is typed and Enter; mono when
/// `is_mono`. The row reports hover so its callout lights.
fn field<'a>(
    facts: &RoomFacts,
    field: RoomField,
    i18n: &I18n,
    is_mono: bool,
) -> Element<'a, PanelMsg> {
    let room = facts.room.uuid;
    let input = text_input(t!(i18n, ids::ROOMS_UNIT_MM), facts.field_text(field))
        .size(TS.body)
        .padding([SP.xs, SP.s])
        .on_input(move |text| msg(RoomMsg::FieldEdited { room, field, text }))
        .on_submit(msg(RoomMsg::FieldSubmitted { room, field }))
        .style(form_look::text_input_style)
        .width(Length::Fill);
    let input = if is_mono { input.font(mono()) } else { input };
    let input: Element<'a, PanelMsg> = input.boxed();
    match field.callout() {
        Some(id) => iced::widget::mouse_area(input)
            .on_enter(msg(RoomMsg::Illustration(IllustrationMsg::FieldHovered(
                FieldId::new(id),
            ))))
            .on_exit(msg(RoomMsg::Illustration(IllustrationMsg::FieldLeft(
                FieldId::new(id),
            ))))
            .boxed(),
        None => input,
    }
}

fn segmented_style(tokens: &ThemeTokens) -> SegmentedStyle {
    let r = resolve(tokens);
    SegmentedStyle::new(
        r.sunken,
        r.raised,
        r.shadow,
        r.ink3,
        ti(tokens.text),
        TS.body,
    )
}

fn room_section<'a>(
    facts: &'a RoomFacts,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, PanelMsg> {
    let room = facts.room.uuid;
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
    let layer = segmented(
        layers,
        facts.room.layer,
        segmented_style(tokens),
        move |layer| msg(RoomMsg::LayerPicked { room, layer }),
    );
    let shapes = [
        (
            RoomShape::Rectangle,
            Label::from(t!(i18n, ids::ROOMS_SHAPE_RECTANGLE)),
        ),
        (
            RoomShape::Polygon,
            Label::from(t!(i18n, ids::ROOMS_SHAPE_POLYGON)),
        ),
    ];
    let shape = segmented(
        shapes,
        facts.room.outline.shape(),
        segmented_style(tokens),
        move |shape| msg(RoomMsg::ShapePicked { room, shape }),
    );
    let current = facts
        .sheets
        .iter()
        .find(|c| c.sheet.as_ref() == facts.room.source.as_ref().map(|s| &s.sheet))
        .cloned();
    let sheet = pick_list(current, facts.sheets.clone(), SheetChoice::to_string)
        .graphite()
        .font(mono())
        .on_select(move |choice: SheetChoice| {
            msg(RoomMsg::SheetPicked {
                room,
                sheet: choice.sheet,
            })
        })
        .width(Length::Fill);
    let colour = room_colour(&facts.room.colour);
    let colour = row![
        container(Space::new())
            .width(Length::Fixed(TS.body * 1.5))
            .height(Length::Fixed(TS.body * 1.5))
            .style(move |_| container::Style {
                background: Some(Background::Color(colour)),
                border: Border {
                    radius: (TS.body / 3.0).into(),
                    ..Default::default()
                },
                ..Default::default()
            }),
        text(UserText::new(facts.room.colour.clone()))
            .size(TS.caption)
            .font(mono())
            .color(resolve(tokens).ink3),
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center);
    column![
        form_row(
            t!(i18n, ids::ROOMS_FIELD_NAME),
            tokens,
            field(facts, RoomField::Name, i18n, false)
        ),
        form_row(t!(i18n, ids::ROOMS_FIELD_SHEET), tokens, sheet.boxed()),
        form_row(t!(i18n, ids::ROOMS_FIELD_LAYER), tokens, layer),
        form_row(t!(i18n, ids::ROOMS_FIELD_SHAPE), tokens, shape),
        form_row(t!(i18n, ids::ROOMS_FIELD_COLOUR), tokens, colour.boxed()),
    ]
    .spacing(SP.xs)
    .boxed()
}

/// Two fields side by side with the unit after them.
fn pair<'a>(
    facts: &RoomFacts,
    fields: [RoomField; 2],
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, PanelMsg> {
    row![
        field(facts, fields[0], i18n, true),
        field(facts, fields[1], i18n, true),
        text(t!(i18n, ids::ROOMS_UNIT_MM))
            .size(TS.caption)
            .color(resolve(tokens).ink3),
    ]
    .spacing(SP.xs)
    .align_y(Alignment::Center)
    .boxed()
}

fn position_section<'a>(
    facts: &RoomFacts,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, PanelMsg> {
    column![
        form_row(
            t!(i18n, ids::ROOMS_FIELD_XY),
            tokens,
            pair(facts, [RoomField::X, RoomField::Y], i18n, tokens)
        ),
        form_row(
            t!(i18n, ids::ROOMS_FIELD_SIZE),
            tokens,
            pair(facts, [RoomField::Width, RoomField::Height], i18n, tokens)
        ),
    ]
    .spacing(SP.xs)
    .boxed()
}

/// A part chip: its designator in mono on a sunken pill, warn-soft and
/// marked when it is outside.
fn chip<'a>(part: &RoomPart, i18n: &I18n, tokens: &ThemeTokens) -> Element<'a, PanelMsg> {
    let r = resolve(tokens);
    let outside = part.outside_nm > 0;
    let (fill, ink) = if outside {
        (r.warn_soft, ti(tokens.warning))
    } else {
        (r.sunken, r.ink3)
    };
    let label: Label<'static> = if outside {
        Label::from(t!(i18n, ids::ROOMS_PART_OUTSIDE, "part" => part.designator.clone()))
    } else {
        Label::from(UserText::new(part.designator.clone()))
    };
    container(text(label).size(TS.caption).font(mono()).color(ink))
        .padding([0.0, SP.s])
        .style(move |_| container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                radius: Radii::GRAPHITE.pill.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .boxed()
}

fn small_button<'a>(label: Localized, on_press: PanelMsg) -> Element<'a, PanelMsg> {
    button(text(label).size(TS.caption))
        .padding([SP.xs, SP.s])
        .style(form_look::ghost_button)
        .on_press(on_press)
        .boxed()
}

fn parts_section<'a>(
    facts: &RoomFacts,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, PanelMsg> {
    let room = facts.room.uuid;
    let outside = facts.outside().len();
    let caption = if outside > 0 {
        text(t!(i18n, ids::ROOMS_PARTS_OUTSIDE, "count" => outside))
            .size(TS.caption)
            .color(ti(tokens.warning))
    } else {
        text(t!(i18n, ids::ROOMS_PARTS_INSIDE, "count" => facts.parts.len()))
            .size(TS.caption)
            .color(resolve(tokens).ink3)
    };
    let head = row![
        text(t!(i18n, ids::ROOMS_SECTION_PARTS))
            .size(TS.section)
            .font(semibold())
            .color(resolve(tokens).ink3),
        caption,
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center);
    let mut chips = iced_aw::Wrap::new().spacing(SP.xs).line_spacing(SP.xs);
    for part in &facts.parts {
        chips = chips.push(chip(part, i18n, tokens));
    }
    let buttons: Row<'a, PanelMsg> = row![
        small_button(
            t!(i18n, ids::ROOMS_PULL),
            msg(RoomMsg::PullRequested { room })
        ),
        small_button(
            t!(i18n, ids::ROOMS_FIT),
            msg(RoomMsg::FitRequested { room })
        ),
    ]
    .spacing(SP.xs);
    column![head, chips, buttons].spacing(SP.s).boxed()
}

fn rule_section<'a>(facts: &RoomFacts, i18n: &I18n, tokens: &ThemeTokens) -> Element<'a, PanelMsg> {
    let room = facts.room.uuid;
    let keep = facts.room.keep_parts_inside;
    let dependents: Column<'a, PanelMsg> = column![
        text(t!(i18n, ids::ROOMS_KEEP_CAPTION))
            .size(TS.caption)
            .color(resolve(tokens).ink3),
        form_row(
            t!(i18n, ids::ROOMS_FIELD_MARGIN),
            tokens,
            row![
                field(facts, RoomField::Margin, i18n, true),
                text(t!(i18n, ids::ROOMS_UNIT_MM))
                    .size(TS.caption)
                    .color(resolve(tokens).ink3),
            ]
            .spacing(SP.xs)
            .align_y(Alignment::Center)
            .boxed()
        ),
    ]
    .spacing(SP.xs);
    let switch = EnableSection::new(t!(i18n, ids::ROOMS_KEEP), keep)
        .on_toggle(move |keep| msg(RoomMsg::KeepToggled { room, keep }))
        .view(
            super::enable_section::enable_section_style(tokens),
            dependents,
        );
    let mut col: Column<'a, PanelMsg> = column![switch].spacing(SP.s);
    if let Some(worst) = facts.outside().first() {
        col = col.push(banner(facts, worst, i18n, tokens));
    }
    col.boxed()
}

/// The warning under the rule: the part furthest outside, and a link that
/// pulls the parts in.
fn banner<'a>(
    facts: &RoomFacts,
    part: &RoomPart,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, PanelMsg> {
    let r = resolve(tokens);
    let warn = ti(tokens.warning);
    let distance = format!("{}", signex_drc::findings::mm(part.outside_nm));
    let line = t!(
        i18n, ids::ROOMS_OUTSIDE_BANNER,
        "part" => part.designator.clone(), "distance" => distance,
        "room" => facts.room.name.clone()
    );
    let room = facts.room.uuid;
    let link = button(
        text(t!(i18n, ids::ROOMS_PULL_LINK))
            .size(TS.caption)
            .color(warn),
    )
    .padding(0)
    .style(form_look::ghost_button)
    .on_press(msg(RoomMsg::PullRequested { room }));
    container(column![text(line).size(TS.caption).color(ti(tokens.text)), link].spacing(SP.xs))
        .padding([SP.s, SP.m])
        .width(Length::Fill)
        .style(move |_| container::Style {
            background: Some(Background::Color(r.warn_soft)),
            border: Border {
                color: warn,
                width: 1.0,
                radius: Radii::GRAPHITE.card.into(),
            },
            ..Default::default()
        })
        .boxed()
}
