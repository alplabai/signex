//! Tab strip overflow (mockup row 5): when the tabs no longer fit, the strip keeps the
//! tabs that fit at their full width (a tab never shrinks below its name)
//! and ends in a "+N" button whose menu lists the rest. The fit is computed
//! from measured text widths, the way the dock strip's is (`dock::tab_fit`).

use iced::widget::{Widget, container, row, svg};
use iced::{Background, Border, Element, Length};
use signex_types::theme::{ThemeId, ThemeTokens};
use signex_widgets::active_bar_dropdown::{DropdownEntry, MenuMetrics, RowPaint};
use signex_widgets::{button, text};

use super::graphite::{
    self, DIRTY_DOT, NAME_PAD_X, PILL_PAD_X, TAB_GAP, TAB_PAD_LEFT, TAB_PAD_RIGHT,
};
use super::{TabMarks, TabMessage};
use crate::app::{LoadedProject, TabInfo, page_tab};
use crate::dock::measure::text_width_in;
use crate::styles::tokens::resolve;
use crate::styles::{Radii, TypeScale, ti};

const TS: TypeScale = TypeScale::DEFAULT;

/// Side padding of the "+N" button and the gap before its chevron.
const BUTTON_PAD_X: f32 = 10.0;
const BUTTON_GAP: f32 = 4.0;
/// Side of the chevron in the "+N" button.
const CHEVRON: f32 = 12.0;
/// Height and side padding of the kind-code key cap in the menu.
const KEY_CAP_H: f32 = 18.0;
const KEY_CAP_PAD_X: f32 = 5.0;

/// Which tabs of the strip show and which hide, as indices into the tabs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Split {
    pub(crate) shown: Vec<usize>,
    pub(crate) hidden: Vec<usize>,
}

/// The split of tabs whose widths are `widths` (strip order) in `room` px,
/// as positions into `widths`. When they all fit every tab shows. Otherwise
/// `button` px go to the "+N" button and the tabs show from the left while
/// they fit; the `active` tab always shows, so a picked hidden tab becomes
/// visible.
pub(crate) fn split(widths: &[f32], active: Option<usize>, room: f32, button: f32) -> Split {
    let total: f32 = widths.iter().sum();
    if total <= room {
        return Split {
            shown: (0..widths.len()).collect(),
            hidden: Vec::new(),
        };
    }
    let room = room - button;
    let mut used = active.and_then(|a| widths.get(a)).copied().unwrap_or(0.0);
    let mut full = false;
    let mut out = Split::default();
    for (i, &w) in widths.iter().enumerate() {
        if Some(i) == active {
            out.shown.push(i);
        } else if !full && used + w <= room {
            used += w;
            out.shown.push(i);
        } else {
            full = true;
            out.hidden.push(i);
        }
    }
    out
}

/// The text a tab shows: its title, or for a page tab "<project> — <page>"
/// from the project's current name among `projects`, read in the current
/// language rather than the one it was opened in; ellipsized like the strip
/// draws it.
pub(crate) fn tab_label(
    tab: &TabInfo,
    projects: &[LoadedProject],
    i18n: &signex_i18n::I18n,
) -> String {
    let page_title = page_tab::tab_title(&tab.kind, projects, i18n);
    super::ellipsize_title(
        page_title.as_deref().unwrap_or(&tab.title),
        super::MAX_TAB_TITLE_CHARS,
    )
}

/// Natural width of `tab` in the strip: its kind code, label, dirty dot,
/// read-only pill, close button, padding and its 1 px edge.
fn tab_width(tab: &TabInfo, marks: &TabMarks<'_>) -> f32 {
    let i18n = marks.i18n;
    let code = signex_i18n::t!(i18n, graphite::kind_glyph(&tab.kind)).into_string();
    let mut w = TAB_PAD_LEFT
        + text_width_in(&code, TS.caption, graphite::mono())
        + TAB_GAP
        + text_width_in(
            &tab_label(tab, marks.projects, i18n),
            TS.body,
            marks.ui_font,
        );
    if super::tab_is_dirty(tab, marks.dirty_paths) {
        w += TAB_GAP + DIRTY_DOT;
    }
    if marks.read_only_paths.contains(&tab.path) {
        let pill = signex_i18n::t!(i18n, signex_i18n::ids::PANELS_TAB_READ_ONLY).into_string();
        w += TAB_GAP + text_width_in(&pill, TS.caption, marks.ui_font) + 2.0 * PILL_PAD_X;
    }
    w + TAB_GAP + graphite::CLOSE_SIZE + TAB_PAD_RIGHT + 1.0
}

/// The "+N" label of the overflow button.
fn button_label(count: usize, i18n: &signex_i18n::I18n) -> signex_i18n::Localized {
    signex_i18n::t!(i18n, signex_i18n::menu_ids::CTX_TAB_OVERFLOW_BUTTON, "count" => count)
}

/// Width of the "+N" button for `count` hidden tabs, its left edge included.
fn button_width(count: usize, marks: &TabMarks<'_>) -> f32 {
    let label = button_label(count, marks.i18n).into_string();
    text_width_in(&label, TS.caption, marks.ui_font)
        + BUTTON_GAP
        + CHEVRON
        + 2.0 * BUTTON_PAD_X
        + 1.0
}

/// Width the project name takes at the strip's right end.
fn project_name_width(marks: &TabMarks<'_>) -> f32 {
    marks.project_name.map_or(0.0, |name| {
        text_width_in(name, TS.caption, marks.ui_font) + 2.0 * NAME_PAD_X
    })
}

/// Left edge of the "+N" button for `hidden` hidden tabs: the menu opens
/// under it.
pub(crate) fn button_left(hidden: usize, marks: &TabMarks<'_>) -> f32 {
    (marks.strip_width - project_name_width(marks) - button_width(hidden, marks)).max(0.0)
}

/// The split of the strip's tabs `visible` (indices into `tabs`, in strip
/// order) for the strip in `marks`. The view and the "+N" menu both read it,
/// so the menu lists exactly the tabs the strip leaves out.
pub(crate) fn strip_split(
    tabs: &[TabInfo],
    visible: &[usize],
    active: usize,
    marks: &TabMarks<'_>,
) -> Split {
    // A lone tab (an undocked window's) never hides behind the button.
    if visible.len() < 2 {
        return Split {
            shown: visible.to_vec(),
            hidden: Vec::new(),
        };
    }
    let widths: Vec<f32> = visible
        .iter()
        .map(|&i| tab_width(&tabs[i], marks))
        .collect();
    let room = marks.strip_width - project_name_width(marks);
    let active_at = visible.iter().position(|&i| i == active);
    let at = split(&widths, active_at, room, button_width(visible.len(), marks));
    Split {
        shown: at.shown.iter().map(|&p| visible[p]).collect(),
        hidden: at.hidden.iter().map(|&p| visible[p]).collect(),
    }
}

/// The "+N" button at the strip's end: caption `ink3`, a chevron, a 1 px
/// line on its left; it takes the hover fill while hovered or while its menu
/// is open.
pub(super) fn overflow_button(
    count: usize,
    open: bool,
    theme: ThemeId,
    tokens: &ThemeTokens,
    i18n: &signex_i18n::I18n,
) -> Element<'static, TabMessage> {
    let ink = resolve(tokens).ink3;
    let (hover, line) = (ti(tokens.hover), ti(tokens.border));
    let chevron = svg(crate::icons::icon_chrome_chevron_down(theme))
        .width(CHEVRON)
        .height(CHEVRON)
        .style(move |_: &iced::Theme, _| svg::Style {
            color: Some(ink),
            ..Default::default()
        });
    let content = row![
        text(button_label(count, i18n))
            .size(TS.caption)
            .color(ink)
            .boxed(),
        chevron.boxed(),
    ]
    .spacing(BUTTON_GAP)
    .align_y(iced::Alignment::Center);
    button(
        container(content)
            .height(Length::Fill)
            .align_y(iced::alignment::Vertical::Center),
    )
    .padding([0.0, BUTTON_PAD_X])
    .height(graphite::strip_height())
    .on_press(TabMessage::OverflowMenuToggled)
    .style(move |_: &iced::Theme, status| button::Style {
        background: (open || matches!(status, button::Status::Hovered | button::Status::Pressed))
            .then_some(Background::Color(hover)),
        border: Border {
            left: iced::border::Side::default().color(line).width(1.0),
            ..Border::default()
        },
        ..button::Style::default()
    })
    .boxed()
}

/// One row of the menu: the kind code in a key cap, then the tab's label.
fn hidden_row<M: Clone + 'static>(
    code: signex_i18n::Localized,
    label: String,
    message: M,
    tokens: &ThemeTokens,
) -> Element<'static, M> {
    let r = resolve(tokens);
    let (cap_fill, cap_line, cap_ink, ink) = (r.sunken, r.line_strong, r.ink3, ti(tokens.text));
    let metrics = MenuMetrics::GRAPHITE;
    let paint = RowPaint::of(tokens);
    let cap = container(
        text(code)
            .size(TS.caption)
            .color(cap_ink)
            .font(graphite::mono()),
    )
    .padding([0.0, KEY_CAP_PAD_X])
    .height(KEY_CAP_H)
    .align_y(iced::alignment::Vertical::Center)
    .style(move |_: &iced::Theme| container::Style {
        background: Some(Background::Color(cap_fill)),
        border: Border {
            width: 1.0,
            color: cap_line,
            radius: Radii::GRAPHITE.small.into(),
            ..Border::default()
        },
        ..container::Style::default()
    });
    let content = row![
        cap.boxed(),
        text(signex_i18n::UserText::new(label))
            .size(metrics.label_size)
            .color(ink)
            .boxed(),
    ]
    .spacing(metrics.row_spacing)
    .align_y(iced::Alignment::Center);
    button(
        container(content)
            .height(Length::Fill)
            .align_y(iced::alignment::Vertical::Center),
    )
    .padding([0.0, metrics.row_pad_x])
    .width(Length::Fill)
    .height(metrics.item_height(false))
    .on_press(message)
    .style(move |_: &iced::Theme, status| button::Style {
        background: matches!(status, button::Status::Hovered | button::Status::Pressed)
            .then_some(Background::Color(paint.hover)),
        border: Border {
            radius: paint.radius.into(),
            ..Border::default()
        },
        ..button::Style::default()
    })
    .boxed()
}

/// The menu of the "+N" button: the "Hidden tabs" caption, then one row
/// per tab in `hidden` (indices into `tabs`, page tabs titled from
/// `projects`); picking a row sends `on_pick(index)`.
pub(crate) fn hidden_tab_entries<M: Clone + 'static>(
    (tabs, projects): (&[TabInfo], &[LoadedProject]),
    hidden: &[usize],
    i18n: &signex_i18n::I18n,
    tokens: &ThemeTokens,
    on_pick: impl Fn(usize) -> M,
) -> Vec<DropdownEntry<M>> {
    let caption =
        signex_i18n::t!(i18n, signex_i18n::menu_ids::CTX_TAB_OVERFLOW_CAPTION).into_string();
    std::iter::once(DropdownEntry::Header(caption))
        .chain(hidden.iter().filter_map(|&i| {
            let tab = tabs.get(i)?;
            let code = signex_i18n::t!(i18n, graphite::kind_glyph(&tab.kind));
            Some(DropdownEntry::Custom(hidden_row(
                code,
                tab_label(tab, projects, i18n),
                on_pick(i),
                tokens,
            )))
        }))
        .collect()
}

/// Width of the menu card for the tabs `hidden`: the widest key cap and
/// label, inside the Graphite card's bounds.
pub(crate) fn hidden_menu_width(
    (tabs, projects): (&[TabInfo], &[LoadedProject]),
    hidden: &[usize],
    i18n: &signex_i18n::I18n,
    ui_font: iced::Font,
) -> f32 {
    let metrics = MenuMetrics::GRAPHITE;
    let widest = hidden
        .iter()
        .filter_map(|&i| tabs.get(i))
        .map(|tab| {
            let code = signex_i18n::t!(i18n, graphite::kind_glyph(&tab.kind)).into_string();
            text_width_in(&code, TS.caption, graphite::mono())
                + 2.0 * KEY_CAP_PAD_X
                + 2.0
                + metrics.row_spacing
                + text_width_in(&tab_label(tab, projects, i18n), metrics.label_size, ui_font)
        })
        .fold(0.0, f32::max);
    (widest + 2.0 * metrics.row_pad_x + 2.0 * metrics.panel_pad)
        .ceil()
        .clamp(metrics.min_width, metrics.max_width)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::TabKind;
    use std::collections::HashSet;
    use std::path::PathBuf;

    #[test]
    fn tabs_that_fit_all_show() {
        let s = split(&[100.0, 120.0, 90.0], Some(0), 400.0, 40.0);
        assert_eq!(s.shown, [0, 1, 2]);
        assert!(s.hidden.is_empty());
    }

    /// Room for two tabs once the button takes its share: the rest hide,
    /// none is squeezed.
    #[test]
    fn tabs_that_do_not_fit_hide_behind_the_button() {
        let s = split(&[100.0, 100.0, 100.0, 100.0], Some(0), 260.0, 40.0);
        assert_eq!(s.shown, [0, 1]);
        assert_eq!(s.hidden, [2, 3]);
    }

    /// A tab is never shown past a hidden one, so the strip reads left to
    /// right without gaps, but the active tab always shows.
    #[test]
    fn the_active_tab_always_shows() {
        let s = split(&[100.0, 100.0, 100.0, 100.0], Some(3), 260.0, 40.0);
        assert_eq!(s.shown, [0, 3]);
        assert_eq!(s.hidden, [1, 2]);
        let narrow_after_wide = split(&[100.0, 200.0, 20.0], None, 200.0, 40.0);
        assert_eq!(narrow_after_wide.shown, [0]);
        assert_eq!(narrow_after_wide.hidden, [1, 2]);
    }

    #[test]
    fn a_strip_with_no_room_still_shows_the_active_tab() {
        let s = split(&[100.0, 100.0], Some(1), 50.0, 40.0);
        assert_eq!(s.shown, [1]);
        assert_eq!(s.hidden, [0]);
    }

    fn tab(name: &str, kind: TabKind) -> TabInfo {
        crate::test_adapter::boardless_tab(name, name, kind)
    }

    fn marks<'p>(
        none: &'p HashSet<PathBuf>,
        i18n: &'p signex_i18n::I18n,
        strip_width: f32,
    ) -> TabMarks<'p> {
        TabMarks {
            dirty_paths: none,
            read_only_paths: none,
            tooltips_enabled: true,
            project_name: None,
            i18n,
            projects: &[],
            ui_font: iced::Font::DEFAULT,
            theme_id: ThemeId::GraphiteLight,
            strip_width,
            overflow_open: false,
        }
    }

    /// Eight sheet tabs in a 600 px strip: the first ones show, the rest are
    /// the hidden tabs, the active one among the shown.
    #[test]
    fn a_narrow_strip_hides_the_tabs_past_its_end() {
        let tabs: Vec<TabInfo> = (0..8)
            .map(|i| tab(&format!("sheet-{i}.snxsch"), TabKind::Schematic))
            .collect();
        let visible: Vec<usize> = (0..tabs.len()).collect();
        let (none, i18n) = (HashSet::new(), signex_i18n::I18n::en_us());
        let wide = strip_split(&tabs, &visible, 0, &marks(&none, &i18n, 4000.0));
        assert_eq!(wide.shown, visible);
        assert!(wide.hidden.is_empty());
        let narrow = strip_split(&tabs, &visible, 6, &marks(&none, &i18n, 600.0));
        assert!(!narrow.hidden.is_empty(), "{narrow:?}");
        assert!(narrow.shown.contains(&6), "{narrow:?}");
        assert_eq!(narrow.shown.len() + narrow.hidden.len(), tabs.len());
        assert!(narrow.hidden.iter().all(|i| !narrow.shown.contains(i)));
    }

    /// The menu is the caption, then one row per hidden tab.
    #[test]
    fn the_menu_lists_the_hidden_tabs_under_its_caption() {
        let tabs = [
            tab("a.snxsch", TabKind::Schematic),
            tab("b.snxpcb", TabKind::Pcb),
            tab("c.snxsch", TabKind::Schematic),
        ];
        let i18n = signex_i18n::I18n::en_us();
        let tokens = signex_types::theme::theme_tokens(ThemeId::GraphiteDark);
        let rows = hidden_tab_entries((&tabs, &[]), &[1, 2], &i18n, &tokens, |i| i);
        assert_eq!(rows.len(), 3);
        assert!(matches!(&rows[0], DropdownEntry::Header(h) if h == "Hidden tabs"));
        assert!(
            rows[1..]
                .iter()
                .all(|r| matches!(r, DropdownEntry::Custom(_)))
        );
        let width = hidden_menu_width((&tabs, &[]), &[1, 2], &i18n, iced::Font::DEFAULT);
        let metrics = MenuMetrics::GRAPHITE;
        assert!((metrics.min_width..=metrics.max_width).contains(&width));
    }

    #[test]
    fn the_button_names_how_many_tabs_are_hidden() {
        let i18n = signex_i18n::I18n::en_us();
        let label = button_label(4, &i18n)
            .into_string()
            .replace(['\u{2068}', '\u{2069}'], "");
        assert_eq!(label, "+4");
    }
}
