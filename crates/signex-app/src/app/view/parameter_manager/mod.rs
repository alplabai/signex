//! The Parameter Manager dialog: one row per placed symbol (power ports left
//! out), one column per custom parameter in use. Cells write through as you
//! type, one undo step per cell; the session of edited cells drives the
//! changed marks, the footer count and Revert changes. The window title bar
//! carries the title and the subtitle; `table.rs` draws the table.
//!
//! Edits go through `ParameterManagerMsg`, so they are undoable and
//! dirty-flagged like any other schematic edit.

mod table;

use super::*;
use iced::widget::{Space, column, svg};
use iced::{Alignment, Padding};
use signex_i18n::{Arg, I18n, ids, t};
use signex_types::schematic::{SchematicSheet, Symbol};
use signex_widgets::{text, text_input, tooltip};

use crate::styles::{self, DialogButtonKind, ti, tokens::resolve};

const TS: styles::TypeScale = styles::TypeScale::DEFAULT;
const SP: styles::Spacing = styles::Spacing::DEFAULT;

/// Width of the row filter, in em of the body text (240 px by default).
const FILTER_WIDTH_EM: f32 = 20.0;
/// Width of the new parameter name box, in em (180 px by default).
const NEW_KEY_WIDTH_EM: f32 = 15.0;

/// What the table area shows.
pub(crate) enum TableBody<'a> {
    /// The parts that pass the filter, in sheet order.
    Rows(Vec<&'a Symbol>),
    /// A filter is typed and no part passes it.
    NoMatch,
}

/// The rows the table lists: every placed symbol except power ports.
fn parts(doc: &SchematicSheet) -> impl Iterator<Item = &Symbol> {
    doc.symbols.iter().filter(|s| !s.is_power)
}

/// The rows that pass `filter`, or [`TableBody::NoMatch`] when a filter is
/// typed and none does.
pub(crate) fn table_body<'a>(i18n: &I18n, filter: &str, doc: &'a SchematicSheet) -> TableBody<'a> {
    let rows: Vec<&Symbol> = parts(doc)
        .filter(|s| row_matches_filter(i18n, filter, s))
        .collect();
    if rows.is_empty() && !filter.trim().is_empty() {
        TableBody::NoMatch
    } else {
        TableBody::Rows(rows)
    }
}

/// The parameter columns: every parameter a listed part carries, sorted.
/// Only the parameters someone actually uses get a column.
fn parameter_keys(doc: &SchematicSheet) -> Vec<String> {
    parts(doc)
        .flat_map(|s| s.fields.keys().cloned())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// Whether a row survives the filter: a case-insensitive substring of its
/// reference, value or footprint, folded with `i18n.fold_for_search` on both
/// sides. An empty filter keeps every row.
fn row_matches_filter(i18n: &I18n, filter: &str, sym: &Symbol) -> bool {
    fields_match_filter(
        i18n,
        filter,
        [
            sym.reference.as_str(),
            sym.value.as_str(),
            sym.footprint.as_str(),
        ],
    )
}

fn fields_match_filter(i18n: &I18n, filter: &str, fields: [&str; 3]) -> bool {
    let needle = i18n.fold_for_search(filter.trim());
    needle.is_empty()
        || fields
            .iter()
            .any(|field| i18n.fold_for_search(field).contains(&needle))
}

impl Signex {
    /// The active sheet's file name, as the subtitle and the no-match card
    /// name it.
    fn parameter_sheet_name(&self) -> String {
        self.document_state
            .active_path
            .as_deref()
            .and_then(std::path::Path::file_name)
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    /// Title-bar subtitle: `<sheet> · N parts · M parameters`. `None`
    /// without an active schematic.
    pub(crate) fn parameter_manager_subtitle(&self) -> Option<String> {
        let doc = self.document_state.active_engine()?.document();
        Some(self.ui_state.i18n.message_plain(
            ids::DIALOG_PARAMETER_MANAGER_SUBTITLE,
            &[
                ("sheet", Arg::from(self.parameter_sheet_name())),
                ("parts", Arg::from(parts(doc).count())),
                ("parameters", Arg::from(parameter_keys(doc).len())),
            ],
        ))
    }

    pub(super) fn view_parameter_manager_body(&self) -> Element<'_, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let area: Element<'_, Message> = if self.document_state.active_engine().is_some() {
            // The column widths share the free width, so the table is laid
            // out per size.
            iced::widget::responsive(move |size| self.view_parameter_table_area(size.width)).boxed()
        } else {
            let card = crate::panels::empty_state_card(
                tokens,
                t!(
                    self.ui_state.i18n,
                    ids::DIALOG_PARAMETER_MANAGER_NO_SCHEMATIC_TITLE
                ),
                t!(
                    self.ui_state.i18n,
                    ids::DIALOG_PARAMETER_MANAGER_NO_SCHEMATIC_BODY
                ),
                None,
            );
            empty_area(column![card].boxed())
        };
        container(
            column![
                self.view_parameter_toolbar(),
                container(area).width(Length::Fill).height(Length::Fill),
                self.view_parameter_footer(),
            ]
            .width(Length::Fill)
            .height(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .style(styles::modal_card(tokens))
        .clip(true)
        .boxed()
    }

    /// The table, or the no-match card when the filter hides every part.
    fn view_parameter_table_area(&self, width: f32) -> Element<'_, Message> {
        let Some(engine) = self.document_state.active_engine() else {
            return Space::new().boxed();
        };
        let doc = engine.document();
        let i18n = &self.ui_state.i18n;
        let filter = &self.ui_state.parameter_table.filter;
        match table_body(i18n, filter, doc) {
            TableBody::Rows(rows) => table::view(self, &rows, parameter_keys(doc), width),
            TableBody::NoMatch => {
                let tokens = &self.document_state.panel_ctx.tokens;
                let card = crate::panels::empty_state_card(
                    tokens,
                    t!(i18n, ids::DIALOG_PARAMETER_MANAGER_NO_MATCH_TITLE),
                    t!(
                        i18n,
                        ids::DIALOG_PARAMETER_MANAGER_NO_MATCH_BODY,
                        "sheet" => self.parameter_sheet_name(),
                        "query" => filter.trim().to_string()
                    ),
                    None,
                );
                let clear = styles::dialog_button(
                    t!(i18n, ids::DIALOG_PARAMETER_MANAGER_CLEAR_FILTER),
                    DialogButtonKind::Secondary,
                    Some(Message::ParameterManager(
                        ParameterManagerMsg::FilterCleared,
                    )),
                    tokens,
                );
                empty_area(column![card, clear].spacing(SP.m).boxed())
            }
        }
    }

    /// Filter on the left with the fields it searches; the new parameter
    /// name and Add column on the right, as one group that folds under the
    /// filter when the dialog is narrow.
    fn view_parameter_toolbar(&self) -> Element<'_, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = &self.ui_state.i18n;
        let table = &self.ui_state.parameter_table;
        let r = resolve(tokens);
        let (ink, ink4) = (ti(tokens.text), r.ink4);
        let icon = svg(crate::icons::icon_chrome_search(
            self.ui_state.effective_theme_id(),
        ))
        .width(TS.body)
        .height(TS.body)
        .style(move |_: &iced::Theme, _| svg::Style {
            color: Some(ink4),
            ..Default::default()
        });
        let filter_input = text_input(
            t!(i18n, ids::PLACEHOLDER_PARAMETER_MANAGER_FILTER),
            &table.filter,
        )
        .on_input(|text| Message::ParameterManager(ParameterManagerMsg::FilterChanged(text)))
        .padding(Padding::ZERO)
        .size(TS.body)
        .width(Length::Fill)
        .style(move |_: &iced::Theme, _| iced::widget::text_input::Style {
            background: iced::Background::Color(iced::Color::TRANSPARENT),
            border: iced::Border::default(),
            placeholder: ink4,
            value: ink,
            selection: iced::Color { a: 0.4, ..ink },
        });
        let filter = container(
            row![icon, filter_input]
                .spacing(SP.m)
                .align_y(Alignment::Center),
        )
        .padding([SP.s, SP.m])
        .width(Length::Fixed(TS.body * FILTER_WIDTH_EM))
        .style(styles::field(tokens, false));
        let hint = text(t!(i18n, ids::DIALOG_PARAMETER_MANAGER_FILTER_HINT))
            .size(TS.caption)
            .color(r.ink3);
        let new_key = text_input(
            t!(i18n, ids::PLACEHOLDER_PARAMETER_MANAGER_NEW_KEY),
            &table.new_key,
        )
        .style(crate::panels::form_look::text_input_style)
        .on_input(|text| Message::ParameterManager(ParameterManagerMsg::NewKeyChanged(text)))
        .on_submit(Message::ParameterManager(ParameterManagerMsg::ColumnAdded))
        .padding([SP.s, SP.m])
        .size(TS.body)
        .width(Length::Fixed(TS.body * NEW_KEY_WIDTH_EM));
        // Mockup row 70: "+ Add column".
        let add = styles::dialog_icon_button(
            crate::icons::icon_plus(self.ui_state.effective_theme_id()),
            t!(i18n, ids::DIALOG_PARAMETER_MANAGER_ADD_COLUMN),
            DialogButtonKind::Secondary,
            Some(Message::ParameterManager(ParameterManagerMsg::ColumnAdded)),
            tokens,
        );
        // Mockup row 70: the toolbar wraps at narrow widths. The filter group
        // and the Add group share one line while both fit, the Add group at
        // the right; else the Add group folds under the filter, and the hint
        // folds under the filter before the filter overflows. FitRow tops its
        // cells, so a strut as tall as the Add button centres the filter on
        // the Add group's line.
        let strut = Space::new().height(Length::Fixed(styles::ChromeMetrics::DEFAULT.button));
        let filter = row![strut, filter].align_y(Alignment::Center);
        let search = row![filter, hint]
            .spacing(SP.m)
            .align_y(Alignment::Center)
            .wrap()
            .vertical_spacing(SP.s);
        let add_column = container(row![new_key, add].spacing(SP.s).align_y(Alignment::Center))
            .align_x(iced::alignment::Horizontal::Right);
        let groups = signex_widgets::FitRow::new(vec![search.boxed(), add_column.boxed()])
            .spacing(SP.m)
            .fill(true);
        let bar = container(groups).padding([SP.m, SP.l]).width(Length::Fill);
        column![bar, table::rule(r.line_soft, Length::Fill, true)].boxed()
    }

    /// How edits apply, Export… and Import… (#1295), the changed-cell count,
    /// Revert changes and Done.
    fn view_parameter_footer(&self) -> Element<'_, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = &self.ui_state.i18n;
        let r = resolve(tokens);
        let changed = self.parameter_session_changes().len();
        let has_sheet = self.document_state.active_engine().is_some();
        let csv_button = |label: signex_i18n::MessageId,
                          msg: crate::app::parameter_io::ParameterIoMsg| {
            styles::dialog_button(
                t!(i18n, label),
                DialogButtonKind::Secondary,
                has_sheet.then(|| Message::from(msg)),
                tokens,
            )
        };
        let mut actions = row![
            csv_button(
                ids::PARAMETER_IO_EXPORT,
                crate::app::parameter_io::ParameterIoMsg::ExportRequested
            ),
            csv_button(
                ids::PARAMETER_IO_IMPORT,
                crate::app::parameter_io::ParameterIoMsg::ImportRequested
            ),
        ]
        .spacing(SP.m)
        .align_y(Alignment::Center);
        if changed > 0 {
            actions = actions.push(
                text(t!(i18n, ids::DIALOG_PARAMETER_MANAGER_CHANGED, "count" => changed))
                    .size(TS.caption)
                    .color(r.accent_text)
                    .boxed(),
            );
        }
        let revert = styles::dialog_button(
            t!(i18n, ids::DIALOG_PARAMETER_MANAGER_REVERT, "count" => changed),
            DialogButtonKind::Secondary,
            (changed > 0).then_some(Message::ParameterManager(
                ParameterManagerMsg::SessionReverted,
            )),
            tokens,
        );
        let revert = if changed > 0 {
            let tip = container(
                text(t!(i18n, ids::DIALOG_PARAMETER_MANAGER_REVERT_TIP, "count" => changed))
                    .size(TS.caption)
                    .color(ti(tokens.text)),
            )
            .padding([SP.xs, SP.m])
            .style(signex_widgets::active_bar_dropdown::tooltip_style(
                ti(tokens.hover),
                ti(tokens.border),
            ));
            tooltip(revert, tip, iced::widget::tooltip::Position::Top)
                .gap(SP.xs)
                .boxed()
        } else {
            revert
        };
        let done = styles::dialog_button(
            t!(i18n, ids::DIALOG_PARAMETER_MANAGER_DONE),
            DialogButtonKind::Primary,
            Some(Message::ParameterManager(ParameterManagerMsg::Close)),
            tokens,
        );
        actions = actions.push(revert).push(done);
        container(
            row![
                text(t!(i18n, ids::DIALOG_PARAMETER_MANAGER_FOOTER_HINT))
                    .size(TS.caption)
                    .color(r.ink3),
                Space::new().width(Length::Fill),
                actions,
            ]
            .align_y(Alignment::Center),
        )
        .padding([SP.m, SP.l])
        .width(Length::Fill)
        .style(styles::modal_footer_strip(tokens))
        .boxed()
    }
}

/// An empty-state card centred in the table area at a readable width.
fn empty_area(content: Element<'_, Message>) -> Element<'_, Message> {
    container(container(content).width(Length::Fit.max(TS.body * 37.0)))
        .padding(SP.xl)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .boxed()
}

#[cfg(test)]
mod toolbar_tests {
    use super::{FILTER_WIDTH_EM, NEW_KEY_WIDTH_EM, SP, TS};
    use crate::app::Signex;
    use crate::layout_probe::{find, lay_out_inputs};
    use iced::{Rectangle, Size};

    /// The filter box ("R1" typed) and the new-key box ("MPN" typed) of the
    /// real toolbar laid out `width` wide.
    fn filter_and_new_key(width: f32) -> (Rectangle, Rectangle) {
        let (mut app, _task) = Signex::new();
        app.ui_state.parameter_table.filter = "R1".to_string();
        app.ui_state.parameter_table.new_key = "MPN".to_string();
        let window = Size::new(width, TS.body * 40.0);
        let inputs = lay_out_inputs(app.view_parameter_toolbar(), window);
        (find(&inputs, "R1").bounds, find(&inputs, "MPN").bounds)
    }

    /// Mockup row 70: the filter and the new key share one line, centred
    /// on it, while both fit; when the toolbar is narrow the new key folds
    /// under the filter instead of running past the toolbar's edge.
    #[test]
    fn the_toolbar_groups_fold_instead_of_overflowing() {
        let (filter, key) = filter_and_new_key(TS.body * 100.0);
        assert!(key.x > filter.x + filter.width, "{filter:?} {key:?}");
        assert!(
            (filter.center_y() - key.center_y()).abs() < 1.0,
            "one line, centred: {filter:?} {key:?}"
        );
        let narrow = 2.0 * SP.l + TS.body * (FILTER_WIDTH_EM + NEW_KEY_WIDTH_EM / 2.0) + SP.m;
        let (filter, key) = filter_and_new_key(narrow);
        assert!(key.y >= filter.y + filter.height, "{filter:?} {key:?}");
        assert!(key.x + key.width <= narrow, "{key:?} within {narrow}");
    }

    /// Mockup row 70: "+ Add column". Between the new-key box and the
    /// button's label, the plus glyph is drawn in the label's ink.
    #[test]
    fn add_column_leads_with_a_plus_glyph() {
        use crate::layout_probe::{draw_pixels, lay_out_texts};
        use signex_types::theme::{ThemeId, theme_tokens};
        for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
            let (mut app, _task) = Signex::new();
            app.document_state.panel_ctx.tokens = theme_tokens(id);
            app.ui_state.parameter_table.new_key = "MPN".to_string();
            let window = Size::new(TS.body * 100.0, TS.body * 10.0);
            let key = find(&lay_out_inputs(app.view_parameter_toolbar(), window), "MPN").bounds;
            let texts = lay_out_texts(app.view_parameter_toolbar(), window);
            let label = find(&texts, "Add column").bounds;
            let pixels = draw_pixels(app.view_parameter_toolbar(), window);
            let ink = crate::styles::ti(app.document_state.panel_ctx.tokens.text).into_rgba8();
            let from = (key.x + key.width + SP.s + SP.xs) as usize;
            let rows = label.y as usize..(label.y + label.height) as usize;
            let glyph = rows.into_iter().any(|y| {
                (from..label.x as usize).any(|x| {
                    let i = (y * window.width as usize + x) * 4;
                    (0..3).all(|c| pixels[i + c].abs_diff(ink[c]) <= 48)
                })
            });
            assert!(glyph, "{id:?}: no plus glyph before the Add column label");
        }
    }

    /// The folding toolbar builds with a filter and a new key typed.
    #[test]
    fn the_toolbar_builds() {
        let (mut app, _task) = Signex::new();
        app.ui_state.parameter_table.filter = "R1".to_string();
        app.ui_state.parameter_table.new_key = "MPN".to_string();
        let _ = app.view_parameter_toolbar();
    }
}

#[cfg(test)]
mod filter_tests {
    use super::{TableBody, fields_match_filter, parameter_keys, table_body};
    use signex_i18n::I18n;
    use signex_types::schematic::{SchematicSheet, Symbol};

    #[test]
    fn a_turkish_filter_finds_dotted_and_dotless_i() {
        let tr = I18n::with_message_locale_for_test("tr");
        let row = ["U1", "İSTANBUL", "ISPARTA"];
        assert!(fields_match_filter(&tr, "istanbul", row));
        assert!(fields_match_filter(&tr, " ısparta ", row));
        assert!(!fields_match_filter(&tr, "isparta", row));
        assert!(fields_match_filter(&tr, "", row));
    }

    #[test]
    fn en_us_is_unchanged() {
        let en = I18n::en_us();
        let row = ["U1", "ISTANBUL", "Bridge"];
        assert!(fields_match_filter(&en, "istanbul", row));
        assert!(fields_match_filter(&en, "BRI", row));
        assert!(!fields_match_filter(&en, "ıstanbul", row));
    }

    fn sheet() -> SchematicSheet {
        let mut r1 = Symbol {
            reference: "R1".to_string(),
            footprint: "R_0603".to_string(),
            ..Default::default()
        };
        r1.fields.insert("MPN".to_string(), "RC0603".to_string());
        let mut gnd = Symbol {
            reference: "#PWR1".to_string(),
            is_power: true,
            ..Default::default()
        };
        gnd.fields.insert("Hidden".to_string(), String::new());
        SchematicSheet {
            symbols: vec![r1, gnd],
            ..Default::default()
        }
    }

    #[test]
    fn a_filter_nothing_matches_is_the_no_match_state() {
        let en = I18n::en_us();
        let doc = sheet();
        assert!(matches!(
            table_body(&en, "QFN-32", &doc),
            TableBody::NoMatch
        ));
        let TableBody::Rows(rows) = table_body(&en, "0603", &doc) else {
            panic!("R1's footprint matches");
        };
        assert_eq!(rows.len(), 1);
    }

    #[test]
    fn power_ports_are_neither_rows_nor_columns() {
        let en = I18n::en_us();
        let doc = sheet();
        let TableBody::Rows(rows) = table_body(&en, "", &doc) else {
            panic!("no filter lists every part");
        };
        assert_eq!(rows.len(), 1);
        assert_eq!(parameter_keys(&doc), vec!["MPN".to_string()]);
    }
}
