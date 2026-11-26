//! The two irregular dropdowns: the NetColor menu (colour swatches in the
//! icon column) and the Selection Filter chip grid, the latter shared by
//! the schematic and board Active Bars (#2383) and riding the widget's
//! `Custom` escape hatch. Graphite paints the grid through `super::graphite`.

use iced::widget::{Space, column, container, row};
use iced::{Color, Element, Length, Widget};
use signex_types::theme::{ThemeId, ThemeTokens};
use signex_widgets::active_bar_dropdown::DropdownEntry;
use signex_widgets::text;

use crate::active_bar::{ActiveBarAction, ActiveBarMsg, CustomFilterPreset, SelectionFilter};
use crate::icons as ic;

use super::{dd_item, dd_swatch, graphite};

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// The seven net colours the menu arms, with the swatch each row shows.
pub(super) const NET_COLORS: [(&str, Color, ActiveBarAction); 7] = [
    (
        "Blue",
        Color::from_rgb(0.40, 0.40, 0.93),
        ActiveBarAction::NetColorBlue,
    ),
    (
        "Light Green",
        Color::from_rgb(0.40, 0.93, 0.40),
        ActiveBarAction::NetColorLightGreen,
    ),
    (
        "Light Blue",
        Color::from_rgb(0.40, 0.80, 0.93),
        ActiveBarAction::NetColorLightBlue,
    ),
    (
        "Red",
        Color::from_rgb(0.93, 0.30, 0.30),
        ActiveBarAction::NetColorRed,
    ),
    (
        "Fuchsia",
        Color::from_rgb(0.80, 0.30, 0.80),
        ActiveBarAction::NetColorFuchsia,
    ),
    (
        "Yellow",
        Color::from_rgb(0.93, 0.80, 0.20),
        ActiveBarAction::NetColorYellow,
    ),
    (
        "Dark Green",
        Color::from_rgb(0.13, 0.55, 0.13),
        ActiveBarAction::NetColorDarkGreen,
    ),
];

/// NetColor menu: seven colour rows, each an ordinary item with its swatch
/// in the shared icon column, then Custom Color… and the two Clear rows, so
/// every label starts at the same x. The Clear rows grey out when no net
/// carries a custom colour (`requires_net_color`).
pub(super) fn net_color_entries(
    i18n: &signex_i18n::I18n,
    tid: ThemeId,
    sel: bool,
    nc: bool,
) -> Vec<DropdownEntry<ActiveBarMsg>> {
    let mut rows: Vec<DropdownEntry<ActiveBarMsg>> = NET_COLORS
        .iter()
        .map(|(label, color, action)| dd_swatch(i18n, *color, label, action.clone(), sel, nc))
        .collect();
    rows.extend([
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
    rows
}

/// One category chip of a Selection Filter dropdown. `msg: None` draws it
/// inert with `why` on hover.
pub(crate) struct FilterChip<M> {
    pub label: String,
    pub on: bool,
    pub msg: Option<M>,
    pub why: Option<signex_i18n::Localized>,
}

/// One saved preset of a Selection Filter dropdown: its label, whether it
/// is the one selected for editing (drawn lit, #2873), and the message a
/// click sends.
pub(crate) struct FilterPresetChip<M> {
    pub label: String,
    pub selected: bool,
    pub msg: M,
}

/// What a Selection Filter dropdown shows and sends, for any editor: the
/// schematic's categories or the board's kinds (#2383).
pub(crate) struct FilterMenu<M> {
    pub chips: Vec<FilterChip<M>>,
    /// The saved presets.
    pub presets: Vec<FilterPresetChip<M>>,
    /// "Editing <name>" while a preset is selected for editing (#2873).
    pub editing: Option<signex_i18n::Localized>,
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

/// "Editing Wiring": the line every Selection Filter surface shows while a
/// preset is selected for editing (#2873).
pub(crate) fn preset_editing_line<K>(
    i18n: &signex_i18n::I18n,
    idx: usize,
    preset: &CustomFilterPreset<K>,
) -> signex_i18n::Localized {
    let name = preset_label(idx, preset);
    signex_i18n::t!(i18n, signex_i18n::ids::PANELS_FILTER_PRESET_EDITING, "name" => name)
}

/// The dropdown chips of `presets`, `editing` lit, each sending `clicked`.
pub(crate) fn preset_chips<K, M>(
    presets: &[CustomFilterPreset<K>],
    editing: Option<usize>,
    clicked: impl Fn(usize) -> M,
) -> Vec<FilterPresetChip<M>> {
    presets
        .iter()
        .enumerate()
        .map(|(idx, preset)| FilterPresetChip {
            label: preset_label(idx, preset),
            selected: editing == Some(idx),
            msg: clicked(idx),
        })
        .collect()
}

/// The "Editing <name>" line for `editing`, while that preset exists.
pub(crate) fn editing_line<K>(
    i18n: &signex_i18n::I18n,
    presets: &[CustomFilterPreset<K>],
    editing: Option<usize>,
) -> Option<signex_i18n::Localized> {
    let idx = editing?;
    Some(preset_editing_line(i18n, idx, presets.get(idx)?))
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
    DropdownEntry::Custom(filter_menu_view(tokens, menu))
}

/// The Selection Filter chip grid itself, six kind chips per line.
pub(crate) fn filter_menu_view<M: Clone + 'static>(
    tokens: &ThemeTokens,
    menu: FilterMenu<M>,
) -> Element<'static, M> {
    let scope = graphite::FilterScope::of(menu.on, menu.chips.len());
    let mut top: Vec<Element<'static, M>> =
        vec![graphite::all_none(tokens, scope, menu.all, menu.none)];
    for preset in menu.presets {
        top.push(preset_chip(tokens, preset));
    }
    let kinds: Vec<Element<'static, M>> = menu
        .chips
        .into_iter()
        .map(|chip| graphite::chip(tokens, chip.label, chip.on, chip.msg, chip.why))
        .collect();
    let (top, kinds) = (one_line(top), lines_of(kinds, 6));
    let mut lines = column![container(top).padding([SP.s, SP.m])].spacing(SP.xs);
    if let Some(editing) = menu.editing {
        let caption = container(editing_caption(tokens, editing)).padding([0.0, SP.m]);
        lines = lines.push(caption.boxed());
    }
    lines
        .push(container(kinds).padding([SP.s, SP.m]).boxed())
        .boxed()
}

fn preset_chip<M: Clone + 'static>(
    tokens: &ThemeTokens,
    preset: FilterPresetChip<M>,
) -> Element<'static, M> {
    graphite::chip(
        tokens,
        preset.label,
        preset.selected,
        Some(preset.msg),
        None,
    )
}

fn editing_caption<M: 'static>(
    tokens: &ThemeTokens,
    line: signex_i18n::Localized,
) -> Element<'static, M> {
    let ink3 = crate::styles::tokens::resolve(tokens).ink3;
    text(line).size(TS.caption).color(ink3).boxed()
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

/// The schematic filter a dropdown edits: the kinds that are on, the saved
/// presets and the one selected for editing (#2873).
pub struct SchematicFilterState<'s> {
    pub filters: &'s std::collections::HashSet<SelectionFilter>,
    pub presets: &'s [CustomFilterPreset],
    pub editing: Option<usize>,
}

/// The schematic's Selection Filter menu for the Active Bar dropdown.
pub(crate) fn schematic_filter_menu<M>(
    i18n: &signex_i18n::I18n,
    state: &SchematicFilterState<'_>,
    toggled: impl Fn(SelectionFilter) -> M,
    applied: impl Fn(usize) -> M,
    all: M,
    none: M,
) -> FilterMenu<M> {
    let chips = SelectionFilter::ALL
        .iter()
        .map(|&filter| FilterChip {
            label: filter.label().to_string(),
            on: state.filters.contains(&filter),
            msg: Some(toggled(filter)),
            why: None,
        })
        .collect();
    FilterMenu {
        chips,
        presets: preset_chips(state.presets, state.editing, applied),
        editing: editing_line(i18n, state.presets, state.editing),
        on: state.filters.len(),
        all,
        none,
    }
}

/// The titled layout of a Selection Filter dropdown: a title row with the
/// All / None control, the kind chips, then the presets under their caption
/// and a "Save current" button (`save: None` at the preset limit).
pub(super) struct FilterChrome<M> {
    pub title: signex_i18n::Localized,
    pub presets_caption: signex_i18n::Localized,
    pub save_label: String,
    pub save: Option<M>,
}

/// `cells` wrapped to the width of the card.
fn wrapped<M: 'static>(cells: Vec<Element<'static, M>>) -> Element<'static, M> {
    iced::widget::Row::with_children(cells)
        .spacing(SP.s)
        .wrap()
        .vertical_spacing(SP.s)
        .boxed()
}

/// [`filter_menu_view`] in the titled dropdown layout.
pub(super) fn titled_filter_menu_view<M: Clone + 'static>(
    tokens: &ThemeTokens,
    menu: FilterMenu<M>,
    chrome: FilterChrome<M>,
) -> Element<'static, M> {
    let r = crate::styles::tokens::resolve(tokens);
    let (ink, ink3, line) = (crate::styles::ti(tokens.text), r.ink3, r.line_soft);
    let scope = graphite::FilterScope::of(menu.on, menu.chips.len());
    let title = row![
        text(chrome.title)
            .size(TS.body)
            .color(ink)
            .font(iced::Font {
                weight: iced::font::Weight::Semibold,
                ..signex_widgets::ui_font::regular()
            }),
        Space::new().width(Length::Fill),
        graphite::all_none(tokens, scope, menu.all, menu.none),
    ]
    .align_y(iced::Alignment::Center);
    let kinds: Vec<Element<'static, M>> = menu
        .chips
        .into_iter()
        .map(|chip| graphite::chip(tokens, chip.label, chip.on, chip.msg, chip.why))
        .collect();
    let mut presets: Vec<Element<'static, M>> = menu
        .presets
        .into_iter()
        .map(|preset| preset_chip(tokens, preset))
        .collect();
    presets.push(graphite::chip(
        tokens,
        chrome.save_label,
        false,
        chrome.save,
        None,
    ));
    let rule = container(Space::new())
        .height(1.0)
        .width(Length::Fill)
        .style(move |_: &iced::Theme| container::Style {
            background: Some(iced::Background::Color(line)),
            ..container::Style::default()
        });
    let mut lines = column![
        title,
        wrapped(kinds),
        rule,
        text(chrome.presets_caption).size(TS.caption).color(ink3),
        wrapped(presets),
    ]
    .spacing(SP.m);
    if let Some(editing) = menu.editing {
        lines = lines.push(editing_caption(tokens, editing));
    }
    container(lines).padding(SP.m).boxed()
}

/// The schematic's Selection Filter menu: the titled layout, with "Save
/// current" while there is room for another preset.
pub(super) fn filter_entry(
    i18n: &signex_i18n::I18n,
    tokens: &ThemeTokens,
    state: &SchematicFilterState<'_>,
) -> DropdownEntry<ActiveBarMsg> {
    let menu = schematic_filter_menu(
        i18n,
        state,
        ActiveBarMsg::ToggleFilter,
        ActiveBarMsg::ApplyCustomFilter,
        ActiveBarMsg::AllFiltersPicked,
        ActiveBarMsg::NoFiltersPicked,
    );
    let chrome = FilterChrome {
        title: i18n.message(signex_i18n::ids::CHROME_ACTIVE_BAR_SELECTION_FILTER, &[]),
        presets_caption: i18n.message(signex_i18n::ids::PANELS_FILTER_PRESETS, &[]),
        save_label: i18n.message_plain(signex_i18n::ids::PANELS_FILTER_SAVE_CURRENT, &[]),
        save: save_current_message(state.presets),
    };
    DropdownEntry::Custom(titled_filter_menu_view(tokens, menu, chrome))
}

/// What "Save current" sends: the save while there is room for another
/// preset, nothing (an inert button) at the limit.
pub(super) fn save_current_message(presets: &[CustomFilterPreset]) -> Option<ActiveBarMsg> {
    (presets.len() < crate::active_bar::CUSTOM_FILTER_PRESET_LIMIT)
        .then_some(ActiveBarMsg::FilterPresetSavePicked)
}
