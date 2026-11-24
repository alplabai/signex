//! The parts of the Library panel that are the same in every kind (mockup
//! row 119): kind chips, breadcrumb, filter field, toolbar, list rows and
//! glyphs.

use iced::widget::text::{Ellipsis, Wrapping};
use iced::widget::tooltip::Position;
use iced::widget::{Row, Space, container, row, svg};
use iced::{Alignment, Background, Border, Color, Element, Font, Length, Padding, Theme, Widget};
use signex_i18n::{I18n, Localized, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::chips::{ChipMetrics, ChipStyle, chip};
use signex_widgets::{button, text, text_input};

pub(super) use super::row_style::{RowDrawing, RowLook, thumb_look};
use crate::dock::DockMessage;
use crate::library::messages::{LibraryMessage, PickerMsg};
use crate::library::state::{LibraryPanelKind, LibraryRowStyle};
use crate::panels::graphite::hint_tooltip;
use crate::styles::tokens::{Overlay, composite_over, overlay, resolve};
use crate::styles::{ChromeMetrics, Radii, Spacing, TypeScale, dimmed, dimmed_button, ti};

pub(super) const TS: TypeScale = TypeScale::DEFAULT;
pub(super) const SP: Spacing = Spacing::DEFAULT;
const CM: ChromeMetrics = ChromeMetrics::DEFAULT;
const RADII: Radii = Radii::GRAPHITE;

/// Height of a list row.
pub(super) const ROW_H: f32 = CM.field + SP.xs;
/// Height of a toolbar button.
const TOOL_H: f32 = CM.row - SP.xs;
/// Side of a toolbar glyph.
const TOOL_GLYPH: f32 = TS.body;
/// Side of a title-bar or icon-button glyph.
pub(super) const ICON_GLYPH: f32 = TS.label;
/// Left inset of a table header under its library.
pub(super) const INDENT_1: f32 = SP.xl + SP.m;
/// Left inset of a part row under its table.
pub(super) const INDENT_2: f32 = INDENT_1 + SP.xl;

const PLACE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16"><path d="M8 2v8M5 7l3 3 3-3M3 13.5h10" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/></svg>"#;
const PLUS: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16"><path d="M8 3v10M3 8h10" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/></svg>"#;
const EDIT: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16"><path d="M10.5 3l2.5 2.5-7 7H3.5V10z" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round"/></svg>"#;
const TRASH: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16"><path d="M3 4.5h10M6.5 4.5V3h3v1.5M4.5 4.5l.6 8.5h5.8l.6-8.5" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/></svg>"#;
const SEARCH: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16"><circle cx="7" cy="7" r="4.3" fill="none" stroke="currentColor" stroke-width="1.4"/><path d="M10.2 10.2l3.3 3.3" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/></svg>"#;
const FOLDER: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16"><path d="M2 4.5V12.5h12V5.5H8L6.5 3.5H2z" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round"/></svg>"#;

/// A monochrome glyph tinted `ink`.
pub(super) fn glyph(bytes: &'static [u8], side: f32, ink: Color) -> iced::widget::Svg<'static> {
    svg(crate::icons::static_glyph(bytes))
        .width(side)
        .height(side)
        .style(move |_: &Theme, _| svg::Style {
            color: Some(ink),
            ..Default::default()
        })
}

/// The folder glyph of the dock title bar's Open Library… button.
pub fn open_library_glyph(tokens: &ThemeTokens) -> iced::widget::Svg<'static> {
    glyph(FOLDER, ICON_GLYPH, ti(tokens.text_secondary))
}

/// The panel message of a Library message.
pub(super) fn lib(message: LibraryMessage) -> DockMessage {
    DockMessage::Library(message)
}

/// The panel message of a dock-panel message.
pub(super) fn panel(message: crate::panels::PanelMsg) -> DockMessage {
    DockMessage::Panel(message)
}

/// The count shown on each kind chip.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct KindCounts {
    pub symbols: usize,
    pub footprints: usize,
    pub parts: usize,
}

fn chip_count(n: usize) -> Option<u32> {
    Some(u32::try_from(n).unwrap_or(u32::MAX))
}

/// The kind chips: Symbols, Footprints and Parts with their counts. With
/// nothing open they are inert and say why.
pub(super) fn kind_chips<'a>(
    kind: LibraryPanelKind,
    counts: KindCounts,
    enabled: bool,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, DockMessage> {
    let r = resolve(tokens);
    let style = ChipStyle {
        fill: r.field,
        edge: ti(tokens.border),
        hover: composite_over(overlay(tokens, Overlay::Hover), r.field),
        ink: r.ink3,
        on_fill: r.accent_soft,
        on_ink: r.accent_soft_ink,
    };
    let metrics = ChipMetrics::from_text_size(TS.body);
    let chips = [
        (
            LibraryPanelKind::Symbols,
            ids::LIBRARY_PANEL_KIND_SYMBOLS,
            ids::LIBRARY_PANEL_SHOW_SYMBOLS,
            counts.symbols,
        ),
        (
            LibraryPanelKind::Footprints,
            ids::LIBRARY_PANEL_KIND_FOOTPRINTS,
            ids::LIBRARY_PANEL_SHOW_FOOTPRINTS,
            counts.footprints,
        ),
        (
            LibraryPanelKind::Parts,
            ids::LIBRARY_PANEL_KIND_PARTS,
            ids::LIBRARY_PANEL_SHOW_PARTS,
            counts.parts,
        ),
    ];
    let mut bar: Row<Element<'a, DockMessage>> = Row::new().spacing(metrics.row_gap);
    for (value, label, hint, count) in chips {
        let on_press =
            enabled.then(|| lib(LibraryMessage::Picker(PickerMsg::PanelKindPicked(value))));
        let shown_count = if enabled { count } else { 0 };
        let pill = chip(
            t!(i18n, label),
            chip_count(shown_count),
            value == kind,
            style,
            metrics,
            i18n,
            on_press,
        );
        let tip = if enabled {
            t!(i18n, hint)
        } else {
            t!(i18n, ids::LIBRARY_PANEL_OPEN_FIRST)
        };
        bar = bar.push(hint_tooltip(pill, tip, tokens, Position::Bottom));
    }
    container(bar.wrap().vertical_spacing(metrics.row_gap))
        .padding(Padding {
            top: SP.m,
            right: SP.m,
            bottom: SP.s,
            left: SP.m,
        })
        .width(Length::Fill)
        .boxed()
}

/// What the breadcrumb under the chips names.
pub(super) enum Crumb {
    /// `library › file · count`; the library is left out when the file
    /// lies in no open library.
    File {
        library: Option<String>,
        file: String,
        count: Localized,
    },
    /// A line saying there is nothing to name.
    Note(Localized),
}

/// One caption line: the breadcrumb.
pub(super) fn crumb<'a>(
    crumb: Crumb,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, DockMessage> {
    let r = resolve(tokens);
    let caption = |label: Localized| -> Element<'a, DockMessage> {
        text(label)
            .size(TS.caption)
            .color(r.ink4)
            .wrapping(Wrapping::None)
            .boxed()
    };
    let line: Element<'a, DockMessage> = match crumb {
        Crumb::Note(note) => caption(note),
        Crumb::File {
            library,
            file,
            count,
        } => {
            let mut parts: Row<Element<'a, DockMessage>> =
                Row::new().spacing(SP.s).align_y(Alignment::Center);
            if let Some(library) = library {
                parts = parts.push(caption(
                    t!(i18n, ids::LIBRARY_PANEL_CRUMB_LIBRARY, "library" => library),
                ));
            }
            parts = parts.push(
                text(UserText::new(file))
                    .size(TS.caption)
                    .font(mono())
                    .color(r.ink3)
                    .wrapping(Wrapping::None)
                    .ellipsis(Ellipsis::End)
                    .boxed(),
            );
            parts.push(caption(count)).boxed()
        }
    };
    container(line)
        .padding(Padding {
            top: SP.xs,
            right: SP.m,
            bottom: SP.s,
            left: SP.m,
        })
        .width(Length::Fill)
        .clip(true)
        .boxed()
}

/// The mono face of part numbers, symbol and footprint names.
pub(super) fn mono() -> Font {
    crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT)
}

/// The filter field: a search glyph and a borderless input in a field
/// frame. Without a library to browse it shows but takes no input.
pub(super) fn filter_field<'a>(
    value: &'a str,
    placeholder: Localized,
    enabled: bool,
    tokens: &ThemeTokens,
) -> Element<'a, DockMessage> {
    let r = resolve(tokens);
    let (ink, ink4, accent) = (ti(tokens.text), r.ink4, ti(tokens.accent));
    let mut input = text_input(placeholder, value)
        .padding(Padding::ZERO)
        .size(TS.body)
        .width(Length::Fill)
        .style(move |_: &Theme, _| text_input::Style {
            background: Background::Color(Color::TRANSPARENT),
            border: Border::default(),
            placeholder: ink4,
            value: ink,
            selection: Color { a: 0.4, ..accent },
        });
    if enabled {
        input = input.on_input(|s| lib(LibraryMessage::Picker(PickerMsg::PanelSearchChanged(s))));
    }
    let (fill, edge) = if enabled {
        (r.field, ti(tokens.border))
    } else {
        (r.sunken, r.line_soft)
    };
    let field = container(
        row![glyph(SEARCH, TOOL_GLYPH, ink4).boxed(), input.boxed()]
            .spacing(SP.s + SP.xs)
            .align_y(Alignment::Center),
    )
    .padding([0.0, SP.m])
    .height(CM.field)
    .center_y(CM.field)
    .width(Length::Fill)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(fill)),
        border: Border {
            width: 1.0,
            color: edge,
            radius: RADII.control.into(),
            ..Border::default()
        },
        ..container::Style::default()
    });
    container(field)
        .padding(Padding {
            top: SP.xs,
            right: SP.m,
            bottom: SP.s + SP.xs,
            left: SP.m,
        })
        .width(Length::Fill)
        .boxed()
}

/// What each toolbar button sends; `None` draws it disabled.
#[derive(Default)]
pub(super) struct Tools {
    pub place: Option<DockMessage>,
    /// Why Place is disabled, shown as its tooltip.
    pub place_blocked: Option<signex_i18n::MessageId>,
    pub add: Option<DockMessage>,
    pub edit: Option<DockMessage>,
    pub delete: Option<DockMessage>,
    /// Why Delete is disabled, when the reason is not plain (its tooltip
    /// then says it instead of "Delete").
    pub delete_blocked: Option<signex_i18n::MessageId>,
}

/// The toolbar: Place, Add and Edit at the left, the row-style switch and
/// Delete as an icon at the right end.
pub(super) fn toolbar<'a>(
    tools: Tools,
    row_style: LibraryRowStyle,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, DockMessage> {
    let Tools {
        place,
        place_blocked,
        add,
        edit,
        delete,
        delete_blocked,
    } = tools;
    let delete_tip = t!(i18n, delete_blocked.unwrap_or(ids::LIBRARY_PANEL_DELETE));
    let place_button = tool_button(PLACE, t!(i18n, ids::LIBRARY_PANEL_PLACE), place, tokens);
    let place_tool = match place_blocked {
        Some(reason) => hint_tooltip(place_button, t!(i18n, reason), tokens, Position::Bottom),
        None => place_button,
    };
    let bar = row![
        place_tool,
        tool_button(PLUS, t!(i18n, ids::LIBRARY_PANEL_ADD), add, tokens),
        tool_button(EDIT, t!(i18n, ids::LIBRARY_PANEL_EDIT), edit, tokens),
        Space::new().width(Length::Fill).boxed(),
        super::row_style::switch(
            row_style,
            |style| lib(LibraryMessage::Picker(PickerMsg::PanelRowStylePicked(
                style
            ))),
            tokens,
            i18n,
        ),
        delete_button(delete, delete_tip, tokens),
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center);
    container(bar)
        .padding(Padding {
            top: 0.0,
            right: SP.m,
            bottom: SP.s + SP.xs,
            left: SP.m,
        })
        .width(Length::Fill)
        .boxed()
}

/// A small outlined toolbar button: glyph and caption-size label.
fn tool_button<'a>(
    glyph_bytes: &'static [u8],
    label: Localized,
    on_press: Option<DockMessage>,
    tokens: &ThemeTokens,
) -> Element<'a, DockMessage> {
    let r = resolve(tokens);
    let enabled = on_press.is_some();
    let ink = ti(tokens.text);
    let shown = if enabled { ink } else { dimmed(ink) };
    let (fill, edge) = (r.field, ti(tokens.border));
    let hover = composite_over(overlay(tokens, Overlay::Hover), fill);
    button(
        row![
            glyph(glyph_bytes, TOOL_GLYPH, shown).boxed(),
            text(label)
                .size(TS.caption)
                .color(shown)
                .wrapping(Wrapping::None)
                .boxed(),
        ]
        .spacing(SP.s)
        .align_y(Alignment::Center),
    )
    .padding([0.0, SP.m])
    .height(TOOL_H)
    .on_press_maybe(on_press)
    .style(move |_: &Theme, status: button::Status| {
        let style = button::Style {
            background: Some(Background::Color(match status {
                button::Status::Hovered | button::Status::Pressed => hover,
                _ => fill,
            })),
            text_color: ink,
            border: Border {
                width: 1.0,
                color: edge,
                radius: RADII.small.into(),
                ..Border::default()
            },
            ..button::Style::default()
        };
        if status == button::Status::Disabled {
            dimmed_button(style)
        } else {
            style
        }
    })
    .boxed()
}

/// The Delete icon button at the right end of the toolbar, `tip` on hover.
fn delete_button<'a>(
    on_press: Option<DockMessage>,
    tip: Localized,
    tokens: &ThemeTokens,
) -> Element<'a, DockMessage> {
    let ink = ti(tokens.text_secondary);
    let shown = if on_press.is_some() { ink } else { dimmed(ink) };
    let hover = overlay(tokens, Overlay::Hover);
    let b = button(container(glyph(TRASH, ICON_GLYPH, shown)).center(Length::Fill))
        .width(CM.row)
        .height(CM.row)
        .padding(0)
        .on_press_maybe(on_press)
        .style(move |_: &Theme, status: button::Status| button::Style {
            background: matches!(status, button::Status::Hovered | button::Status::Pressed)
                .then_some(Background::Color(hover)),
            text_color: shown,
            border: Border {
                radius: RADII.small.into(),
                ..Border::default()
            },
            ..button::Style::default()
        });
    hint_tooltip(b.boxed(), tip, tokens, Position::Bottom)
}

/// Fill of a list row: the neutral selection token when selected (the
/// Library Browser's look), a lighter one when only highlighted.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum RowMark {
    None,
    /// Highlighted but not the active one: a lighter selection fill.
    Highlighted,
    Selected,
}

/// Alpha of the selection fill on a highlighted, not active, row.
const HIGHLIGHT_ALPHA: f32 = 0.4;

/// A list row's text: its mono name and the caption at its right end, and
/// what its thumbnail tile draws.
pub(super) struct RowText<'a> {
    pub name: String,
    pub caption: Option<signex_widgets::Label<'a>>,
    pub drawing: RowDrawing,
}

/// One list row. Compact: a mono name and a caption at the right end. A
/// thumbnail row: the row's own drawing, then the name over the caption.
pub(super) fn list_row<'a>(
    row_text: RowText<'a>,
    look: RowLook,
    mark: RowMark,
    indent: f32,
    on_press: DockMessage,
    tokens: &ThemeTokens,
) -> Element<'a, DockMessage> {
    let RowText {
        name,
        caption,
        drawing,
    } = row_text;
    let r = resolve(tokens);
    let ink = ti(tokens.text);
    let selection = ti(tokens.selection);
    let hover = overlay(tokens, Overlay::Hover);
    let name: Element<'a, DockMessage> = text(UserText::new(name))
        .size(TS.body)
        .font(mono())
        .color(ink)
        .wrapping(Wrapping::None)
        .ellipsis(Ellipsis::End)
        .width(Length::Fill)
        .boxed();
    let caption: Option<Element<'a, DockMessage>> = caption
        .filter(|c| !c.as_str().trim().is_empty())
        .map(|label| {
            text(label)
                .size(TS.caption)
                .color(r.ink4)
                .wrapping(Wrapping::None)
                .ellipsis(Ellipsis::End)
                .boxed()
        });
    let (line, height): (Row<Element<'a, DockMessage>>, f32) = match look {
        RowLook::Compact => {
            let mut line = Row::new()
                .spacing(SP.m)
                .align_y(Alignment::Center)
                .push(name);
            if let Some(caption) = caption {
                line = line.push(caption);
            }
            (line, ROW_H)
        }
        RowLook::Thumbnail(tile_look) => {
            let mut lines: iced::widget::Column<Element<'a, DockMessage>> =
                iced::widget::Column::new().width(Length::Fill).push(name);
            if let Some(caption) = caption {
                lines = lines.push(caption);
            }
            let line = Row::new()
                .spacing(SP.m)
                .align_y(Alignment::Center)
                .push(super::row_style::tile(drawing, tile_look, tokens))
                .push(lines.boxed());
            (line, super::row_style::THUMB_ROW_H)
        }
    };
    button(line)
        .padding(Padding {
            top: 0.0,
            right: SP.m,
            bottom: 0.0,
            left: SP.m + indent,
        })
        .height(height)
        .width(Length::Fill)
        .on_press(on_press)
        .style(move |_: &Theme, status: button::Status| button::Style {
            background: match mark {
                RowMark::Selected => Some(Background::Color(selection)),
                RowMark::Highlighted => Some(Background::Color(Color {
                    a: selection.a * HIGHLIGHT_ALPHA,
                    ..selection
                })),
                RowMark::None => {
                    matches!(status, button::Status::Hovered | button::Status::Pressed)
                        .then_some(Background::Color(hover))
                }
            },
            text_color: ink,
            border: Border {
                radius: RADII.small.into(),
                ..Border::default()
            },
            ..button::Style::default()
        })
        .boxed()
}

/// A muted line inside the list: a hint or "nothing matches".
pub(super) fn list_note<'a>(
    note: Localized,
    indent: f32,
    tokens: &ThemeTokens,
) -> Element<'a, DockMessage> {
    container(text(note).size(TS.caption).color(resolve(tokens).ink4))
        .padding(Padding {
            top: SP.s,
            right: SP.m,
            bottom: SP.s,
            left: SP.m + indent,
        })
        .width(Length::Fill)
        .boxed()
}

/// The library holding `path`: the open library whose folder contains it,
/// else the `.snxlib` folder among its ancestors. Reads no disk.
pub(super) fn library_name_of(
    path: &std::path::Path,
    library: &crate::library::state::LibraryState,
) -> Option<String> {
    if let Some(open) = library
        .open_libraries
        .iter()
        .find(|lib| lib.root.parent().is_some_and(|dir| path.starts_with(dir)))
    {
        return Some(open.display_name.clone());
    }
    path.ancestors()
        .find(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("snxlib"))
        })
        .and_then(|p| p.file_stem())
        .and_then(|s| s.to_str())
        .map(str::to_owned)
}

/// The file name of `path`, or `fallback`.
pub(super) fn file_name_or(path: &std::path::Path, fallback: Localized) -> String {
    path.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| fallback.into_string())
}
