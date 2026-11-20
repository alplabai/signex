//! Pads-mode Active Bar — floating tool row for the footprint editor
//! when the editor is in [`EditorMode::Normal`].
//!
//! Mirrors Altium's PCB Library editor active-bar layout: Select +
//! Place Pad / Track / Arc / String / Polygon / Hole, an Auto-fit
//! Courtyard toggle, and a Sketch-mode entry button. Tools that
//! aren't wired yet ship as stubs (greyed icons, no
//! `on_press`) so the bar reads as the eventual finished surface,
//! not a half-built one.
//!
//! The full set is intentional Altium parity — every pad/track/poly
//! placement lands as wiring goes in. The current wiring is:
//! - Select: cursor (no specific message; the canvas's empty-space
//!   click already adds a pad in this mode, so "Place Pad" stays
//!   visually present in the bar but acts as a discoverability
//!   reminder until a proper place-pad gesture lands).
//! - Auto-fit Courtyard: existing `FootprintToggleAutoFit` toggle.
//! - Edit Sketch: mode-switch to `EditorMode::Sketch`.

use std::path::PathBuf;

use iced::widget::{container, row};
use iced::{Border, Length, Theme, Widget};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, text};

use crate::styles::{Spacing, TypeScale};
use signex_widgets::active_bar::{ActiveBarButton, ActiveBarIcon, ActiveBarItem};
use signex_widgets::theme_ext;

use crate::app::FootprintDocument;
use crate::icons;
use crate::library::editor::footprint::state::{EditorMode, PadsTool};
use crate::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};

/// The footprint editor bar (Graphite mockup m187): the two-state
/// Footprint / Sketch switch at its left, the 3D toggle beside it, then
/// `trailing` (the sketch info caption and Parameters in Sketch mode). It
/// floats at the canvas's top right, just under the Active Bar's band so the
/// two never overlap at any width, clear of the DOF chips at the top left
/// and ending above the parameters table card.
pub fn mode_switcher_overlay<'a>(
    editor: &'a FootprintDocument,
    tokens: &'a ThemeTokens,
    i18n: &signex_i18n::I18n,
    trailing: Option<iced::Element<'a, LibraryMessage>>,
) -> iced::Element<'a, LibraryMessage> {
    use crate::library::editor::footprint::state::mode::ModeSwitch;
    use crate::styles::Radii;
    use iced::widget::{Space, button as button_style, container as container_style};
    use iced::{Alignment, Background, Shadow, Vector};

    // Height of one segment (m187 `.seg > span`: 22 px).
    const SEGMENT_H: f32 = crate::styles::chrome_px(22.0 * 11.0 / 12.0);

    // One segment: the lit one on the field fill with ink, a small shadow
    // and medium weight, the other in ink3 on the sunken track.
    fn segment<'b>(
        mode: EditorMode,
        on: bool,
        msg: LibraryMessage,
        tokens: &ThemeTokens,
        i18n: &signex_i18n::I18n,
    ) -> iced::Element<'b, LibraryMessage> {
        let r = crate::styles::tokens::resolve(tokens);
        let hover = crate::styles::tokens::hover_overlay(tokens);
        let ink = if on {
            theme_ext::text_primary(tokens)
        } else {
            r.ink3
        };
        let mut label = text(signex_i18n::t!(i18n, mode.switch_label()))
            .size(TypeScale::default().body)
            .color(ink);
        if on {
            label = label.font(iced::Font {
                weight: iced::font::Weight::Medium,
                ..iced::Font::DEFAULT
            });
        }
        button(container(label).center_y(Length::Fixed(SEGMENT_H)))
            .padding([0.0, Spacing::default().m])
            .on_press(msg)
            .style(move |_: &Theme, status| button_style::Style {
                background: if on {
                    Some(Background::Color(r.field))
                } else if status == button_style::Status::Hovered {
                    Some(Background::Color(hover))
                } else {
                    None
                },
                border: Border {
                    radius: Radii::GRAPHITE.small.into(),
                    ..Border::default()
                },
                shadow: if on {
                    Shadow {
                        color: r.shadow,
                        offset: Vector::new(0.0, 1.0),
                        blur_radius: 2.0,
                    }
                } else {
                    Shadow::default()
                },
                ..button_style::Style::default()
            })
            .boxed()
    }

    // The 3D toggle, as tall as the segment track: accent while on.
    fn toggle_3d<'b>(
        on: bool,
        msg: LibraryMessage,
        tokens: &ThemeTokens,
        i18n: &signex_i18n::I18n,
    ) -> iced::Element<'b, LibraryMessage> {
        let (ts, sp) = (TypeScale::default(), Spacing::default());
        let r = crate::styles::tokens::resolve(tokens);
        let ink = theme_ext::text_primary(tokens);
        let label_ink = if on {
            crate::styles::tokens::on_accent(tokens)
        } else {
            ink
        };
        let label = text(signex_i18n::t!(i18n, EditorMode::View3d.switch_label()))
            .size(ts.body)
            .color(label_ink);
        let toggle = button(container(label).center_y(Length::Fixed(SEGMENT_H + 2.0 * sp.xs)))
            .padding([0.0, sp.m])
            .on_press(msg)
            .style(chip_style(tokens, on))
            .boxed();
        let tip = text(signex_i18n::t!(
            i18n,
            signex_i18n::ids::EDITORS_MODE_3D_TOOLTIP
        ))
        .size(ts.caption)
        .color(ink);
        let tip = container(tip).padding([sp.xs, sp.s]).style(
            signex_widgets::active_bar_dropdown::tooltip_style(r.raised, r.line_strong),
        );
        signex_widgets::tooltip(toggle, tip, signex_widgets::tooltip::Position::Bottom).boxed()
    }

    let sp = Spacing::default();
    let switch = ModeSwitch::of(editor.state.mode, editor.state.mode_before_3d);
    let send = |mode: EditorMode| LibraryMessage::PrimitiveEditorEvent {
        path: editor.path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SetMode(mode)),
    };
    let mut segments = row![].spacing(sp.xs).align_y(Alignment::Center);
    for mode in ModeSwitch::SEGMENTS {
        segments = segments.push(segment(mode, switch.is_lit(mode), send(mode), tokens, i18n));
    }
    let sunken = crate::styles::tokens::resolve(tokens).sunken;
    let segments =
        container(segments)
            .padding(sp.xs)
            .style(move |_: &Theme| container_style::Style {
                background: Some(Background::Color(sunken)),
                border: Border {
                    radius: Radii::GRAPHITE.control.into(),
                    ..Border::default()
                },
                ..container_style::Style::default()
            });
    // A click on the toggle goes back to the mode 3D was entered from.
    let toggle = toggle_3d(switch.is_3d_on, send(switch.toggle_target), tokens, i18n);

    let mut bar = row![segments, toggle]
        .spacing(sp.m)
        .align_y(Alignment::Center);
    if let Some(trailing) = trailing {
        bar = bar.push(trailing);
    }
    let fill = theme_ext::to_color(&tokens.panel_bg);
    let line = theme_ext::border_color(tokens);
    let bar = container(bar)
        .padding([sp.xs, sp.s])
        .style(move |_: &Theme| container_style::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                width: 1.0,
                radius: Radii::GRAPHITE.card.into(),
                color: line,
                ..Border::default()
            },
            ..container_style::Style::default()
        });
    let below_active_bar = crate::active_bar::bar_top_margin()
        + crate::active_bar::bar_height_for(&signex_widgets::active_bar::Metrics::GRAPHITE);
    container(iced::widget::column![
        Space::new().height(below_active_bar),
        bar
    ])
    .padding([0.0, sp.m])
    .align_x(iced::alignment::Horizontal::Right)
    .width(Length::Fill)
    .height(Length::Fill)
    .boxed()
}

/// Build the Pads-mode Active Bar items.
pub fn items(
    editor: &FootprintDocument,
    theme_id: signex_types::theme::ThemeId,
) -> Vec<ActiveBarItem<LibraryMessage>> {
    let path: PathBuf = editor.path.clone();

    let _stub = |label: &str, glyph: &'static str| -> ActiveBarItem<LibraryMessage> {
        ActiveBarItem::Button(ActiveBarButton {
            icon: ActiveBarIcon::Glyph(glyph),
            tooltip: format!("{label} (coming soon)"),
            enabled: false,
            selected: false,
            on_press: None,
            ..ActiveBarButton::default()
        })
    };

    let _stub_svg = |label: &str, icon: ActiveBarIcon| -> ActiveBarItem<LibraryMessage> {
        ActiveBarItem::Button(ActiveBarButton {
            icon,
            tooltip: format!("{label} (coming soon)"),
            enabled: false,
            selected: false,
            on_press: None,
            ..ActiveBarButton::default()
        })
    };

    // v0.15 — Pads-mode tool state machine. Select is the default;
    // PlacePad makes empty-canvas clicks drop a pad at the cursor.
    let pads_tool = editor.state.pads_tool;
    let select_path = path.clone();
    let select = ActiveBarItem::Button(ActiveBarButton {
        icon: ActiveBarIcon::Svg(icons::icon_select(theme_id)),
        tooltip: "Select".into(),
        enabled: true,
        selected: pads_tool == PadsTool::Select,
        on_press: Some(LibraryMessage::PrimitiveEditorEvent {
            path: select_path,
            msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SetPadsTool(PadsTool::Select)),
        }),
        ..ActiveBarButton::default()
    });

    // Place Pad. Activate the tool, then click an
    // empty area of the canvas to drop a pad there.
    let place_pad_path = path.clone();
    let place_pad = ActiveBarItem::Button(ActiveBarButton {
        icon: ActiveBarIcon::Glyph("\u{25CF}"), // ●
        tooltip: "Place Pad — click empty canvas to drop pads".into(),
        enabled: true,
        selected: pads_tool == PadsTool::PlacePad,
        on_press: Some(LibraryMessage::PrimitiveEditorEvent {
            path: place_pad_path,
            msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SetPadsTool(PadsTool::PlacePad)),
        }),
        ..ActiveBarButton::default()
    });

    // Auto-fit Courtyard moved to the Properties panel
    // default body (Settings section). The active bar stays focused
    // on placement/edit tools.

    // Dedicated "Edit Sketch" button removed — mode
    // segments at the left of the bar drive mode switching.

    // Pads-mode active bar simplified. Pure graphics
    // (Line / Arc / Polygon / Region) live in Sketch mode only;
    // PlaceHole is folded into PlacePad (the user picks `kind` via
    // the Properties panel). Place Via is the new through-hole tool.
    // Delete button removed — Delete key handles it.
    vec![
        select,
        ActiveBarItem::Separator,
        place_pad,
        // Place Via — through-hole via. 1-click drop. Wired as a stub
        // until the via primitive ships; the tool state survives so
        // Esc + the Properties panel remain coherent.
        ActiveBarItem::Button(ActiveBarButton {
            icon: ActiveBarIcon::Glyph("\u{25CE}"), // ◎ — annular ring glyph
            tooltip: "Place Via — click empty canvas to drop a through-hole via".into(),
            enabled: true,
            selected: pads_tool == PadsTool::PlaceVia,
            on_press: Some(LibraryMessage::PrimitiveEditorEvent {
                path: path.clone(),
                msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SetPadsTool(PadsTool::PlaceVia)),
            }),
            ..ActiveBarButton::default()
        }),
        // Place String — silk-layer 1-click text drop.
        ActiveBarItem::Button(ActiveBarButton {
            icon: ActiveBarIcon::Glyph("T"),
            tooltip: "Place String — click empty canvas to drop silk-layer text".into(),
            enabled: true,
            selected: pads_tool == PadsTool::PlaceString,
            on_press: Some(LibraryMessage::PrimitiveEditorEvent {
                path: path.clone(),
                msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SetPadsTool(
                    PadsTool::PlaceString,
                )),
            }),
            ..ActiveBarButton::default()
        }),
    ]
}

/// Convenience wrapper — build items + render via
/// [`signex_widgets::active_bar::view`].
pub fn view<'a>(
    editor: &'a FootprintDocument,
    theme_id: signex_types::theme::ThemeId,
    tokens: &'a ThemeTokens,
) -> iced::Element<'a, LibraryMessage> {
    signex_widgets::active_bar::view(items(editor, theme_id), tokens)
}

type ChipStyle = Box<dyn Fn(&Theme, iced::widget::button::Status) -> iced::widget::button::Style>;

/// Mode / tab chip: accent fill when `active`, outline otherwise.
fn chip_style(tokens: &ThemeTokens, active: bool) -> ChipStyle {
    if active {
        Box::new(crate::styles::accent_button(tokens))
    } else {
        Box::new(crate::styles::outline_button(tokens))
    }
}
