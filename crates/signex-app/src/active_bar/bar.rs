//! The schematic Active Bar strip: its 13 group buttons in the Select /
//! Place / Annotate groups, and the corner-mode pill shown while a wire or
//! a bus is drawn.

use iced::widget::{Widget, svg};
use iced::{Background, Border, Element, Length, Theme};
use signex_i18n::{I18n, MessageId, ids};
use signex_types::corner::CornerMode;
use signex_types::theme::{ThemeId, ThemeTokens};
use signex_widgets::active_bar::{ActiveBarButton, ActiveBarIcon, ActiveBarItem};
use signex_widgets::{button, text};

use super::{ActiveBarAction, ActiveBarMenu, ActiveBarMsg, action_enabled, glyphs, is_planned};
use super::{DIRECTIVES_DEFAULT_ACTION, layout};
use crate::app::Tool;
use crate::icons as ic;
use crate::styles::{self, Radii, Spacing, TypeScale};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;
/// Inset of the pill cells inside their sunken frame.
const PILL_INSET: f32 = 2.0;

/// Hover text for a bar button: its name, plus the shared "planned" note
/// when the button's left-click action is a placeholder tool (#1249), so a
/// greyed Harness button explains itself like the rows in its dropdown.
pub(super) fn button_tooltip(left: &ActiveBarMsg, name: MessageId, i18n: &I18n) -> String {
    let name = i18n.message_plain(name, &[]);
    match left {
        ActiveBarMsg::Action(action) if is_planned(action) => i18n.message_plain(
            ids::CHROME_ACTIVE_BAR_PLANNED_TOOLTIP,
            &[("name", name.into())],
        ),
        _ => name,
    }
}

/// The three cells of the corner-mode pill, left to right, each with the
/// mode a click picks.
pub(super) const CORNER_CELLS: [(MessageId, CornerMode); 3] = [
    (ids::CHROME_ACTIVE_BAR_CORNER_90, CornerMode::Ortho90),
    (ids::CHROME_ACTIVE_BAR_CORNER_45, CornerMode::Angle45Start),
    (ids::CHROME_ACTIVE_BAR_CORNER_ANY, CornerMode::FreeAngle),
];

/// Whether the pill cell for `cell` is lit while `mode` is active: both
/// 45° leg orders light the one 45° cell (the status bar says which leads).
pub(super) fn corner_cell_lit(cell: CornerMode, mode: CornerMode) -> bool {
    let family = |m: CornerMode| match m {
        CornerMode::Angle45End => CornerMode::Angle45Start,
        other => other,
    };
    family(cell) == family(mode)
}

/// What a click on the pill cell for `cell` sends while `mode` is active:
/// the lit 45° cell keeps the leg order already chosen. A cell never sends a
/// tool action, so a click cannot flip Bus drawing over to Wire (#1379).
pub(super) fn corner_cell_message(cell: CornerMode, mode: CornerMode) -> ActiveBarMsg {
    let picked = if corner_cell_lit(cell, mode) {
        mode
    } else {
        cell
    };
    ActiveBarMsg::CornerModePicked(picked)
}

/// The corner-mode pill: a segmented 90° / 45° / Any control, the active
/// mode raised on the sunken frame.
fn corner_pill(
    mode: CornerMode,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'static, ActiveBarMsg> {
    let r = styles::tokens::resolve(tokens);
    let (raised, sunken, ink, ink3) = (r.raised, r.sunken, styles::ti(tokens.text), r.ink3);
    let radius = Radii::GRAPHITE.small;
    let cells: Vec<Element<'static, ActiveBarMsg>> = CORNER_CELLS
        .iter()
        .map(|&(label, cell)| {
            let lit = corner_cell_lit(cell, mode);
            button(
                text(i18n.message(label, &[]))
                    .size(TS.body)
                    .color(if lit { ink } else { ink3 })
                    .width(Length::Fill)
                    .align_x(iced::alignment::Horizontal::Center),
            )
            .width(Length::FillPortion(1))
            .padding([SP.xs, 0.0])
            .on_press(corner_cell_message(cell, mode))
            .style(move |_: &Theme, _| button::Style {
                background: lit.then_some(Background::Color(raised)),
                border: Border {
                    radius: radius.into(),
                    ..Border::default()
                },
                ..button::Style::default()
            })
            .boxed()
        })
        .collect();
    iced::widget::container(iced::widget::Row::with_children(cells).spacing(PILL_INSET))
        .width(Length::Fixed(layout::DRAW_MODE_PILL_WIDTH_PX))
        .padding(PILL_INSET)
        .style(move |_: &Theme| iced::widget::container::Style {
            background: Some(Background::Color(sunken)),
            border: Border {
                radius: radius.into(),
                ..Border::default()
            },
            ..iced::widget::container::Style::default()
        })
        .boxed()
}

/// The bar's state that decides how each button draws.
pub(super) struct BarState<'a> {
    pub current_tool: Tool,
    pub corner_mode: CornerMode,
    pub last_tool: &'a std::collections::HashMap<String, ActiveBarAction>,
    pub tid: ThemeId,
    pub has_selection: bool,
    pub has_net_colors: bool,
}

/// One group button: its glyph, whether its tool is armed, what a left
/// click sends, the menu behind its corner triangle and its hover name.
/// Every group opens its menu: none holds only planned tools
/// (`dropdown::tests::no_group_is_planned`).
fn group_button(
    s: &BarState<'_>,
    i18n: &I18n,
    icon: svg::Handle,
    selected: bool,
    left: ActiveBarMsg,
    menu: ActiveBarMenu,
    tooltip: MessageId,
) -> ActiveBarItem<ActiveBarMsg> {
    let enabled = match &left {
        ActiveBarMsg::Action(a) => action_enabled(a, s.has_selection, s.has_net_colors),
        _ => true,
    };
    ActiveBarItem::Button(ActiveBarButton {
        icon: ActiveBarIcon::Svg(icon),
        tooltip: button_tooltip(&left, tooltip, i18n),
        enabled,
        selected,
        on_press: Some(left),
        dropdown_indicator: Some(ActiveBarIcon::Svg(ic::icon_chevron_45(s.tid))),
        on_right_press: Some(ActiveBarMsg::ToggleMenu(menu)),
    })
}

/// The bar's slots in draw order; they must match `layout::bar_slots`
/// (pinned by `bar_items_follow_the_slot_table`):
/// Filter | Move, Select, Align | Wiring, Power, Harness, Sheet Symbol,
/// Port, Directives | Text, Drawing, Net Color [| corner pill].
pub(super) fn bar_items(
    s: &BarState<'_>,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Vec<ActiveBarItem<ActiveBarMsg>> {
    use ActiveBarMenu as Menu;
    let tid = s.tid;
    // The last-used action of a group, or its default. A planned action is
    // never a valid "last used" (it is disabled), so it falls back to the
    // default rather than killing the button.
    let last = |group: &str, default: ActiveBarAction| {
        ActiveBarMsg::Action(
            s.last_tool
                .get(group)
                .filter(|a| !is_planned(a))
                .cloned()
                .unwrap_or(default),
        )
    };
    let act = ActiveBarMsg::Action;
    let tool = s.current_tool;
    let b = |icon, selected, left, menu, tooltip| {
        group_button(s, i18n, icon, selected, left, menu, tooltip)
    };
    // Graphite keeps the monochrome group glyph on each button; the power
    // button follows the family of the last-used port (see `glyphs`).
    let power_icon =
        glyphs::PowerGlyph::of(s.last_tool.get("power").filter(|a| !is_planned(a))).handle(tid);
    let mut items = vec![
        // Select
        b(
            ic::icon_filter(tid),
            false,
            ActiveBarMsg::ToggleMenu(Menu::Filter),
            Menu::Filter,
            ids::CHROME_ACTIVE_BAR_SELECTION_FILTER,
        ),
        ActiveBarItem::Separator,
        b(
            ic::icon_move(tid),
            false,
            act(ActiveBarAction::Move),
            Menu::Select,
            ids::CHROME_ACTIVE_BAR_MOVE,
        ),
        b(
            ic::icon_select(tid),
            tool == Tool::Select,
            act(ActiveBarAction::ToolSelect),
            Menu::SelectMode,
            ids::CHROME_ACTIVE_BAR_SELECT,
        ),
        b(
            ic::icon_align(tid),
            false,
            act(ActiveBarAction::AlignToGrid),
            Menu::Align,
            ids::CHROME_ACTIVE_BAR_ALIGN,
        ),
        ActiveBarItem::Separator,
        // Place
        b(
            ic::icon_wire(tid),
            matches!(tool, Tool::Wire | Tool::Bus),
            last("wiring", ActiveBarAction::DrawWire),
            Menu::Wiring,
            ids::CHROME_ACTIVE_BAR_WIRING,
        ),
        b(
            power_icon,
            false,
            last("power", ActiveBarAction::PlacePowerGND),
            Menu::Power,
            ids::CHROME_ACTIVE_BAR_POWER_PORT,
        ),
        b(
            ic::icon_harness(tid),
            false,
            last("harness", ActiveBarAction::PlaceSignalHarness),
            Menu::Harness,
            ids::CHROME_ACTIVE_BAR_HARNESS,
        ),
        b(
            ic::icon_sheetsym(tid),
            false,
            last("sheet", ActiveBarAction::PlaceSheetSymbol),
            Menu::SheetSymbol,
            ids::CHROME_ACTIVE_BAR_SHEET_SYMBOL,
        ),
        b(
            ic::icon_port(tid),
            false,
            last("port", ActiveBarAction::PlacePort),
            Menu::Port,
            ids::CHROME_ACTIVE_BAR_PORT,
        ),
        b(
            ic::icon_directives(tid),
            false,
            last("directives", DIRECTIVES_DEFAULT_ACTION),
            Menu::Directives,
            ids::CHROME_ACTIVE_BAR_DIRECTIVES,
        ),
        ActiveBarItem::Separator,
        // Annotate
        b(
            ic::icon_text(tid),
            tool == Tool::Text,
            last("text", ActiveBarAction::PlaceTextString),
            Menu::TextTools,
            ids::CHROME_ACTIVE_BAR_TEXT,
        ),
        b(
            ic::icon_shapes(tid),
            matches!(
                tool,
                Tool::Line
                    | Tool::Rectangle
                    | Tool::Circle
                    | Tool::Bezier
                    | Tool::Table
                    | Tool::LinearDimension
                    | Tool::AlignedDimension
            ),
            last("shapes", ActiveBarAction::DrawLine),
            Menu::Shapes,
            ids::CHROME_ACTIVE_BAR_DRAWING_TOOLS,
        ),
        b(
            ic::icon_netcolor(tid),
            false,
            act(ActiveBarAction::ToolSelect),
            Menu::NetColor,
            ids::CHROME_ACTIVE_BAR_NET_COLOR,
        ),
    ];
    if layout::shows_draw_mode_pill(tool) {
        items.push(ActiveBarItem::Separator);
        // Declared width, not measured: `slot_offsets` places every
        // dropdown from these widths (see `ActiveBarItem::Custom`).
        items.push(ActiveBarItem::custom(
            corner_pill(s.corner_mode, tokens, i18n),
            layout::DRAW_MODE_PILL_WIDTH_PX,
        ));
    }
    items
}

/// The bar a test draws for `tool`: nothing selected, no net colour, no
/// group used yet.
#[cfg(test)]
pub(super) fn test_items(
    tool: Tool,
    tokens: &ThemeTokens,
    tid: ThemeId,
) -> Vec<ActiveBarItem<ActiveBarMsg>> {
    let last = std::collections::HashMap::new();
    let state = BarState {
        current_tool: tool,
        corner_mode: CornerMode::Ortho90,
        last_tool: &last,
        tid,
        has_selection: false,
        has_net_colors: false,
    };
    bar_items(&state, tokens, &I18n::en_us())
}

/// Window-space centre x of every dropdown trigger the view draws for
/// `tool` in a window `window_width` wide under theme `tid`, derived from
/// the drawn items and the widget's own row maths (not from `layout`'s
/// slot table).
#[cfg(test)]
pub(crate) fn drawn_trigger_centres_for(
    tool: Tool,
    window_width: f32,
    tid: ThemeId,
) -> Vec<(ActiveBarMenu, f32)> {
    let tokens = signex_types::theme::theme_tokens(tid);
    drawn_trigger_centres_with(tool, window_width, &tokens, tid)
}

/// [`drawn_trigger_centres_for`] with explicit `tokens`.
#[cfg(test)]
pub(crate) fn drawn_trigger_centres_with(
    tool: Tool,
    window_width: f32,
    tokens: &ThemeTokens,
    tid: ThemeId,
) -> Vec<(ActiveBarMenu, f32)> {
    use signex_widgets::active_bar::{Metrics, slot_offsets};
    let m = Metrics::GRAPHITE;
    let items = test_items(tool, tokens, tid);
    let (offsets, total) = slot_offsets(&items, &m);
    let bar_left = (window_width - total) / 2.0;
    items
        .iter()
        .zip(offsets)
        .filter_map(|(item, left)| match item {
            ActiveBarItem::Button(b) => match b.on_right_press {
                Some(ActiveBarMsg::ToggleMenu(menu)) => {
                    Some((menu, bar_left + left + item.width(&m) / 2.0))
                }
                _ => None,
            },
            _ => None,
        })
        .collect()
}

/// [`drawn_trigger_centres_with`] under the Graphite metrics, which the
/// canvas hit-test tests (Graphite prefs) use.
#[cfg(test)]
pub(crate) fn drawn_trigger_centres(tool: Tool, window_width: f32) -> Vec<(ActiveBarMenu, f32)> {
    let tokens = signex_types::theme::theme_tokens(ThemeId::DEFAULT);
    drawn_trigger_centres_with(tool, window_width, &tokens, ThemeId::DEFAULT)
}

#[cfg(test)]
mod pill_tests {
    use super::*;

    /// #1379: a pill click while drawing a bus must not switch the tool to
    /// Wire; every cell picks a corner mode and nothing else.
    #[test]
    fn the_corner_pill_picks_a_mode_and_never_a_tool() {
        for mode in [
            CornerMode::Ortho90,
            CornerMode::Angle45Start,
            CornerMode::Angle45End,
            CornerMode::FreeAngle,
        ] {
            for (_, cell) in CORNER_CELLS {
                assert!(matches!(
                    corner_cell_message(cell, mode),
                    ActiveBarMsg::CornerModePicked(_)
                ));
            }
        }
    }

    /// One cell is lit per mode; both 45° leg orders light the 45° cell,
    /// and clicking it keeps the leg order already chosen.
    #[test]
    fn one_cell_is_lit_and_the_45_cell_keeps_its_leg_order() {
        for mode in [
            CornerMode::Ortho90,
            CornerMode::Angle45Start,
            CornerMode::Angle45End,
            CornerMode::FreeAngle,
        ] {
            let lit = CORNER_CELLS
                .iter()
                .filter(|(_, cell)| corner_cell_lit(*cell, mode))
                .count();
            assert_eq!(lit, 1, "{mode:?}");
        }
        assert!(matches!(
            corner_cell_message(CornerMode::Angle45Start, CornerMode::Angle45End),
            ActiveBarMsg::CornerModePicked(CornerMode::Angle45End)
        ));
        assert!(matches!(
            corner_cell_message(CornerMode::FreeAngle, CornerMode::Ortho90),
            ActiveBarMsg::CornerModePicked(CornerMode::FreeAngle)
        ));
    }
}
