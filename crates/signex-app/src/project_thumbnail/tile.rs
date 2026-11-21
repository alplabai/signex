//! The 64 x 46 project tile of the Start screen rows: the cached render on
//! the board canvas or the schematic paper with a PCB / SCH badge, a sunken
//! "No preview yet" tile, or a dashed tile for a project missing on disk.
//! Pure: it draws what the slot holds; the only state is the canvas cache
//! iced keeps.

use std::sync::Arc;

use iced::widget::text::LineHeight;
use iced::widget::{canvas, container, stack, tooltip};
use iced::{
    Alignment, Background, Border, Color, Element, Length, Point, Size, Theme, Widget as _,
};
use signex_i18n::{I18n, MessageId, ids, t};
use signex_renderer::pcb::BOARD_OUTLINE;
use signex_types::pcb_layer::{CopperLayer, PcbLayer, TechnicalLayer};
use signex_types::theme::{CanvasColors, ThemeId, ThemeTokens, canvas_colors, theme_tokens};
use signex_widgets::text;
use signex_widgets::theme_ext::{DEFAULT_BASE_SIZE, power_port};

use super::{Geometry, Ink, Thumbnail, ThumbnailKind, ThumbnailSlot};
use crate::styles::board_palette::BoardPalette;
use crate::styles::tokens::resolve;
use crate::styles::{RADIUS, Radii, TypeScale, ti};

const TS: TypeScale = TypeScale::DEFAULT;

/// `px` at the default base size, following the type scale: the mockup's
/// lengths are drawn at that base.
const fn at_base(px: f32) -> f32 {
    TS.body * px / DEFAULT_BASE_SIZE
}

/// Tile size (the mockup's 64 x 46).
pub const TILE_W: f32 = at_base(64.0);
pub const TILE_H: f32 = at_base(46.0);
/// Gap between the tile edge and the line art.
const ART_INSET: f32 = at_base(5.0);
/// Gap between the tile edge and the sheet frame.
const SHEET_FRAME_INSET: f32 = at_base(2.5);
/// The PCB / SCH badge: text size, side padding and inset from the corner.
const BADGE_TEXT: f32 = at_base(8.0);
const BADGE_PAD_X: f32 = at_base(3.0);
const BADGE_INSET: f32 = at_base(2.0);
/// Badge fill: the board canvas ground, at this opacity.
const BADGE_ALPHA: f32 = 0.72;
/// "No preview yet": text size, line height and padding.
const NONE_TEXT: f32 = at_base(9.0);
const NONE_LINE_HEIGHT: f32 = 1.15;
const NONE_PAD: f32 = at_base(2.0);
/// The missing tile's dash pattern and its file glyph size.
const MISSING_DASH: &[f32] = &[3.0, 2.0];
const FILE_GLYPH: f32 = at_base(16.0);
/// Smallest radius a circle is drawn at, px, so a via stays visible.
const MIN_RADIUS: f32 = 0.6;
/// The palette lists one silkscreen colour; the bottom face draws it at
/// this brightness, the same hue darker, as the other sided pairs.
const BOTTOM_SILK_SHADE: f32 = 0.6;

/// The tile of a project row.
pub fn tile<'a, M: 'a>(tokens: &ThemeTokens, i18n: &I18n, slot: ThumbnailSlot) -> Element<'a, M> {
    let (body, hint) = match slot {
        ThumbnailSlot::Ready(thumbnail) => {
            let hint = match thumbnail.kind {
                ThumbnailKind::Board => ids::START_THUMBNAIL_BOARD_HINT,
                ThumbnailKind::Sheet => ids::START_THUMBNAIL_SHEET_HINT,
            };
            (ready(tokens, i18n, thumbnail), hint)
        }
        ThumbnailSlot::NoPreview => (no_preview(tokens, i18n), ids::START_THUMBNAIL_NONE_HINT),
        ThumbnailSlot::MissingOnDisk => (missing(tokens), ids::START_THUMBNAIL_MISSING_HINT),
    };
    crate::panels::graphite::hint_tooltip(body, t!(i18n, hint), tokens, tooltip::Position::Bottom)
}

/// The colour of each ink: the board palette for board inks, the
/// schematic canvas of the theme for sheet inks.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Inks {
    board: BoardPalette,
    sheet: CanvasColors,
}

impl Inks {
    pub fn new(tokens: &ThemeTokens) -> Self {
        Self {
            board: BoardPalette::new(),
            sheet: sheet_canvas(tokens),
        }
    }

    pub fn colour(&self, ink: Ink) -> Color {
        ink_colour(ink, &self.board, &self.sheet)
    }
}

/// The colour `ink` draws in.
pub fn ink_colour(ink: Ink, board: &BoardPalette, sheet: &CanvasColors) -> Color {
    let silk = board.layer(PcbLayer::Technical(TechnicalLayer::SilkscreenBottom));
    match ink {
        Ink::BoardOutline => board.layer(BOARD_OUTLINE),
        Ink::TopCopper => board.layer(PcbLayer::Copper(CopperLayer::Top)),
        Ink::BottomCopper => board.layer(PcbLayer::Copper(CopperLayer::Bottom)),
        Ink::MultiLayer => board.multi_layer,
        Ink::TopSilkscreen => board.layer(PcbLayer::Technical(TechnicalLayer::SilkscreenTop)),
        Ink::BottomSilkscreen => Color {
            r: silk.r * BOTTOM_SILK_SHADE,
            g: silk.g * BOTTOM_SILK_SHADE,
            b: silk.b * BOTTOM_SILK_SHADE,
            a: silk.a,
        },
        Ink::Wire => ti(sheet.wire),
        Ink::SymbolBody => ti(sheet.body),
        Ink::PowerPort => power_port(sheet),
    }
}

/// The schematic canvas colours of the Graphite theme `tokens` belong to.
pub fn sheet_canvas(tokens: &ThemeTokens) -> CanvasColors {
    // By background: the selected-option preference changes other tokens.
    let id = [ThemeId::GraphiteLight, ThemeId::GraphiteDark]
        .into_iter()
        .find(|id| theme_tokens(*id).bg == tokens.bg)
        .unwrap_or(ThemeId::GraphiteLight);
    canvas_colors(id)
}

/// Stroke width, px, of each ink at tile size.
fn stroke_width(ink: Ink) -> f32 {
    match ink {
        Ink::BoardOutline => 1.0,
        Ink::TopCopper | Ink::BottomCopper | Ink::MultiLayer => 1.1,
        Ink::TopSilkscreen | Ink::BottomSilkscreen => 0.6,
        Ink::Wire => 0.9,
        Ink::SymbolBody => 0.8,
        Ink::PowerPort => 1.0,
    }
}

/// The scale and top-left offset that fit `view_box` centred in `area`
/// (`x, y, width, height`), aspect kept.
pub(super) fn fit(view_box: [f32; 2], area: [f32; 4]) -> (f32, [f32; 2]) {
    let [x, y, width, height] = area;
    let scale = (width / view_box[0].max(1.0)).min(height / view_box[1].max(1.0));
    let offset = [
        x + (width - view_box[0] * scale) / 2.0,
        y + (height - view_box[1] * scale) / 2.0,
    ];
    (scale, offset)
}

fn tile_frame<'a, M: 'a>(
    content: Element<'a, M>,
    ground: Option<Color>,
    edge: Option<Color>,
) -> Element<'a, M> {
    container(content)
        .width(Length::Fixed(TILE_W))
        .height(Length::Fixed(TILE_H))
        .clip(true)
        .style(move |_: &Theme| container::Style {
            background: ground.map(Background::Color),
            border: Border {
                width: if edge.is_some() { 1.0 } else { 0.0 },
                color: edge.unwrap_or(Color::TRANSPARENT),
                radius: Radii::GRAPHITE.small.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

fn ready<'a, M: 'a>(
    tokens: &ThemeTokens,
    i18n: &I18n,
    thumbnail: Arc<Thumbnail>,
) -> Element<'a, M> {
    let inks = Inks::new(tokens);
    let r = resolve(tokens);
    let board_chrome = theme_tokens(ThemeId::GraphiteDark);
    let (ground, edge, frame, badge): (Color, Color, Option<Color>, MessageId) =
        match thumbnail.kind {
            ThumbnailKind::Board => (
                inks.board.canvas,
                ti(board_chrome.border),
                None,
                ids::PANELS_TAB_KIND_PCB,
            ),
            ThumbnailKind::Sheet => (
                ti(inks.sheet.paper),
                r.line_soft,
                Some(r.line_strong),
                ids::PANELS_TAB_KIND_SCHEMATIC,
            ),
        };
    let art: canvas::Canvas<ThumbnailArt, M> = canvas::Canvas::new(ThumbnailArt {
        thumbnail,
        inks,
        frame,
    });
    let art: Element<'a, M> = art.width(Length::Fill).height(Length::Fill).boxed();
    let badge_ground = Color {
        a: BADGE_ALPHA,
        ..ti(board_chrome.bg)
    };
    let badge: Element<'a, M> = container(
        text(t!(i18n, badge))
            .size(BADGE_TEXT)
            .font(mono())
            .color(ti(board_chrome.text))
            .boxed(),
    )
    .padding([0.0, BADGE_PAD_X])
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(badge_ground)),
        border: Border {
            radius: RADIUS.into(),
            ..Border::default()
        },
        ..container::Style::default()
    })
    .boxed();
    let corner: Element<'a, M> = container(badge)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::End)
        .align_y(Alignment::End)
        .padding(BADGE_INSET)
        .boxed();
    tile_frame(stack![art, corner].boxed(), Some(ground), Some(edge))
}

fn no_preview<'a, M: 'a>(tokens: &ThemeTokens, i18n: &I18n) -> Element<'a, M> {
    let r = resolve(tokens);
    let caption: Element<'a, M> = container(
        text(t!(i18n, ids::START_THUMBNAIL_NONE))
            .size(NONE_TEXT)
            .line_height(LineHeight::Relative(NONE_LINE_HEIGHT))
            .center()
            .color(r.ink4)
            .boxed(),
    )
    .center(Length::Fill)
    .padding(NONE_PAD)
    .boxed();
    tile_frame(caption, Some(r.sunken), Some(r.line_soft))
}

fn missing<'a, M: 'a>(tokens: &ThemeTokens) -> Element<'a, M> {
    let r = resolve(tokens);
    let art: canvas::Canvas<MissingArt, M> = canvas::Canvas::new(MissingArt {
        ground: r.sunken,
        edge: r.line_strong,
        glyph: r.ink4,
    });
    tile_frame(
        art.width(Length::Fill).height(Length::Fill).boxed(),
        None,
        None,
    )
}

fn mono() -> iced::Font {
    crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT)
}

/// Canvas program of a cached render.
struct ThumbnailArt {
    thumbnail: Arc<Thumbnail>,
    inks: Inks,
    /// The sheet frame, for a root sheet render.
    frame: Option<Color>,
}

impl<M> canvas::Program<M> for ThumbnailArt {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: iced::Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        if let Some(colour) = self.frame {
            let inset = SHEET_FRAME_INSET;
            let sheet = canvas::Path::rectangle(
                Point::new(inset, inset),
                Size::new(bounds.width - 2.0 * inset, bounds.height - 2.0 * inset),
            );
            frame.stroke(
                &sheet,
                canvas::Stroke::default().with_color(colour).with_width(0.8),
            );
        }
        let area = [
            ART_INSET,
            ART_INSET,
            (bounds.width - 2.0 * ART_INSET).max(1.0),
            (bounds.height - 2.0 * ART_INSET).max(1.0),
        ];
        let (scale, [ox, oy]) = fit(self.thumbnail.view_box, area);
        let place = |[x, y]: [f32; 2]| Point::new(ox + x * scale, oy + y * scale);
        for primitive in &self.thumbnail.primitives {
            let colour = self.inks.colour(primitive.ink);
            let path = match &primitive.geometry {
                Geometry::Polyline { points, closed } => canvas::Path::new(|b| {
                    let mut points = points.iter().copied().map(place);
                    if let Some(first) = points.next() {
                        b.move_to(first);
                        points.for_each(|p| b.line_to(p));
                        if *closed {
                            b.close();
                        }
                    }
                }),
                Geometry::Circle { center, radius } => {
                    canvas::Path::circle(place(*center), (radius * scale).max(MIN_RADIUS))
                }
            };
            if primitive.filled {
                frame.fill(&path, colour);
            } else {
                frame.stroke(
                    &path,
                    canvas::Stroke::default()
                        .with_color(colour)
                        .with_width(stroke_width(primitive.ink))
                        .with_line_join(canvas::LineJoin::Round),
                );
            }
        }
        vec![frame.into_geometry()]
    }
}

/// Canvas program of the missing tile: sunken ground, dashed edge and a
/// file glyph.
struct MissingArt {
    ground: Color,
    edge: Color,
    glyph: Color,
}

impl<M> canvas::Program<M> for MissingArt {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: iced::Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let tile = canvas::Path::rounded_rectangle(
            Point::new(0.5, 0.5),
            Size::new(bounds.width - 1.0, bounds.height - 1.0),
            Radii::GRAPHITE.small.into(),
        );
        frame.fill(&tile, self.ground);
        let mut dashed = canvas::Stroke::default()
            .with_color(self.edge)
            .with_width(1.0);
        dashed.line_dash = canvas::LineDash {
            segments: MISSING_DASH,
            offset: 0,
        };
        frame.stroke(&tile, dashed);
        // The file glyph, a 16-unit page with a folded corner, centred.
        let unit = FILE_GLYPH / 16.0;
        let x0 = (bounds.width - FILE_GLYPH) / 2.0;
        let y0 = (bounds.height - FILE_GLYPH) / 2.0;
        let at = |x: f32, y: f32| Point::new(x0 + x * unit, y0 + y * unit);
        let page = canvas::Path::new(|b| {
            b.move_to(at(4.0, 2.0));
            b.line_to(at(9.0, 2.0));
            b.line_to(at(12.0, 5.0));
            b.line_to(at(12.0, 14.0));
            b.line_to(at(4.0, 14.0));
            b.close();
            b.move_to(at(9.0, 2.0));
            b.line_to(at(9.0, 5.0));
            b.line_to(at(12.0, 5.0));
        });
        frame.stroke(
            &page,
            canvas::Stroke::default()
                .with_color(self.glyph)
                .with_width(1.3 * unit)
                .with_line_join(canvas::LineJoin::Round),
        );
        vec![frame.into_geometry()]
    }
}
