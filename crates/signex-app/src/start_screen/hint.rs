//! The centre region of a loaded project with no document on screen
//! (#331, mock m2): a heading, a line under it, and New sheet / New PCB /
//! Project Options…, the commands the Projects panel's menu runs. A project
//! with no sheet and no board says so instead. The column is at most
//! [`MAX_W`] wide and never wider than the window minus [`EDGE`] on each
//! side.

use iced::widget::{column, container, responsive, row};
use iced::{Alignment, Element, Length, Widget as _};
use signex_i18n::{I18n, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::text;

use super::kit::{GLYPH_THEME, Look, Size, action_button, command, weighted};
use crate::app::Message;
use crate::styles::{self, Spacing, TypeScale, ti, tokens::resolve};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;
/// Widest the hint column gets (420 px at the default base).
pub(super) const MAX_W: f32 = TS.body * 35.0;
/// Room kept clear on each side of the column (12 px at the default base).
pub(super) const EDGE: f32 = SP.l;

/// What the hint needs to know about the active project.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HintFacts {
    /// The project has a schematic sheet or a board.
    pub has_documents: bool,
}

impl HintFacts {
    /// The facts of `project`; no project counts as one with documents, so
    /// the hint keeps its usual wording.
    pub fn of(project: Option<&crate::app::LoadedProject>) -> Self {
        let has_documents = project.is_none_or(|project| {
            let data = &project.data;
            !data.sheets.is_empty() || data.schematic_root.is_some() || data.pcb_file.is_some()
        });
        Self { has_documents }
    }
}

pub fn hint_view<'a>(
    tokens: &'a ThemeTokens,
    i18n: &'a I18n,
    facts: HintFacts,
) -> Element<'a, Message> {
    let body = responsive(move |size| {
        let width = (size.width - 2.0 * EDGE).clamp(0.0, MAX_W);
        container(hint_column(tokens, i18n, facts, width))
            .center(Length::Fill)
            .boxed()
    });
    container(body)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(styles::panel_region(tokens))
        .boxed()
}

fn hint_column<'a>(
    tokens: &ThemeTokens,
    i18n: &I18n,
    facts: HintFacts,
    width: f32,
) -> Element<'a, Message> {
    let (title, body) = if facts.has_documents {
        (ids::START_HINT_TITLE, ids::START_HINT_BODY)
    } else {
        (ids::START_HINT_EMPTY_TITLE, ids::START_HINT_EMPTY_BODY)
    };
    let centred = iced::alignment::Horizontal::Center;
    let plus = || Some(crate::icons::icon_plus(GLYPH_THEME));
    let buttons = row![
        action_button(
            tokens,
            (Look::Outline, Size::Regular),
            plus(),
            t!(i18n, ids::START_HINT_NEW_SHEET),
            command("new_schematic_sheet"),
        ),
        action_button(
            tokens,
            (Look::Outline, Size::Regular),
            plus(),
            t!(i18n, ids::START_HINT_NEW_PCB),
            command("new_pcb_document"),
        ),
        action_button(
            tokens,
            (Look::Ghost, Size::Regular),
            None,
            t!(i18n, ids::START_HINT_PROJECT_OPTIONS),
            command("open_project_options"),
        ),
    ]
    .spacing(SP.m)
    .width(Length::Fill)
    .wrap()
    .vertical_spacing(SP.m)
    .align_x(centred);
    column![
        text(t!(i18n, title))
            .size(TS.heading)
            .font(weighted(iced::font::Weight::Semibold))
            .color(ti(tokens.text))
            .width(Length::Fill)
            .align_x(centred),
        text(t!(i18n, body))
            .size(TS.body)
            .color(resolve(tokens).ink3)
            .width(Length::Fill)
            .align_x(centred),
        container(buttons)
            .width(Length::Fill)
            .padding(iced::Padding {
                top: SP.xl + SP.l - SP.xs,
                ..iced::Padding::ZERO
            }),
    ]
    .spacing(SP.xs * 2.0)
    .width(width)
    .align_x(Alignment::Center)
    .boxed()
}
