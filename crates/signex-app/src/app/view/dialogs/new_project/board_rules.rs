//! The board rules drawing of the New Project and New PCB preview column
//! (mockup rows 21 and 22): two tracks at their width and clearance and a
//! via at its size and drill, on the board ground, with the values as
//! editable labels and handles. Interaction: `app/new_project/preview.rs`.

use std::sync::LazyLock;

use iced::alignment::{Horizontal, Vertical};
use iced::widget::{Space, canvas, column, container, mouse_area, opaque, row, stack, svg};
use iced::{
    Background, Border, Color, Element, Length, Point, Rectangle, Renderer, Size, Theme,
    Widget as _, mouse,
};
use signex_i18n::{I18n, ids, t};
use signex_types::project::PcbDefaults;
use signex_types::theme::{CanvasColors, ThemeId, ThemeTokens, theme_tokens};
use signex_widgets::dimension_label::DimensionLabelStyle;
use signex_widgets::{button, text};

use super::{SP, TS, customize, preview};
use crate::app::new_project::preview::{
    CENTRE_Y, EXPANDED_ZOOM, RuleGeometry, TRACK_X, VIA_X, VIEW_H, VIEW_W, rule_labels,
};
use crate::app::new_project::{DialogPreview, NewProjectMsg, NewProjectState, SetupPreviewMsg};
use crate::styles::board_palette::{BoardPalette, BoardSwatch};
use crate::styles::{HAIRLINE, Radii, ti, tokens};

/// Height of the sheet thumbnail in the column.
const SHEET_H: f32 = TS.body * 15.0;
/// How much taller the sheet is in the larger view.
const SHEET_ZOOM: f32 = 2.5;
/// How strongly the dialog behind the larger view is dimmed.
const BACKDROP_ALPHA: f32 = 0.38;

static EXPAND: LazyLock<svg::Handle> = LazyLock::new(|| {
    svg::Handle::from_memory(include_bytes!("../../../rules_tab/drawing/expand.svg").as_slice())
});

fn msg(preview: SetupPreviewMsg) -> NewProjectMsg {
    NewProjectMsg::Preview(preview)
}

/// The drawn rules, on the board ground in both chrome themes.
#[derive(Debug, Clone, Copy)]
struct BoardRules {
    geometry: RuleGeometry,
    zoom: f32,
    ground: Color,
    copper: Color,
    plating: Color,
    error: Option<Color>,
}

impl BoardRules {
    fn of(pcb: &PcbDefaults, zoom: f32) -> Self {
        let palette = BoardPalette::new();
        let dark = theme_tokens(ThemeId::GraphiteDark);
        let is_bad = pcb.via_drill_mm >= pcb.via_diameter_mm;
        Self {
            geometry: RuleGeometry::of(pcb),
            zoom,
            ground: palette.canvas,
            copper: palette.swatch(BoardSwatch::TopCopper),
            plating: palette.multi_layer,
            error: is_bad.then(|| ti(dark.error)),
        }
    }

    fn draw_tracks(&self, frame: &mut canvas::Frame) {
        let g = self.geometry;
        let z = self.zoom;
        let (x0, x1) = TRACK_X;
        for top in [g.upper_top(), g.lower_top()] {
            let track = canvas::Path::rounded_rectangle(
                Point::new(x0 * z, top * z),
                Size::new((x1 - x0) * z, g.track * z),
                (g.track * z / 2.0).into(),
            );
            frame.fill(&track, self.copper);
        }
    }

    fn draw_via(&self, frame: &mut canvas::Frame) {
        let g = self.geometry;
        let z = self.zoom;
        let centre = Point::new(VIA_X * z, CENTRE_Y * z);
        frame.fill(&canvas::Path::circle(centre, g.via_r * z), self.plating);
        let hole = canvas::Path::circle(centre, g.drill_r * z);
        frame.fill(&hole, self.ground);
        if let Some(error) = self.error {
            let stroke = canvas::Stroke::default()
                .with_color(error)
                .with_width(HAIRLINE * 2.0);
            frame.stroke(&hole, stroke);
        }
    }
}

impl<M> canvas::Program<M> for BoardRules {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        frame.fill_rectangle(Point::ORIGIN, bounds.size(), self.ground);
        self.draw_tracks(&mut frame);
        self.draw_via(&mut frame);
        vec![frame.into_geometry()]
    }
}

/// The value labels' colours, from the Graphite Dark ink, so they read on the
/// board ground under both chrome themes.
fn label_style(zoom: f32) -> DimensionLabelStyle {
    let dark = theme_tokens(ThemeId::GraphiteDark);
    let resolved = tokens::resolve(&dark);
    DimensionLabelStyle {
        ink: ti(dark.warning),
        fill: ti(dark.bg),
        field: resolved.field,
        selection: ti(dark.selection),
        handle: ti(dark.text),
        text_size: TS.caption * zoom.sqrt(),
    }
}

/// A square button with the expand glyph, for the corner of a drawing.
fn expand_button<'a>(
    view: DialogPreview,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, NewProjectMsg> {
    let ink = ti(tokens.text_secondary);
    let side = TS.body * 2.0;
    let glyph = svg(EXPAND.clone())
        .width(TS.title)
        .height(TS.title)
        .style(move |_: &Theme, _| svg::Style {
            color: Some(ink),
            ..Default::default()
        });
    let press = button(container(glyph).center(Length::Fill))
        .width(Length::Fixed(side))
        .height(Length::Fixed(side))
        .padding(0.0)
        .on_press(msg(SetupPreviewMsg::Expanded(Some(view))))
        .style(crate::styles::outline_button(tokens));
    let tip = container(
        text(t!(i18n, ids::DIALOG_NEW_PROJECT_PREVIEW_EXPAND))
            .size(TS.caption)
            .color(ti(tokens.text)),
    )
    .padding([SP.xs, SP.s])
    .style(crate::styles::context_menu(tokens));
    let tipped = signex_widgets::tooltip(press, tip, iced::widget::tooltip::Position::Bottom);
    container(tipped)
        .width(Length::Fill)
        .align_x(Horizontal::Right)
        .padding(SP.xs)
        .boxed()
}

/// A section head of the column: its name, and a quiet note at the right.
fn head<'a>(
    name: signex_i18n::Localized,
    note: signex_i18n::Localized,
    tokens: &ThemeTokens,
) -> Element<'a, NewProjectMsg> {
    let faint = tokens::resolve(tokens).ink4;
    row![
        text(name).size(TS.section).color(faint),
        Space::new().width(Length::Fill),
        text(note).size(TS.caption).color(faint),
    ]
    .align_y(iced::Alignment::Center)
    .boxed()
}

/// The drawing with its value labels at `zoom`; `is_live` leaves out the
/// labels of a drawing that another view is editing.
fn drawing<'a>(
    state: &'a NewProjectState,
    zoom: f32,
    is_live: bool,
    i18n: &'a I18n,
) -> Element<'a, NewProjectMsg> {
    let (width, height) = (VIEW_W * zoom, VIEW_H * zoom);
    let board: Element<'a, NewProjectMsg> = canvas(BoardRules::of(&state.setup.pcb, zoom))
        .width(Length::Fixed(width))
        .height(Length::Fixed(height))
        .boxed();
    let mut layers = vec![board];
    if state.setup.pcb.via_drill_mm >= state.setup.pcb.via_diameter_mm {
        let dark = theme_tokens(ThemeId::GraphiteDark);
        layers.push(
            container(
                text(t!(i18n, ids::DIALOG_NEW_PROJECT_PREVIEW_DRILL_ERROR))
                    .size(TS.caption)
                    .color(ti(dark.error)),
            )
            .width(Length::Fixed(width))
            .align_x(Horizontal::Center)
            .align_y(Vertical::Top)
            .padding(SP.s)
            .boxed(),
        );
    }
    if is_live {
        let shown = rule_labels(&state.setup.pcb, zoom, i18n);
        layers.push(
            state
                .preview
                .labels
                .view(&shown, label_style(zoom))
                .map(|m| msg(SetupPreviewMsg::Labels(m)))
                .boxed(),
        );
    }
    iced::widget::Stack::with_children(layers)
        .width(Length::Fixed(width))
        .height(Length::Fixed(height))
        .boxed()
}

/// The Board rules part of the column: head, drawing and the rules line.
pub(super) fn section<'a>(
    state: &'a NewProjectState,
    i18n: &'a I18n,
    tokens: &ThemeTokens,
) -> Element<'a, NewProjectMsg> {
    let is_live = state.preview.expanded != Some(DialogPreview::BoardRules);
    let framed = container(stack![
        drawing(state, 1.0, is_live, i18n),
        expand_button(DialogPreview::BoardRules, i18n, tokens),
    ])
    .width(Length::Fixed(VIEW_W))
    .clip(true)
    .style(|_: &Theme| container::Style {
        border: Border {
            radius: Radii::GRAPHITE.card.into(),
            ..Border::default()
        },
        ..container::Style::default()
    });
    let pcb = &state.setup.pcb;
    let line = t!(
        i18n,
        ids::DIALOG_NEW_PROJECT_PREVIEW_RULES_LINE,
        "clearance" => format!("{}", pcb.clearance_mm),
        "track" => format!("{}", pcb.track_width_mm)
    );
    column![
        head(
            t!(i18n, ids::DIALOG_NEW_PROJECT_PREVIEW_RULES),
            t!(i18n, ids::DIALOG_NEW_PROJECT_PREVIEW_RULES_HINT),
            tokens
        ),
        framed,
        text(line)
            .size(TS.body)
            .color(tokens::muted_text(tokens)),
    ]
    .spacing(SP.s)
    .boxed()
}

/// The Sheet part of the column: head with size, the thumbnail and expand.
pub(super) fn sheet_section<'a>(
    state: &NewProjectState,
    canvas_colors: &CanvasColors,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, NewProjectMsg> {
    let paper = state.setup.sheet_line();
    let sheet: Element<'a, NewProjectMsg> = crate::app::project_tab::sheet_thumbnail(
        &state.setup.sheet,
        SHEET_H,
        canvas_colors,
        i18n,
        tokens,
    );
    let border = ti(tokens.border);
    let framed = container(stack![
        sheet,
        expand_button(DialogPreview::Sheet, i18n, tokens),
    ])
    .padding(HAIRLINE)
    .clip(true)
    .style(move |_: &Theme| container::Style {
        border: Border {
            width: HAIRLINE,
            color: border,
            radius: Radii::GRAPHITE.button.into(),
            ..Border::default()
        },
        ..container::Style::default()
    });
    column![
        head(
            t!(i18n, ids::DIALOG_NEW_PROJECT_PREVIEW_SHEET_HEADING),
            t!(i18n, ids::DIALOG_NEW_PROJECT_PREVIEW_SHEET, "paper" => paper),
            tokens
        ),
        framed,
    ]
    .spacing(SP.s)
    .boxed()
}

/// What the larger view shows: the rules drawing beside the stack-up with
/// the layer count and thickness, or the sheet.
fn expanded_body<'a>(
    view: DialogPreview,
    state: &'a NewProjectState,
    canvas_colors: &CanvasColors,
    i18n: &'a I18n,
    tokens: &ThemeTokens,
) -> Element<'a, NewProjectMsg> {
    match view {
        DialogPreview::BoardRules => {
            let ground = BoardPalette::new().canvas;
            let board = container(drawing(state, EXPANDED_ZOOM, true, i18n)).style(
                move |_: &Theme| container::Style {
                    background: Some(Background::Color(ground)),
                    ..container::Style::default()
                },
            );
            let side = column![
                preview::stack_card(state, i18n),
                customize::board_size_fields(state, i18n, tokens),
            ]
            .spacing(SP.m)
            .width(Length::Fill);
            row![board, side].spacing(SP.l).boxed()
        }
        DialogPreview::Sheet => crate::app::project_tab::sheet_thumbnail(
            &state.setup.sheet,
            SHEET_H * SHEET_ZOOM,
            canvas_colors,
            i18n,
            tokens,
        ),
    }
}

/// The larger view over the dialog's body, while one is open. A press on
/// the dimmed dialog closes it, as Close and Esc do.
pub(super) fn expanded_layer<'a>(
    state: &'a NewProjectState,
    canvas_colors: &CanvasColors,
    i18n: &'a I18n,
    tokens: &ThemeTokens,
) -> Option<Element<'a, NewProjectMsg>> {
    let view = state.preview.expanded?;
    let close = msg(SetupPreviewMsg::Expanded(None));
    let dim = Color {
        a: BACKDROP_ALPHA,
        ..ti(tokens.text)
    };
    let backdrop = mouse_area(
        container(Space::new())
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(dim)),
                ..container::Style::default()
            }),
    )
    .on_press(close.clone());
    let close_button = button(text(t!(i18n, ids::DIALOG_NEW_PROJECT_PREVIEW_CLOSE)).size(TS.body))
        .padding([SP.xs, SP.m])
        .on_press(close)
        .style(crate::styles::outline_button(tokens));
    let bar = row![Space::new().width(Length::Fill), close_button];
    let (fill, line) = (ti(tokens.panel_bg), ti(tokens.border));
    let card = container(
        column![bar, expanded_body(view, state, canvas_colors, i18n, tokens)].spacing(SP.m),
    )
    .padding(SP.l)
    .clip(true)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(fill)),
        border: Border {
            width: HAIRLINE,
            color: line,
            radius: Radii::GRAPHITE.card.into(),
            ..Border::default()
        },
        ..container::Style::default()
    });
    let centred = container(opaque(card))
        .center(Length::Fill)
        .padding(SP.l)
        .boxed();
    Some(
        stack![opaque(backdrop).capture_scroll(true).boxed(), centred]
            .width(Length::Fill)
            .height(Length::Fill)
            .boxed(),
    )
}
