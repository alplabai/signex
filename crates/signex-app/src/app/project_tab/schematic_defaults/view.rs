//! Schematic defaults section view (mockup row 36): the Lines and Text and
//! markers cards side by side, a caption, then the live preview. Pure:
//! reads the project, emits `ProjectTabMsg`s.

use std::collections::HashMap;

use iced::widget::{Column, Space, canvas, column, container, pick_list, row, stack};
use iced::{Alignment, Background, Border, Element, Font, Length, Widget as _};
use signex_i18n::{I18n, Localized, MessageId, Translatable, ids, t};
use signex_types::net_class::NetLineStyle;
use signex_types::project::{SchematicDefaultField, SchematicDefaultOverrides};
use signex_types::schematic::{FillType, SchematicDefaults};
use signex_types::theme::{CanvasColors, ThemeTokens};
use signex_widgets::dropdown_style::{DropdownLook, GraphiteDropdown as _};
use signex_widgets::text;

use super::super::look::{CardHeader, card, mono_font};
use super::super::table::{SP, TS, section};
use super::field::{WIDE_BOX_EMS, font_field, label_words, numeric_field};
use super::labels::{label_style, preview_labels};
use super::preview::{Preview, PreviewInk, VIEW_HEIGHT, VIEW_WIDTH, value_labels};
use crate::app::Message;
use crate::app::project_tab::ProjectTabMsg;
use crate::app::project_tab::general::tab_message;
use crate::app::project_tab::preview_ui::{
    PreviewPopup, ProjectPreviewMsg, ProjectPreviewUi, expand_layer, preview_message,
};
use crate::app::state::{LoadedProject, ProjectId};
use crate::styles::{HAIRLINE, Radii, ti, tokens as ink};

/// The fields of the Lines card, before its line style picker.
const LINE_FIELDS: [SchematicDefaultField; 4] = [
    SchematicDefaultField::WireWidth,
    SchematicDefaultField::BusWidth,
    SchematicDefaultField::SymbolLineWidth,
    SchematicDefaultField::ShapeStroke,
];

pub(in crate::app::project_tab) fn view<'a>(
    project: &'a LoadedProject,
    drafts: Option<&'a HashMap<SchematicDefaultField, String>>,
    ui: &'a ProjectPreviewUi,
    canvas_colors: &CanvasColors,
    i18n: &'a I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let project_id = project.id;
    let overrides = &project.data.schematic_defaults;
    let numeric = |which: SchematicDefaultField| -> Element<'a, Message> {
        let draft = drafts.and_then(|drafts| drafts.get(&which));
        let (control, note) = numeric_field(project_id, which, overrides, draft, i18n, tokens);
        let line = grid_row(t!(i18n, label_words(which)), control, tokens);
        match note {
            // The refusal note spans the row, under the label and the box.
            Some(note) => column![line, note].spacing(SP.xs).boxed(),
            None => line,
        }
    };

    let mut lines: Vec<Element<'a, Message>> = LINE_FIELDS.into_iter().map(numeric).collect();
    lines.push(grid_row(
        t!(i18n, ids::PROJECT_DEFAULTS_WIRE_LINE_STYLE),
        line_picker(project_id, overrides, i18n, tokens),
        tokens,
    ));
    let markers = vec![
        numeric(SchematicDefaultField::TextSize),
        grid_row(
            t!(i18n, ids::PROJECT_DEFAULTS_FONT),
            font_field(project_id, overrides, i18n, tokens),
            tokens,
        ),
        numeric(SchematicDefaultField::JunctionDiameter),
        grid_row(
            t!(i18n, ids::PROJECT_DEFAULTS_SHAPE_FILL),
            fill_picker(project_id, overrides, i18n, tokens),
            tokens,
        ),
    ];
    let lines_card = form_card(ids::PROJECT_DEFAULTS_LINES, lines, i18n, tokens);
    let markers_card = form_card(ids::PROJECT_DEFAULTS_TEXT_MARKERS, markers, i18n, tokens);
    let cards = row![
        container(lines_card).width(Length::FillPortion(1)),
        container(markers_card).width(Length::FillPortion(1)),
    ]
    .spacing(SP.l)
    .width(Length::Fill);
    let caption = text(t!(i18n, ids::PROJECT_DEFAULTS_CAPTION))
        .size(TS.caption)
        .color(ink::resolve(tokens).ink4);
    let body = vec![
        cards.boxed(),
        caption.boxed(),
        preview(project_id, overrides, ui, canvas_colors, i18n, tokens),
    ];
    section(
        ids::PROJECT_SECTION_SCHEMATIC_DEFAULTS,
        ids::PROJECT_DEFAULTS_HELP,
        body,
        i18n,
        tokens,
    )
}

/// A titled card whose body is the form grid `rows`.
fn form_card<'a>(
    title: MessageId,
    rows: Vec<Element<'a, Message>>,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let mut grid: Column<Element<'a, Message>> = column![].spacing(SP.m).width(Length::Fill);
    for line in rows {
        grid = grid.push(line);
    }
    let header = CardHeader {
        title: t!(i18n, title),
        trailing: Vec::new(),
    };
    card(Some(header), grid.boxed(), tokens)
}

/// One form grid row: the label in ink3, then the control at its own
/// width. The label takes the room the control leaves and wraps in a
/// narrow card, so the control and its reset button always fit (#2996).
fn grid_row<'a>(
    label: Localized,
    control: Element<'a, Message>,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let words = text(label).size(TS.body).color(ink::resolve(tokens).ink3);
    row![container(words).width(Length::Fill), control]
        .spacing(SP.l)
        .align_y(Alignment::Center)
        .boxed()
}

/// A choice of a picker: a value and its words.
#[derive(Debug, Clone, PartialEq)]
struct Choice<T> {
    value: T,
    label: String,
}

impl<T> std::fmt::Display for Choice<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}

fn line_words(line: NetLineStyle) -> MessageId {
    match line {
        NetLineStyle::Solid => ids::PROJECT_DEFAULTS_LINE_SOLID,
        NetLineStyle::Dashed => ids::PROJECT_DEFAULTS_LINE_DASHED,
        NetLineStyle::Dotted => ids::PROJECT_DEFAULTS_LINE_DOTTED,
    }
}

fn fill_words(fill: FillType) -> MessageId {
    match fill {
        FillType::None => ids::PROJECT_DEFAULTS_FILL_NONE,
        FillType::Outline => ids::PROJECT_DEFAULTS_FILL_OUTLINE,
        FillType::Background => ids::PROJECT_DEFAULTS_FILL_BACKGROUND,
    }
}

/// The choices of a default picker: "Built-in (…)" for `None`, then each
/// value.
fn choices<T: Copy>(
    built_in: T,
    values: &[T],
    words: fn(T) -> MessageId,
    i18n: &I18n,
) -> Vec<Choice<Option<T>>> {
    let built_in = Choice {
        value: None,
        label: t!(
            i18n,
            ids::PROJECT_DEFAULTS_BUILT_IN,
            "value" => Translatable::new(words(built_in)),
        )
        .into_string(),
    };
    std::iter::once(built_in)
        .chain(values.iter().map(|value| Choice {
            value: Some(*value),
            label: t!(i18n, words(*value)).into_string(),
        }))
        .collect()
}

/// A wide picker of `choices` at `current`; the built-in choice reads in
/// ink3.
fn picker<'a, T: Copy + PartialEq + 'static>(
    choices: Vec<Choice<Option<T>>>,
    current: Option<T>,
    picked: impl Fn(Option<T>) -> Message + 'a,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let selected = choices.iter().find(|c| c.value == current).cloned();
    let look = DropdownLook::of(tokens);
    let built_in = current.is_none();
    pick_list(selected, choices, Choice::to_string)
        .graphite()
        .style(move |_, status| {
            let mut style = look.field_style(status);
            if built_in {
                style.text_color = look.ink3;
            }
            style
        })
        .on_select(move |choice: Choice<Option<T>>| picked(choice.value))
        .padding([SP.s, SP.m])
        .width(Length::Fixed(TS.body * WIDE_BOX_EMS))
        .boxed()
}

fn line_picker<'a>(
    project_id: ProjectId,
    overrides: &SchematicDefaultOverrides,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let all = [
        NetLineStyle::Solid,
        NetLineStyle::Dashed,
        NetLineStyle::Dotted,
    ];
    let built_in = SchematicDefaults::BUILT_IN.wire_line_style;
    picker(
        choices(built_in, &all, line_words, i18n),
        overrides.wire_line_style,
        move |line| {
            tab_message(ProjectTabMsg::SchematicDefaultLineStylePicked { project_id, line })
        },
        tokens,
    )
}

fn fill_picker<'a>(
    project_id: ProjectId,
    overrides: &SchematicDefaultOverrides,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let all = [FillType::None, FillType::Outline, FillType::Background];
    let built_in = SchematicDefaults::BUILT_IN.shape_fill;
    picker(
        choices(built_in, &all, fill_words, i18n),
        overrides.shape_fill,
        move |fill| {
            tab_message(ProjectTabMsg::SchematicDefaultShapeFillPicked { project_id, fill })
        },
        tokens,
    )
}

/// The live preview: a bold header with its caption, then the drawing on
/// the paper in a hairline frame with the button radius. While the larger
/// window shows the drawing, this one is static: a label has one open field.
fn preview<'a>(
    project_id: ProjectId,
    overrides: &SchematicDefaultOverrides,
    ui: &'a ProjectPreviewUi,
    canvas_colors: &CanvasColors,
    i18n: &'a I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let r = ink::resolve(tokens);
    let bold = Font {
        weight: iced::font::Weight::Semibold,
        ..signex_widgets::ui_font::regular()
    };
    let header = row![
        text(t!(i18n, ids::PROJECT_DEFAULTS_PREVIEW))
            .size(TS.section)
            .font(bold)
            .color(r.ink3),
        Space::new().width(Length::Fill),
        text(t!(i18n, ids::PROJECT_DEFAULTS_PREVIEW_CAPTION))
            .size(TS.caption)
            .color(r.ink4),
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center);
    let is_in_window = ui.popup_of(project_id) == Some(PreviewPopup::SchematicDefaults);
    let art = if is_in_window {
        drawn(overrides, 1.0, canvas_colors, i18n, tokens)
    } else {
        editable(project_id, overrides, ui, 1.0, canvas_colors, i18n, tokens)
    };
    let expand = expand_layer(project_id, PreviewPopup::SchematicDefaults, i18n, tokens);
    let line = ti(tokens.border);
    let framed = container(stack![on_paper(art, canvas_colors), expand])
        .width(Length::Fill)
        .clip(true)
        .style(move |_| container::Style {
            border: Border {
                width: HAIRLINE,
                color: line,
                radius: Radii::GRAPHITE.button.into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
    column![header, framed]
        .spacing(SP.xs)
        .width(Length::Fill)
        .boxed()
}

/// The drawing for the larger preview window.
pub(in crate::app::project_tab) fn popup_drawing<'a>(
    project: &LoadedProject,
    ui: &'a ProjectPreviewUi,
    canvas_colors: &CanvasColors,
    i18n: &'a I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let art = editable(
        project.id,
        &project.data.schematic_defaults,
        ui,
        ui.zoom(),
        canvas_colors,
        i18n,
        tokens,
    );
    on_paper(art, canvas_colors)
}

/// `art` centred across on the canvas paper.
fn on_paper<'a>(art: Element<'a, Message>, canvas_colors: &CanvasColors) -> Element<'a, Message> {
    let paper = ti(canvas_colors.paper);
    container(art)
        .center_x(Length::Fill)
        .style(move |_| container::Style {
            background: Some(Background::Color(paper)),
            ..container::Style::default()
        })
        .boxed()
}

/// The canvas alone, `zoom` pixels per layout unit.
fn drawn<'a>(
    overrides: &SchematicDefaultOverrides,
    zoom: f32,
    canvas_colors: &CanvasColors,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let program = Preview {
        values: overrides.resolve(),
        text_font: crate::fonts::canvas_iced_font(overrides.font_family()),
        label_font: mono_font(),
        ink: PreviewInk::new(canvas_colors, tokens),
        labels: value_labels(overrides, i18n),
    };
    canvas(program)
        .width(Length::Fixed(VIEW_WIDTH * zoom))
        .height(Length::Fixed(VIEW_HEIGHT * zoom))
        .boxed()
}

/// The canvas with its value labels and handles stacked over it.
fn editable<'a>(
    project_id: ProjectId,
    overrides: &SchematicDefaultOverrides,
    ui: &'a ProjectPreviewUi,
    zoom: f32,
    canvas_colors: &CanvasColors,
    i18n: &'a I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let refused = ti(tokens.error);
    let shown: Vec<_> = preview_labels(overrides, zoom, i18n)
        .into_iter()
        .map(|label| label.refused_ink(refused))
        .collect();
    let overlay = ui
        .labels_of(project_id)
        .view(&shown, label_style(canvas_colors, tokens, zoom))
        .map(move |msg| preview_message(project_id, ProjectPreviewMsg::Labels(msg)))
        .boxed();
    container(stack![
        drawn(overrides, zoom, canvas_colors, i18n, tokens),
        overlay
    ])
    .width(Length::Fixed(VIEW_WIDTH * zoom))
    .height(Length::Fixed(VIEW_HEIGHT * zoom))
    .boxed()
}
