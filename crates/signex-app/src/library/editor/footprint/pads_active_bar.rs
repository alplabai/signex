//! Pads-mode Active Bar — floating tool row for the footprint editor
//! when the editor is in [`EditorMode::Normal`].
//!
//! The Pads-mode tools: Select ([`selection_items`], at the end of the
//! bar's selection group) and Place Pad / Place Via / Place String
//! ([`placement_items`], at the start of its placement group). The mode
//! switch (Sketch / Pads / 3D) is the floating overlay
//! [`mode_switcher_overlay`].

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

/// The footprint editor bar (Graphite mockup row 187): the two-state
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

    // Height of one segment (mockup row 187 `.seg > span`: 22 px).
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

/// One Pads-mode tool button, selected while `tool` is the armed tool.
fn tool_button(
    editor: &FootprintDocument,
    icon: ActiveBarIcon,
    tooltip: &str,
    tool: PadsTool,
) -> ActiveBarItem<LibraryMessage> {
    ActiveBarItem::Button(ActiveBarButton {
        icon,
        tooltip: tooltip.to_string(),
        enabled: true,
        selected: editor.state.pads_tool == tool,
        on_press: Some(LibraryMessage::PrimitiveEditorEvent {
            path: editor.path.clone(),
            msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SetPadsTool(tool)),
        }),
        ..ActiveBarButton::default()
    })
}

/// The Pads-mode Select tool, the default tool.
pub fn selection_items(
    editor: &FootprintDocument,
    theme_id: signex_types::theme::ThemeId,
) -> Vec<ActiveBarItem<LibraryMessage>> {
    vec![tool_button(
        editor,
        ActiveBarIcon::Svg(icons::icon_select(theme_id)),
        "Select",
        PadsTool::Select,
    )]
}

/// The Pads-mode place tools: arm one, then click empty canvas to drop a
/// pad, a through-hole via or silk-layer text. Pure graphics live in
/// Sketch mode; a hole is a pad `kind` set in the Properties panel.
pub fn placement_items(
    editor: &FootprintDocument,
    _theme_id: signex_types::theme::ThemeId,
) -> Vec<ActiveBarItem<LibraryMessage>> {
    vec![
        tool_button(
            editor,
            ActiveBarIcon::Glyph("\u{25CF}"), // ●
            "Place Pad — click empty canvas to drop pads",
            PadsTool::PlacePad,
        ),
        tool_button(
            editor,
            ActiveBarIcon::Glyph("\u{25CE}"), // ◎ — annular ring glyph
            "Place Via — click empty canvas to drop a through-hole via",
            PadsTool::PlaceVia,
        ),
        tool_button(
            editor,
            ActiveBarIcon::Glyph("T"),
            "Place String — click empty canvas to drop silk-layer text",
            PadsTool::PlaceString,
        ),
    ]
}

/// The Pads-mode tools alone (Select, a divider, the place tools) through
/// [`signex_widgets::active_bar::view`].
pub fn view<'a>(
    editor: &'a FootprintDocument,
    theme_id: signex_types::theme::ThemeId,
    tokens: &'a ThemeTokens,
) -> iced::Element<'a, LibraryMessage> {
    let mut items = selection_items(editor, theme_id);
    items.push(ActiveBarItem::Separator);
    items.extend(placement_items(editor, theme_id));
    signex_widgets::active_bar::view(items, tokens)
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
