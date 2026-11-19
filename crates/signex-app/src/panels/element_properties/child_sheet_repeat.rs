//! The multi-channel part of a sheet symbol's Properties (#1279): the
//! caption and line-art illustration under the header, the Repeat section
//! whose header is its switch, the Channel naming section and the Entries
//! list. Layout: the m151 mockup; rules: `docs/MULTI_CHANNEL.md`.

use iced::Widget as _;
use iced::widget::{Column, Space, column, container, mouse_area, row};
use iced::{Alignment, Background, Border, Element, Length, Point, Rectangle, Size, Theme};
use signex_i18n::{UserText, ids, t};
use signex_types::schematic::ChannelRepeat;
use signex_widgets::enable_section::EnableSection;
use signex_widgets::property_illustration::{
    Anchor, Callout, FieldId, Illustration, IllustrationMsg, Ink, Part, Primitive, Style,
    TextLabel, TextRole, Weight, property_illustration,
};
use signex_widgets::{text, text_input};

use super::super::compiled_design::{NamingTarget, RepeatField, SheetSymbolRepeatView, msg};
use super::super::{PROPERTY_LABEL_WIDTH, PanelContext, PanelMsg};
use crate::app::CompiledDesignMsg;
use crate::styles::{Radii, Spacing, TypeScale, ti, tokens::resolve};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// The fields the illustration's callouts are linked to.
pub(super) const FIELD_DESIGNATOR: FieldId = FieldId::new("sheet-symbol-designator");
pub(super) const FIELD_WIDTH: FieldId = FieldId::new("sheet-symbol-width");
pub(super) const FIELD_HEIGHT: FieldId = FieldId::new("sheet-symbol-height");

/// View box of the illustration.
const VIEW_W: f32 = 260.0;
const VIEW_H: f32 = 150.0;
/// Offsets of the two outlines behind a repeated sheet.
const STACK_STEP: f32 = 6.0;

/// "buck_channel.snxsch · repeated ×4" (or "· one sheet") under the header.
pub(super) fn caption<'a>(
    ctx: &'a PanelContext,
    view: &'a SheetSymbolRepeatView,
) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let line = match &view.repeat {
        Some(repeat) => t!(
            i18n,
            ids::PROPERTIES_SHEET_SYMBOL_REPEATED,
            "file" => view.file.as_str(),
            "count" => repeat.count()
        ),
        None => t!(
            i18n,
            ids::PROPERTIES_SHEET_SYMBOL_ONE_SHEET,
            "file" => view.file.as_str()
        ),
    };
    container(text(line).size(TS.caption).color(resolve(&ctx.tokens).ink3))
        .padding([SP.xs, SP.m])
        .width(Length::Fill)
        .boxed()
}

/// The transparent line-art drawing: the sheet (a stack with a ×N badge
/// when repeated), its entries, and Designator / Width / Height callouts.
pub(super) fn illustration<'a>(
    ctx: &'a PanelContext,
    view: &'a SheetSymbolRepeatView,
) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let count = view.repeat.as_ref().map_or(1, ChannelRepeat::count);
    let (w_mm, h_mm) = (
        view.size_mm.0.max(1.0) as f32,
        view.size_mm.1.max(1.0) as f32,
    );
    let scale = ((VIEW_W - 110.0) / w_mm).min((VIEW_H - 60.0) / h_mm);
    let (bw, bh) = (w_mm * scale, h_mm * scale);
    let (x0, y0) = (((VIEW_W - bw - 40.0) / 2.0).max(24.0), 24.0);
    let rect = |dx: f32| Rectangle::new(Point::new(x0 + dx, y0 + dx), Size::new(bw, bh));
    let mut ill = Illustration::new(VIEW_W, VIEW_H);
    if count > 2 {
        ill = ill.part(Part::new(
            Primitive::Rect {
                rect: rect(2.0 * STACK_STEP),
            },
            Style::outline(Ink::Faint, Weight::Thin),
        ));
    }
    if count > 1 {
        ill = ill.part(Part::new(
            Primitive::Rect {
                rect: rect(STACK_STEP),
            },
            Style::outline(Ink::Tertiary, Weight::Thin),
        ));
    }
    ill = ill.part(Part::new(
        Primitive::Rect { rect: rect(0.0) },
        Style::edge(),
    ));
    let shared = view
        .entries
        .iter()
        .filter(|(_, repeated)| !repeated)
        .count()
        .max(1);
    for (i, (name, repeated)) in view.entries.iter().enumerate().take(6) {
        let (x, y, anchor) = if *repeated {
            (x0 + bw - 4.0, y0 + bh * 0.375, Anchor::End)
        } else {
            let slot = (i + 1) as f32 / (shared + 1) as f32;
            (x0 + 4.0, y0 + bh * slot, Anchor::Start)
        };
        let ink = if *repeated {
            Ink::Accent
        } else {
            Ink::Secondary
        };
        ill = ill.part(Part::label(
            TextLabel {
                at: Point::new(x, y),
                text: UserText::new(name.clone()).into(),
                anchor,
                role: TextRole::Note,
            },
            ink,
        ));
    }
    if count > 1 {
        ill = ill.part(Part::label(
            TextLabel {
                at: Point::new(x0 + bw - 6.0, y0 + bh - 8.0),
                text: UserText::new(format!("×{count}")).into(),
                anchor: Anchor::End,
                role: TextRole::Mono,
            },
            Ink::Accent,
        ));
    }
    let designator_ink = if count > 1 {
        Ink::Accent
    } else {
        Ink::Secondary
    };
    ill = ill.part(
        Part::label(
            TextLabel {
                at: Point::new(x0, y0 - 8.0),
                text: UserText::new(view.designator.clone()).into(),
                anchor: Anchor::Start,
                role: TextRole::Mono,
            },
            designator_ink,
        )
        .linked(FIELD_DESIGNATOR),
    );
    let stack = if count > 2 {
        2.0 * STACK_STEP
    } else if count > 1 {
        STACK_STEP
    } else {
        0.0
    };
    let yb = y0 + bh + stack + 12.0;
    ill = ill.part(
        Part::callout(
            Callout::new(Point::new(x0, yb), Point::new(x0 + bw, yb))
                .label(t!(i18n, ids::PROPERTIES_SHEET_SYMBOL_WIDTH))
                .extension(Point::new(x0, y0 + bh + 2.0), Point::new(x0, yb + 3.0))
                .extension(
                    Point::new(x0 + bw, y0 + bh + 2.0),
                    Point::new(x0 + bw, yb + 3.0),
                ),
        )
        .linked(FIELD_WIDTH),
    );
    let xr = x0 + bw + stack + 12.0;
    ill = ill.part(
        Part::callout(
            Callout::new(Point::new(xr, y0), Point::new(xr, y0 + bh))
                .label(t!(i18n, ids::PROPERTIES_SHEET_SYMBOL_HEIGHT))
                .label_at(Point::new(xr + 6.0, y0 + bh / 2.0), Anchor::Start)
                .extension(Point::new(x0 + bw + 2.0, y0), Point::new(xr + 3.0, y0))
                .extension(
                    Point::new(x0 + bw + 2.0, y0 + bh),
                    Point::new(xr + 3.0, y0 + bh),
                ),
        )
        .linked(FIELD_HEIGHT),
    );
    container(
        property_illustration(ill, &view.hover, &ctx.tokens, TS.caption)
            .map(|m| msg(CompiledDesignMsg::IllustrationHovered(m))),
    )
    .padding([SP.xs, SP.m])
    .width(Length::Fill)
    .boxed()
}

/// A field row that lights its callout while hovered.
pub(super) fn linked<'a>(field: FieldId, row: Element<'a, PanelMsg>) -> Element<'a, PanelMsg> {
    mouse_area(row)
        .on_enter(msg(CompiledDesignMsg::IllustrationHovered(
            IllustrationMsg::FieldHovered(field.clone()),
        )))
        .on_exit(msg(CompiledDesignMsg::IllustrationHovered(
            IllustrationMsg::FieldLeft(field),
        )))
        .boxed()
}

/// Label | control row of the 96 px label grid.
fn form_row<'a>(
    ctx: &PanelContext,
    label: signex_i18n::Localized,
    control: Element<'a, PanelMsg>,
) -> Element<'a, PanelMsg> {
    container(
        row![
            text(label)
                .size(TS.body)
                .color(resolve(&ctx.tokens).ink3)
                .width(PROPERTY_LABEL_WIDTH),
            control,
        ]
        .spacing(SP.s)
        .align_y(Alignment::Center),
    )
    .padding([SP.s, SP.m])
    .width(Length::Fill)
    .boxed()
}

fn field<'a>(
    view: &SheetSymbolRepeatView,
    which: RepeatField,
    value: &str,
) -> Element<'a, PanelMsg> {
    let id = view.sheet_symbol;
    text_input(UserText::new(String::new()), value.to_string())
        .size(TS.body)
        .padding(SP.s)
        .width(Length::Fill)
        .on_input(move |text| {
            msg(CompiledDesignMsg::RepeatFieldTyped {
                sheet_symbol: id,
                field: which,
                text,
            })
        })
        .on_submit(msg(CompiledDesignMsg::RepeatFieldSubmitted {
            sheet_symbol: id,
        }))
        .style(super::super::form_look::text_input_style)
        .boxed()
}

/// The Repeat section (its header is the switch), then Channel naming and
/// the Entries list.
pub(super) fn sections<'a>(
    ctx: &'a PanelContext,
    view: &'a SheetSymbolRepeatView,
) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let id = view.sheet_symbol;
    let is_on = view.repeat.is_some();
    let names: Vec<String> = view
        .repeat
        .iter()
        .flat_map(|r| r.occurrences())
        .map(|c| c.name)
        .collect();
    let shown = if names.len() > 6 {
        format!("{}, …", names[..6].join(", "))
    } else {
        names.join(", ")
    };
    let channels = text(t!(
        i18n,
        ids::PROPERTIES_SHEET_SYMBOL_CHANNELS,
        "count" => names.len(),
        "names" => shown
    ))
    .size(TS.caption)
    .color(resolve(&ctx.tokens).ink3);
    let repeat_rows = column![
        form_row(
            ctx,
            t!(i18n, ids::PROPERTIES_SHEET_SYMBOL_CHANNEL_NAME),
            field(view, RepeatField::Channel, &view.channel_text),
        ),
        form_row(
            ctx,
            t!(i18n, ids::PROPERTIES_SHEET_SYMBOL_FROM_TO),
            row![
                field(view, RepeatField::From, &view.from_text),
                field(view, RepeatField::To, &view.to_text),
            ]
            .spacing(SP.s)
            .boxed(),
        ),
        container(channels).padding([SP.xs, SP.m]),
    ];
    let repeat = EnableSection::new(t!(i18n, ids::PROPERTIES_SHEET_SYMBOL_REPEAT), is_on)
        .on_toggle(move |on| {
            msg(CompiledDesignMsg::RepeatToggled {
                sheet_symbol: id,
                on,
            })
        })
        .tip(t!(i18n, ids::PROPERTIES_SHEET_SYMBOL_REPEAT_TIP))
        .view(
            super::super::enable_section::enable_section_style(&ctx.tokens),
            repeat_rows,
        );
    let naming = super::channel_naming_menu::naming_section(ctx, view);
    let naming: Element<'a, PanelMsg> = if is_on {
        naming
    } else {
        let veil = signex_widgets::enable_section::dependents_veil(ti(ctx.tokens.panel_bg), false)
            .unwrap_or(iced::Color::TRANSPARENT);
        signex_widgets::inert::Inert::new(naming, veil).boxed()
    };
    column![repeat, naming, entries(ctx, view)]
        .width(Length::Fill)
        .boxed()
}

/// Entry | Per channel: plain entries are "shared", `Repeat(…)` entries
/// carry an accent Repeat pill.
fn entries<'a>(ctx: &'a PanelContext, view: &'a SheetSymbolRepeatView) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let tokens = &ctx.tokens;
    let res = resolve(tokens);
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    let head = row![
        text(t!(i18n, ids::PROPERTIES_SHEET_SYMBOL_ENTRY))
            .size(TS.section)
            .color(res.ink3)
            .width(Length::Fill),
        text(t!(i18n, ids::PROPERTIES_SHEET_SYMBOL_PER_CHANNEL))
            .size(TS.section)
            .color(res.ink3),
    ];
    let mut list: Column<Element<'a, PanelMsg>> = column![head].spacing(SP.s).padding([SP.s, SP.m]);
    let (accent, on_accent) = (ti(tokens.accent), res.primary_ink);
    for (name, repeated) in &view.entries {
        let tag: Element<'a, PanelMsg> = if *repeated {
            container(
                text(t!(i18n, ids::PROPERTIES_SHEET_SYMBOL_REPEAT_PILL))
                    .size(TS.caption)
                    .color(on_accent),
            )
            .padding([0.0, SP.s])
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(accent)),
                border: Border {
                    radius: Radii::GRAPHITE.pill.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            })
            .boxed()
        } else {
            text(t!(i18n, ids::PROPERTIES_SHEET_SYMBOL_SHARED))
                .size(TS.caption)
                .color(res.ink3)
                .boxed()
        };
        list = list.push(
            row![
                text(UserText::new(name.as_str()))
                    .size(TS.body)
                    .font(mono)
                    .width(Length::Fill),
                tag,
            ]
            .align_y(Alignment::Center),
        );
    }
    ctx.collapsible(
        "sel_child_sheet_entries",
        ids::PROPERTIES_SHEET_SYMBOL_ENTRIES,
        &ctx.collapsed_sections,
        move || column![list.boxed(), Space::new().height(SP.s)],
    )
    .boxed()
}

/// The naming target a menu row belongs to, for the menu module.
pub(super) fn target_label(target: NamingTarget) -> signex_i18n::MessageId {
    match target {
        NamingTarget::Designators => ids::PROPERTIES_SHEET_SYMBOL_DESIGNATORS,
        NamingTarget::Nets => ids::PROPERTIES_SHEET_SYMBOL_LOCAL_NETS,
    }
}
