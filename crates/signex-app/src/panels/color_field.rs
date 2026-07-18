//! Reusable colour-selection field — one swatch button that expands
//! into an inline preset palette (6×2 grid) plus a `Custom…` button
//! that opens the `iced_aw` HSV / RGB `ColorPicker` overlay.
//!
//! Ported verbatim from the former inline `child_sheet_color_row` so
//! every call site (child-sheet border/fill, symbol graphic fill,
//! symbol local colours) renders an identical control. The widget is
//! message-generic: callers hand in the five messages the control can
//! emit (`on_toggle` / `on_advanced` / `on_cancel` / `on_pick` /
//! `on_clear`) and read the open-state back out of their own context.
//!
//! Colours cross the boundary as `[u8; 4]` RGBA; the widget converts
//! to / from `iced::Color` internally (preset cells are opaque, the
//! HSV submit is quantised to 8-bit).

use std::rc::Rc;

use iced::widget::{Column, Row, Space, button, column, container, row, text};
use iced::{Alignment, Background, Border, Color, Element, Length, Theme};

/// 12-colour preset palette shared by every `color_field`. Rendered as
/// a 6×2 grid; ported verbatim from the former child-sheet colour row.
const PRESETS: [(&str, [u8; 3]); 12] = [
    ("Black", [0x00, 0x00, 0x00]),
    ("Dark Gray", [0x40, 0x40, 0x40]),
    ("Gray", [0x80, 0x80, 0x80]),
    ("White", [0xFF, 0xFF, 0xFF]),
    ("Red", [0xC0, 0x39, 0x2B]),
    ("Orange", [0xE6, 0x7E, 0x22]),
    ("Yellow", [0xF1, 0xC4, 0x0F]),
    ("Olive", [0xB4, 0xA5, 0x58]),
    ("Green", [0x27, 0xAE, 0x60]),
    ("Teal", [0x16, 0xA0, 0x85]),
    ("Blue", [0x29, 0x80, 0xB9]),
    ("Purple", [0x8E, 0x44, 0xAD]),
];

/// Configuration for one [`color_field`]. `M` is the caller's message
/// type; the widget is otherwise state-free — the caller owns the
/// open / advanced flags and feeds them back in each render.
pub struct ColorFieldProps<'a, M> {
    /// Left-hand row label (e.g. "Fill", "Fills", "Border Colour").
    pub label: &'a str,
    /// Current colour as RGBA, or `None` for the default/inherit/no-fill
    /// state (drawn as a muted translucent swatch + `none_label` caption).
    pub current: Option<[u8; 4]>,
    /// Caption shown when `current` is `None` (e.g. "Default", "None",
    /// "Inherit").
    pub none_label: &'a str,
    /// Whether the inline preset grid is expanded below the row.
    pub show_palette: bool,
    /// Whether the HSV / RGB overlay is open (takes precedence over the
    /// preset grid).
    pub show_advanced: bool,
    /// Muted text colour for the row label.
    pub muted: Color,
    /// Border colour for swatches and the palette panel.
    pub border_c: Color,
    /// Toggle the preset palette open / closed (swatch click).
    pub on_toggle: M,
    /// Open the HSV / RGB overlay (`Custom…` click).
    pub on_advanced: M,
    /// Cancel / close the HSV / RGB overlay without committing.
    pub on_cancel: M,
    /// User picked a colour — from a preset cell or the HSV submit.
    /// `'static` (not `'a`) because `iced_aw::ColorPicker::new` requires
    /// its submit closure to be `'static`; every call site captures only
    /// `Copy` / `'static` data so this is not a real restriction.
    pub on_pick: Rc<dyn Fn([u8; 4]) -> M + 'static>,
    /// Reset to `None`; the button is hidden when this is `None`.
    pub on_clear: Option<M>,
}

/// Render a colour-selection field. See [`ColorFieldProps`].
///
/// `M: 'static` because the `on_pick` callback (and the `iced_aw`
/// submit closure it feeds) must outlive the widget; panel messages are
/// all `'static`, so this holds everywhere.
pub fn color_field<'a, M: Clone + 'static>(props: ColorFieldProps<'a, M>) -> Element<'a, M> {
    let ColorFieldProps {
        label,
        current,
        none_label,
        show_palette,
        show_advanced,
        muted,
        border_c,
        on_toggle,
        on_advanced,
        on_cancel,
        on_pick,
        on_clear,
    } = props;

    let preview_color = current
        .map(|c| {
            Color::from_rgba(
                c[0] as f32 / 255.0,
                c[1] as f32 / 255.0,
                c[2] as f32 / 255.0,
                c[3] as f32 / 255.0,
            )
        })
        .unwrap_or(Color::from_rgba(0.5, 0.5, 0.5, 0.4));
    let label_text = if let Some(c) = current {
        format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2])
    } else {
        none_label.to_string()
    };

    // Swatch button: 18x18 colour fill + small hex / none caption.
    let swatch_color = preview_color;
    let swatch: Element<'a, M> = container(Space::new())
        .width(18)
        .height(18)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(swatch_color)),
            border: Border {
                width: 1.0,
                color: border_c,
                radius: 2.0.into(),
            },
            ..container::Style::default()
        })
        .into();

    let swatch_button = button(
        row![
            swatch,
            text(label_text)
                .size(10)
                .color(Color::from_rgb(0.90, 0.90, 0.92)),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .padding([2, 6])
    .on_press(on_toggle)
    .style(button::secondary);

    // ── Advanced (HSV / RGB) overlay ──
    if show_advanced {
        let on_pick_hsv = Rc::clone(&on_pick);
        let picker = iced_aw::ColorPicker::new(
            true,
            preview_color,
            swatch_button,
            on_cancel,
            move |c: Color| (on_pick_hsv)(color_to_rgba(c)),
        );
        return container(
            row![
                text(label.to_string()).size(10).color(muted).width(96),
                picker,
            ]
            .spacing(4)
            .align_y(Alignment::Center),
        )
        .padding([4, 8])
        .width(Length::Fill)
        .into();
    }

    // The header row (label + swatch button) is always shown.
    let header = container(
        row![
            text(label.to_string()).size(10).color(muted).width(96),
            swatch_button,
        ]
        .spacing(4)
        .align_y(Alignment::Center),
    )
    .padding([4, 8])
    .width(Length::Fill);

    if !show_palette {
        return header.into();
    }

    // ── Inline preset palette (rendered below the row, full width) ──
    // 6 columns × 2 rows of preset swatches; each cell stretches
    // proportionally so the grid always fills the available panel
    // width (no clipping in narrow docks).
    let mut palette_grid: Column<'a, M> = Column::new().spacing(4);
    for chunk in PRESETS.chunks(6) {
        let mut r: Row<'a, M> = Row::new().spacing(4);
        for (_name, rgb) in chunk {
            let c = Color::from_rgb(
                rgb[0] as f32 / 255.0,
                rgb[1] as f32 / 255.0,
                rgb[2] as f32 / 255.0,
            );
            let rgba = [rgb[0], rgb[1], rgb[2], 255];
            let on_pick_cell = Rc::clone(&on_pick);
            let swatch_btn = button(Space::new())
                .width(Length::Fill)
                .height(22)
                .padding(0)
                .on_press((on_pick_cell)(rgba))
                .style(move |_t: &Theme, _s| button::Style {
                    background: Some(Background::Color(c)),
                    border: Border {
                        width: 1.0,
                        color: border_c,
                        radius: 2.0.into(),
                    },
                    ..button::Style::default()
                });
            r = r.push(swatch_btn);
        }
        palette_grid = palette_grid.push(r);
    }

    let mut palette_col: Column<'a, M> =
        Column::new().spacing(6).padding([6, 8]).width(Length::Fill);
    palette_col = palette_col.push(text("Preset Colours").size(10).color(muted));
    palette_col = palette_col.push(palette_grid);

    let mut action_row: Row<'a, M> = Row::new().spacing(4).width(Length::Fill);
    action_row = action_row.push(
        button(
            text("Custom…")
                .size(10)
                .color(Color::from_rgb(0.92, 0.92, 0.94)),
        )
        .padding([4, 10])
        .width(Length::Fill)
        .on_press(on_advanced)
        .style(button::secondary),
    );
    if let Some(on_clear) = on_clear {
        action_row = action_row.push(
            button(
                text("Reset to Default")
                    .size(10)
                    .color(Color::from_rgb(0.92, 0.92, 0.94)),
            )
            .padding([4, 10])
            .width(Length::Fill)
            .on_press(on_clear)
            .style(button::secondary),
        );
    }
    palette_col = palette_col.push(action_row);

    let palette_panel = container(palette_col)
        .width(Length::Fill)
        .style(move |_t: &Theme| container::Style {
            background: Some(Background::Color(Color::from_rgb(0.16, 0.16, 0.18))),
            border: Border {
                width: 1.0,
                color: border_c,
                radius: 4.0.into(),
            },
            ..container::Style::default()
        });

    column![header, container(palette_panel).padding([0, 8])]
        .spacing(4)
        .width(Length::Fill)
        .into()
}

/// Quantise an `iced::Color` to 8-bit RGBA. Clamps each channel into
/// `[0, 1]` before scaling so an out-of-gamut picker value can't wrap.
fn color_to_rgba(c: Color) -> [u8; 4] {
    [
        (c.r.clamp(0.0, 1.0) * 255.0).round() as u8,
        (c.g.clamp(0.0, 1.0) * 255.0).round() as u8,
        (c.b.clamp(0.0, 1.0) * 255.0).round() as u8,
        (c.a.clamp(0.0, 1.0) * 255.0).round() as u8,
    ]
}
