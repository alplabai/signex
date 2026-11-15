//! The board 3D view in the main window (#2390): the board's 3D scene on
//! the board canvas's background, with the view's bar (2D, the viewpoints
//! and Fit). Undocked board windows stay flat.

use iced::alignment::{Horizontal, Vertical};
use iced::widget::{container, row, shader, stack};
use iced::{Element, Length, Widget as _};
use signex_gfx::camera3d::ViewPreset;
use signex_i18n::{MessageId, ids, t};
use signex_widgets::{text, theme_ext};

use crate::app::{Board3dMsg, Message, PcbMsg, Signex};
use crate::board_3d_shader::Board3dProgram;
use crate::pcb_canvas::layer_style::{board_3d_background, board_3d_highlight};
use crate::styles::{DialogButtonKind, Spacing, TypeScale, dialog_button};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Each viewpoint button and its label, in the bar's order.
const PRESETS: [(ViewPreset, MessageId); 6] = [
    (ViewPreset::Top, ids::PCB_3D_TOP),
    (ViewPreset::Bottom, ids::PCB_3D_BOTTOM),
    (ViewPreset::Front, ids::PCB_3D_FRONT),
    (ViewPreset::Left, ids::PCB_3D_LEFT),
    (ViewPreset::Right, ids::PCB_3D_RIGHT),
    (ViewPreset::Iso, ids::PCB_3D_ISO),
];

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
            scene: built.scene,
            generation: built.generation,
            camera: session.camera,
            highlight: board_3d_highlight(),
        };
        let background = board_3d_background();
        let backdrop = container(iced::widget::Space::new())
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| container::Style {
                background: Some(background.into()),
                ..container::Style::default()
            });
        let scene = shader(program).width(Length::Fill).height(Length::Fill);
        Some(stack![backdrop, scene, self.board_3d_bar()].boxed())
    }

    /// The view's bar: its name and hint, 2D, the viewpoints and Fit.
    fn board_3d_bar(&self) -> Element<'_, Message> {
        let i18n = &self.ui_state.i18n;
        let tokens = &self.document_state.panel_ctx.tokens;
        let event = |event| Some(Message::Pcb(PcbMsg::Board3d(event)));
        let mut bar = row![
            text(t!(i18n, ids::PCB_3D_TITLE))
                .size(TS.body)
                .color(theme_ext::text_primary(tokens)),
            text(t!(i18n, ids::PCB_3D_HINT))
                .size(TS.caption)
                .color(theme_ext::text_secondary(tokens)),
            dialog_button(
                t!(i18n, ids::PCB_3D_FLAT),
                DialogButtonKind::Primary,
                event(Board3dMsg::LeaveRequested),
                tokens,
            ),
        ]
        .spacing(SP.m)
        .align_y(iced::Alignment::Center);
        for (preset, label) in PRESETS {
            bar = bar.push(dialog_button(
                t!(i18n, label),
                DialogButtonKind::Secondary,
                event(Board3dMsg::PresetPicked(preset)),
                tokens,
            ));
        }
        bar = bar.push(dialog_button(
            t!(i18n, ids::PCB_3D_FIT),
            DialogButtonKind::Secondary,
            event(Board3dMsg::FitRequested),
            tokens,
        ));
        container(
            container(bar)
                .padding([SP.s, SP.m])
                .style(crate::styles::modal_card(tokens)),
        )
        .padding(SP.xl)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Horizontal::Left)
        .align_y(Vertical::Bottom)
        .boxed()
    }
}
