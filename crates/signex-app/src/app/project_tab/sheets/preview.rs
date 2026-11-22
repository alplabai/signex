//! The New sheets preview (mockup row 31, refined by Caner 2026-11-18): the
//! sheet a new page starts as, at the size and orientation the defaults
//! column shows, in the schematic canvas colours. The outline, zone border
//! and title block are the canvas's own lowering
//! (`signex_output::template::lower_sheet_frame`), scaled to fit.

use iced::alignment::{Horizontal, Vertical};
use iced::widget::{canvas, column, container, row, stack};
use iced::{
    Alignment, Border, Color, Element, Length, Point, Rectangle, Size, Theme, Widget as _, mouse,
};
use signex_i18n::{I18n, UserText, ids, t};
use signex_output::template::{Frame, FramePrimitive, lower_sheet_frame, standard_title_block};
use signex_types::project::SheetDefaults;
use signex_types::schematic::{SheetOrientation, SheetSize};
use signex_types::theme::{CanvasColors, ThemeTokens};
use signex_widgets::text;

use super::super::look::{mono_font, ui_font};
use super::super::preview_ui::{PreviewPopup, expand_layer};
use super::super::table::{SP, TS};
use crate::app::Message;
use crate::app::state::ProjectId;
use crate::styles::{HAIRLINE, Radii, ti, tokens::resolve};

/// Height of the drawing (200 px); it is as wide as its column, up to
/// [`PREVIEW_MAX_W`].
pub(super) const PREVIEW_H: f32 = TS.body * 50.0 / 3.0;
/// Widest the drawing grows (360 px).
const PREVIEW_MAX_W: f32 = TS.body * 30.0;
/// Clear space left and right of the sheet (about 20 px).
const SIDE: f32 = SP.xl + SP.xs;
/// Clear space above the sheet (about 12 px).
const TOP: f32 = SP.l;
/// Space under the sheet, which holds the size label (32 px).
const BOTTOM: f32 = TS.body * 8.0 / 3.0;
/// Offset of the drop shadow under the sheet.
const SHADOW: f32 = SP.xs;
/// Frame text smaller than this on screen is left out, as on the canvas.
const MIN_TEXT_PX: f32 = 3.0;

/// Where the sheet sits in the drawing: px per mm and its top-left corner.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct SheetFit {
    pub scale: f32,
    pub origin: Point,
}

/// Fit a `width_mm` x `height_mm` sheet into `area`, centred across, under
/// the top margin, leaving the bottom band for the size label.
pub(super) fn fit_sheet(width_mm: f64, height_mm: f64, area: Size) -> SheetFit {
    let room_w = (area.width - 2.0 * SIDE).max(1.0);
    let room_h = (area.height - TOP - BOTTOM).max(1.0);
    let (w, h) = (width_mm.max(1.0) as f32, height_mm.max(1.0) as f32);
    let scale = (room_w / w).min(room_h / h);
    SheetFit {
        scale,
        origin: Point::new((area.width - w * scale) / 2.0, TOP),
    }
}

/// The page size and orientation a sheet defaults row stores.
pub(super) fn sheet_of(defaults: &SheetDefaults) -> (SheetSize, SheetOrientation) {
    SheetSize::parse(&defaults.paper_size)
}

/// A length in mm as the preview labels it: whole millimetres bare, else
/// one decimal (`215.9`).
pub(super) fn mm_text(mm: f64) -> String {
    let tenths = (mm * 10.0).round() / 10.0;
    if tenths.fract() == 0.0 {
        format!("{tenths:.0}")
    } else {
        format!("{tenths:.1}")
    }
}

/// The colours of the drawing.
#[derive(Debug, Clone, Copy)]
struct Ink {
    ground: Color,
    shadow: Color,
    paper: Color,
    outline: Color,
    /// The frame's lines and its text, in the colours the schematic canvas
    /// draws its sheet frame with.
    frame_line: Color,
    frame_text: Color,
    label: Color,
}

/// The drawing: the sheet on the sunken ground with its frame and the size
/// label. Stateless; rebuilt by every view.
#[derive(Debug, Clone)]
struct SheetPreview {
    width_mm: f64,
    height_mm: f64,
    frame: Vec<FramePrimitive>,
    label: String,
    ink: Ink,
}

impl<M> canvas::Program<M> for SheetPreview {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        // Rounded inside the frame's border, so no corner shows past it.
        let ground = canvas::Path::rounded_rectangle(
            Point::ORIGIN,
            bounds.size(),
            (Radii::GRAPHITE.button - HAIRLINE).into(),
        );
        frame.fill(&ground, self.ink.ground);
        let fit = fit_sheet(self.width_mm, self.height_mm, bounds.size());
        let sheet = Size::new(
            self.width_mm as f32 * fit.scale,
            self.height_mm as f32 * fit.scale,
        );
        let shadow = Point::new(fit.origin.x + SHADOW, fit.origin.y + SHADOW);
        frame.fill_rectangle(shadow, sheet, self.ink.shadow);
        let paper = canvas::Path::rectangle(fit.origin, sheet);
        frame.fill(&paper, self.ink.paper);
        frame.stroke(
            &paper,
            canvas::Stroke::default()
                .with_color(self.ink.outline)
                .with_width(HAIRLINE),
        );
        self.draw_frame(&mut frame, fit);
        frame.fill_text(canvas::Text {
            content: self.label.clone(),
            position: Point::new(bounds.width / 2.0, bounds.height - BOTTOM / 2.0),
            color: self.ink.label,
            size: iced::Pixels(TS.caption),
            font: iced::Font {
                weight: iced::font::Weight::Medium,
                ..mono_font()
            },
            align_x: Horizontal::Center.into(),
            align_y: Vertical::Center,
            ..canvas::Text::default()
        });
        vec![frame.into_geometry()]
    }
}

impl SheetPreview {
    /// The lowered frame, mapped from sheet mm to the drawing; lines first,
    /// then text, as the canvas draws it.
    fn draw_frame(&self, frame: &mut canvas::Frame, fit: SheetFit) {
        let at = |x: f64, y: f64| {
            Point::new(
                fit.origin.x + x as f32 * fit.scale,
                fit.origin.y + y as f32 * fit.scale,
            )
        };
        let stroke = canvas::Stroke::default()
            .with_color(self.ink.frame_line)
            .with_width(HAIRLINE * 0.8);
        for primitive in &self.frame {
            match primitive {
                FramePrimitive::Line { from, to, .. } => {
                    frame.stroke(
                        &canvas::Path::line(at(from.0, from.1), at(to.0, to.1)),
                        stroke,
                    );
                }
                FramePrimitive::Rect { x, y, w, h, .. } => {
                    let size = Size::new(*w as f32 * fit.scale, *h as f32 * fit.scale);
                    frame.stroke(&canvas::Path::rectangle(at(*x, *y), size), stroke);
                }
                FramePrimitive::Text { .. } => {}
            }
        }
        for primitive in &self.frame {
            let FramePrimitive::Text {
                x,
                y,
                size_mm,
                anchor,
                text,
                ..
            } = primitive
            else {
                continue;
            };
            let size_px = *size_mm as f32 * fit.scale;
            if size_px < MIN_TEXT_PX {
                continue;
            }
            let (align_x, align_y) = match anchor {
                signex_output::template::FrameTextAnchor::TopLeft => {
                    (Horizontal::Left, Vertical::Top)
                }
                signex_output::template::FrameTextAnchor::Center => {
                    (Horizontal::Center, Vertical::Center)
                }
            };
            frame.fill_text(canvas::Text {
                content: text.clone(),
                position: at(*x, *y),
                color: self.ink.frame_text,
                size: iced::Pixels(size_px),
                font: mono_font(),
                align_x: align_x.into(),
                align_y,
                ..canvas::Text::default()
            });
        }
    }
}

impl SheetPreview {
    /// The drawing of a `width_mm` × `height_mm` sheet with `defaults`'
    /// title block, labelled with its size in mm.
    fn of(
        defaults: &SheetDefaults,
        (width_mm, height_mm): (f64, f64),
        canvas_colors: &CanvasColors,
        i18n: &I18n,
        tokens: &ThemeTokens,
    ) -> Self {
        let r = resolve(tokens);
        let label = t!(
            i18n,
            ids::PROJECT_SHEETS_PREVIEW_MM,
            "width" => mm_text(width_mm),
            "height" => mm_text(height_mm),
        )
        .into_string();
        // A field the title block fills from the sheet (`${TITLE}`) has no
        // value yet: it is left out.
        let unresolved = |field: &str| {
            if field.contains("${") {
                String::new()
            } else {
                field.to_string()
            }
        };
        let block = defaults.show_title_block.then(standard_title_block);
        Self {
            width_mm,
            height_mm,
            frame: lower_sheet_frame(
                width_mm,
                height_mm,
                &Frame::default(),
                block.as_ref(),
                unresolved,
            ),
            label,
            ink: Ink {
                ground: r.sunken,
                shadow: r.shadow,
                paper: ti(canvas_colors.paper),
                outline: r.line_strong,
                frame_line: crate::canvas::GraphiteCanvasInk::from_tokens(tokens).line_strong,
                frame_text: ti(canvas_colors.body),
                label: ti(tokens.text_secondary),
            },
        }
    }
}

/// The sheet alone at `height`, for the larger preview window.
pub(super) fn large<'a>(
    defaults: &SheetDefaults,
    height: f32,
    canvas_colors: &CanvasColors,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let (size, orientation) = sheet_of(defaults);
    let program = SheetPreview::of(
        defaults,
        size.dimensions_mm(orientation),
        canvas_colors,
        i18n,
        tokens,
    );
    canvas(program)
        .width(Length::Fill)
        .height(Length::Fixed(height))
        .boxed()
}

/// The preview column (`.lp`): the uppercase "Preview" and the size caption
/// over the drawing, with the expand button on it.
pub(super) fn view<'a>(
    project_id: ProjectId,
    defaults: &SheetDefaults,
    canvas_colors: &CanvasColors,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let r = resolve(tokens);
    let (size, orientation) = sheet_of(defaults);
    let (width_mm, height_mm) = size.dimensions_mm(orientation);
    let name = size.token(SheetOrientation::Landscape);
    let caption = match orientation {
        SheetOrientation::Landscape => {
            t!(i18n, ids::PROJECT_SHEETS_PREVIEW_LANDSCAPE, "size" => name)
        }
        SheetOrientation::Portrait => {
            t!(i18n, ids::PROJECT_SHEETS_PREVIEW_PORTRAIT, "size" => name)
        }
    };
    let program = SheetPreview::of(defaults, (width_mm, height_mm), canvas_colors, i18n, tokens);
    let title = text(UserText::new(
        i18n.upper_for_display(&t!(i18n, ids::PROJECT_SHEETS_PREVIEW).into_string()),
    ))
    .size(TS.section)
    .font(ui_font(iced::font::Weight::Medium))
    .color(r.ink3);
    let head = row![
        title,
        text(caption)
            .size(TS.caption)
            .color(r.ink4)
            .wrapping(iced::widget::text::Wrapping::None)
            .ellipsis(iced::widget::text::Ellipsis::End)
            .align_x(Horizontal::Right)
            .width(Length::Fill),
    ]
    .spacing(SP.s + SP.xs * 0.5)
    .align_y(Alignment::Center);
    let border = ti(tokens.border);
    let sheet: Element<'a, Message> = canvas(program)
        .width(Length::Fill)
        .height(Length::Fixed(PREVIEW_H))
        .boxed();
    let expand = expand_layer(project_id, PreviewPopup::Sheet, i18n, tokens);
    let drawing = container(stack![sheet, expand])
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
    column![head, drawing]
        .spacing(SP.s)
        .width(Length::Fill.max(PREVIEW_MAX_W))
        .boxed()
}
