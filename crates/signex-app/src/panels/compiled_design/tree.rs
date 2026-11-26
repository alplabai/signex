//! The Sheets mode: the compiled hierarchy as a tree of rows with Parts and
//! Nets columns, and the out-of-date banner above it.

use iced::widget::{Column, Space, column, container, row, scrollable, svg};
use iced::{Alignment, Background, Border, Element, Length, Theme, Widget as _};
use signex_i18n::{UserText, ids, t};
use signex_widgets::command_press::CommandPress;
use signex_widgets::text;

use super::{
    CompiledPanelView, CompiledRow, CompiledRowKind, PanelContext, PanelMsg, SP, TS, msg, rule,
};
use crate::app::CompiledDesignMsg;
use crate::styles::{ti, tokens::resolve};

/// Row height: 26 px at the 12 px body.
pub(super) const ROW_H: f32 = TS.body * 26.0 / 12.0;
/// Width of the Parts and Nets columns: 40 px at the 12 px body.
pub(super) const COUNT_W: f32 = TS.body * 40.0 / 12.0;
/// Indent per hierarchy level: 16 px at the 12 px body.
const INDENT: f32 = TS.body * 16.0 / 12.0;
/// Width of the selected row's accent bar.
const BAR_W: f32 = 2.0;
/// Diameter of the amber out-of-date dot.
const DOT: f32 = TS.body * 6.0 / 12.0;
/// Size of a row icon.
const ICON: f32 = TS.body * 14.0 / 12.0;

/// The heading row, then one row per sheet, repeated sheet and channel.
pub(super) fn view<'a>(
    ctx: &'a PanelContext,
    view: &'a CompiledPanelView,
) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let muted = resolve(&ctx.tokens).ink3;
    let mut list: Column<Element<'a, PanelMsg>> = column![
        heading(
            ctx,
            t!(i18n, ids::PANELS_COMPILED_DESIGN_COLUMN_SHEET),
            t!(i18n, ids::PANELS_COMPILED_DESIGN_COLUMN_PARTS),
            Some(t!(i18n, ids::PANELS_COMPILED_DESIGN_COLUMN_NETS)),
        ),
        rule(ctx),
    ]
    .width(Length::Fill);
    if view.rows.is_empty() {
        list = list.push(
            container(
                text(t!(i18n, ids::PANELS_COMPILED_DESIGN_NO_MATCH))
                    .size(TS.body)
                    .color(muted),
            )
            .padding([SP.m, SP.l])
            .boxed(),
        );
    }
    for r in &view.rows {
        list = list.push(sheet_row(ctx, view, r));
        if r.is_expanded {
            for child in child_rows(ctx, r) {
                list = list.push(child);
            }
        }
    }
    scrollable(list)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(super::super::form_look::scrollbar_style)
        .boxed()
}

/// The column heads of both modes.
pub(super) fn heading<'a>(
    ctx: &PanelContext,
    first: signex_i18n::Localized,
    second: signex_i18n::Localized,
    third: Option<signex_i18n::Localized>,
) -> Element<'a, PanelMsg> {
    let muted = resolve(&ctx.tokens).ink3;
    let head = |label: signex_i18n::Localized, w: Length, right: bool| -> Element<'a, PanelMsg> {
        let mut t = text(label).size(TS.section).color(muted).width(w);
        if right {
            t = t.align_x(iced::alignment::Horizontal::Right);
        }
        t.boxed()
    };
    let third: Element<'a, PanelMsg> = match third {
        Some(label) => head(label, Length::Fixed(COUNT_W), true),
        None => Space::new().width(COUNT_W).height(Length::Shrink).boxed(),
    };
    row![
        head(first, Length::Fill, false),
        head(second, Length::Fixed(COUNT_W), true),
        third,
    ]
    .height(ROW_H)
    .padding([0.0, SP.l])
    .align_y(Alignment::Center)
    .boxed()
}

/// One tree row: the amber dot when stale, the indent, the fold chevron of a
/// repeated row, the icon, the name (never cut) and its caption (cut first),
/// then the counts.
fn sheet_row<'a>(
    ctx: &'a PanelContext,
    view: &'a CompiledPanelView,
    r: &'a CompiledRow,
) -> Element<'a, PanelMsg> {
    let tokens = &ctx.tokens;
    let res = resolve(tokens);
    let warn = ti(tokens.warning);
    let ink = if r.is_stale { warn } else { ti(tokens.text) };
    let muted = if r.is_stale { warn } else { res.ink3 };
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);

    let mut name = text(UserText::new(r.name.as_str()))
        .size(TS.body)
        .color(ink)
        .wrapping(iced::widget::text::Wrapping::None);
    if r.kind == CompiledRowKind::Channel {
        name = name.font(mono);
    }
    let mut label = row![name].spacing(SP.s).align_y(Alignment::Center);
    if let Some(caption) = &r.caption {
        label = label.push(
            container(
                text(UserText::new(caption.as_str()))
                    .size(TS.caption)
                    .font(mono)
                    .color(muted)
                    .wrapping(iced::widget::text::Wrapping::None),
            )
            .width(Length::Fill)
            .clip(true)
            .boxed(),
        );
    }

    let lead: Element<'a, PanelMsg> = match r.kind {
        CompiledRowKind::Repeated { open } => {
            signex_widgets::button(signex_widgets::chevron::chevron(open, TS.body, res.ink3))
                .padding(0)
                .on_press(msg(CompiledDesignMsg::RowToggled(r.index)))
                .style(crate::styles::menu_item(tokens))
                .boxed()
        }
        _ if r.has_children => signex_widgets::button(signex_widgets::chevron::chevron(
            r.is_expanded,
            TS.body,
            res.ink3,
        ))
        .padding(0)
        .on_press(msg(CompiledDesignMsg::RowExpanded(r.index)))
        .style(crate::styles::menu_item(tokens))
        .boxed(),
        _ => Space::new().width(TS.body).height(Length::Shrink).boxed(),
    };
    let icon = match r.kind {
        CompiledRowKind::Sheet => crate::icons::icon_dd_sheet_symbol(ctx.theme_id),
        CompiledRowKind::Repeated { .. } => crate::icons::icon_dd_sheet_actions(ctx.theme_id),
        CompiledRowKind::Channel => crate::icons::icon_dd_device_sheet(ctx.theme_id),
    };
    let dot: Element<'a, PanelMsg> = if r.is_stale {
        stale_dot(warn)
    } else {
        Space::new().width(DOT).height(Length::Shrink).boxed()
    };
    let count = |n: usize| -> Element<'a, PanelMsg> {
        text(UserText::new(n.to_string()))
            .size(TS.caption)
            .font(mono)
            .color(ink)
            .width(COUNT_W)
            .align_x(iced::alignment::Horizontal::Right)
            .boxed()
    };
    let line = row![
        dot,
        Space::new()
            .width(INDENT * r.depth as f32)
            .height(Length::Shrink),
        lead,
        svg(icon).width(ICON).height(ICON),
        container(label).width(Length::Fill).clip(true),
        count(r.parts),
        count(r.nets),
    ]
    .spacing(SP.s)
    .height(ROW_H)
    .align_y(Alignment::Center);

    let selected = r.is_selected;
    let (soft, accent) = (res.accent_soft, ti(tokens.accent));
    let hover = ti(tokens.hover);
    let tip = (r.is_stale).then(|| {
        t!(
            ctx.i18n,
            ids::PANELS_COMPILED_DESIGN_STALE_ROW,
            "time" => view.compiled_at.as_str()
        )
    });
    let button = signex_widgets::button(line)
        .width(Length::Fill)
        .padding([0.0, SP.l])
        .on_press(msg(CompiledDesignMsg::RowClicked(r.index)))
        .style(move |_: &Theme, status: iced::widget::button::Status| {
            let hovered = matches!(status, iced::widget::button::Status::Hovered);
            let fill = match (selected, hovered) {
                (true, _) => Some(Background::Color(soft)),
                (false, true) => Some(Background::Color(hover)),
                (false, false) => None,
            };
            iced::widget::button::Style {
                background: fill,
                ..iced::widget::button::Style::default()
            }
        });
    // Ctrl+click (Cmd on macOS) opens the source sheet on Editor (#2658).
    let button: Element<'a, PanelMsg> = CommandPress::new(
        button.boxed(),
        msg(CompiledDesignMsg::RowSourceClicked(r.index)),
    )
    .boxed();
    let body: Element<'a, PanelMsg> = match tip {
        Some(tip) => signex_widgets::tooltip(
            button,
            container(text(tip).size(TS.caption))
                .padding([SP.xs, SP.s])
                .style(crate::styles::context_menu(tokens)),
            signex_widgets::tooltip::Position::Bottom,
        )
        .boxed(),
        None => Element::from(button),
    };
    if !selected {
        return body;
    }
    row![bar(accent), body].boxed()
}

/// The Parts and Nets of an expanded sheet instance, one level in.
fn child_rows<'a>(ctx: &'a PanelContext, r: &'a CompiledRow) -> Vec<Element<'a, PanelMsg>> {
    let i18n = &*ctx.i18n;
    let indent = DOT + SP.s + INDENT * (r.depth + 1) as f32 + TS.body + SP.s;
    let mut out = Vec::with_capacity(r.part_rows.len() + r.net_rows.len() + 2);
    let groups = [
        (
            t!(i18n, ids::PANELS_COMPILED_DESIGN_COLUMN_PARTS),
            &r.part_rows,
        ),
        (
            t!(i18n, ids::PANELS_COMPILED_DESIGN_COLUMN_NETS),
            &r.net_rows,
        ),
    ];
    for (title, rows) in groups {
        if rows.is_empty() {
            continue;
        }
        out.push(
            row![
                Space::new().width(indent).height(Length::Shrink),
                text(title)
                    .size(TS.section)
                    .color(resolve(&ctx.tokens).ink3),
            ]
            .height(ROW_H)
            .padding([0.0, SP.l])
            .align_y(Alignment::Center)
            .boxed(),
        );
        out.extend(rows.iter().map(|child| child_row(ctx, child, indent)));
    }
    out
}

/// One part or net under a sheet instance; a click goes to it.
fn child_row<'a>(
    ctx: &'a PanelContext,
    child: &'a super::CompiledChildRow,
    indent: f32,
) -> Element<'a, PanelMsg> {
    let ink = if child.is_power {
        ti(ctx.canvas_colors.power)
    } else {
        ti(ctx.tokens.text)
    };
    let hover = ti(ctx.tokens.hover);
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    let line = row![
        Space::new().width(indent).height(Length::Shrink),
        container(
            text(UserText::new(child.label.as_str()))
                .size(TS.body)
                .font(mono)
                .color(ink)
                .wrapping(iced::widget::text::Wrapping::None),
        )
        .width(Length::Fill)
        .clip(true),
    ]
    .height(ROW_H)
    .align_y(Alignment::Center);
    signex_widgets::button(line)
        .width(Length::Fill)
        .padding([0.0, SP.l])
        .on_press(msg(CompiledDesignMsg::TargetClicked(child.target.clone())))
        .style(
            move |_: &Theme, status: iced::widget::button::Status| iced::widget::button::Style {
                background: matches!(status, iced::widget::button::Status::Hovered)
                    .then_some(Background::Color(hover)),
                ..iced::widget::button::Style::default()
            },
        )
        .boxed()
}

/// The 2 px accent bar at the start of the selected row.
fn bar<'a>(accent: iced::Color) -> Element<'a, PanelMsg> {
    container(Space::new())
        .width(BAR_W)
        .height(ROW_H)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(accent)),
            ..container::Style::default()
        })
        .boxed()
}

/// The amber dot of a sheet changed since the compile.
fn stale_dot<'a>(warn: iced::Color) -> Element<'a, PanelMsg> {
    container(Space::new())
        .width(DOT)
        .height(DOT)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(warn)),
            border: Border {
                radius: (DOT / 2.0).into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// The warn banner over the tree: how many sheets changed since the
/// compile, and a Recompile button. Recompile is never automatic.
pub(super) fn stale_banner<'a>(
    ctx: &'a PanelContext,
    view: &'a CompiledPanelView,
) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let tokens = &ctx.tokens;
    let res = resolve(tokens);
    let (fill, edge) = (res.warn_soft, ti(tokens.warning));
    let recompile = signex_widgets::button(
        text(t!(i18n, ids::PANELS_COMPILED_DESIGN_RECOMPILE)).size(TS.caption),
    )
    .padding([SP.xs, SP.m])
    .on_press(msg(CompiledDesignMsg::CompileRequested))
    .style(crate::styles::ghost_button(tokens));
    let line = text(t!(
        i18n,
        ids::PANELS_COMPILED_DESIGN_STALE,
        "count" => view.stale_count,
        "time" => view.compiled_at.as_str()
    ))
    .size(TS.caption)
    .color(ti(tokens.text));
    container(
        container(column![line, recompile].spacing(SP.s))
            .padding([SP.s, SP.m])
            .width(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(fill)),
                border: Border {
                    color: edge,
                    width: 1.0,
                    radius: crate::styles::Radii::GRAPHITE.control.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            }),
    )
    .padding(iced::Padding {
        top: SP.m,
        right: SP.l,
        bottom: 0.0,
        left: SP.l,
    })
    .width(Length::Fill)
    .boxed()
}
