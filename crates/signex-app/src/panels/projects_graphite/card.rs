//! The project card of the Projects panel: icon tile, name, real counts, a
//! "git" pill when the project has version control on, and a sliders button
//! that opens the project's options.

use iced::widget::{container, mouse_area, svg};
use iced::{Background, Border, Color, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::tree_view::{TreeIcon, TreeMsg, TreeNode};
use signex_widgets::{button, text};

use super::super::graphite::hint_tooltip;
use super::super::{PanelContext, PanelMsg, ProjectPanelInfo, SheetInfo};
use super::{ProjectsMsg, SP, TS};
use crate::styles::tokens::{Overlay, overlay, resolve};
use crate::styles::{self, PillTone, Radii, ti};

/// Side of the project icon tile: a field's height, so it scales with the UI
/// font.
pub(super) const CARD_ICON_TILE: f32 = crate::styles::ChromeMetrics::DEFAULT.field;
/// Side of the Project Options button: the tile it shares the row with.
pub(super) const OPTIONS_BUTTON: f32 = CARD_ICON_TILE;
/// Side of the Project Options glyph: a third larger than the body text.
const OPTIONS_GLYPH: f32 = TS.body * 4.0 / 3.0;
/// Side of the glyph in the project icon tile.
const ICON_GLYPH: f32 = 14.0;
/// Height of the "git" pill.
const PILL_HEIGHT: f32 = 16.0;
/// Space left and right of the pill's text.
const PILL_PAD_X: f32 = 6.0;
/// Side of the unsaved-changes dot.
const DIRTY_DOT: f32 = TS.caption / 2.0;

fn count_sheets(sheets: &[SheetInfo]) -> usize {
    sheets.iter().map(|s| 1 + count_sheets(&s.children)).sum()
}

/// Give each sheet row of the Schematics group its page position, "1/2" and
/// "2/2", in tree order with child sheets after their parent, so the total
/// is the card's sheet count. The row keeps it when the filter hides its
/// neighbours; a lone sheet gets none (mockup rows 5 and 19).
pub(super) fn number_sheets(rows: &mut [TreeNode]) {
    fn count(rows: &[TreeNode]) -> usize {
        rows.iter().map(|row| 1 + count(&row.children)).sum()
    }
    fn walk(rows: &mut [TreeNode], next: &mut usize, total: usize) {
        for row in rows {
            *next += 1;
            row.meta = Some(format!("{next}/{total}"));
            walk(&mut row.children, next, total);
        }
    }
    let total = count(rows);
    if total > 1 {
        walk(rows, &mut 0, total);
    }
}

/// "3 sheets · 1 board · 2 libraries": the counts the project really has.
fn card_summary(i18n: &I18n, project: &ProjectPanelInfo) -> String {
    let sheets = if project.sheets.is_empty() {
        usize::from(project.project_file.is_some())
    } else {
        count_sheets(&project.sheets)
    };
    let mut parts = Vec::new();
    if sheets > 0 {
        parts.push(t!(i18n, ids::PANELS_PROJECTS_SHEETS, "count" => sheets).into_string());
    }
    if project.pcb_file.is_some() {
        parts.push(t!(i18n, ids::PANELS_PROJECTS_BOARDS, "count" => 1_usize).into_string());
    }
    if !project.libraries.is_empty() {
        parts.push(
            t!(i18n, ids::PANELS_PROJECTS_LIBRARIES, "count" => project.libraries.len())
                .into_string(),
        );
    }
    parts.join(" · ")
}

/// A square glyph button of `side` px: no fill at rest, the hover overlay,
/// small radius. The glyph is tinted `ink`.
pub(super) fn icon_button<'a>(
    glyph: svg::Handle,
    side: f32,
    glyph_side: f32,
    ink: Color,
    on_press: PanelMsg,
    tokens: &ThemeTokens,
) -> Element<'a, PanelMsg> {
    let (hover, pressed) = (
        overlay(tokens, Overlay::Hover),
        overlay(tokens, Overlay::Pressed),
    );
    let face = svg(glyph)
        .width(glyph_side)
        .height(glyph_side)
        .style(move |_: &Theme, _| svg::Style {
            color: Some(ink),
            ..Default::default()
        });
    button(
        container(face)
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .width(side)
    .height(side)
    .padding(0)
    .on_press(on_press)
    .style(move |_: &Theme, status: button::Status| button::Style {
        background: match status {
            button::Status::Hovered => Some(Background::Color(hover)),
            button::Status::Pressed => Some(Background::Color(pressed)),
            _ => None,
        },
        border: Border {
            radius: Radii::GRAPHITE.small.into(),
            ..Border::default()
        },
        ..button::Style::default()
    })
    .boxed()
}

/// The unsaved-changes dot after the project's name. The tree has no project
/// row to carry it when the groups sit directly under the card.
fn dirty_dot<'a>(colour: Color) -> Element<'a, PanelMsg> {
    container(iced::widget::Space::new().boxed())
        .width(DIRTY_DOT)
        .height(DIRTY_DOT)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(colour)),
            border: Border {
                radius: (DIRTY_DOT / 2.0).into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// The project card; the sliders button opens Project Options… for this
/// project, and a right-click opens the project's menu (mockup row 20).
pub(super) fn project_card<'a>(
    project: &'a ProjectPanelInfo,
    ctx: &'a PanelContext,
) -> Element<'a, PanelMsg> {
    let (tokens, i18n) = (&ctx.tokens, &*ctx.i18n);
    let r = resolve(tokens);
    let (tile_fill, ink, ink4) = (r.accent_soft, ti(tokens.text), r.ink4);
    let tint = r.accent_text;
    let icon = svg(TreeIcon::SnxProject.svg())
        .width(ICON_GLYPH)
        .height(ICON_GLYPH)
        .style(move |_: &Theme, _| svg::Style {
            color: Some(tint),
            ..Default::default()
        });
    let tile = container(icon)
        .width(CARD_ICON_TILE)
        .height(CARD_ICON_TILE)
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(tile_fill)),
            border: Border {
                radius: Radii::GRAPHITE.control.into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
    let name = text(UserText::new(project.name.clone()))
        .size(TS.title)
        .color(ink)
        .font(iced::Font {
            weight: iced::font::Weight::Semibold,
            ..signex_widgets::ui_font::regular()
        })
        .wrapping(iced::widget::text::Wrapping::None);
    let summary = text(UserText::new(card_summary(i18n, project)))
        .size(TS.caption)
        .color(ink4)
        .wrapping(iced::widget::text::Wrapping::None);
    let titles: iced::widget::Column<Element<'a, PanelMsg>> = iced::widget::Column::new()
        .push(name.boxed())
        .push(summary.boxed());
    let mut head: iced::widget::Row<Element<'a, PanelMsg>> = iced::widget::Row::new()
        .spacing(SP.l)
        .align_y(iced::Alignment::Center)
        .push(tile.boxed())
        .push(titles.boxed());
    if project.is_dirty {
        head = head.push(dirty_dot(tint));
    }
    head = head.push(iced::widget::space::horizontal().boxed());
    if project.version_control {
        let pill = container(
            text(t!(i18n, ids::PANELS_PROJECTS_GIT))
                .size(TS.caption)
                .font(iced::Font::MONOSPACE)
                .wrapping(iced::widget::text::Wrapping::None),
        )
        .padding([0.0, PILL_PAD_X])
        .height(PILL_HEIGHT)
        .align_y(iced::alignment::Vertical::Center)
        .style(styles::pill(tokens, PillTone::Neutral));
        head = head.push(pill.boxed());
    }
    let options = icon_button(
        crate::icons::icon_chrome_sliders(ctx.theme_id),
        OPTIONS_BUTTON,
        OPTIONS_GLYPH,
        ti(tokens.text_secondary),
        PanelMsg::Projects(ProjectsMsg::OptionsPressed(project.id)),
        tokens,
    );
    let options = hint_tooltip(
        options,
        t!(i18n, ids::PANELS_PROJECTS_OPTIONS),
        tokens,
        iced::widget::tooltip::Position::Bottom,
    );
    let project_row = vec![project.id.to_string()];
    mouse_area(head.push(options).boxed())
        .on_right_press(PanelMsg::Tree(TreeMsg::ContextMenu(project_row)))
        .boxed()
}
