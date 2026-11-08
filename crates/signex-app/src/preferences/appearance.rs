//! Appearance section — theme picker cards, custom-theme import/export,
//! UI font, and the schematic / symbol-editor render-style pick-lists.
//!
//! Moved verbatim from the former single-file `preferences` module —
//! pure view code, zero behaviour change.

use super::*;
use crate::fonts;
use crate::render_config::{GridStyle, PinSelectionMode};
use iced::widget::{Space, column, container, row};
use iced::{Background, Border, Element, Length, Theme, Widget};
use signex_types::theme::ThemeId;
use signex_widgets::{button, raw, text};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

const TRACKPAD_PAN_HELP: &str = "When on, a two-finger trackpad scroll pans the schematic, PCB, symbol and footprint canvases instead of zooming them. Pinching and Ctrl/Cmd + scroll still zoom, and a mouse wheel notch still zooms. Off by default.";

const REMEMBER_GEOMETRY_HELP: &str = "When on, Signex reopens the main window at the size and position it had when you last closed it. Off by default.";

pub(super) fn content_appearance<'a>(v: PrefsView<'a>) -> Element<'a, PrefMsg> {
    let PrefsView {
        draft_theme,
        draft_font,
        draft_grid_style,
        draft_pcb_gpu_render,
        draft_antialiasing,
        draft_symbol_grid_size_mm,
        draft_symbol_grid_style,
        draft_symbol_pin_selection,
        draft_zoom_step_percent,
        draft_trackpad_pan,
        draft_remember_window_geometry,
        draft_ui_scale_percent,
        draft_window_controls_placement,
        window_controls_restart_note,
        custom_name,
        theme_status,
        ..
    } = v;
    let mut col: iced::widget::Column<Element<'a, PrefMsg>> =
        column![].spacing(0).padding([SP.xl, 20.0]);

    // ── Section: Theme ──
    col = col.push(section_title("Theme"));
    col = col.push(Space::new().height(10).boxed());

    // Built-in theme data: (id, display name, description)
    let builtins: &[(ThemeId, &str, &str)] = &[
        (
            ThemeId::Signex,
            "Signex",
            "Default Signex schematic palette",
        ),
        (
            ThemeId::Alplab,
            "Alp Lab",
            "Alp Lab brand cyan accent on the Signex chrome",
        ),
        (
            ThemeId::VsCodeDark,
            "VS Code Dark",
            "VS Code inspired dark theme",
        ),
        (
            ThemeId::CatppuccinMocha,
            "Catppuccin Mocha",
            "Warm soft dark with pastels",
        ),
        (
            ThemeId::GitHubDark,
            "GitHub Dark",
            "GitHub's dark mode colors",
        ),
        (
            ThemeId::SolarizedLight,
            "Solarized Light",
            "Warm light tone-on-tone",
        ),
        (ThemeId::Nord, "Nord", "Arctic blue cool dark theme"),
    ];

    // Build list of all theme entries including optional custom
    let mut entries: Vec<(ThemeId, String, &'static str)> = builtins
        .iter()
        .map(|&(id, name, desc)| (id, name.to_string(), desc))
        .collect();
    if let Some(name) = custom_name {
        entries.push((
            ThemeId::Custom,
            format!("\u{2728} {name}"),
            "Custom imported theme",
        ));
    }

    // Rows of 2
    let mut i = 0;
    while i < entries.len() {
        let (id_a, ref name_a, desc_a) = entries[i];
        let card_a = theme_card(id_a, name_a, desc_a, draft_theme);
        let row_elem: Element<'_, PrefMsg> = if i + 1 < entries.len() {
            let (id_b, ref name_b, desc_b) = entries[i + 1];
            let card_b = theme_card(id_b, name_b, desc_b, draft_theme);
            row![card_a, Space::new().width(12), card_b]
                .width(Length::Fill)
                .boxed()
        } else {
            row![card_a, Space::new().width(Length::Fill)]
                .width(Length::Fill)
                .boxed()
        };
        col = col.push(row_elem);
        col = col.push(Space::new().height(10).boxed());
        i += 2;
    }

    // ── Custom theme import/export ──
    col = col.push(Space::new().height(4).boxed());
    col = col.push(
        row![
            import_btn(),
            Space::new().width(8),
            export_btn(),
            Space::new().width(Length::Fill),
        ]
        .align_y(iced::Alignment::Center)
        .boxed(),
    );

    // Export writes a file the user picked in a save dialog; the write
    // can fail (read-only volume, full disk, a path that vanished
    // between pick and write). Report it here rather than letting the
    // user assume the theme was saved (#533). Empty = nothing painted,
    // so the row keeps its old height until there is something to say.
    if !theme_status.is_empty() {
        col = col.push(Space::new().height(6).boxed());
        col = col.push(
            text(raw(theme_status))
                .size(TS.caption)
                .style(text_muted)
                .boxed(),
        );
    }

    // ── Divider ──
    col = col.push(Space::new().height(16).boxed());
    col = col.push(h_sep());
    col = col.push(Space::new().height(16).boxed());

    // ── Section: UI Font ──
    col = col.push(section_title("Font"));
    col = col.push(Space::new().height(10).boxed());
    col = col.push(
        row![
            column![
                text(raw("UI Font")).size(TS.label).style(text_primary),
                text(raw("Applies to all panels and menus. Requires restart."))
                    .size(TS.caption)
                    .style(text_muted),
            ]
            .spacing(3)
            .width(200),
            Space::new().width(Length::Fill),
            {
                let families = fonts::system_font_families();
                let current_owned = draft_font.to_string();
                iced::widget::pick_list(Some(current_owned), families.as_slice(), |s: &String| {
                    s.clone()
                })
                .on_select(PrefMsg::DraftFont)
                .text_size(12)
                .width(200)
            },
        ]
        .align_y(iced::Alignment::Center)
        .boxed(),
    );
    col = col.push(Space::new().height(20).boxed());

    // ── Section: Window controls ──
    col = col.push(h_sep());
    col = col.push(Space::new().height(16).boxed());
    col = col.push(section_title("Window controls"));
    col = col.push(Space::new().height(10).boxed());
    col = col.push(window_controls_row(
        draft_window_controls_placement,
        window_controls_restart_note,
    ));
    col = col.push(Space::new().height(20).boxed());

    // ── Section: Schematic Editor ──
    col = col.push(h_sep());
    col = col.push(Space::new().height(16).boxed());
    col = col.push(section_title("Schematic Editor"));
    col = col.push(Space::new().height(10).boxed());
    col = col.push(
        row![
            column![
                text(raw("Grid Style")).size(TS.label).style(text_primary),
                text(raw("Appearance of snap points on the schematic canvas."))
                    .size(TS.caption)
                    .style(text_muted),
            ]
            .spacing(3)
            .width(200),
            Space::new().width(Length::Fill),
            iced::widget::pick_list(Some(draft_grid_style), GridStyle::ALL, |v: &GridStyle| {
                v.to_string()
            })
            .on_select(PrefMsg::DraftGridStyle)
            .text_size(12)
            .width(200),
        ]
        .align_y(iced::Alignment::Center)
        .boxed(),
    );
    col = col.push(Space::new().height(20).boxed());

    // ── Section: PCB Editor ──
    col = col.push(h_sep());
    col = col.push(Space::new().height(16).boxed());
    col = col.push(section_title("PCB Editor"));
    col = col.push(Space::new().height(10).boxed());
    col = col.push(
        row![
            column![
                text(raw("GPU Render (experimental)"))
                    .size(TS.label)
                    .style(text_primary),
                text(raw("Draw the PCB canvas on the GPU (shader) instead of the CPU. Applies immediately."))
                    .size(TS.caption)
                    .style(text_muted),
                // This used to carry a second line naming the known GPU gaps.
                // The last of them is closed: dashed lines, concave fills and
                // overlay z-order earlier, the faded hairline (#645), and the
                // fill-over-trace stacking (#653) — see the `scene::order`
                // parity tests. "experimental" stays because GPU parity is
                // still unconfirmed on hardware.
            ]
            .spacing(3)
            .width(200),
            Space::new().width(Length::Fill),
            signex_widgets::checkbox(draft_pcb_gpu_render)
                .size(TS.heading)
                .on_toggle(PrefMsg::DraftPcbGpuRender),
        ]
        .align_y(iced::Alignment::Center)
        .boxed(),
    );
    col = col.push(Space::new().height(20).boxed());

    // ── Section: Rendering ──
    col = col.push(h_sep());
    col = col.push(Space::new().height(16).boxed());
    col = col.push(section_title("Rendering"));
    col = col.push(Space::new().height(10).boxed());
    col = col.push(
        row![
            column![
                text(raw("Anti-aliasing (MSAA 4x)"))
                    .size(TS.label)
                    .style(text_primary),
                text(raw(
                    "Smooth the edges of canvas shapes. Applies after restart."
                ))
                .size(TS.caption)
                .style(text_muted),
            ]
            .spacing(3)
            .width(200),
            Space::new().width(Length::Fill),
            signex_widgets::checkbox(draft_antialiasing)
                .size(TS.heading)
                .on_toggle(PrefMsg::DraftAntialiasing),
        ]
        .align_y(iced::Alignment::Center)
        .boxed(),
    );
    col = col.push(Space::new().height(20).boxed());

    // ── Section: Symbol Editor ──
    col = col.push(h_sep());
    col = col.push(Space::new().height(16).boxed());
    col = col.push(section_title("Symbol Editor"));
    col = col.push(Space::new().height(10).boxed());
    col = col.push(
        row![
            column![
                text(raw("Default Grid Size"))
                    .size(TS.label)
                    .style(text_primary),
                text(raw("Applied when a symbol library is first opened. \
                      Can be changed per-library from the canvas status bar."))
                .size(TS.caption)
                .style(text_muted),
            ]
            .spacing(3)
            .width(200),
            Space::new().width(Length::Fill),
            iced::widget::pick_list(
                crate::canvas::grid::GRID_SIZES_MM
                    .iter()
                    .zip(crate::canvas::grid::GRID_SIZE_LABELS.iter())
                    .find(|(sz, _)| (**sz - draft_symbol_grid_size_mm).abs() < 0.001)
                    .map(|(_, lbl)| *lbl),
                crate::canvas::grid::GRID_SIZE_LABELS,
                |lbl: &&'static str| lbl.to_string(),
            )
            .on_select(|lbl: &'static str| {
                let mm = crate::canvas::grid::GRID_SIZES_MM
                    .iter()
                    .zip(crate::canvas::grid::GRID_SIZE_LABELS.iter())
                    .find(|(_, l)| **l == lbl)
                    .map(|(sz, _)| *sz)
                    .unwrap_or(1.27);
                PrefMsg::DraftSymbolGridSize(mm)
            })
            .text_size(12)
            .width(200),
        ]
        .align_y(iced::Alignment::Center)
        .boxed(),
    );
    col = col.push(Space::new().height(16).boxed());
    col = col.push(
        row![
            column![
                text(raw("Grid Style")).size(TS.label).style(text_primary),
                text(raw(
                    "Appearance of snap points on the symbol editor canvas."
                ))
                .size(TS.caption)
                .style(text_muted),
            ]
            .spacing(3)
            .width(200),
            Space::new().width(Length::Fill),
            iced::widget::pick_list(
                Some(draft_symbol_grid_style),
                GridStyle::ALL,
                |v: &GridStyle| v.to_string(),
            )
            .on_select(PrefMsg::DraftSymbolGridStyle)
            .text_size(12)
            .width(200),
        ]
        .align_y(iced::Alignment::Center)
        .boxed(),
    );
    col = col.push(Space::new().height(16).boxed());
    col = col.push(
        row![
            column![
                text(raw("Pin Selection")).size(TS.label).style(text_primary),
                text(raw("How a click selects a pin on the symbol editor canvas — the pin body only, or its name and number labels too."))
                    .size(TS.caption)
                    .style(text_muted),
            ]
            .spacing(3)
            .width(200),
            Space::new().width(Length::Fill),
            iced::widget::pick_list(
                Some(draft_symbol_pin_selection),
                PinSelectionMode::ALL,
                |v: &PinSelectionMode| v.to_string(),
            )
            .on_select(PrefMsg::DraftSymbolPinSelection)
            .text_size(12)
            .width(200),
        ]
        .align_y(iced::Alignment::Center)
        .boxed(),
    );
    col = col.push(Space::new().height(20).boxed());

    // ── Section: Interface scale ──
    col = col.push(section_title("Interface scale"));
    col = col.push(Space::new().height(10).boxed());
    col = col.push(
        row![
            column![
                text(raw("Interface scale"))
                    .size(TS.label)
                    .style(text_primary),
                text(raw(
                    "Scales text, panels, menus and dialogs on top of the display \
                     scale. Applied to every window when you save."
                ))
                .size(TS.caption)
                .style(text_muted),
            ]
            .spacing(3)
            .width(200),
            Space::new().width(Length::Fill),
            iced::widget::slider(
                crate::ui_scale::UI_SCALE_MIN_PERCENT..=crate::ui_scale::UI_SCALE_MAX_PERCENT,
                draft_ui_scale_percent,
                PrefMsg::DraftUiScalePercent,
            )
            .step(crate::ui_scale::UI_SCALE_STEP_PERCENT)
            .width(140),
            text(raw(format!("{draft_ui_scale_percent}%")))
                .size(TS.label)
                .style(text_primary)
                .width(100)
                .align_x(iced::alignment::Horizontal::Right),
        ]
        .spacing(10)
        .align_y(iced::Alignment::Center)
        .boxed(),
    );
    col = col.push(Space::new().height(20).boxed());

    // ── Section: Navigation ──
    col = col.push(section_title("Navigation"));
    col = col.push(Space::new().height(10).boxed());
    col = col.push(
        row![
            column![
                text(raw("Zoom speed")).size(TS.label).style(text_primary),
                text(raw(
                    "How far one mouse-wheel notch zooms on the schematic, PCB, \
                      symbol and footprint canvases."
                ))
                .size(TS.caption)
                .style(text_muted),
            ]
            .spacing(3)
            .width(200),
            Space::new().width(Length::Fill),
            iced::widget::slider(
                crate::canvas::Camera::ZOOM_STEP_PERCENT_MIN
                    ..=crate::canvas::Camera::ZOOM_STEP_PERCENT_MAX,
                draft_zoom_step_percent,
                PrefMsg::DraftZoomStepPercent,
            )
            .width(140),
            text(raw(format!("{draft_zoom_step_percent}% per notch")))
                .size(TS.label)
                .style(text_primary)
                .width(100)
                .align_x(iced::alignment::Horizontal::Right),
        ]
        .spacing(10)
        .align_y(iced::Alignment::Center)
        .boxed(),
    );
    col = col.push(Space::new().height(14).boxed());
    col = col.push(
        row![
            column![
                text(raw("Two-finger scroll pans"))
                    .size(TS.label)
                    .style(text_primary),
                text(raw(TRACKPAD_PAN_HELP))
                    .size(TS.caption)
                    .style(text_muted),
            ]
            .spacing(3)
            .width(320),
            Space::new().width(Length::Fill),
            signex_widgets::checkbox(draft_trackpad_pan)
                .size(TS.heading)
                .on_toggle(PrefMsg::DraftTrackpadPan),
        ]
        .align_y(iced::Alignment::Center)
        .boxed(),
    );
    col = col.push(Space::new().height(20).boxed());

    // ── Section: Window ──
    col = col.push(section_title("Window"));
    col = col.push(Space::new().height(10).boxed());
    col = col.push(
        row![
            column![
                text(raw("Remember window size and position"))
                    .size(TS.label)
                    .style(text_primary),
                text(raw(REMEMBER_GEOMETRY_HELP))
                    .size(TS.caption)
                    .style(text_muted),
            ]
            .spacing(3)
            .width(320),
            Space::new().width(Length::Fill),
            signex_widgets::checkbox(draft_remember_window_geometry)
                .size(TS.heading)
                .on_toggle(PrefMsg::DraftRememberWindowGeometry),
        ]
        .align_y(iced::Alignment::Center)
        .boxed(),
    );
    col = col.push(Space::new().height(20).boxed());

    col.boxed()
}

fn theme_card<'a>(
    id: ThemeId,
    name: &str,
    desc: &'static str,
    current: ThemeId,
) -> Element<'a, PrefMsg> {
    let is_active = id == current;
    let label = format!("{}{name}", if is_active { "✓ " } else { "" });
    let msg = PrefMsg::DraftTheme(id);

    // Card title / description resolve their colour against the card's own
    // (active vs inactive) background so they stay legible when the theme
    // flips light — active cards sit on a primary tint, inactive cards on
    // the weak background surface.
    let title_style = move |theme: &Theme| text::Style {
        color: Some(if is_active {
            theme.palette().primary.weak.text
        } else {
            theme.palette().background.base.text
        }),
        ..text::Style::default()
    };
    let desc_style = move |theme: &Theme| text::Style {
        color: Some(if is_active {
            theme.palette().primary.weak.text
        } else {
            theme.palette().secondary.base.color
        }),
        ..text::Style::default()
    };

    button(
        container(
            column![
                text(raw(label)).size(TS.label).style(title_style),
                text(raw(desc))
                    .size(TS.caption)
                    .style(desc_style)
                    .wrapping(iced::widget::text::Wrapping::None),
            ]
            .spacing(SP.s),
        )
        .padding([10.0, SP.l])
        .width(Length::Fill),
    )
    .padding(0)
    .width(Length::Fill)
    .on_press(msg)
    .style(move |theme: &Theme, status: button::Status| {
        let palette = theme.palette();
        let bg = if is_active {
            palette.primary.weak.color
        } else if matches!(status, button::Status::Hovered | button::Status::Pressed) {
            palette.background.strong.color
        } else {
            palette.background.weak.color
        };
        button::Style {
            background: Some(Background::Color(bg)),
            border: Border {
                width: 1.0,
                radius: 4.0.into(),
                color: if is_active {
                    palette.primary.base.color
                } else {
                    palette.background.strong.color
                },
                ..Border::default()
            },
            ..button::Style::default()
        }
    })
    .boxed()
}

fn import_btn<'a>() -> Element<'a, PrefMsg> {
    button(text(raw("⬆ Import Theme…")).size(TS.body))
        .padding([5.0, SP.l])
        .on_press(PrefMsg::ImportTheme)
        .style(primary_button_style)
        .boxed()
}

fn export_btn<'a>() -> Element<'a, PrefMsg> {
    button(text(raw("⬇ Export Theme…")).size(TS.body))
        .padding([5.0, SP.l])
        .on_press(PrefMsg::ExportTheme)
        .style(primary_button_style)
        .boxed()
}

/// Where the minimize / maximize / close buttons go, with a restart note
/// when the drafted choice changes how macOS creates the window.
fn window_controls_row<'a>(
    draft: crate::chrome::controls::WindowControlsPlacement,
    restart_note: bool,
) -> Element<'a, PrefMsg> {
    let mut help = column![
        text(raw("Window buttons"))
            .size(TS.label)
            .style(text_primary),
        text(raw(
            "Where minimize, maximize and close sit. Automatic follows your system."
        ))
        .size(TS.caption)
        .style(text_muted),
    ]
    .spacing(3)
    .width(200);
    if restart_note {
        help = help.push(
            text(raw("Takes effect after restart."))
                .size(TS.caption)
                .style(text_muted)
                .boxed(),
        );
    }
    row![
        help,
        Space::new().width(Length::Fill),
        iced::widget::pick_list(
            Some(draft),
            crate::chrome::controls::WindowControlsPlacement::ALL,
            |v: &crate::chrome::controls::WindowControlsPlacement| v.to_string(),
        )
        .on_select(PrefMsg::DraftWindowControlsPlacement)
        .text_size(12)
        .width(200),
    ]
    .align_y(iced::Alignment::Center)
    .boxed()
}
