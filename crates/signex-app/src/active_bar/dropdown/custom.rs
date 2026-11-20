//! The net-colour menu and the Selection Filter chip grid. The chip grid
//! rides the widget's `Custom` escape hatch and is shared by the schematic
//! and board Active Bars (#2383); Graphite themes paint it through
//! `super::graphite`.

use iced::widget::{column, container};
use iced::{Color, Element, Widget};
use signex_types::theme::{ThemeId, ThemeTokens};
use signex_widgets::active_bar_dropdown::DropdownEntry;

use crate::active_bar::{ActiveBarAction, ActiveBarMsg, CustomFilterPreset, SelectionFilter};
use crate::icons as ic;

use super::{dd_item, dd_swatch, graphite};

const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// One preset net colour of the menu.
struct Swatch {
    label: &'static str,
    rgb: [f32; 3],
    action: ActiveBarAction,
}

/// The seven preset net colours, in menu order.
const NET_COLOR_SWATCHES: [Swatch; 7] = [
    Swatch {
        label: "Blue",
        rgb: [0.40, 0.40, 0.93],
        action: ActiveBarAction::NetColorBlue,
    },
    Swatch {
        label: "Light Green",
        rgb: [0.40, 0.93, 0.40],
        action: ActiveBarAction::NetColorLightGreen,
    },
    Swatch {
        label: "Light Blue",
        rgb: [0.40, 0.80, 0.93],
        action: ActiveBarAction::NetColorLightBlue,
    },
    Swatch {
        label: "Red",
        rgb: [0.93, 0.30, 0.30],
        action: ActiveBarAction::NetColorRed,
    },
    Swatch {
        label: "Fuchsia",
        rgb: [0.80, 0.30, 0.80],
        action: ActiveBarAction::NetColorFuchsia,
    },
    Swatch {
        label: "Yellow",
        rgb: [0.93, 0.80, 0.20],
        action: ActiveBarAction::NetColorYellow,
    },
    Swatch {
        label: "Dark Green",
        rgb: [0.13, 0.55, 0.13],
        action: ActiveBarAction::NetColorDarkGreen,
    },
];

fn rgb([r, g, b]: [f32; 3]) -> Color {
    Color::from_rgb(r, g, b)
}

/// NetColor menu: seven colour swatch rows, then the Custom / Clear rows.
/// Every row is an ordinary item, so the swatches and the icons share the
/// widget's one icon column and every label starts at the same x. The
/// Clear rows grey out when no net carries a custom colour
/// (`requires_net_color`).
pub(super) fn net_color_entries(
    i18n: &signex_i18n::I18n,
    tid: ThemeId,
    sel: bool,
    nc: bool,
) -> Vec<DropdownEntry<ActiveBarMsg>> {
    let mut entries: Vec<DropdownEntry<ActiveBarMsg>> = NET_COLOR_SWATCHES
        .into_iter()
        .map(|s| dd_swatch(i18n, rgb(s.rgb), s.label, s.action, sel, nc))
        .collect();
    entries.extend([
        DropdownEntry::Separator,
        dd_item(
            i18n,
            ic::icon_dd_net_color_custom(tid),
            "Custom Color...",
            ActiveBarAction::NetColorCustom,
            sel,
            nc,
        ),
        DropdownEntry::Separator,
        dd_item(
            i18n,
            ic::icon_dd_net_color_clear(tid),
            "Clear Net Color",
            ActiveBarAction::ClearNetColor,
            sel,
            nc,
        ),
        dd_item(
            i18n,
            ic::icon_dd_net_color_clear_all(tid),
            "Clear All Net Colors",
            ActiveBarAction::ClearAllNetColors,
            sel,
            nc,
        ),
    ]);
    entries
}

/// One category chip of a Selection Filter dropdown. `msg: None` draws it
/// inert with `why` on hover.
pub(crate) struct FilterChip<M> {
    pub label: String,
    pub on: bool,
    pub msg: Option<M>,
    pub why: Option<signex_i18n::Localized>,
}

/// What a Selection Filter dropdown shows and sends, for any editor: the
/// schematic's categories or the board's kinds (#2383).
pub(crate) struct FilterMenu<M> {
    pub chips: Vec<FilterChip<M>>,
    /// The saved presets, by label, each with the message that applies it.
    pub presets: Vec<(String, M)>,
    /// How many categories are on, out of `chips.len()`.
    pub on: usize,
    pub all: M,
    pub none: M,
}

/// The preset's button label: its name, or "Filter N" when unnamed.
pub(crate) fn preset_label<K>(idx: usize, preset: &CustomFilterPreset<K>) -> String {
    if preset.name.trim().is_empty() {
        format!("Filter {}", idx + 1)
    } else {
        preset.name.clone()
    }
}

/// How a Selection Filter grid lays out its chips: six per line in a
/// dropdown, wrapped to the available width in a panel (#2417).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChipLayout {
    SixPerLine,
    Wrapped,
}

/// Selection Filter menu: an irregular chip-wrap layout (All / None and the
/// presets on top, then the categories as toggle chips, six per row) that
/// can't be expressed as a vertical `Item` list, so it rides the widget's
/// `Custom` escape hatch as a single owned `Element<'static>`. The one
/// builder of every editor's Selection Filter dropdown.
pub(crate) fn filter_menu_entry<M: Clone + 'static>(
    tokens: &ThemeTokens,
    menu: FilterMenu<M>,
) -> DropdownEntry<M> {
    DropdownEntry::Custom(filter_menu_view(tokens, menu, ChipLayout::SixPerLine))
}

/// The Selection Filter chip grid itself, as the dropdowns and the SCH
/// Filter panel (#2417) show it.
pub(crate) fn filter_menu_view<M: Clone + 'static>(
    tokens: &ThemeTokens,
    menu: FilterMenu<M>,
    layout: ChipLayout,
) -> Element<'static, M> {
    let scope = graphite::FilterScope::of(menu.on, menu.chips.len());
    let mut top: Vec<Element<'static, M>> =
        vec![graphite::all_none(tokens, scope, menu.all, menu.none)];
    for (label, msg) in menu.presets {
        top.push(graphite::chip(tokens, label, false, Some(msg), None));
    }
    let kinds: Vec<Element<'static, M>> = menu
        .chips
        .into_iter()
        .map(|chip| graphite::chip(tokens, chip.label, chip.on, chip.msg, chip.why))
        .collect();
    let (top, kinds) = match layout {
        ChipLayout::SixPerLine => (one_line(top), lines_of(kinds, 6)),
        ChipLayout::Wrapped => (wrapped(top), wrapped(kinds)),
    };
    column![
        container(top).padding([SP.s, SP.m]),
        container(kinds).padding([SP.s, SP.m]),
    ]
    .spacing(SP.xs)
    .boxed()
}

fn one_line<M: 'static>(cells: Vec<Element<'static, M>>) -> Element<'static, M> {
    iced::widget::Row::with_children(cells)
        .spacing(SP.s)
        .align_y(iced::Alignment::Center)
        .boxed()
}

fn lines_of<M: 'static>(cells: Vec<Element<'static, M>>, per_line: usize) -> Element<'static, M> {
    let mut lines = iced::widget::Column::<Element<'static, M>>::new().spacing(SP.s);
    let mut cells = cells.into_iter().peekable();
    while cells.peek().is_some() {
        let mut line = iced::widget::Row::<Element<'static, M>>::new().spacing(SP.s);
        for cell in cells.by_ref().take(per_line) {
            line = line.push(cell);
        }
        lines = lines.push(line.boxed());
    }
    lines.boxed()
}

fn wrapped<M: 'static>(cells: Vec<Element<'static, M>>) -> Element<'static, M> {
    let mut wrap = iced_aw::Wrap::new().spacing(SP.s).line_spacing(SP.s);
    for cell in cells {
        wrap = wrap.push(cell);
    }
    container(wrap).width(iced::Length::Fill).boxed()
}

/// The schematic's Selection Filter menu, for any surface that sends `M`:
/// the Active Bar dropdown and the SCH Filter panel (#2417).
pub(crate) fn schematic_filter_menu<M>(
    filters: &std::collections::HashSet<SelectionFilter>,
    custom_presets: &[CustomFilterPreset],
    toggled: impl Fn(SelectionFilter) -> M,
    applied: impl Fn(usize) -> M,
    all: M,
    none: M,
) -> FilterMenu<M> {
    let chips = SelectionFilter::ALL
        .iter()
        .map(|&filter| FilterChip {
            label: filter.label().to_string(),
            on: filters.contains(&filter),
            msg: Some(toggled(filter)),
            why: None,
        })
        .collect();
    let presets = custom_presets
        .iter()
        .enumerate()
        .map(|(idx, preset)| (preset_label(idx, preset), applied(idx)))
        .collect();
    FilterMenu {
        chips,
        presets,
        on: filters.len(),
        all,
        none,
    }
}

/// The schematic's Selection Filter menu.
pub(super) fn filter_entry(
    tokens: &ThemeTokens,
    filters: &std::collections::HashSet<SelectionFilter>,
    custom_presets: &[CustomFilterPreset],
) -> DropdownEntry<ActiveBarMsg> {
    let menu = schematic_filter_menu(
        filters,
        custom_presets,
        ActiveBarMsg::ToggleFilter,
        ActiveBarMsg::ApplyCustomFilter,
        ActiveBarMsg::AllFiltersPicked,
        ActiveBarMsg::NoFiltersPicked,
    );
    filter_menu_entry(tokens, menu)
}
