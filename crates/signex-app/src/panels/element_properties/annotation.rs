//! The Properties panels of the Bezier, dimension and table drawings
//! (#1267, #1300): their messages, the half-typed text of their boxes, and
//! the Graphite rows they share (label | control, X/Y pair, segmented
//! choice, dropdown, check, collapsible section).

use iced::widget::text::Wrapping;
use iced::widget::{Column, container, pick_list, row};
use iced::{Alignment, Element, Length, Theme, Widget as _};
use signex_i18n::{Localized, MessageId, t};
use signex_types::schematic::SchDrawing;
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::{Label, button, checkbox, text, text_input};

use super::super::{PROPERTY_LABEL_COLUMN, PanelContext, PanelMsg, form_look};
use crate::app::sch_tables::{AnnotationEdit, Axis, table_padding_holds, table_text_size_holds};
use crate::choice::Choice;
use crate::styles::tokens::resolve;
use crate::styles::{Spacing, TypeScale, ti};

pub(super) const TS: TypeScale = TypeScale::DEFAULT;
pub(super) const SP: Spacing = Spacing::DEFAULT;

/// A typed box of an annotation drawing's Properties.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnnotationField {
    /// A Bezier point (0 start, 1 C1, 2 C2, 3 end).
    BezierPoint(u8, Axis),
    /// A dimension's first (`false`) or second point.
    DimensionPoint(bool, Axis),
    OverrideText,
    TableTextSize,
    TableCellPadding,
}

impl AnnotationField {
    /// The edit typed `text` gives; `None` while it does not parse. A Y is
    /// typed from the page origin and `page_y` turns it into a sheet Y.
    pub fn edit(self, text: &str, page_y: impl Fn(f64) -> f64) -> Option<AnnotationEdit> {
        let number = || crate::numeric_field::parse_field_f64(text);
        let coordinate = |axis: Axis| {
            number().map(|v| match axis {
                Axis::X => v,
                Axis::Y => page_y(v),
            })
        };
        Some(match self {
            Self::BezierPoint(i, axis) => {
                AnnotationEdit::BezierPoint(usize::from(i), axis, coordinate(axis)?)
            }
            Self::DimensionPoint(second, axis) => {
                AnnotationEdit::DimensionPoint(second, axis, coordinate(axis)?)
            }
            Self::OverrideText => AnnotationEdit::OverrideText(text.to_string()),
            Self::TableTextSize => AnnotationEdit::TableTextSize(number()?),
            Self::TableCellPadding => AnnotationEdit::TableCellPadding(number()?),
        })
    }

    /// Whether a table style box holds text the table refuses (#2792).
    fn refuses(self, text: &str) -> bool {
        let number = signex_types::numeric::parse_finite_f64(text).ok();
        match self {
            Self::TableTextSize => !number.is_some_and(table_text_size_holds),
            Self::TableCellPadding => !number.is_some_and(table_padding_holds),
            Self::BezierPoint(..) | Self::DimensionPoint(..) | Self::OverrideText => false,
        }
    }
}

/// What happened in the Properties of a Bezier, dimension or table.
#[derive(Debug, Clone)]
pub enum AnnotationPanelMsg {
    /// A keystroke in a box: kept as a draft, applied once it parses.
    FieldTyped(AnnotationField, String),
    /// Enter in a box: the draft is dropped, the box shows the stored value.
    FieldSubmitted(AnnotationField),
    /// A choice was picked (segment, dropdown, checkbox).
    Picked(AnnotationEdit),
    CsvImportRequested,
    CsvExportRequested,
}

/// The Properties state of the selected annotation drawing.
#[derive(Debug, Clone, Default)]
pub struct AnnotationPanelState {
    /// The box being typed in: the drawing, the field and its raw text.
    pub draft: Option<(uuid::Uuid, AnnotationField, String)>,
    /// The parts on the active sheet, for a pin table's Component list.
    pub designators: Vec<String>,
    /// The selected table's part is gone from the sheet.
    pub source_missing: bool,
}

/// The Properties of the selected Bezier, dimension or table; `None` for
/// any other selection.
pub(super) fn view_annotation_properties<'a>(
    ctx: &'a PanelContext,
) -> Option<Element<'a, PanelMsg>> {
    let drawing = ctx.selected_drawing.as_ref()?;
    match drawing {
        SchDrawing::Bezier(b) => Some(super::bezier::view_bezier_properties(
            ctx,
            drawing,
            b,
            |y| ctx.shown_y(y),
        )),
        SchDrawing::Dimension(d) => {
            Some(super::dimension::view_dimension_properties(ctx, drawing, d))
        }
        SchDrawing::Table(t) => Some(super::table::view_table_properties(ctx, drawing, t)),
        SchDrawing::Line { .. }
        | SchDrawing::Rect { .. }
        | SchDrawing::Circle { .. }
        | SchDrawing::Arc { .. }
        | SchDrawing::Polyline { .. } => None,
    }
}

pub(super) fn msg(m: AnnotationPanelMsg) -> PanelMsg {
    PanelMsg::Annotation(m)
}

pub(super) fn picked(edit: AnnotationEdit) -> PanelMsg {
    msg(AnnotationPanelMsg::Picked(edit))
}

/// The panel context with the shared row builders.
pub(super) struct Rows<'a> {
    pub ctx: &'a PanelContext,
    pub id: uuid::Uuid,
}

impl<'a> Rows<'a> {
    pub fn new(ctx: &'a PanelContext, drawing: &SchDrawing) -> Self {
        Self {
            ctx,
            id: drawing.uuid(),
        }
    }

    pub fn t(&self, id: MessageId) -> Localized {
        t!(self.ctx.i18n, id)
    }

    fn muted(&self) -> iced::Color {
        resolve(&self.ctx.tokens).ink3
    }

    pub fn faint(&self) -> iced::Color {
        resolve(&self.ctx.tokens).ink4
    }

    pub fn ink(&self) -> iced::Color {
        ti(self.ctx.tokens.text)
    }

    /// The header: the drawing's icon, its kind in bold and a caption.
    pub fn header(
        &self,
        icon: iced::widget::svg::Handle,
        title: Localized,
        caption: impl Into<Label<'a>>,
    ) -> Element<'a, PanelMsg> {
        let title = text(title)
            .size(TS.body)
            .color(self.ink())
            .font(iced::Font {
                weight: iced::font::Weight::Semibold,
                ..signex_widgets::ui_font::regular()
            });
        let caption = text(caption).size(TS.caption).color(self.faint());
        container(
            row![
                iced::widget::svg(icon).width(TS.heading).height(TS.heading),
                iced::widget::column![title, caption].spacing(SP.xs / 2.0),
            ]
            .spacing(SP.m)
            .align_y(Alignment::Center),
        )
        .padding([SP.s, SP.l])
        .width(Length::Fill)
        .boxed()
    }

    /// A collapsible section of `rows`.
    pub fn section(
        &self,
        key: &'static str,
        title: MessageId,
        rows: Vec<Element<'a, PanelMsg>>,
    ) -> Element<'a, PanelMsg> {
        let ctx = self.ctx;
        ctx.collapsible(key, title, &ctx.collapsed_sections, move || {
            Column::with_children(rows)
                .spacing(SP.xs)
                .width(Length::Fill)
        })
        .boxed()
    }

    /// Label | content: the Graphite property row.
    pub fn field(&self, label: Localized, content: Element<'a, PanelMsg>) -> Element<'a, PanelMsg> {
        let label = text(label)
            .size(TS.body)
            .color(self.muted())
            .wrapping(Wrapping::None);
        container(
            row![
                container(label).width(PROPERTY_LABEL_COLUMN).clip(true),
                container(content).width(Length::Fill),
            ]
            .spacing(SP.m)
            .align_y(Alignment::Center),
        )
        .padding([SP.xs, SP.l])
        .width(Length::Fill)
        .boxed()
    }

    /// A muted note line.
    pub fn note(&self, line: impl Into<Label<'a>>) -> Element<'a, PanelMsg> {
        container(text(line).size(TS.caption).color(self.faint()))
            .padding([SP.s, SP.l])
            .width(Length::Fill)
            .boxed()
    }

    /// A box showing `value` or its draft; typing applies once it parses.
    pub fn input(&self, field: AnnotationField, value: String) -> Element<'a, PanelMsg> {
        let draft = match &self.ctx.annotation.draft {
            Some((id, f, draft)) if *id == self.id && *f == field => Some(draft.clone()),
            _ => None,
        };
        let error = draft
            .as_deref()
            .filter(|typed| field.refuses(typed))
            .map(|_| ti(self.ctx.tokens.error));
        let shown = draft.unwrap_or(value);
        text_input(signex_i18n::UserText::new(String::new()), shown)
            .size(TS.body)
            .padding([SP.xs, SP.s])
            .on_input(move |s| msg(AnnotationPanelMsg::FieldTyped(field, s)))
            .on_submit(msg(AnnotationPanelMsg::FieldSubmitted(field)))
            .style(form_look::refusable_input_style(error))
            .width(Length::Fill)
            .boxed()
    }

    /// `prefix` (X, Y) beside a box.
    fn prefixed(
        &self,
        prefix: MessageId,
        field: AnnotationField,
        value: String,
    ) -> Element<'a, PanelMsg> {
        row![
            text(self.t(prefix)).size(TS.caption).color(self.faint()),
            self.input(field, value),
        ]
        .spacing(SP.xs)
        .align_y(Alignment::Center)
        .width(Length::Fill)
        .boxed()
    }

    /// A point as an X and a Y box (mm, Y from the page origin).
    pub fn point(
        &self,
        label: Localized,
        fields: (AnnotationField, AnnotationField),
        (x, y): (f64, f64),
    ) -> Element<'a, PanelMsg> {
        let pair = row![
            self.prefixed(
                signex_i18n::ids::PROPERTIES_ANNOTATION_X,
                fields.0,
                format!("{x:.2}")
            ),
            self.prefixed(
                signex_i18n::ids::PROPERTIES_ANNOTATION_Y,
                fields.1,
                format!("{y:.2}")
            ),
        ]
        .spacing(SP.s)
        .width(Length::Fill);
        self.field(label, pair.boxed())
    }

    /// A segmented control: one cell per `(label, is chosen, edit)`.
    pub fn segmented(
        &self,
        options: Vec<(MessageId, bool, AnnotationEdit)>,
    ) -> Element<'a, PanelMsg> {
        let mut cells: Vec<Element<'a, PanelMsg>> = Vec::with_capacity(options.len());
        for (label, active, edit) in options {
            let cell = button(
                text(self.t(label))
                    .size(TS.body)
                    .align_x(iced::alignment::Horizontal::Center)
                    .wrapping(Wrapping::None)
                    .ellipsis(iced::widget::text::Ellipsis::End),
            )
            .padding([SP.s, SP.xs])
            .on_press(picked(edit))
            .style(move |theme: &Theme, status: button::Status| {
                let hovered = matches!(status, button::Status::Hovered);
                form_look::segment_cell(theme, active, hovered && !active)
            });
            cells.push(cell.boxed());
        }
        container(signex_widgets::FitRow::new(cells).fill(true))
            .padding(SP.xs)
            .width(Length::Fill)
            .style(form_look::segmented_frame)
            .boxed()
    }

    /// A dropdown of `choices`; picking one sends its edit.
    pub fn dropdown<T: Clone + PartialEq + 'a>(
        &self,
        choices: Vec<Choice<T>>,
        current: Option<T>,
        edit: impl Fn(T) -> AnnotationEdit + 'a,
    ) -> Element<'a, PanelMsg> {
        let selected = current.and_then(|v| crate::choice::chosen(&choices, &v));
        pick_list(selected, choices, |c: &Choice<T>| c.label.clone())
            .graphite()
            .on_select(move |c: Choice<T>| picked(edit(c.value)))
            .padding([SP.xs, SP.s])
            .width(Length::Fill)
            .boxed()
    }

    /// Checkbox then label; ticking sends `edit(on)`.
    pub fn check(
        &self,
        label: impl Into<Label<'a>>,
        checked: bool,
        edit: impl Fn(bool) -> AnnotationEdit + 'a,
    ) -> Element<'a, PanelMsg> {
        let tick = checkbox(checked)
            .size(TS.heading)
            .style(form_look::checkbox_style)
            .on_toggle(move |on| picked(edit(on)));
        row![
            tick,
            text(label)
                .size(TS.body)
                .color(self.muted())
                .wrapping(Wrapping::None)
        ]
        .spacing(SP.m)
        .align_y(Alignment::Center)
        .boxed()
    }

    /// A small ghost button.
    pub fn action(&self, label: MessageId, press: PanelMsg) -> Element<'a, PanelMsg> {
        button(text(self.t(label)).size(TS.body))
            .padding([SP.xs, SP.m])
            .on_press(press)
            .style(form_look::ghost_button)
            .boxed()
    }

    /// A read-only value.
    pub fn value(&self, value: impl Into<Label<'a>>) -> Element<'a, PanelMsg> {
        text(value)
            .size(TS.body)
            .color(self.faint())
            .wrapping(Wrapping::None)
            .boxed()
    }

    /// A property illustration: hover is not tracked yet, so its
    /// messages are dropped.
    pub fn illustration(
        &self,
        drawing: signex_widgets::property_illustration::Illustration,
    ) -> Element<'a, PanelMsg> {
        let hover = signex_widgets::property_illustration::IllustrationHover::default();
        let art = signex_widgets::property_illustration::property_illustration(
            drawing,
            &hover,
            &self.ctx.tokens,
            TS.caption,
        )
        .map(|_| PanelMsg::Noop);
        container(art)
            .padding([SP.s, SP.l])
            .width(Length::Fill)
            .boxed()
    }
}
