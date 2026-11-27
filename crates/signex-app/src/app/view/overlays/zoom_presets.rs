//! The zoom presets (Graphite mockup #64): the small menu the zoom pill's percentage opens
//! above the pill. 50 / 100 / 200 / 400 % zoom the schematic about the canvas
//! centre (the current one checked), then Fit All and Fit Selection run their
//! commands; Fit Selection is greyed with nothing selected. A pick, Esc or a
//! click outside closes it (`OverlayId::ZoomPresets`).

use iced::Element;
use iced::widget::Widget as _;
use signex_i18n::{Arg, I18n, menu_ids, t};
use signex_widgets::active_bar_dropdown::{DropdownEntry, DropdownItem, MenuMetrics};

use super::zoom_pill::pill_origin;
use super::*;
use crate::canvas::zoom_pill::{PRESETS, pill_size};

const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// The command the Fit All row runs.
const FIT_ALL_COMMAND: &str = "zoom_to_fit";
/// The command the Fit Selection row runs.
const FIT_SELECTION_COMMAND: &str = "zoom_to_selection_area";

/// The menu's rows for a canvas at `zoom_percent`. `has_selection` lets Fit
/// Selection run.
pub(in crate::app::view) fn zoom_preset_entries(
    i18n: &I18n,
    zoom_percent: f64,
    has_selection: bool,
) -> Vec<DropdownEntry<Message>> {
    let mut rows: Vec<DropdownEntry<Message>> = PRESETS
        .iter()
        .map(|&percent| {
            let label = i18n.message_plain(
                menu_ids::ZOOM_PRESETS_PERCENT,
                &[("percent", Arg::from(percent.to_string()))],
            );
            // Checked when the pill's whole-percent read-out shows this level.
            let current = (zoom_percent - f64::from(percent)).abs() < 0.5;
            DropdownEntry::Item(
                DropdownItem::new(label, Message::Ui(UiMsg::ZoomPresetPicked(percent)))
                    .checked(current),
            )
        })
        .collect();
    rows.push(DropdownEntry::Separator);
    let fit_all = crate::keymap::command_menu_text(i18n, FIT_ALL_COMMAND);
    rows.push(command_row(fit_all, FIT_ALL_COMMAND, true));
    let fit_selection = t!(i18n, menu_ids::ZOOM_PRESETS_FIT_SELECTION).into_string();
    rows.push(command_row(
        fit_selection,
        FIT_SELECTION_COMMAND,
        has_selection,
    ));
    rows
}

/// A row that closes the menu and runs `command_id`; greyed unless `live`
/// (and when the id does not parse, which the catalog rules out).
fn command_row(label: String, command_id: &str, live: bool) -> DropdownEntry<Message> {
    let item = match crate::keymap::AppCommandId::new(command_id) {
        Ok(id) if live => DropdownItem::new(label, Message::Ui(UiMsg::ZoomPresetCommandPicked(id))),
        _ => DropdownItem::passive(label).disabled(true),
    };
    DropdownEntry::Item(item)
}

impl Signex {
    /// Whether the preset menu paints: it is open and the main window shows
    /// a sheet, so the pill it hangs from is there. Esc reads the same.
    pub(crate) fn zoom_presets_shown(&self) -> bool {
        self.interaction_state.zoom_presets_open && self.document_state.active_engine().is_some()
    }

    /// The preset menu over the zoom pill: the click-outside layer, then the
    /// card, its bottom one gap above the pill and its right edge on the
    /// pill's. Main window only, like the pill. Placed against the canvas
    /// of the laid-out size, as the pill is, so a stale `window_size` cannot
    /// part the two (#2911).
    pub(in crate::app::view) fn zoom_presets_overlay(
        &self,
        window_id: iced::window::Id,
    ) -> Vec<Element<'_, Message>> {
        if self.ui_state.main_window_id != Some(window_id) || !self.zoom_presets_shown() {
            return Vec::new();
        }
        let zoom = self
            .interaction_state
            .canvas_for_window(window_id)
            .zoom_percent();
        let has_selection = self.command_context(None).has_selection;
        let layer = iced::widget::responsive(move |size: iced::Size| {
            let canvas = self.canvas_bounds_sized(None, (size.width, size.height));
            let (card, extent) = self.zoom_presets_card(zoom, has_selection);
            let layers = Self::menu_overlay(
                Message::Ui(UiMsg::ZoomPresetsDismissed),
                card,
                presets_anchor(canvas, extent),
            );
            layers
                .into_iter()
                .fold(
                    iced::widget::Stack::new()
                        .width(iced::Length::Fill)
                        .height(iced::Length::Fill),
                    |stack, layer| stack.push(layer),
                )
                .boxed()
        });
        vec![layer.boxed()]
    }

    /// The preset card for a canvas at `zoom` percent, and its (width, height).
    pub(in crate::app::view) fn zoom_presets_card(
        &self,
        zoom: f64,
        has_selection: bool,
    ) -> (Element<'_, Message>, (f32, f32)) {
        let panel_ctx = &self.document_view.panel_ctx;
        let entries = zoom_preset_entries(&self.ui_state.i18n, zoom, has_selection);
        let metrics = MenuMetrics::GRAPHITE;
        let width = metrics.fitted_width(&entries, f32::MAX, &panel_ctx.ui_font_name);
        let height = metrics.card_height(&entries).unwrap_or_default();
        let card =
            signex_widgets::active_bar_dropdown::view(entries, &panel_ctx.tokens, Some(width));
        (card, (width, height))
    }
}

/// Top-left of a `card` (width, height) whose bottom sits one gap above the
/// pill of a canvas at `canvas`, right edges aligned.
pub(in crate::app::view) fn presets_anchor(
    canvas: iced::Rectangle,
    card: (f32, f32),
) -> (f32, f32) {
    let (x, y) = pill_origin(canvas);
    let (w, _) = pill_size();
    (x + w - card.0, y - SP.m - card.1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items(rows: &[DropdownEntry<Message>]) -> Vec<&DropdownItem<Message>> {
        rows.iter()
            .filter_map(|row| match row {
                DropdownEntry::Item(item) => Some(item),
                _ => None,
            })
            .collect()
    }

    /// Four levels, a separator, Fit All and Fit Selection, as drawn.
    #[test]
    fn the_menu_lists_the_levels_then_the_fits() {
        let rows = zoom_preset_entries(I18n::en_us_ref(), 100.0, true);
        let labels: Vec<&str> = items(&rows).iter().map(|i| i.label.as_str()).collect();
        assert_eq!(
            labels,
            ["50%", "100%", "200%", "400%", "Fit All", "Fit Selection"]
        );
        assert!(matches!(rows[4], DropdownEntry::Separator));
        assert_eq!(rows.len(), 7);
    }

    /// Each level row picks its level; only the one the read-out shows is
    /// checked.
    #[test]
    fn each_level_picks_itself_and_the_current_one_is_checked() {
        let rows = zoom_preset_entries(I18n::en_us_ref(), 200.3, true);
        for item in items(&rows).into_iter().take(4) {
            let Some(Message::Ui(UiMsg::ZoomPresetPicked(percent))) = item.on_press else {
                panic!("{} picks no level", item.label);
            };
            assert_eq!(item.checked, percent == 200, "{}", item.label);
        }
        let off_level = zoom_preset_entries(I18n::en_us_ref(), 137.0, true);
        assert!(items(&off_level).iter().all(|item| !item.checked));
    }

    /// Fit Selection is greyed with nothing selected; Fit All never is, and
    /// both run their catalog commands.
    #[test]
    fn fit_selection_needs_a_selection() {
        let command = |item: &DropdownItem<Message>| match &item.on_press {
            Some(Message::Ui(UiMsg::ZoomPresetCommandPicked(id))) => Some(id.as_str().to_owned()),
            _ => None,
        };
        let rows = zoom_preset_entries(I18n::en_us_ref(), 100.0, false);
        let rows = items(&rows);
        assert_eq!(command(rows[4]).as_deref(), Some(FIT_ALL_COMMAND));
        assert!(!rows[4].disabled);
        assert!(rows[5].disabled, "Fit Selection is live with no selection");
        assert_eq!(command(rows[5]), None);

        let live = zoom_preset_entries(I18n::en_us_ref(), 100.0, true);
        let live = items(&live);
        assert!(!live[5].disabled);
        assert_eq!(command(live[5]).as_deref(), Some(FIT_SELECTION_COMMAND));
    }

    /// The card opens above the pill, right edges aligned.
    #[test]
    fn the_menu_sits_above_the_pill() {
        let canvas =
            iced::Rectangle::new(iced::Point::new(100.0, 80.0), iced::Size::new(800.0, 600.0));
        let card = (200.0, 180.0);
        let (x, y) = presets_anchor(canvas, card);
        let (px, py) = pill_origin(canvas);
        let (pw, _) = pill_size();
        assert_eq!(x + card.0, px + pw);
        assert!(y + card.1 < py, "the card overlaps the pill");
    }
}
