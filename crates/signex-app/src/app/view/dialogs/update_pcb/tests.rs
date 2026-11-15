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
        app.ui_state.update_pcb = Some(Box::new(state));
        let _ = app.view_update_pcb_dialog();
    }
    app.ui_state.update_pcb = None;
    let _ = app.view_update_pcb_dialog();
}

/// The table builds with a problem row, a pad row and every filter, in a
/// window shorter than the dialog (#2371).
#[test]
fn the_table_builds_with_problems_under_every_filter() {
    use signex_engine::pcb::eco::{EcoBlock, EcoChange, EcoChangeList, EcoGroup, EcoKey, EcoRow};
    use signex_types::part::FootprintResolveError;

    use crate::app::update_pcb::{RowAction, RowFilter};

    let (mut app, _t) = crate::app::Signex::new();
    app.ui_state.window_size = (640.0, 300.0);
    let project = app.document_state.mint_project_id();
    let net = |name: &str, blocked: Option<EcoBlock>| EcoRow {
        key: EcoKey::AddNet(name.into()),
        group: EcoGroup::Nets,
        change: EcoChange::AddNet {
            number: 1,
            name: name.into(),
        },
        default_on: true,
        on: blocked.is_none(),
        blocked,
        needs: Vec::new(),
    };
    let list = EcoChangeList {
        revision: 1,
        rows: vec![
            net("VCC", None),
            net(
                "GND",
                Some(EcoBlock::MissingFootprint(
                    FootprintResolveError::NoFootprint,
                )),
            ),
        ],
        notes: Vec::new(),
    };
    let filters = [
        None,
        Some(RowFilter::Problems),
        Some(RowFilter::Action(RowAction::Add)),
        Some(RowFilter::Action(RowAction::Remove)),
    ];
    for filter in filters {
        let mut state = UpdatePcbState::new(
            project,
            "demo.snxprj".to_string(),
            Some(std::path::PathBuf::from("/w/demo.snxpcb")),
            false,
        );
        state.phase = Phase::Ready;
        state.list = Some(list.clone());
        state.table.filter = filter;
        state.validation = Some(Ok(()));
        app.ui_state.update_pcb = Some(Box::new(state));
        let _ = app.view_update_pcb_dialog();
    }
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
