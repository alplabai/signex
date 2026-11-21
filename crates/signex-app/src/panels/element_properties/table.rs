//! Properties of a table (#1300): Data source (by source kind), the column
//! checkboxes in two columns, and the Style section every source shows
//! (Text size, Cell padding, Borders, and Header fill behind its enable
//! header). The illustration draws the table's style, not its data.

use iced::widget::{Column, Row, row};
use iced::{Alignment, Element, Length, Point, Rectangle, Size, Widget as _};
use signex_i18n::{UserText, ids, t};
use signex_types::schematic::{
    PinSort, SchDrawing, SchTable, TableBorders, TableRefresh, TableSource,
};
use signex_widgets::property_illustration::{
    Anchor, Callout, Illustration, Ink, Paint, Part, Primitive, Style, Tint, Weight,
};

use super::super::{PanelContext, PanelMsg};
use super::annotation::{AnnotationField, AnnotationPanelMsg, Rows, SP, msg};
use crate::app::sch_tables::{AnnotationEdit, TableSourceKind};
use crate::choice::Choice;

const TEXT_SIZE_KEY: &str = "table.text_size";
const PADDING_KEY: &str = "table.cell_padding";
/// Body rows the illustration draws at most.
const SHOWN_ROWS: usize = 4;

pub(super) fn view_table_properties<'a>(
    ctx: &'a PanelContext,
    drawing: &SchDrawing,
    table: &SchTable,
) -> Element<'a, PanelMsg> {
    let rows = Rows::new(ctx, drawing);
    let (row_count, column_count) = table.size();
    let mut col = Column::new().spacing(0).width(Length::Fill);
    col = col.push(rows.header(
        crate::icons::icon_dd_table(ctx.theme_id),
        rows.t(ids::PROPERTIES_TABLE_TITLE),
        t!(ctx.i18n, ids::PROPERTIES_TABLE_CAPTION, "rows" => row_count, "columns" => column_count),
    ));
    col = col.push(rows.illustration(illustration(&rows, table)));
    col = col.push(rows.section(
        "table_source",
        ids::PROPERTIES_TABLE_SECTION_SOURCE,
        source_rows(&rows, table),
    ));
    col = col.push(rows.section(
        "table_columns",
        ids::PROPERTIES_TABLE_SECTION_COLUMNS,
        vec![column_checks(&rows, table)],
    ));
    col = col.push(rows.section(
        "table_style",
        ids::PROPERTIES_TABLE_SECTION_STYLE,
        style_rows(&rows, table),
    ));
    col.boxed()
}

fn source_label(kind: TableSourceKind) -> signex_i18n::MessageId {
    match kind {
        TableSourceKind::ComponentPins => ids::PROPERTIES_TABLE_SOURCE_PINS,
        TableSourceKind::Typed => ids::PROPERTIES_TABLE_SOURCE_TYPED,
        TableSourceKind::ProjectSheets => ids::PROPERTIES_TABLE_SOURCE_SHEETS,
    }
}

fn source_rows<'a>(rows: &Rows<'a>, table: &SchTable) -> Vec<Element<'a, PanelMsg>> {
    let ctx = rows.ctx;
    let first = ctx.annotation.designators.first().cloned();
    let source = rows.dropdown(
        Choice::named(&TableSourceKind::ALL, source_label, &ctx.i18n),
        Some(TableSourceKind::of(&table.source)),
        move |kind| AnnotationEdit::TableSource(kind, first.clone()),
    );
    let mut out = vec![rows.field(rows.t(ids::PROPERTIES_TABLE_SOURCE), source)];
    let refresh = || {
        rows.field(
            rows.t(ids::PROPERTIES_TABLE_REFRESH),
            rows.dropdown(
                Choice::named(
                    &[TableRefresh::OnCompile, TableRefresh::Off],
                    |r| match r {
                        TableRefresh::OnCompile => ids::PROPERTIES_TABLE_REFRESH_ON_COMPILE,
                        TableRefresh::Off => ids::PROPERTIES_TABLE_REFRESH_OFF,
                    },
                    &ctx.i18n,
                ),
                Some(table.refresh),
                AnnotationEdit::TableRefresh,
            ),
        )
    };
    match &table.source {
        TableSource::ComponentPins { designator, sort } => {
            let parts: Vec<Choice<String>> = ctx
                .annotation
                .designators
                .iter()
                .map(|d| Choice {
                    value: d.clone(),
                    label: d.clone(),
                })
                .collect();
            let current = (!designator.is_empty()).then(|| designator.clone());
            out.push(rows.field(
                rows.t(ids::PROPERTIES_TABLE_COMPONENT),
                rows.dropdown(parts, current, AnnotationEdit::TableDesignator),
            ));
            out.push(rows.field(
                rows.t(ids::PROPERTIES_TABLE_SORT),
                rows.dropdown(
                    Choice::named(
                        &[PinSort::Number, PinSort::Name],
                        |s| match s {
                            PinSort::Number => ids::PROPERTIES_TABLE_SORT_NUMBER,
                            PinSort::Name => ids::PROPERTIES_TABLE_SORT_NAME,
                        },
                        &ctx.i18n,
                    ),
                    Some(*sort),
                    AnnotationEdit::TablePinSort,
                ),
            ));
            out.push(refresh());
            if ctx.annotation.source_missing {
                out.push(rows.note(rows.t(ids::PROPERTIES_TABLE_SOURCE_MISSING)));
            }
        }
        TableSource::Typed => {
            let buttons = row![
                rows.action(
                    ids::PROPERTIES_TABLE_CSV_IMPORT,
                    msg(AnnotationPanelMsg::CsvImportRequested)
                ),
                rows.action(
                    ids::PROPERTIES_TABLE_CSV_EXPORT,
                    msg(AnnotationPanelMsg::CsvExportRequested)
                ),
            ]
            .spacing(SP.s)
            .align_y(Alignment::Center);
            out.push(rows.field(rows.t(ids::PROPERTIES_TABLE_CSV), buttons.boxed()));
        }
        TableSource::ProjectSheets { per_channel } => {
            out.push(rows.field(
                rows.t(ids::PROPERTIES_TABLE_CHANNELS),
                rows.dropdown(
                    Choice::named(
                        &[true, false],
                        |on| {
                            if on {
                                ids::PROPERTIES_TABLE_PER_CHANNEL
                            } else {
                                ids::PROPERTIES_TABLE_PER_SHEET
                            }
                        },
                        &ctx.i18n,
                    ),
                    Some(*per_channel),
                    AnnotationEdit::TablePerChannel,
                ),
            ));
            out.push(refresh());
        }
    }
    out
}

/// One checkbox per column, in two columns; unchecking hides it on the
/// sheet, checking shows it again.
fn column_checks<'a>(rows: &Rows<'a>, table: &SchTable) -> Element<'a, PanelMsg> {
    let checks: Vec<Element<'a, PanelMsg>> = table
        .columns
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let label: signex_widgets::Label<'a> = if c.title.trim().is_empty() {
                t!(rows.ctx.i18n, ids::PROPERTIES_TABLE_UNTITLED_COLUMN, "index" => i + 1).into()
            } else {
                UserText::new(c.title.clone()).into()
            };
            let key = c.key.clone();
            rows.check(label, c.shown, move |on| {
                AnnotationEdit::TableColumnShown(key.clone(), on)
            })
        })
        .collect();
    let mut grid = Column::new().spacing(SP.xs).width(Length::Fill);
    let mut iter = checks.into_iter();
    while let Some(left) = iter.next() {
        let mut line = Row::new().spacing(SP.m).width(Length::Fill);
        line = line.push(iced::widget::container(left).width(Length::FillPortion(1)));
        if let Some(right) = iter.next() {
            line = line.push(iced::widget::container(right).width(Length::FillPortion(1)));
        }
        grid = grid.push(line);
    }
    iced::widget::container(grid)
        .padding([SP.xs, SP.l])
        .width(Length::Fill)
        .boxed()
}

fn style_rows<'a>(rows: &Rows<'a>, table: &SchTable) -> Vec<Element<'a, PanelMsg>> {
    let ctx = rows.ctx;
    let mm = ctx.i18n.message_plain(ids::PROPERTIES_ANNOTATION_MM, &[]);
    let with_unit = |field, value: f64| {
        row![
            rows.input(field, format!("{value:.2}")),
            rows.value(UserText::new(mm.clone())),
        ]
        .spacing(SP.xs)
        .align_y(Alignment::Center)
        .boxed()
    };
    let swatch_note = rows.field(
        rows.t(ids::PROPERTIES_TABLE_HEADER_COLOUR),
        rows.value(rows.t(ids::PROPERTIES_TABLE_HEADER_COLOUR_NOTE)),
    );
    vec![
        rows.field(
            rows.t(ids::PROPERTIES_TABLE_TEXT_SIZE),
            with_unit(AnnotationField::TableTextSize, table.style.text_size),
        ),
        rows.field(
            rows.t(ids::PROPERTIES_TABLE_CELL_PADDING),
            with_unit(AnnotationField::TableCellPadding, table.style.cell_padding),
        ),
        rows.field(
            rows.t(ids::PROPERTIES_TABLE_BORDERS),
            rows.segmented(
                [
                    (TableBorders::All, ids::PROPERTIES_TABLE_BORDERS_ALL),
                    (TableBorders::Outer, ids::PROPERTIES_TABLE_BORDERS_OUTER),
                    (TableBorders::None, ids::PROPERTIES_TABLE_BORDERS_NONE),
                ]
                .into_iter()
                .map(|(b, label)| {
                    (
                        label,
                        table.style.borders == b,
                        AnnotationEdit::TableBorders(b),
                    )
                })
                .collect(),
            ),
        ),
        signex_widgets::enable_section::EnableSection::new(
            rows.t(ids::PROPERTIES_TABLE_HEADER_FILL),
            table.style.header_fill,
        )
        .on_toggle(|on| super::annotation::picked(AnnotationEdit::TableHeaderFill(on)))
        .view(
            crate::panels::enable_section::enable_section_style(&ctx.tokens),
            swatch_note,
        ),
    ]
}

/// The table's style as line art: an outline grid of the header and a few
/// rows, cell text as placeholder bars, Text size and Cell padding callouts.
fn illustration(rows: &Rows<'_>, table: &SchTable) -> Illustration {
    const W: f32 = 240.0;
    const H: f32 = 120.0;
    const X0: f32 = 56.0;
    const Y0: f32 = 14.0;
    let shown = table.shown_columns();
    let body = table.rows.len().min(SHOWN_ROWS);
    let text = table.style.text_size.max(0.1) as f32;
    let pad = table.style.cell_padding.max(0.0) as f32;
    let row_h_mm = text + 2.0 * pad;
    let scale = ((H - 2.0 * Y0) / (row_h_mm * (body + 1) as f32)).min(8.0);
    let row_h = row_h_mm * scale;
    let width = W - X0 - 48.0;
    let height = row_h * (body + 1) as f32;
    let mut art = Illustration::new(W, H);
    if shown.is_empty() {
        return art;
    }
    let edge = if matches!(table.style.borders, TableBorders::None) {
        Style::dashed()
    } else {
        Style::edge()
    };
    let rect = |x: f32, y: f32, w: f32, h: f32| Rectangle::new(Point::new(x, y), Size::new(w, h));
    if table.style.header_fill {
        art = art.part(Part::new(
            Primitive::Rect {
                rect: rect(X0, Y0, width, row_h),
            },
            Style::filled(Paint::Ink(Ink::Secondary), Tint::Light),
        ));
    }
    art = art.part(Part::new(
        Primitive::Rect {
            rect: rect(X0, Y0, width, height),
        },
        edge,
    ));
    let rule = Style::outline(Ink::Rule, Weight::Hairline);
    if !matches!(table.style.borders, TableBorders::None) {
        art = art.part(Part::new(
            Primitive::Line {
                from: Point::new(X0, Y0 + row_h),
                to: Point::new(X0 + width, Y0 + row_h),
            },
            rule,
        ));
    }
    let col_w = width / shown.len() as f32;
    let all = matches!(table.style.borders, TableBorders::All);
    for j in 0..shown.len() {
        let x = X0 + col_w * j as f32;
        if all && j > 0 {
            art = art.part(Part::new(
                Primitive::Line {
                    from: Point::new(x, Y0),
                    to: Point::new(x, Y0 + height),
                },
                rule,
            ));
        }
        for r in 0..=body {
            let bar_w = (col_w - 2.0 * pad * scale - 4.0).max(4.0) * if r == 0 { 0.7 } else { 0.5 };
            let y = Y0 + row_h * r as f32 + (row_h - text * scale) / 2.0;
            art = art.part(Part::new(
                Primitive::Rect {
                    rect: rect(x + pad * scale + 2.0, y, bar_w, text * scale),
                },
                Style::filled(Paint::Ink(Ink::Tertiary), Tint::Body),
            ));
        }
    }
    if all {
        for r in 2..=body {
            let y = Y0 + row_h * r as f32;
            art = art.part(Part::new(
                Primitive::Line {
                    from: Point::new(X0, y),
                    to: Point::new(X0 + width, y),
                },
                rule,
            ));
        }
    }
    let ty = Y0 + row_h + (row_h - text * scale) / 2.0;
    let xr = X0 + width + 10.0;
    let text_callout = Callout::new(Point::new(xr, ty), Point::new(xr, ty + text * scale))
        .label(rows.t(ids::PROPERTIES_TABLE_TEXT_SIZE))
        .label_at(Point::new(xr + 4.0, ty + text * scale / 2.0), Anchor::Start);
    let pad_top = Y0 + row_h;
    let xl = X0 - 10.0;
    let pad_callout = Callout::new(
        Point::new(xl, pad_top),
        Point::new(xl, pad_top + pad * scale),
    )
    .label(rows.t(ids::PROPERTIES_TABLE_CELL_PADDING))
    .label_at(
        Point::new(xl - 4.0, pad_top + pad * scale / 2.0),
        Anchor::End,
    );
    art.part(Part::callout(text_callout).linked(TEXT_SIZE_KEY))
        .part(Part::callout(pad_callout).linked(PADDING_KEY))
}
