//! The shared branch switcher (mockup rows 35 and 39): a chip naming the
//! current branch and the popover it opens, with a find-or-create field,
//! local and remote branches with their distance from the current one, and
//! New branch here…
//!
//! Stateless: the caller owns the rows, the query and every message, and
//! places the popover over its own surface.

use iced::font::Weight;
use iced::widget::text::{Ellipsis, Wrapping};
use iced::widget::{Column, Space, column, container, row, scrollable, svg};
use iced::{Alignment, Background, Border, Element, Font, Length, Padding, Theme, Widget};
use signex_i18n::{I18n, Localized, UserText, ids, t};

use crate::history_graph::HistoryGraphStyle;
use crate::theme_ext::HAIRLINE;
use crate::{Label, button, text, text_input};

const CHIP_HEIGHT_EM: f32 = 22.0 / 12.0;
const CHIP_PAD_EM: f32 = 8.0 / 12.0;
const CHIP_GAP_EM: f32 = 4.0 / 12.0;
/// Longest branch name the chip shows in full.
const CHIP_MAX_CHARS: usize = 18;
const ICON_EM: f32 = 1.0;
const SECTION_HEIGHT_EM: f32 = 20.0 / 12.0;
const POPOVER_WIDTH_EM: f32 = 248.0 / 12.0;
const POPOVER_PAD_EM: f32 = 4.0 / 12.0;
const LIST_MAX_HEIGHT_EM: f32 = 220.0 / 12.0;
const ROW_HEIGHT_EM: f32 = 24.0 / 12.0;
const ROW_PAD_EM: f32 = 8.0 / 12.0;
const ROW_GAP_EM: f32 = 6.0 / 12.0;

/// One branch in the switcher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchRow {
    /// Short name: `main`, `origin/main`.
    pub name: String,
    pub is_remote: bool,
    pub is_current: bool,
    /// Commits on this branch that the current one lacks.
    pub ahead: usize,
    /// Commits on the current branch that this one lacks.
    pub behind: usize,
}

/// The glyphs of the switcher, from the caller's icon set.
#[derive(Debug, Clone)]
pub struct BranchSwitcherIcons {
    pub branch: svg::Handle,
    pub chevron: svg::Handle,
    pub check: svg::Handle,
    pub plus: svg::Handle,
}

/// What the popover shows.
pub struct BranchSwitcherState<'a> {
    pub rows: &'a [BranchRow],
    pub query: &'a str,
    /// Why a switch cannot happen now, shown at the foot of the popover.
    /// The rows stay enabled only when this is `None`.
    pub notice: Option<Label<'a>>,
    /// The notice's colour (the theme's warning).
    pub notice_ink: iced::Color,
}

/// What the popover sends.
pub struct BranchSwitcherMessages<'a, M> {
    pub on_query: Box<dyn Fn(String) -> M + 'a>,
    pub on_pick: Box<dyn Fn(&BranchRow) -> M + 'a>,
    /// New branch here…; `None` draws it disabled (no valid new name typed).
    pub on_create: Option<M>,
}

/// Whether `name` matches the find field, ignoring case.
pub fn branch_matches(name: &str, query: &str) -> bool {
    // A search key, never shown, so no language case rules apply.
    // ast-grep-ignore: no-case-in-view
    let (name, query) = (name.to_lowercase(), query.trim().to_lowercase());
    name.contains(&query)
}

/// The distance caption of a row: current, up to date, or ahead / behind.
pub fn distance_caption(i18n: &I18n, row: &BranchRow) -> Localized {
    if row.is_current {
        return t!(i18n, ids::PANELS_HISTORY_BRANCH_CURRENT);
    }
    match (row.ahead, row.behind) {
        (0, 0) => t!(i18n, ids::PANELS_HISTORY_BRANCH_UP_TO_DATE),
        (ahead, 0) => t!(i18n, ids::PANELS_HISTORY_BRANCH_AHEAD, "count" => ahead.to_string()),
        (0, behind) => t!(i18n, ids::PANELS_HISTORY_BRANCH_BEHIND, "count" => behind.to_string()),
        (ahead, behind) => t!(i18n, ids::PANELS_HISTORY_BRANCH_AHEAD_BEHIND,
            "ahead" => ahead.to_string(), "behind" => behind.to_string()),
    }
}

fn glyph<'a>(handle: svg::Handle, side: f32) -> svg::Svg<'a, Theme> {
    svg(handle).width(side).height(side)
}

/// The chip naming the current branch; a press opens the popover.
pub fn branch_chip<'a, M: Clone + 'a>(
    style: &HistoryGraphStyle,
    name: impl Into<Label<'a>>,
    icons: &BranchSwitcherIcons,
    on_press: Option<M>,
) -> Element<'a, M> {
    let s = *style;
    let side = s.em(ICON_EM);
    let face = row![
        glyph(icons.branch.clone(), side),
        text(name)
            .size(s.caption_size)
            .font(Font::MONOSPACE)
            .color(s.ink)
            .wrapping(Wrapping::None)
            .ellipsis(Ellipsis::End),
        glyph(icons.chevron.clone(), side),
    ]
    .spacing(s.em(CHIP_GAP_EM))
    .align_y(Alignment::Center);
    button(
        container(face)
            .height(Length::Fill)
            .align_y(Alignment::Center),
    )
    .height(s.em(CHIP_HEIGHT_EM))
    .padding(Padding::from([0.0, s.em(CHIP_PAD_EM)]))
    .on_press_maybe(on_press)
    .style(move |_: &Theme, status: button::Status| {
        let hot = matches!(status, button::Status::Hovered | button::Status::Pressed);
        button::Style {
            background: Some(Background::Color(if hot { s.hover } else { s.field })),
            text_color: s.ink,
            border: Border {
                color: s.border,
                width: HAIRLINE,
                radius: s.radii.small.into(),
                ..Border::default()
            },
            ..button::Style::default()
        }
    })
    .boxed()
}

/// A branch name as the chip shows it: long names end in an ellipsis, so
/// the chip cannot push the file name out of the header.
pub fn chip_name(name: &str) -> String {
    if name.chars().count() <= CHIP_MAX_CHARS {
        return name.to_owned();
    }
    let kept: String = name.chars().take(CHIP_MAX_CHARS - 1).collect();
    format!("{kept}…")
}

fn section<'a, M: 'a>(s: &HistoryGraphStyle, label: Localized) -> Element<'a, M> {
    container(text(label).size(s.caption_size).color(s.ink4))
        .height(s.em(SECTION_HEIGHT_EM))
        .align_y(Alignment::Center)
        .padding(Padding::from([0.0, s.em(ROW_PAD_EM)]))
        .boxed()
}

/// Height of the branch list: its content, up to the cap, past which it
/// scrolls.
fn list_height(s: &HistoryGraphStyle, rows: usize, sections: usize) -> f32 {
    let content = rows as f32 * s.em(ROW_HEIGHT_EM) + sections as f32 * s.em(SECTION_HEIGHT_EM);
    content.min(s.em(LIST_MAX_HEIGHT_EM))
}

/// One selectable line: a lead glyph slot, the label and a right caption.
fn menu_row<'a, M: Clone + 'a>(
    s: &HistoryGraphStyle,
    lead: Option<svg::Handle>,
    label: Element<'a, M>,
    caption: Option<Localized>,
    on_press: Option<M>,
) -> Element<'a, M> {
    let s = *s;
    let side = s.em(ICON_EM);
    let slot: Element<'a, M> = match lead {
        Some(handle) => glyph(handle, side).boxed(),
        None => Space::new().width(side).height(side).boxed(),
    };
    let mut line = row![slot, container(label).width(Length::Fill)]
        .spacing(s.em(ROW_GAP_EM))
        .align_y(Alignment::Center);
    if let Some(caption) = caption {
        line = line.push(
            text(caption)
                .size(s.caption_size)
                .color(s.ink4)
                .wrapping(Wrapping::None)
                .boxed(),
        );
    }
    let enabled = on_press.is_some();
    button(
        container(line)
            .height(Length::Fill)
            .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .height(s.em(ROW_HEIGHT_EM))
    .padding(Padding::from([0.0, s.em(ROW_PAD_EM)]))
    .on_press_maybe(on_press)
    .style(move |_: &Theme, status: button::Status| {
        let hot = enabled && matches!(status, button::Status::Hovered | button::Status::Pressed);
        button::Style {
            background: hot.then_some(Background::Color(s.hover)),
            text_color: s.ink,
            border: Border {
                radius: s.radii.small.into(),
                ..Border::default()
            },
            ..button::Style::default()
        }
    })
    .boxed()
}

fn branch_line<'a, M: Clone + 'a>(
    s: &HistoryGraphStyle,
    i18n: &I18n,
    branch: &BranchRow,
    check: &svg::Handle,
    on_press: Option<M>,
) -> Element<'a, M> {
    let weight = if branch.is_current {
        Weight::Semibold
    } else {
        Weight::Normal
    };
    let name = text(UserText::new(branch.name.clone()))
        .size(s.text_size)
        .font(Font {
            weight,
            ..crate::ui_font::regular()
        })
        .color(s.ink)
        .wrapping(Wrapping::None)
        .ellipsis(Ellipsis::End);
    menu_row(
        s,
        branch.is_current.then(|| check.clone()),
        name.boxed(),
        Some(distance_caption(i18n, branch)),
        on_press,
    )
}

/// The rows of one section, or nothing when the find field hides them all.
fn section_rows<'a, M: Clone + 'a>(
    s: &HistoryGraphStyle,
    i18n: &I18n,
    state: &BranchSwitcherState<'a>,
    messages: &BranchSwitcherMessages<'a, M>,
    check: &svg::Handle,
    remote: bool,
) -> Vec<Element<'a, M>> {
    let can_switch = state.notice.is_none();
    state
        .rows
        .iter()
        .filter(|b| b.is_remote == remote && branch_matches(&b.name, state.query))
        .map(|b| {
            let on_press = (can_switch && !b.is_current).then(|| (messages.on_pick)(b));
            branch_line(s, i18n, b, check, on_press)
        })
        .collect()
}

/// The open popover. The caller stacks it over its surface.
pub fn branch_popover<'a, M: Clone + 'a>(
    style: &HistoryGraphStyle,
    i18n: &I18n,
    state: BranchSwitcherState<'a>,
    icons: &BranchSwitcherIcons,
    messages: BranchSwitcherMessages<'a, M>,
) -> Element<'a, M> {
    let s = *style;
    let local = section_rows(&s, i18n, &state, &messages, &icons.check, false);
    let remote = section_rows(&s, i18n, &state, &messages, &icons.check, true);
    let rows = local.len() + remote.len();
    let sections = usize::from(!local.is_empty()) + usize::from(!remote.is_empty());
    let mut list: Column<Element<'a, M>> = Column::new().width(Length::Fill);
    if !local.is_empty() {
        list = list.push(section(&s, t!(i18n, ids::PANELS_HISTORY_BRANCH_LOCAL)));
        list = list.extend(local);
    }
    if !remote.is_empty() {
        list = list.push(section(&s, t!(i18n, ids::PANELS_HISTORY_BRANCH_REMOTE)));
        list = list.extend(remote);
    }
    let BranchSwitcherMessages {
        on_query,
        on_create,
        ..
    } = messages;
    let find = text_input(t!(i18n, ids::PANELS_HISTORY_BRANCH_FIND), state.query)
        .on_input(on_query)
        .size(s.text_size)
        .padding(s.em(POPOVER_PAD_EM));
    let create = menu_row(
        &s,
        Some(icons.plus.clone()),
        text(t!(i18n, ids::PANELS_HISTORY_NEW_BRANCH_HERE))
            .size(s.text_size)
            .color(s.ink)
            .boxed(),
        None,
        on_create,
    );
    let mut body = column![
        find,
        scrollable(list.boxed()).height(list_height(&s, rows, sections)),
        create,
    ]
    .spacing(s.em(POPOVER_PAD_EM));
    if let Some(notice) = state.notice {
        body = body.push(
            container(text(notice).size(s.caption_size).color(state.notice_ink))
                .padding(Padding::from([s.em(POPOVER_PAD_EM), s.em(ROW_PAD_EM)]))
                .boxed(),
        );
    }
    container(body)
        .width(s.em(POPOVER_WIDTH_EM))
        .padding(s.em(POPOVER_PAD_EM))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(s.panel)),
            text_color: Some(s.ink),
            border: Border {
                color: s.border,
                width: HAIRLINE,
                radius: s.radii.button.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn branch(ahead: usize, behind: usize, is_current: bool) -> BranchRow {
        BranchRow {
            name: "ripple-fix".into(),
            is_remote: false,
            is_current,
            ahead,
            behind,
        }
    }

    fn caption(row: &BranchRow) -> String {
        distance_caption(&I18n::en_us(), row)
            .into_string()
            .replace(['\u{2068}', '\u{2069}'], "")
    }

    /// The captions the mockup draws on row 39.
    #[test]
    fn the_distance_reads_as_the_mockup_draws_it() {
        assert_eq!(caption(&branch(0, 0, true)), "current");
        assert_eq!(caption(&branch(0, 0, false)), "up to date");
        assert_eq!(caption(&branch(1, 0, false)), "1 ahead");
        assert_eq!(caption(&branch(0, 2, false)), "2 behind");
        assert_eq!(caption(&branch(5, 3, false)), "5 ahead · 3 behind");
    }

    #[test]
    fn a_long_branch_name_is_cut_on_the_chip() {
        assert_eq!(chip_name("main"), "main");
        let cut = chip_name("feature/a-very-long-branch-name");
        assert_eq!(cut.chars().count(), CHIP_MAX_CHARS);
        assert!(cut.ends_with('…'));
    }

    #[test]
    fn the_list_scrolls_past_its_cap() {
        let s = HistoryGraphStyle::from_tokens(
            &signex_types::theme::theme_tokens(signex_types::theme::ThemeId::GraphiteLight),
            &crate::theme_ext::TypeScale::DEFAULT,
        );
        assert_eq!(
            list_height(&s, 2, 1),
            2.0 * s.em(ROW_HEIGHT_EM) + s.em(SECTION_HEIGHT_EM)
        );
        assert_eq!(list_height(&s, 500, 2), s.em(LIST_MAX_HEIGHT_EM));
    }

    #[test]
    fn the_find_field_ignores_case_and_surrounding_space() {
        assert!(branch_matches("Ripple-Fix", " ripple "));
        assert!(branch_matches("main", ""));
        assert!(!branch_matches("main", "dev"));
    }
}
