//! The Projects panel with no project open: the empty card with New
//! Project… and Open…, then a foldable list of the recent projects that
//! opens one as the Start screen's recent row does.

use std::path::Path;

use iced::widget::{Column, column, container, row, svg};
use iced::{Background, Border, Color, Element, Length, Theme, Widget as _};
use signex_i18n::{Localized, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::tree_view::TreeIcon;
use signex_widgets::{button, text};

use super::super::{PanelContext, PanelMsg};
use super::{ProjectsMsg, SP, TS};
use crate::styles::tokens::{Overlay, overlay, resolve};
use crate::styles::{self, ChromeMetrics, Radii, ti};

/// Space above the empty card.
const CARD_TOP: f32 = 24.0;
/// Padding inside the empty card, vertical and horizontal.
const CARD_PAD: [f32; 2] = [18.0, 14.0];
/// Side of the round disc behind the project glyph.
const DISC: f32 = 36.0;
/// Side of a project glyph (disc and recent rows).
const GLYPH: f32 = 16.0;
/// Space left and right of a card button's label.
const BUTTON_PAD_X: f32 = 10.0;
/// Height of the Recent projects header.
const SECTION_HEIGHT: f32 = 26.0;
/// Space between a recent row's glyph and its name, and its side padding.
const ROW_GAP: f32 = 6.0;

/// The panel body when no project is open.
pub(crate) fn view<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let mut col: Column<Element<'a, PanelMsg>> =
        Column::new().width(Length::Fill).push(empty_card(ctx));
    if !ctx.recent_projects.is_empty() {
        col = col.push(recent_header(ctx));
        if !ctx.projects_view.recent_collapsed {
            for path in &ctx.recent_projects {
                col = col.push(recent_row(path, ctx));
            }
        }
    }
    col.boxed()
}

/// A project glyph of [`GLYPH`] px tinted `ink`.
fn project_glyph<'a>(ink: Color) -> Element<'a, PanelMsg> {
    svg(TreeIcon::SnxProject.svg())
        .width(GLYPH)
        .height(GLYPH)
        .style(move |_: &Theme, _| svg::Style {
            color: Some(ink),
            ..Default::default()
        })
        .boxed()
}

/// "No project open", the hint and the two buttons, centred in a card. iced
/// draws no dashed border; the mock's dashed edge is a solid `line_strong`
/// hairline here.
fn empty_card<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let (tokens, i18n) = (&ctx.tokens, &*ctx.i18n);
    let r = resolve(tokens);
    let sunken = r.sunken;
    let disc = container(project_glyph(r.ink3))
        .width(DISC)
        .height(DISC)
        .center_x(DISC)
        .center_y(DISC)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(sunken)),
            border: Border {
                radius: (DISC / 2.0).into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
    let title = text(t!(i18n, ids::PANELS_PROJECTS_NONE_OPEN))
        .size(TS.body)
        .color(ti(tokens.text))
        .font(iced::Font {
            weight: iced::font::Weight::Medium,
            ..signex_widgets::ui_font::regular()
        });
    let hint = text(t!(i18n, ids::PANELS_PROJECTS_OPEN_HINT))
        .size(TS.body)
        .color(r.ink3);
    let buttons = row![
        card_button(
            t!(i18n, ids::PANELS_PROJECTS_NEW_PROJECT),
            true,
            ProjectsMsg::NewProjectPressed,
            tokens,
        ),
        card_button(
            t!(i18n, ids::PANELS_PROJECTS_OPEN),
            false,
            ProjectsMsg::OpenPressed,
            tokens,
        ),
    ]
    .spacing(SP.s);
    let line = r.line_strong;
    let card = container(
        column![disc.boxed(), title.boxed(), hint.boxed(), buttons.boxed()]
            .spacing(SP.m)
            .align_x(iced::Alignment::Center)
            .width(Length::Fill),
    )
    .padding(CARD_PAD)
    .width(Length::Fill)
    .style(move |_: &Theme| container::Style {
        border: Border {
            width: 1.0,
            color: line,
            radius: Radii::GRAPHITE.card.into(),
            ..Border::default()
        },
        ..container::Style::default()
    });
    container(card)
        .padding(iced::Padding {
            top: CARD_TOP,
            right: SP.m,
            bottom: SP.m,
            left: SP.m,
        })
        .width(Length::Fill)
        .boxed()
}

/// A button of the empty card: the theme's primary fill, or the field fill
/// with a hairline border.
fn card_button<'a>(
    label: Localized,
    primary: bool,
    msg: ProjectsMsg,
    tokens: &ThemeTokens,
) -> Element<'a, PanelMsg> {
    let r = resolve(tokens);
    let ink = if primary {
        r.primary_ink
    } else {
        ti(tokens.text)
    };
    let face = text(label)
        .size(TS.body)
        .color(ink)
        .wrapping(iced::widget::text::Wrapping::None);
    let btn = button(container(face).center_y(Length::Fill))
        .height(ChromeMetrics::DEFAULT.field)
        .padding([0.0, BUTTON_PAD_X])
        .on_press(PanelMsg::Projects(msg));
    if primary {
        return btn.style(styles::primary_button(tokens)).boxed();
    }
    let (field, edge) = (r.field, ti(tokens.border));
    let hover = overlay(tokens, Overlay::Hover);
    btn.style(move |_: &Theme, status: button::Status| button::Style {
        background: Some(Background::Color(match status {
            button::Status::Hovered | button::Status::Pressed => hover,
            _ => field,
        })),
        text_color: ink,
        border: Border {
            width: 1.0,
            color: edge,
            radius: Radii::GRAPHITE.button.into(),
            ..Border::default()
        },
        ..button::Style::default()
    })
    .boxed()
}

/// The Recent projects header: a chevron and the uppercase label over a
/// hairline; a click folds the list.
fn recent_header<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let (tokens, i18n) = (&ctx.tokens, &*ctx.i18n);
    let r = resolve(tokens);
    let open = !ctx.projects_view.recent_collapsed;
    let label = i18n.upper_for_display(&i18n.message_plain(ids::PANELS_PROJECTS_RECENT, &[]));
    let face = row![
        signex_widgets::chevron::chevron(open, TS.section, r.ink4).boxed(),
        text(UserText::new(label))
            .size(TS.section)
            .color(r.ink3)
            .font(iced::Font {
                weight: iced::font::Weight::Medium,
                ..signex_widgets::ui_font::regular()
            })
            .wrapping(iced::widget::text::Wrapping::None)
            .boxed(),
    ]
    .spacing(SP.s)
    .align_y(iced::Alignment::Center);
    let header = button(face)
        .width(Length::Fill)
        .height(SECTION_HEIGHT)
        .padding(iced::Padding {
            top: 0.0,
            right: SP.s,
            bottom: 0.0,
            left: 2.0,
        })
        .on_press(PanelMsg::Projects(ProjectsMsg::RecentToggled))
        .style(|_: &Theme, _| button::Style::default());
    let line = r.line_soft;
    let ruled = container(header)
        .width(Length::Fill)
        .style(move |_: &Theme| {
            let mut border = Border {
                color: line,
                ..Border::default()
            };
            border.top = iced::border::Side::default().width(1.0);
            container::Style {
                border,
                ..container::Style::default()
            }
        });
    container(ruled)
        .padding(iced::Padding {
            top: SP.m,
            right: SP.s,
            bottom: 0.0,
            left: SP.s,
        })
        .width(Length::Fill)
        .boxed()
}

/// The name a recent row shows: the project file's stem.
fn recent_name(path: &Path) -> String {
    path.file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

/// One recent project: the accent project glyph and the file stem; a click
/// opens it through File ▸ Open Recent.
fn recent_row<'a>(path: &'a Path, ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let tokens = &ctx.tokens;
    let ink = ti(tokens.text);
    let face = row![
        project_glyph(ti(tokens.accent)),
        text(UserText::new(recent_name(path)))
            .size(TS.body)
            .color(ink)
            .wrapping(iced::widget::text::Wrapping::None)
            .boxed(),
    ]
    .spacing(ROW_GAP)
    .align_y(iced::Alignment::Center);
    let (hover, pressed) = (
        overlay(tokens, Overlay::Hover),
        overlay(tokens, Overlay::Pressed),
    );
    let row_button = button(container(face).center_y(Length::Fill))
        .width(Length::Fill)
        .height(ChromeMetrics::DEFAULT.row)
        .padding([0.0, ROW_GAP])
        .on_press(PanelMsg::Projects(ProjectsMsg::RecentProjectPressed(
            path.to_path_buf(),
        )))
        .style(move |_: &Theme, status: button::Status| button::Style {
            background: match status {
                button::Status::Hovered => Some(Background::Color(hover)),
                button::Status::Pressed => Some(Background::Color(pressed)),
                _ => None,
            },
            text_color: ink,
            border: Border {
                radius: Radii::GRAPHITE.small.into(),
                ..Border::default()
            },
            ..button::Style::default()
        });
    container(row_button)
        .padding([0.0, SP.s])
        .width(Length::Fill)
        .boxed()
}
