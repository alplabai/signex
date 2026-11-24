//! The board 3D view in the main window (#2390, mockup row 175): the board's 3D scene
//! on its lighting preset's backdrop, with a floating toolbar at the top
//! centre (its title, 2D | 3D, the viewpoints, Reset and Fit, Flip and Explode, the
//! lighting preset, the screenshot button), its popovers and toasts, the
//! view cube, the status pill and the interaction hint. Undocked board
//! windows stay flat. Layout follows mockup rows 109 and 175.

mod icons;
mod legend;
mod overlays;
mod toolbar;
mod view_cube;

use iced::alignment::{Horizontal, Vertical};
use iced::widget::{Space, column, container, shader, stack};
use iced::{Alignment, Background, Element, Length, Padding, Radians, Widget as _, gradient};
use signex_gfx::camera3d::{LightingPreset, ViewPreset};
use signex_i18n::{I18n, MessageId, ids};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use crate::app::state::{Board3dSession, BuiltScene, MatteBackdrop};
use crate::app::{Board3dMsg, Message, PcbMsg, Signex};
use crate::board_3d_shader::Board3dProgram;
use crate::pcb_canvas::layer_style::board_3d_highlight;
use crate::styles::board_palette::LightingBackdrop;
use crate::styles::tokens::{ResolvedTokens, hover_overlay, resolve};
use crate::styles::{TypeScale, ti};

const TS: TypeScale = TypeScale::DEFAULT;

/// The view's chrome sizes, as ratios of the 12 px body the mockups are
/// drawn at, so they follow the type scale.
#[derive(Debug, Clone, Copy)]
struct Metrics {
    /// Toolbar button height, its side padding and icon-to-label gap.
    button: f32,
    button_pad: f32,
    button_gap: f32,
    /// Toolbar padding and the gap between its buttons.
    bar_pad: f32,
    bar_gap: f32,
    /// Toolbar separator height and its side margin.
    sep: f32,
    sep_margin: f32,
    /// Toolbar icons and the lighting swatch; the lighting toast swatches.
    icon: f32,
    swatch: f32,
    toast_swatch: f32,
    /// Offsets: toolbar top, popover and toast gaps under the toolbar, the
    /// view cube and its corner inset, the status pill and hint corners.
    bar_top: f32,
    popover_gap: f32,
    toast_gap: f32,
    cube: f32,
    cube_inset: f32,
    pill_left: f32,
    pill_bottom: f32,
    hint_right: f32,
    hint_bottom: f32,
    /// Popover width and padding; menu padding and item height.
    popover: f32,
    popover_pad: f32,
    menu_pad: f32,
    menu_item: f32,
    /// Toast padding; the warn toast's padding and widest width.
    toast_pad_v: f32,
    toast_pad_h: f32,
    warn_pad_v: f32,
    warn_pad_h: f32,
    toast_max: f32,
    /// Status pill padding.
    pill_pad_v: f32,
    pill_pad_h: f32,
    /// Drop shadows `[y offset, blur]`: the toolbar and the warn toast, the
    /// lighting toast.
    bar_shadow: [f32; 2],
    toast_shadow: [f32; 2],
    popover_shadow: [f32; 2],
    /// The chevron glyph on the lighting and screenshot buttons.
    chevron: f32,
}

impl Metrics {
    const fn at(body: f32) -> Self {
        let em = body / 12.0;
        Self {
            button: 26.0 * em,
            button_pad: 8.0 * em,
            button_gap: 5.0 * em,
            bar_pad: 3.0 * em,
            bar_gap: 2.0 * em,
            sep: 18.0 * em,
            sep_margin: 3.0 * em,
            icon: 14.0 * em,
            swatch: 14.0 * em,
            toast_swatch: 16.0 * em,
            bar_top: 10.0 * em,
            popover_gap: 4.0 * em,
            toast_gap: 12.0 * em,
            cube: 64.0 * em,
            cube_inset: 14.0 * em,
            pill_left: 12.0 * em,
            pill_bottom: 10.0 * em,
            hint_right: 12.0 * em,
            hint_bottom: 8.0 * em,
            popover: 250.0 * em,
            popover_pad: 10.0 * em,
            menu_pad: 4.0 * em,
            menu_item: 26.0 * em,
            toast_pad_v: 8.0 * em,
            toast_pad_h: 12.0 * em,
            warn_pad_v: 7.0 * em,
            warn_pad_h: 10.0 * em,
            toast_max: 420.0 * em,
            pill_pad_v: 4.0 * em,
            pill_pad_h: 10.0 * em,
            bar_shadow: [4.0 * em, 14.0 * em],
            toast_shadow: [8.0 * em, 24.0 * em],
            popover_shadow: [6.0 * em, 20.0 * em],
            chevron: 12.0 * em,
        }
    }

    /// Where the view cube starts: under the toolbar's band, so the two
    /// never overlap whatever the view's width (mockup row 175).
    const fn cube_top(&self) -> f32 {
        self.bar_top + self.button + 2.0 * self.bar_pad + self.popover_gap
    }
}

/// A drop shadow `[y offset, blur]` in `look`'s shadow colour.
fn shadow(look: &Look, [y, blur]: [f32; 2]) -> iced::Shadow {
    iced::Shadow {
        color: look.r.shadow,
        offset: iced::Vector::new(0.0, y),
        blur_radius: blur,
    }
}

/// A floating card: raised fill, hairline border, `radius`, `shadow`.
fn card_style(
    look: &Look,
    radius: f32,
    shadow: iced::Shadow,
) -> impl Fn(&iced::Theme) -> container::Style + 'static {
    let (fill, line, ink) = (look.r.raised, look.border, look.text);
    move |_| container::Style {
        background: Some(Background::Color(fill)),
        text_color: Some(ink),
        border: iced::Border {
            width: theme_ext::HAIRLINE,
            color: line,
            radius: radius.into(),
            ..iced::Border::default()
        },
        shadow,
        ..container::Style::default()
    }
}

const M: Metrics = Metrics::at(TS.body);

/// The colours the view's chrome reads, resolved once per frame.
#[derive(Debug, Clone, Copy)]
struct Look {
    r: ResolvedTokens,
    text: iced::Color,
    ink2: iced::Color,
    border: iced::Color,
    hover: iced::Color,
    hover_fill: iced::Color,
    accent: iced::Color,
    warn: iced::Color,
}

impl Look {
    fn of(tokens: &ThemeTokens) -> Self {
        Self {
            r: resolve(tokens),
            text: ti(tokens.text),
            ink2: ti(tokens.text_secondary),
            border: ti(tokens.border),
            hover: hover_overlay(tokens),
            hover_fill: ti(tokens.hover),
            accent: theme_ext::accent(tokens),
            warn: theme_ext::warning_color(tokens),
        }
    }
}

/// The catalog name of a viewpoint.
fn preset_name(preset: ViewPreset) -> MessageId {
    match preset {
        ViewPreset::Top => ids::PCB_3D_TOP,
        ViewPreset::Bottom => ids::PCB_3D_BOTTOM,
        ViewPreset::Front => ids::PCB_3D_FRONT,
        ViewPreset::Left => ids::PCB_3D_LEFT,
        ViewPreset::Right => ids::PCB_3D_RIGHT,
        ViewPreset::Iso => ids::PCB_3D_ISO,
        ViewPreset::Back => ids::PCB_3D_BACK,
    }
}

/// The catalog name of a lighting preset.
fn lighting_name(preset: LightingPreset) -> MessageId {
    match preset {
        LightingPreset::Studio => ids::PCB_3D_LIGHTING_STUDIO,
        LightingPreset::Workshop => ids::PCB_3D_LIGHTING_WORKSHOP,
        LightingPreset::Outdoor => ids::PCB_3D_LIGHTING_OUTDOOR,
        LightingPreset::Dark => ids::PCB_3D_LIGHTING_DARK,
    }
}

/// `preset`'s backdrop as a top-to-bottom gradient, at `alpha`.
fn backdrop_fill(preset: LightingPreset, alpha: f32) -> Background {
    let LightingBackdrop { top, bottom } = LightingBackdrop::of(preset);
    // Angle π runs the gradient from the top edge to the bottom edge.
    let linear = gradient::Linear::new(Radians(std::f32::consts::PI))
        .add_stop(0.0, top.scale_alpha(alpha))
        .add_stop(1.0, bottom.scale_alpha(alpha));
    Background::Gradient(linear.into())
}

/// `event` as the app message that carries it.
fn event(event: Board3dMsg) -> Message {
    Message::Pcb(PcbMsg::Board3d(event))
}

/// The keys the chrome names (mockup row 175: "Flip board (F)", "Copy to
/// clipboard Ctrl+Shift+C", "press H for the next"): each command's
/// shortcut label in the active keymap, empty when it has none.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Board3dKeys {
    flip: String,
    lighting: String,
    copy: String,
}

impl Board3dKeys {
    fn of(keymap: &crate::keymap::CompiledKeymap) -> Self {
        Self {
            flip: keymap.shortcut_label_for_id("flip_board_3d"),
            lighting: keymap.shortcut_label_for_id("cycle_board_3d_lighting"),
            copy: keymap.shortcut_label_for_id("copy_board_3d_screenshot"),
        }
    }
}

impl Signex {
    /// The main window's body while its board is in the 3D view; `None`
    /// otherwise, and in every undocked window.
    pub(in crate::app::view) fn board_3d_view(
        &self,
        window_id: iced::window::Id,
    ) -> Option<Element<'_, Message>> {
        if self.ui_state.main_window_id != Some(window_id) {
            return None;
        }
        let session = self.board_3d_in_view()?;
        let built = self.board_3d_scene()?;
        let selected = self
            .interaction_state
            .pcb_selection
            .selection()
            .footprints();
        let program = Board3dProgram {
            highlights: built.scene.highlight_ranges(&selected),
            scene: built.scene.clone(),
            generation: built.generation,
            camera: session.camera,
            highlight: board_3d_highlight(),
            viewport: session.viewport,
            origin: session.origin,
            lighting: session.lighting,
        };
        // The backdrop follows the lighting preset only, the same in both
        // themes (DECISIONS, the 2026-11-19 board 3D view toolbar row). A
        // transparent screenshot draws its passes over black and white.
        let fill = match session.matte_backdrop() {
            Some(MatteBackdrop::Black) => Background::Color(iced::Color::BLACK),
            Some(MatteBackdrop::White) => Background::Color(iced::Color::WHITE),
            None => backdrop_fill(session.lighting, 1.0),
        };
        let backdrop = container(Space::new())
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| container::Style {
                background: Some(fill),
                ..container::Style::default()
            });
        let scene = shader(program).width(Length::Fill).height(Length::Fill);
        // A screenshot hides the chrome; the stack keeps its three layers.
        let chrome = if session.is_capturing() {
            Space::new().boxed()
        } else {
            self.board_3d_chrome(session, &built)
        };
        Some(stack![backdrop, scene, chrome].boxed())
    }

    /// Everything drawn over the scene: the status pill and the warn toast
    /// at the bottom left, the hint at the bottom right, the view cube at
    /// the top right, and the toolbar with its popover or toast on top.
    fn board_3d_chrome(
        &self,
        session: &Board3dSession,
        built: &BuiltScene,
    ) -> Element<'_, Message> {
        let i18n = &self.ui_state.i18n;
        let look = Look::of(&self.document_state.panel_ctx.tokens);
        let board = self.document_state.pcb_board(&session.board_path);

        let mut corner = column![].spacing(M.popover_gap * 2.0);
        if let Some(toast) = overlays::unread_toast(i18n, session, &look) {
            corner = corner.push(toast);
        }
        corner = corner.push(overlays::status_pill(
            i18n,
            session,
            &built.scene,
            board,
            &look,
        ));
        let bottom_left = container(corner)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Horizontal::Left)
            .align_y(Vertical::Bottom)
            .padding(Padding {
                left: M.pill_left,
                bottom: M.pill_bottom,
                ..Padding::ZERO
            });
        let hint = container(overlays::hint(i18n))
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Horizontal::Right)
            .align_y(Vertical::Bottom)
            .padding(Padding {
                right: M.hint_right,
                bottom: M.hint_bottom,
                ..Padding::ZERO
            });
        let cube = cube_corner(i18n, session, &look);

        let keys = Board3dKeys::of(&self.ui_state.active_keymap);
        let bar = toolbar::toolbar(i18n, session, &keys, &look);
        // The popovers hang under their own buttons (the toolbar draws
        // them); with neither open, the lighting toast is centred under it.
        let popover_open = session.explode_open || session.shot_menu_open;
        let top: Element<'_, Message> = if !popover_open && session.lighting_toast.is_some() {
            let toast = overlays::lighting_toast(i18n, session.lighting, &keys.lighting, &look);
            column![bar, toast]
                .spacing(M.toast_gap)
                .align_x(Alignment::Center)
                .boxed()
        } else {
            bar
        };
        let top = container(top)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Horizontal::Center)
            .align_y(Vertical::Top)
            .padding(Padding {
                top: M.bar_top,
                ..Padding::ZERO
            });
        stack![bottom_left, hint, cube, top].boxed()
    }
}

/// The top right corner: the view cube under the toolbar's band, and the
/// stack legend under it while the view is tall enough.
fn cube_corner<'a>(i18n: &I18n, session: &Board3dSession, look: &Look) -> Element<'a, Message> {
    let mut corner = column![view_cube::view_cube(i18n, session.side, look)]
        .spacing(M.popover_gap * 2.0)
        .align_x(Alignment::End);
    if legend::legend_fits(session.viewport[1]) {
        corner = corner.push(legend::stack_legend(i18n, look));
    }
    container(corner)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Horizontal::Right)
        .align_y(Vertical::Top)
        .padding(Padding {
            top: M.cube_top(),
            right: M.cube_inset,
            ..Padding::ZERO
        })
        .boxed()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use iced::Size;
    use signex_types::theme::{ThemeId, theme_tokens};

    use super::*;
    use crate::layout_probe::root_size;

    /// Mockup row 175: the cube starts below the toolbar as laid out, so it
    /// is clear of it at any view width.
    #[test]
    fn the_view_cube_starts_below_the_toolbar() {
        let tokens = theme_tokens(ThemeId::GraphiteLight);
        let look = Look::of(&tokens);
        let session = Board3dSession::open(Path::new("board.snxpcb"));
        let bar = toolbar::toolbar(I18n::en_us_ref(), &session, &Board3dKeys::default(), &look);
        let size = root_size(bar, Size::new(2000.0, 200.0));
        assert!(
            M.bar_top + size.height <= M.cube_top(),
            "the toolbar ends at {} and the cube starts at {}",
            M.bar_top + size.height,
            M.cube_top()
        );
    }

    /// Mockup row 175: the chrome names the keys both built-in profiles
    /// give Flip, the next lighting and the screenshot copy.
    #[test]
    fn the_chrome_names_the_bound_keys_of_flip_lighting_and_copy() {
        use crate::keymap::{CompiledKeymap, Platform, ShortcutProfileSet};

        let set = ShortcutProfileSet::built_ins().expect("built-in profiles");
        for profile in set.profiles() {
            let keymap = CompiledKeymap::compile_for(profile, Platform::Windows);
            assert_eq!(
                Board3dKeys::of(&keymap),
                Board3dKeys {
                    flip: "F".to_owned(),
                    lighting: "H".to_owned(),
                    copy: "Ctrl+Shift+C".to_owned(),
                },
                "{}",
                profile.id
            );
        }
    }
}
