//! The Import Parameters window (#1295): what the CSV file changes on its
//! sheet, worked out from the sheet as it is now, and Cancel / Apply. It only
//! ever opens as its own window; state and rules are `app/parameter_io/`.

use iced::widget::{Column, Space, column, container, row, scrollable};
use iced::{Alignment, Background, Element, Length, Theme, Widget as _};
use signex_i18n::{MessageId, UserText, ids, t};
use signex_output::parameter_csv::{ImportPlan, ParameterChange};
use signex_widgets::text;

use crate::app::parameter_io::{ParameterImportState, ParameterIoMsg};
use crate::app::{Message, Signex};
use crate::styles::{
    DialogButtonKind, Spacing, TypeScale, dialog_body, dialog_button, dialog_footer, tokens,
};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Width of the Part column, in em of the body text.
const PART_EM: f32 = 6.0;

fn io(msg: ParameterIoMsg) -> Message {
    msg.into()
}

/// The file name of `path`, as the window names files.
fn shown_name(path: &std::path::Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// A row's four cells on the table's columns.
fn table_row<'a>(cells: [Element<'a, Message>; 4]) -> Element<'a, Message> {
    let [part, key, before, after] = cells;
    row![
        container(part).width(Length::Fixed(TS.body * PART_EM)),
        container(key).width(Length::FillPortion(2)),
        container(before).width(Length::FillPortion(3)),
        container(after).width(Length::FillPortion(3)),
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center)
    .boxed()
}

impl Signex {
    pub(in crate::app::view) fn view_parameter_import_body(&self) -> Element<'_, Message> {
        let Some(state) = self.ui_state.parameter_import.as_deref() else {
            return Space::new().boxed();
        };
        let i18n = self.ui_state.i18n.as_ref();
        let tokens = &self.document_state.panel_ctx.tokens;
        let plan = self.parameter_import_plan();
        let count = plan.as_ref().map_or(0, |plan| plan.changes.len());
        let body: Element<'_, Message> = match &plan {
            Some(plan) => self.parameter_import_content(state, plan),
            None => text(t!(
                i18n,
                ids::PARAMETER_IO_PREVIEW_SHEET_GONE,
                "sheet" => shown_name(&state.sheet)
            ))
            .size(TS.body)
            .boxed(),
        };
        let footer = dialog_footer(vec![
            dialog_button(
                t!(i18n, ids::DIALOG_BUTTON_CANCEL),
                DialogButtonKind::Secondary,
                Some(io(ParameterIoMsg::PreviewCancelled)),
                tokens,
            ),
            dialog_button(
                t!(i18n, ids::PARAMETER_IO_APPLY, "count" => count),
                DialogButtonKind::Primary,
                (count > 0).then(|| io(ParameterIoMsg::PreviewAccepted)),
                tokens,
            ),
        ]);
        container(
            column![container(dialog_body(body)).height(Length::Fill), footer]
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .style(crate::styles::modal_card(tokens))
        .clip(true)
        .boxed()
    }

    /// The summary, the undo note, the skipped references and the table.
    fn parameter_import_content<'a>(
        &'a self,
        state: &'a ParameterImportState,
        plan: &ImportPlan,
    ) -> Element<'a, Message> {
        let i18n = self.ui_state.i18n.as_ref();
        let r = tokens::resolve(&self.document_state.panel_ctx.tokens);
        let sheet = shown_name(&state.sheet);
        let mut content: Column<'a, Message> = column![].spacing(SP.m).height(Length::Fill);
        if plan.changes.is_empty() {
            content = content.push(
                text(t!(i18n, ids::PARAMETER_IO_PREVIEW_NOTHING, "sheet" => sheet)).size(TS.body),
            );
        } else {
            content = content
                .push(
                    text(t!(
                        i18n,
                        ids::PARAMETER_IO_PREVIEW_SUMMARY,
                        "changes" => plan.changes.len(),
                        "parts" => plan.part_count()
                    ))
                    .size(TS.body),
                )
                .push(
                    text(t!(
                        i18n,
                        ids::PARAMETER_IO_PREVIEW_COUNTS,
                        "added" => plan.added_count(),
                        "changed" => plan.changed_count()
                    ))
                    .size(TS.caption)
                    .color(r.ink3),
                )
                .push(
                    text(t!(i18n, ids::PARAMETER_IO_PREVIEW_UNDO))
                        .size(TS.caption)
                        .color(r.ink3),
                );
        }
        if plan.unmatched_lines > 0 {
            content = content.push(
                text(t!(
                    i18n,
                    ids::PARAMETER_IO_PREVIEW_UNMATCHED,
                    "count" => plan.unmatched_lines,
                    "references" => plan.unmatched.join(", ")
                ))
                .size(TS.caption)
                .color(r.ink3),
            );
        }
        if !plan.changes.is_empty() {
            content = content.push(self.parameter_import_table(&plan.changes));
        }
        content.boxed()
    }

    /// One line per change: the part, the parameter, its value now (or Not
    /// set) and the value Apply writes.
    fn parameter_import_table<'a>(&'a self, changes: &[ParameterChange]) -> Element<'a, Message> {
        let i18n = self.ui_state.i18n.as_ref();
        let tokens = &self.document_state.panel_ctx.tokens;
        let r = tokens::resolve(tokens);
        let head = |id: MessageId| text(t!(i18n, id)).size(TS.caption).color(r.ink4).boxed();
        let sunken = r.sunken;
        let header = container(table_row([
            head(ids::PARAMETER_IO_COLUMN_PART),
            head(ids::PARAMETER_IO_COLUMN_PARAMETER),
            head(ids::PARAMETER_IO_COLUMN_BEFORE),
            head(ids::PARAMETER_IO_COLUMN_AFTER),
        ]))
        .padding([SP.s, SP.m])
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(sunken)),
            ..container::Style::default()
        });
        let user = |s: &str| text(UserText::new(s.to_string())).size(TS.body);
        let mut rows: Column<'a, Message> = column![].width(Length::Fill);
        for change in changes {
            let before = match &change.before {
                Some(value) => user(value).color(r.ink3).boxed(),
                None => text(t!(i18n, ids::PARAMETER_IO_NOT_SET))
                    .size(TS.body)
                    .color(r.ink4)
                    .boxed(),
            };
            rows = rows.push(
                container(table_row([
                    user(&change.reference).boxed(),
                    user(&change.key).boxed(),
                    before,
                    user(&change.after).color(r.accent_text).boxed(),
                ]))
                .padding([SP.xs, SP.m]),
            );
        }
        container(column![
            header,
            scrollable(rows)
                .direction(crate::styles::thin_vertical())
                .style(crate::styles::scrollbar_style(tokens))
                .height(Length::Fill),
        ])
        .width(Length::Fill)
        .height(Length::Fill)
        .clip(true)
        .style(crate::styles::card(tokens))
        .boxed()
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use signex_output::parameter_csv;
    use signex_types::schematic::{SchematicSheet, Symbol};

    use crate::app::Signex;
    use crate::app::handlers::editing_commands::net_color_driver::add_sheet_with;
    use crate::app::parameter_io::ParameterImportState;

    /// The window builds for a closed sheet, a file that changes nothing and
    /// a file with changes and a skipped reference.
    #[test]
    fn the_preview_builds_in_each_state() {
        let (mut app, _task) = Signex::new();
        let sheet = Path::new("/w/power.snxsch");
        let open = |app: &mut Signex, text: &str| {
            app.ui_state.parameter_import = Some(Box::new(ParameterImportState {
                sheet: sheet.to_path_buf(),
                file: "/w/edited.csv".into(),
                csv: parameter_csv::parse(text.as_bytes()).expect("parse"),
            }));
        };
        open(&mut app, "Reference,MPN\nR1,x\n");
        let _ = app.view_parameter_import_body();
        let doc = SchematicSheet {
            symbols: vec![Symbol {
                reference: "R1".to_string(),
                ..Default::default()
            }],
            ..Default::default()
        };
        add_sheet_with(&mut app, sheet, doc);
        let _ = app.view_parameter_import_body();
        open(&mut app, "Reference,MPN\nR1,x\nR7,y\n");
        let _ = app.view_parameter_import_body();
        open(&mut app, "Reference,MPN\nR1,\n");
        let _ = app.view_parameter_import_body();
    }
}
