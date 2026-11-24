//! The grid picker popup (mockup row 63): the standard grid ladder under Imperial and
//! Metric subheads, mil rows captioned with their mm value, the active step
//! checked on the accent-soft fill, and a final Grid Properties… row; a
//! read-only preview of the hovered step (the active one when no row is
//! hovered) sits beside it.

use super::*;

use super::items::text;
use signex_i18n::{Arg, I18n, Localized, MessageId, menu_ids};
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

/// The rung whose step is `step_mm`, when there is one.
fn rung_of(step_mm: f64) -> Option<&'static Rung> {
    IMPERIAL
        .iter()
        .chain(METRIC)
        .find(|rung| is_step(rung.step_mm, step_mm))
}

/// The ladder label of `step_mm`, when it is a rung.
fn rung_name(i18n: &I18n, step_mm: f64) -> Option<String> {
    rung_of(step_mm).map(|rung| text(i18n, rung.label))
}

/// The arguments of the "1.27 mm" caption; engineering values keep '.' in
/// every locale.
fn mm_caption_args(step_mm: f64) -> [(&'static str, Arg); 1] {
    [("mm", step_mm.to_string().into())]
}

/// `step_mm` as the "1.27 mm" caption.
fn mm_caption(i18n: &I18n, step_mm: f64) -> String {
    i18n.message_plain(menu_ids::CTX_GRID_MM_CAPTION, &mm_caption_args(step_mm))
}

/// The step the preview draws: the hovered rung, else the active step.
fn previewed_step(hovered_mm: Option<f64>, active_mm: f64) -> f64 {
    hovered_mm.unwrap_or(active_mm)
}

/// The preview's caption for `step_mm`: its ladder name, marked active when
/// it is `active_mm`.
fn preview_caption(i18n: &I18n, step_mm: f64, active_mm: f64) -> Localized {
    let rung = rung_of(step_mm);
    if is_step(step_mm, active_mm) {
        let name = rung.map_or_else(|| mm_caption(i18n, step_mm), |r| text(i18n, r.label));
        return i18n.message(menu_ids::CTX_GRID_PREVIEW_ACTIVE, &[("step", name.into())]);
    }
    match rung {
        Some(rung) => i18n.message(rung.label, &[]),
        None => i18n.message(menu_ids::CTX_GRID_MM_CAPTION, &mm_caption_args(step_mm)),
    }
}

impl Signex {
    /// The grid step of the footprint editor the open picker targets.
    fn grid_picker_active_step(&self) -> Option<f64> {
        let picker = self.interaction_state.grid_picker.as_ref()?;
        self.document_state
            .footprint_editors
            .get(&picker.target)
            .map(|e| e.state.snap_options.grid_step_mm)
    }

    /// The step the picker's preview draws, with its caption: the hovered
    /// row's step, else the active one; `None` when no editor is targeted.
    fn grid_preview_of(&self) -> Option<(f64, Localized)> {
        let active = self.grid_picker_active_step()?;
        let hovered = self
            .interaction_state
            .grid_picker
            .as_ref()
            .and_then(|picker| picker.hovered_step_mm);
        let step = previewed_step(hovered, active);
        Some((step, preview_caption(&self.ui_state.i18n, step, active)))
    }

    /// The grid picker popup: the ladder card and, beside it, the preview of
    /// the hovered step, or the active one when no row is hovered (mockup
    /// row 63). Clicking a step sends
    /// `Message::Ui(UiMsg::GridPickerSelect(step_mm))` and closes the popup.
    pub(in crate::app::view) fn view_grid_picker_menu(&self) -> Element<'_, Message> {
        use iced::widget::Widget as _;
        let panel_ctx = &self.document_state.panel_ctx;
        let i18n = &self.ui_state.i18n;
        let ladder = super::placement::menu_card(
            grid_picker_entries(i18n, self.grid_picker_active_step()),
            &panel_ctx.tokens,
            &panel_ctx.ui_font_name,
        )
        .element;
        let Some((step, caption)) = self.grid_preview_of() else {
            return ladder;
        };
        let preview = super::grid_preview::grid_preview_card(
            i18n,
            &panel_ctx.tokens,
            self.ui_state.effective_theme_id(),
            step,
            caption,
        );
        iced::widget::row![ladder, preview]
            .spacing(crate::styles::Spacing::DEFAULT.m)
            .align_y(iced::Alignment::End)
            .boxed()
    }
}

/// One ladder row: picks its step, and hovering it shows the step in the
/// preview; the active one is checked on the accent-soft fill; a mil row
/// shows its mm value as a caption.
fn rung_row(i18n: &I18n, rung: &Rung, active_step: Option<f64>) -> DropdownEntry<Message> {
    let is_active = active_step.is_some_and(|s| is_step(s, rung.step_mm));
    let mut item = DropdownItem::new(
        text(i18n, rung.label),
        Message::Ui(UiMsg::GridPickerSelect(rung.step_mm)),
    )
    .hover(
        Message::Ui(UiMsg::GridPickerStepHovered(rung.step_mm)),
        Message::Ui(UiMsg::GridPickerStepLeft(rung.step_mm)),
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
        Message::GridProperties(GridPropertiesMsg::OpenFromPicker),
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
            Some(Message::GridProperties(GridPropertiesMsg::OpenFromPicker))
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

    /// Every step row publishes its own step as the pointer enters and
    /// leaves it, so the preview can follow the hovered row (mockup row 63).
    #[test]
    fn each_step_row_reports_hovering_its_step() {
        let rows = grid_picker_entries(I18n::en_us_ref(), Some(1.27));
        let mut hovered = 0;
        for item in items(&rows) {
            let Some(Message::Ui(UiMsg::GridPickerSelect(step))) = item.on_press else {
                assert!(item.on_hover.is_none(), "{}", item.label);
                continue;
            };
            match &item.on_hover {
                Some((
                    Message::Ui(UiMsg::GridPickerStepHovered(enter)),
                    Message::Ui(UiMsg::GridPickerStepLeft(leave)),
                )) => {
                    assert!(is_step(*enter, step), "{}", item.label);
                    assert!(is_step(*leave, step), "{}", item.label);
                }
                _ => panic!("{} reports no hover", item.label),
            }
            hovered += 1;
        }
        assert_eq!(hovered, 13);
    }

    /// The preview draws the hovered step, and the active one when no row
    /// is hovered; its caption names the step and marks only the active one.
    #[test]
    fn the_preview_follows_the_hovered_step() {
        let i18n = I18n::en_us_ref();
        let caption = |step: f64, active: f64| {
            preview_caption(i18n, step, active)
                .into_string()
                .replace(['\u{2068}', '\u{2069}'], "")
        };
        assert!(is_step(previewed_step(None, 1.27), 1.27));
        assert!(is_step(previewed_step(Some(1.0), 1.27), 1.0));
        assert_eq!(caption(1.27, 1.27), "50 Mil · active");
        assert_eq!(caption(1.0, 1.27), "1.000 mm");
        assert_eq!(caption(2.54, 1.27), "100 Mil");
    }

    fn app_with_picker() -> Signex {
        let (mut app, _boot) = Signex::new();
        app.interaction_state.grid_picker = Some(crate::app::GridPickerState {
            x: 0.0,
            y: 0.0,
            target: std::path::PathBuf::from("part.snxfpt"),
            owner: None,
            hovered_step_mm: None,
        });
        app
    }

    fn hovered(app: &Signex) -> Option<f64> {
        app.interaction_state
            .grid_picker
            .as_ref()
            .and_then(|picker| picker.hovered_step_mm)
    }

    /// Entering a row hovers its step; leaving it returns to none; a late
    /// leave of the row before keeps the newer hover.
    #[test]
    fn hovering_a_row_sets_the_previewed_step_and_leaving_clears_it() {
        let mut app = app_with_picker();
        let _ = app.update(Message::Ui(UiMsg::GridPickerStepHovered(2.54)));
        assert_eq!(hovered(&app), Some(2.54));
        let _ = app.update(Message::Ui(UiMsg::GridPickerStepHovered(1.0)));
        let _ = app.update(Message::Ui(UiMsg::GridPickerStepLeft(2.54)));
        assert_eq!(hovered(&app), Some(1.0), "a stale leave keeps the hover");
        let _ = app.update(Message::Ui(UiMsg::GridPickerStepLeft(1.0)));
        assert_eq!(hovered(&app), None);
        let _ = app.update(Message::Ui(UiMsg::GridPickerStepHovered(f64::NAN)));
        assert_eq!(hovered(&app), None, "a bad step is not hovered");
    }

    /// The open picker's preview draws the active step until a row is
    /// hovered, then the hovered row's step under its own name (mockup row
    /// 63).
    #[test]
    fn the_open_pickers_preview_draws_the_hovered_row() {
        let mut app = app_with_picker();
        let path = std::path::PathBuf::from("part.snxfpt");
        let file =
            signex_library::FootprintFile::from_footprint(signex_library::Footprint::empty("fp"));
        let mut editor = crate::app::FootprintDocument::new(path.clone(), file);
        editor.state.snap_options.grid_step_mm = 1.27;
        app.document_state.footprint_editors.insert(path, editor);
        let preview = |app: &Signex| {
            app.grid_preview_of().map(|(step, caption)| {
                let caption = caption.into_string().replace(['\u{2068}', '\u{2069}'], "");
                (step, caption)
            })
        };
        assert_eq!(preview(&app), Some((1.27, "50 Mil · active".to_owned())));
        let _ = app.update(Message::Ui(UiMsg::GridPickerStepHovered(2.54)));
        assert_eq!(preview(&app), Some((2.54, "100 Mil".to_owned())));
        let _ = app.update(Message::Ui(UiMsg::GridPickerStepLeft(2.54)));
        assert_eq!(preview(&app), Some((1.27, "50 Mil · active".to_owned())));
    }

    /// The preview needs the editor the picker targets; without it there is
    /// none to draw.
    #[test]
    fn a_picker_without_its_editor_has_no_preview() {
        let app = app_with_picker();
        assert!(app.grid_preview_of().is_none());
    }

    /// The ladder marks no row as off the 2.54 mm pin pitch: a metric row
    /// is its plain label, whatever step is active, and the preview's
    /// rings show the miss instead (mockup row 63, accepted 2026-11-24).
    #[test]
    fn the_ladder_leaves_off_pitch_marking_to_the_preview() {
        let i18n = I18n::en_us_ref();
        for active in [None, Some(1.27), Some(1.0)] {
            let rows = grid_picker_entries(i18n, active);
            let items = items(&rows);
            for rung in METRIC {
                let label = text(i18n, rung.label);
                let item = items
                    .iter()
                    .find(|item| item.label == label)
                    .expect("metric row");
                assert!(item.caption.is_none(), "{label}");
                assert!(item.badge.is_none(), "{label}");
                assert!(item.tooltip.is_none(), "{label}");
                assert!(item.icon.is_none() && item.swatch.is_none(), "{label}");
                assert!(!item.disabled, "{label}");
                assert!(
                    !super::super::grid_preview::pins_on_grid(rung.step_mm),
                    "{label}: the preview rings mark it off the pitch"
                );
            }
        }
    }

    /// Both ladders are listed whatever unit the active step is in; there
    /// is no More row hiding the other one (mockup row 63, accepted
    /// 2026-11-24).
    #[test]
    fn both_ladders_show_whatever_the_active_unit() {
        let i18n = I18n::en_us_ref();
        for active in [None, Some(1.27), Some(1.0)] {
            let rows = grid_picker_entries(i18n, active);
            let heads: Vec<&str> = rows
                .iter()
                .filter_map(|row| match row {
                    DropdownEntry::Header(h) => Some(h.as_str()),
                    _ => None,
                })
                .collect();
            assert_eq!(heads, ["Imperial", "Metric"], "{active:?}");
            let items = items(&rows);
            let steps = items
                .iter()
                .filter(|item| {
                    matches!(item.on_press, Some(Message::Ui(UiMsg::GridPickerSelect(_))))
                })
                .count();
            assert_eq!(steps, IMPERIAL.len() + METRIC.len(), "{active:?}");
            assert!(items.iter().all(|item| !item.submenu), "{active:?}");
        }
    }

    /// A hover with the picker closed opens nothing.
    #[test]
    fn a_hover_with_the_picker_closed_does_nothing() {
        let (mut app, _boot) = Signex::new();
        let _ = app.update(Message::Ui(UiMsg::GridPickerStepHovered(1.27)));
        assert!(app.interaction_state.grid_picker.is_none());
    }
}
