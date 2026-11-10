//! The Update PCB dialog builds in every phase, and the mixed group box has
//! the Graphite accent look (#1312).

use iced::Background;
use signex_types::theme::{ThemeId, theme_tokens};
use signex_widgets::checkbox::Status;

use crate::app::update_pcb::{Phase, UpdatePcbState};
use crate::styles::{ti, tokens};

#[test]
fn the_dialog_builds_in_every_phase() {
    let (mut app, _t) = crate::app::Signex::new();
    let project = app.document_state.mint_project_id();
    let phases = [
        Phase::NoBoard,
        Phase::Opening,
        Phase::Computing,
        Phase::Failed("the root sheet could not be read".to_string()),
        Phase::Ready,
    ];
    for phase in phases {
        let mut state = UpdatePcbState::new(
            project,
            "demo.snxprj".to_string(),
            Some(std::path::PathBuf::from("/w/demo.snxpcb")),
            false,
        );
        state.phase = phase;
        state.list = Some(Default::default());
        app.ui_state.update_pcb = Some(state);
        let _ = app.view_update_pcb_dialog();
    }
    app.ui_state.update_pcb = None;
    let _ = app.view_update_pcb_dialog();
}

#[test]
fn a_mixed_group_box_is_drawn_on_the_soft_accent() {
    for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
        let t = theme_tokens(id);
        let theme = crate::app::Signex::id_to_iced_theme(id);
        let mixed = crate::panels::form_look::mixed_checkbox_style(
            &theme,
            Status::Active { is_checked: true },
        );
        assert_eq!(
            mixed.background,
            Background::Color(tokens::resolve(&t).accent_soft),
            "{id:?}"
        );
        assert_eq!(mixed.border.color, ti(t.accent), "{id:?}");
    }
}
