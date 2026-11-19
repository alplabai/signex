//! The small parts a dialog body is built from beside the kit: callouts and
//! inline errors, field and section labels, captions, the bordered row list,
//! the file-kind icon and the dirty dot. Generic over the message type; sizes
//! come from the type and spacing scales, colours from the theme tokens.

use std::path::Path;

use std::sync::LazyLock;

use iced::alignment::Vertical;
use iced::widget::{Column, Row, Space, container, row, scrollable, svg};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding, Theme, Widget as _};
use signex_i18n::Localized;
use signex_types::theme::ThemeTokens;
use signex_widgets::tree_view::TreeIcon;
use signex_widgets::{Label, text};

use super::tokens::{relative_luminance, resolve};
use super::{Radii, TypeScale, scrollbar_style, thin_vertical, ti};

const TS: TypeScale = TypeScale::DEFAULT;

/// Side of the glyph at the start of a callout.
const CALLOUT_GLYPH: f32 = 14.0;
/// Gap between a callout's glyph and its text.
const CALLOUT_GAP: f32 = 6.0;
/// Padding inside a filled callout: `[vertical, horizontal]`.
const CALLOUT_PADDING: [f32; 2] = [7.0, 9.0];
/// Gap between a field label and its field.
const FIELD_LABEL_GAP: f32 = 4.0;
/// Least height of a list row and of the list's head row.
const ROW_MIN_HEIGHT: f32 = 30.0;
const HEAD_MIN_HEIGHT: f32 = 26.0;
/// Padding of a list row: `[vertical, horizontal]`.
const ROW_PADDING: [f32; 2] = [4.0, 10.0];
/// Gap between the cells of a list row.
const ROW_GAP: f32 = 8.0;
/// Side of a file-kind glyph.
const FILE_KIND_GLYPH: f32 = 16.0;
/// Diameter of the unsaved-changes dot.
const DIRTY_DOT: f32 = 7.0;

/// The tone of a [`dialog_callout`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalloutTone {
    /// A neutral note: the sunken fill, muted ink.
    Info,
    /// Something to look at before going on: the warn tint.
    Warn,
    /// An operation failed: the error tint.
    Error,
}

impl CalloutTone {
    /// `(fill, ink)` of the tone.
    fn colors(self, tokens: &ThemeTokens) -> (Color, Color) {
        let r = resolve(tokens);
        match self {
            Self::Info => (r.sunken, r.ink3),
            Self::Warn => (r.warn_soft, ti(tokens.warning)),
            Self::Error => (r.err_soft, ti(tokens.error)),
        }
    }

    fn glyph(self) -> &'static svg::Handle {
        match self {
            Self::Info => &*GLYPH_INFO,
            Self::Warn | Self::Error => &*GLYPH_WARN,
        }
    }
}

const SVG_INFO: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round"><circle cx="12" cy="12" r="9"/><path d="M12 11v5M12 8h.01"/></svg>"##;
const SVG_WARN: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 3l10 18H2z"/><path d="M12 10v5M12 18h.01"/></svg>"##;

/// The callout glyphs, parsed once.
static GLYPH_INFO: LazyLock<svg::Handle> = LazyLock::new(|| svg::Handle::from_memory(SVG_INFO));
static GLYPH_WARN: LazyLock<svg::Handle> = LazyLock::new(|| svg::Handle::from_memory(SVG_WARN));

/// `handle` drawn `size` square in `ink`.
fn glyph<'a, M: 'a>(handle: &svg::Handle, size: f32, ink: Color) -> Element<'a, M> {
    svg(handle.clone())
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
        .style(move |_: &Theme, _| svg::Style {
            color: Some(ink),
            ..Default::default()
        })
        .boxed()
}

/// The glyph and the body of a callout, the glyph nudged down to the first
/// text line.
fn glyph_line<'a, M: 'a>(
    handle: &svg::Handle,
    ink: Color,
    body: Element<'a, M>,
) -> Row<Element<'a, M>> {
    let glyph = container(glyph(handle, CALLOUT_GLYPH, ink)).padding(Padding {
        top: 1.0,
        ..Padding::ZERO
    });
    row![glyph, container(body).width(Length::Fill)]
        .spacing(CALLOUT_GAP)
        .align_y(Alignment::Start)
}

/// A note line in a dialog body: a 14 px glyph and `body` on the tone's soft
/// fill, control radius. Text in `body` takes the tone's ink unless it sets
/// its own colour.
pub fn dialog_callout<'a, M: 'a>(
    tone: CalloutTone,
    body: Element<'a, M>,
    tokens: &ThemeTokens,
) -> Element<'a, M> {
    let (fill, ink) = tone.colors(tokens);
    container(glyph_line(tone.glyph(), ink, body))
        .width(Length::Fill)
        .padding(CALLOUT_PADDING)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            text_color: Some(ink),
            border: Border {
                radius: Radii::GRAPHITE.control.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// An error under a field: the warn glyph and `body` in the error ink, no
/// fill.
pub fn dialog_inline_error<'a, M: 'a>(
    body: Element<'a, M>,
    tokens: &ThemeTokens,
) -> Element<'a, M> {
    let ink = ti(tokens.error);
    container(glyph_line(&*GLYPH_WARN, ink, body))
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            text_color: Some(ink),
            ..container::Style::default()
        })
        .boxed()
}

/// The label over a field: body size, muted ink, 4 px above the field.
pub fn dialog_field_label<'a, M: 'a>(label: Localized, tokens: &ThemeTokens) -> Element<'a, M> {
    container(text(label).size(TS.body).color(resolve(tokens).ink3))
        .padding(Padding {
            bottom: FIELD_LABEL_GAP,
            ..Padding::ZERO
        })
        .boxed()
}

/// A group heading inside a dialog body: caption size, muted ink, medium
/// weight. The label is drawn as the catalog gives it.
pub fn dialog_section_label<'a, M: 'a>(label: Localized, tokens: &ThemeTokens) -> Element<'a, M> {
    text(label)
        .size(TS.caption)
        .font(medium_font())
        .color(resolve(tokens).ink3)
        .boxed()
}

/// A caption: the smallest text size in the quietest ink.
pub fn dialog_caption<'a, M: 'a>(
    caption: impl Into<Label<'a>>,
    tokens: &ThemeTokens,
) -> Element<'a, M> {
    text(caption)
        .size(TS.caption)
        .color(resolve(tokens).ink4)
        .boxed()
}

/// The medium (500) UI face, for the weight the mockups give labels.
pub(crate) fn medium_font() -> iced::Font {
    iced::Font {
        weight: iced::font::Weight::Medium,
        ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
    }
}

/// A bordered list of rows: the panel fill, a 1 px border at the button
/// radius and a 1 px soft rule between rows. `head` stays above the rows;
/// past `max_height` the rows scroll under the thin scrollbar.
pub fn dialog_rows<'a, M: 'a>(
    head: Option<Element<'a, M>>,
    rows: Vec<Element<'a, M>>,
    max_height: Option<f32>,
    tokens: &ThemeTokens,
) -> Element<'a, M> {
    let rule = resolve(tokens).line_soft;
    let mut list: Column<Element<'a, M>> = Column::new().width(Length::Fill);
    for (i, r) in rows.into_iter().enumerate() {
        if i > 0 {
            list = list.push(hairline(rule));
        }
        list = list.push(r);
    }
    let body: Element<'a, M> = match max_height {
        Some(max) => container(
            scrollable(list)
                .direction(thin_vertical())
                .style(scrollbar_style(tokens))
                .width(Length::Fill)
                .height(Length::Shrink),
        )
        .height(Length::Fit.max(max))
        .boxed(),
        None => list.boxed(),
    };
    let mut outer: Column<Element<'a, M>> = Column::new().width(Length::Fill);
    if let Some(head) = head {
        outer = outer.push(head).push(hairline(rule));
    }
    outer = outer.push(body);
    let (fill, line) = (ti(tokens.panel_bg), ti(tokens.border));
    container(outer)
        .width(Length::Fill)
        .padding(1.0)
        .clip(true)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                width: 1.0,
                color: line,
                radius: Radii::GRAPHITE.button.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// A 1 px horizontal rule in `color`.
fn hairline<'a, M: 'a>(color: Color) -> Element<'a, M> {
    container(Space::new().height(1.0))
        .width(Length::Fill)
        .height(1.0)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(color)),
            ..container::Style::default()
        })
        .boxed()
}

/// One row of a [`dialog_rows`] list: the cells side by side, centred, at
/// least 30 px tall.
pub fn dialog_row<'a, M: 'a>(cells: Vec<Element<'a, M>>) -> Element<'a, M> {
    container(row(cells).spacing(ROW_GAP).align_y(Alignment::Center))
        .width(Length::Fill)
        .height(Length::Fit.min(ROW_MIN_HEIGHT))
        .padding(ROW_PADDING)
        .align_y(Vertical::Center)
        .boxed()
}

/// The head row of a [`dialog_rows`] list: the sunken fill and muted ink,
/// at least 26 px tall. The cells set their own size (section size).
pub fn dialog_head_row<'a, M: 'a>(
    cells: Vec<Element<'a, M>>,
    tokens: &ThemeTokens,
) -> Element<'a, M> {
    let r = resolve(tokens);
    let (fill, ink) = (r.sunken, r.ink3);
    // Inside the list's 1 px border, so its own corners sit 1 px tighter.
    let radius = Radii::GRAPHITE.button - 1.0;
    container(row(cells).spacing(ROW_GAP).align_y(Alignment::Center))
        .width(Length::Fill)
        .height(Length::Fit.min(HEAD_MIN_HEIGHT))
        .padding(ROW_PADDING)
        .align_y(Vertical::Center)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            text_color: Some(ink),
            border: Border {
                radius: iced::border::Radius::default()
                    .top_left(radius)
                    .top_right(radius),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// What a file is, for its icon tint. Matched on the extension: data, not UI
/// text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileKind {
    Project,
    Schematic,
    Pcb,
    Library,
    Bom,
    Output,
    Folder,
    Other,
}

impl FileKind {
    fn of(path: &Path) -> (Self, TreeIcon) {
        let name = path.as_os_str().to_string_lossy();
        let is_folder = name.ends_with(['/', '\\']);
        let name = name.trim_end_matches(['/', '\\']);
        let icon = TreeIcon::for_path(name);
        let ext = Path::new(name)
            .extension()
            .map(|e| e.to_string_lossy().into_owned())
            .unwrap_or_default();
        let is = |want: &str| ext.eq_ignore_ascii_case(want);
        let kind = if is("snxbom") {
            Self::Bom
        } else if is("snxout") {
            Self::Output
        } else {
            match icon {
                TreeIcon::SnxProject => Self::Project,
                TreeIcon::SnxSchematic | TreeIcon::Schematic | TreeIcon::Sheet => Self::Schematic,
                TreeIcon::SnxPcb | TreeIcon::Pcb => Self::Pcb,
                TreeIcon::SnxLibrary | TreeIcon::SnxSymbol | TreeIcon::SnxFootprint => {
                    Self::Library
                }
                _ if is_folder => Self::Folder,
                _ => Self::Other,
            }
        };
        let icon = if kind == Self::Folder {
            TreeIcon::Folder
        } else {
            icon
        };
        (kind, icon)
    }

    /// The ink of the kind: the tones of the Projects tree's kind tags.
    fn ink(self, tokens: &ThemeTokens) -> Color {
        let r = resolve(tokens);
        match self {
            Self::Project => super::tokens::accent(tokens),
            Self::Schematic => r.info,
            Self::Pcb => ti(tokens.success),
            Self::Library => violet_pair(tokens).1,
            Self::Bom => ti(tokens.warning),
            Self::Output | Self::Folder | Self::Other => r.ink3,
        }
    }
}

/// The 16 px icon of the file at `path`, tinted by its kind: schematic info,
/// PCB ok, library violet, BOM warn, project accent, outputs, folders and the
/// rest muted. A `path` ending in a separator is a folder.
pub fn file_kind_icon<'a, M: 'a>(path: &Path, tokens: &ThemeTokens) -> Element<'a, M> {
    let (kind, icon) = FileKind::of(path);
    let ink = kind.ink(tokens);
    svg(icon.svg())
        .width(Length::Fixed(FILE_KIND_GLYPH))
        .height(Length::Fixed(FILE_KIND_GLYPH))
        .style(move |_: &Theme, _| svg::Style {
            color: Some(ink),
            ..Default::default()
        })
        .boxed()
}

/// The dot that marks unsaved changes: a 7 px accent disc.
pub fn dirty_dot<'a, M: 'a>(tokens: &ThemeTokens) -> Element<'a, M> {
    let fill = super::tokens::accent(tokens);
    container(Space::new().width(DIRTY_DOT).height(DIRTY_DOT))
        .width(DIRTY_DOT)
        .height(DIRTY_DOT)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                radius: (DIRTY_DOT / 2.0).into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// `(fill, ink)` of the violet the library kind uses. It has no token: a
/// fixed pair per light / dark panel.
pub fn violet_pair(tokens: &ThemeTokens) -> (Color, Color) {
    if relative_luminance(ti(tokens.panel_bg)) < 0.5 {
        (
            Color::from_rgb8(0x2A, 0x21, 0x38),
            Color::from_rgb8(0xC9, 0xB3, 0xEE),
        )
    } else {
        (
            Color::from_rgb8(0xF1, 0xEA, 0xF8),
            Color::from_rgb8(0x6B, 0x3F, 0xA0),
        )
    }
}

#[cfg(test)]
#[path = "dialog_parts_tests.rs"]
mod tests;
