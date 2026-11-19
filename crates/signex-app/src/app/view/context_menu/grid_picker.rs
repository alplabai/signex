//! The grid picker popup (#63): the standard grid ladder under Imperial and
//! Metric subheads, mil rows captioned with their mm value, the active step
//! checked on the accent-soft fill, and a final Grid Properties… row; a
//! read-only preview of the active step sits beside it.

use super::*;

use super::items::text;
use signex_i18n::{I18n, MessageId, menu_ids};
use signex_widgets::active_bar_dropdown::{DropdownEntry, DropdownItem};

/// One rung of the ladder: its label, its step in mm and whether its row
/// carries the mm value as a caption (the mil rows).
struct Rung {
    label: MessageId,
    step_mm: f64,
    captioned: bool,
}

const fn mil(label: MessageId, step_mm: f64) -> Rung {
    Rung {
        label,
        step_mm,
        captioned: true,
    }
}

const fn mm(label: MessageId, step_mm: f64) -> Rung {
    Rung {
        label,
        step_mm,
        captioned: false,
    }
}

/// The imperial ladder (imperial designs anchor on 50 mil).
const IMPERIAL: &[Rung] = &[
    mil(menu_ids::CTX_GRID_1_MIL, 0.0254),
    mil(menu_ids::CTX_GRID_5_MIL, 0.127),
    mil(menu_ids::CTX_GRID_10_MIL, 0.254),
    mil(menu_ids::CTX_GRID_20_MIL, 0.508),
    mil(menu_ids::CTX_GRID_25_MIL, 0.635),
    mil(menu_ids::CTX_GRID_50_MIL, 1.27),
    mil(menu_ids::CTX_GRID_100_MIL, 2.54),
];

/// The metric ladder.
const METRIC: &[Rung] = &[
    mm(menu_ids::CTX_GRID_0_025_MM, 0.025),
    mm(menu_ids::CTX_GRID_0_100_MM, 0.100),
    mm(menu_ids::CTX_GRID_0_250_MM, 0.250),
    mm(menu_ids::CTX_GRID_0_500_MM, 0.500),
    mm(menu_ids::CTX_GRID_1_000_MM, 1.000),
    mm(menu_ids::CTX_GRID_2_500_MM, 2.500),
];

fn is_step(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

/// The ladder label of `step_mm`, when it is a rung.
fn rung_name(i18n: &I18n, step_mm: f64) -> Option<String> {
    IMPERIAL
        .iter()
        .chain(METRIC)
        .find(|rung| is_step(rung.step_mm, step_mm))
        .map(|rung| text(i18n, rung.label))
}

/// `step_mm` as the "1.27 mm" caption; engineering values keep '.' in every
/// locale.
fn mm_caption(i18n: &I18n, step_mm: f64) -> String {
    i18n.message_plain(
        menu_ids::CTX_GRID_MM_CAPTION,
        &[("mm", step_mm.to_string().into())],
    )
}

impl Signex {
    /// The grid picker popup: the ladder card and, beside it, the preview of
    /// the active step. Clicking a step sends
    /// `Message::Ui(UiMsg::GridPickerSelect(step_mm))` and closes the popup.
    pub(in crate::app::view) fn view_grid_picker_menu(&self) -> Element<'_, Message> {
        use iced::widget::Widget as _;
        let active_step = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)
            .and_then(|t| match &t.kind {
                crate::app::TabKind::FootprintEditor(p) => {
                    self.document_state.footprint_editors.get(p)
                }
                _ => None,
            })
            .map(|e| e.state.snap_options.grid_step_mm);
        let panel_ctx = &self.document_state.panel_ctx;
        let i18n = &self.ui_state.i18n;
        let ladder = super::placement::menu_card(
            grid_picker_entries(i18n, active_step),
            &panel_ctx.tokens,
            &panel_ctx.ui_font_name,
        )
        .element;
        let Some(step) = active_step else {
            return ladder;
        };
        let name = rung_name(i18n, step).unwrap_or_else(|| mm_caption(i18n, step));
        let preview = super::grid_preview::grid_preview_card(
            i18n,
            &panel_ctx.tokens,
            self.ui_state.effective_theme_id(),
            step,
            name,
        );
        iced::widget::row![ladder, preview]
            .spacing(crate::styles::Spacing::DEFAULT.m)
            .align_y(iced::Alignment::End)
            .boxed()
    }
}

/// One ladder row: picks its step; the active one is checked on the
/// accent-soft fill; a mil row shows its mm value as a caption.
fn rung_row(i18n: &I18n, rung: &Rung, active_step: Option<f64>) -> DropdownEntry<Message> {
    let is_active = active_step.is_some_and(|s| is_step(s, rung.step_mm));
    let mut item = DropdownItem::new(
        text(i18n, rung.label),
        Message::Ui(UiMsg::GridPickerSelect(rung.step_mm)),
    )
    .checked(is_active)
    .selected(is_active);
    if rung.captioned {
        item = item.caption(mm_caption(i18n, rung.step_mm));
    }
    DropdownEntry::Item(item)
}

/// The grid picker's rows: Imperial then Metric, each under its subhead,
/// then Grid Properties…, which opens the Grid Properties dialog.
fn grid_picker_entries(i18n: &I18n, active_step: Option<f64>) -> Vec<DropdownEntry<Message>> {
    let mut v = vec![DropdownEntry::Header(text(
        i18n,
        menu_ids::CTX_GRID_IMPERIAL,
    ))];
    v.extend(
        IMPERIAL
            .iter()
            .map(|rung| rung_row(i18n, rung, active_step)),
    );
    v.push(DropdownEntry::Header(text(i18n, menu_ids::CTX_GRID_METRIC)));
    v.extend(METRIC.iter().map(|rung| rung_row(i18n, rung, active_step)));
    v.push(DropdownEntry::Separator);
    v.push(DropdownEntry::Item(DropdownItem::new(
        text(i18n, menu_ids::CTX_GRID_PROPERTIES),
        Message::GridProperties(GridPropertiesMsg::Open),
    )));
    v
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

    /// Every step is a row that picks it; only the active one is checked and
    /// filled.
    #[test]
    fn each_row_picks_its_step_and_the_active_one_is_checked_and_filled() {
        let rows = grid_picker_entries(I18n::en_us_ref(), Some(1.27));
        let steps: Vec<_> = items(&rows)
            .into_iter()
            .filter_map(|item| match item.on_press {
                Some(Message::Ui(UiMsg::GridPickerSelect(step))) => Some((item, step)),
                _ => None,
            })
            .collect();
        assert_eq!(steps.len(), 13);
        for (item, step) in steps {
            let active = (step - 1.27).abs() < 1e-9;
            assert_eq!(item.checked, active, "{}", item.label);
            assert_eq!(item.selected, active, "{}", item.label);
        }
    }

    /// The ladder sits under the Imperial and Metric subheads, the mil rows
    /// carry their mm value, and the last row opens Grid Properties.
    #[test]
    fn the_ladder_has_two_subheads_mm_captions_and_a_properties_row() {
        let rows = grid_picker_entries(I18n::en_us_ref(), None);
        let heads: Vec<&str> = rows
            .iter()
            .filter_map(|row| match row {
                DropdownEntry::Header(h) => Some(h.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(heads, ["Imperial", "Metric"]);
        assert!(matches!(&rows[0], DropdownEntry::Header(h) if h == "Imperial"));
        let items = items(&rows);
        let caption = |label: &str| {
            items
                .iter()
                .find(|item| item.label == label)
                .and_then(|item| item.caption.clone())
        };
        assert_eq!(caption("50 Mil").as_deref(), Some("1.27 mm"));
        assert_eq!(caption("1 Mil").as_deref(), Some("0.0254 mm"));
        assert_eq!(caption("100 Mil").as_deref(), Some("2.54 mm"));
        assert_eq!(caption("1.000 mm"), None, "a metric row has no caption");
        let last = items.last().expect("rows");
        assert_eq!(last.label, "Grid Properties…");
        assert!(matches!(
            last.on_press,
            Some(Message::GridProperties(GridPropertiesMsg::Open))
        ));
    }

    /// The preview names the active rung by its ladder label.
    #[test]
    fn the_active_rung_is_named_by_its_label() {
        assert_eq!(
            rung_name(I18n::en_us_ref(), 1.27).as_deref(),
            Some("50 Mil")
        );
        assert_eq!(rung_name(I18n::en_us_ref(), 0.3), None);
    }
}
