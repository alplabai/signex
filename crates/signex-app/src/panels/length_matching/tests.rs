//! #2797, mockup row 107: the view state of the Length Matching pair and
//! what the tab and the dock panel draw from the same rows.

use std::path::PathBuf;

use signex_engine::pcb::length_match::MemberStatus;
use signex_types::pcb_length::LengthTopology;
use uuid::Uuid;

use super::*;
use crate::app::Signex;
use crate::layout_probe::lay_out_texts;
use crate::panels::PanelContext;

const GROUP: Uuid = Uuid::from_u128(7);

fn member(net: &str, length: f64, status: MemberStatus) -> LengthMemberRow {
    LengthMemberRow {
        net: net.into(),
        length_mm: Some(length),
        delta_mm: Some(length - 28.5),
        status,
        pair: None,
    }
}

fn facts() -> LengthMatchingFacts {
    LengthMatchingFacts {
        board: PathBuf::from("/b/board.snxpcb"),
        read_only: false,
        groups: vec![LengthGroupRow {
            uuid: GROUP,
            name: "DDR_DQ".into(),
            rule: "MatchedLengths_DDR_DQ".into(),
            target_mm: 28.5,
            tolerance_mm: 0.25,
            topology: LengthTopology::Chain,
            members: vec![
                member("DDR_DQ0", 28.40, MemberStatus::InBand),
                member("DDR_DQ6", 28.91, MemberStatus::Over),
            ],
        }],
        nets: vec!["CLK".into(), "DDR_DQ0".into(), "DDR_DQ6".into()],
    }
}

fn context(facts: Option<LengthMatchingFacts>) -> PanelContext {
    let (app, _boot) = Signex::new();
    let mut ctx = app.document_view.panel_ctx.clone();
    ctx.length_matching = facts;
    ctx
}

fn texts(element: iced::Element<'_, crate::panels::PanelMsg>) -> Vec<String> {
    lay_out_texts(element, iced::Size::new(1200.0, 900.0))
        .into_iter()
        .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect()
}

#[test]
fn a_header_click_folds_a_panel_group_and_a_member_click_picks_its_group() {
    let mut ui = LengthMatchingUi::default();
    ui.follow(std::path::Path::new("/b/board.snxpcb"));
    ui.apply(&LengthMatchingMsg::GroupToggled(GROUP));
    assert!(ui.closed.contains(&GROUP));
    ui.apply(&LengthMatchingMsg::GroupToggled(GROUP));
    assert!(!ui.closed.contains(&GROUP));
    ui.apply(&LengthMatchingMsg::FieldTyped {
        field: LengthField::Target,
        text: "30".into(),
    });
    ui.apply(&LengthMatchingMsg::MemberClicked {
        group: GROUP,
        net: "DDR_DQ6".into(),
    });
    assert_eq!(ui.current, Some(GROUP));
    assert_eq!(ui.selected.as_deref(), Some("DDR_DQ6"));
    assert!(
        ui.drafts.is_empty(),
        "text typed for no group goes when one is picked"
    );
}

#[test]
fn another_board_starts_the_view_state_afresh() {
    let mut ui = LengthMatchingUi::default();
    ui.follow(std::path::Path::new("/a.snxpcb"));
    ui.apply(&LengthMatchingMsg::GroupPicked(GROUP));
    ui.follow(std::path::Path::new("/a.snxpcb"));
    assert_eq!(ui.current, Some(GROUP), "the same board keeps it");
    ui.follow(std::path::Path::new("/b.snxpcb"));
    assert_eq!(ui.current, None);
}

#[test]
fn the_out_of_band_filter_keeps_groups_with_a_member_outside() {
    let rows = facts();
    assert!(LengthFilter::OutOfBand.shows(&rows.groups[0]));
    let mut in_band = rows.groups[0].clone();
    in_band.members.truncate(1);
    assert!(!LengthFilter::OutOfBand.shows(&in_band));
    assert!(LengthFilter::All.shows(&in_band));
}

#[test]
fn the_panel_and_the_tab_draw_the_same_group() {
    let ctx = context(Some(facts()));
    for (view, shown) in [
        ("panel", texts(panel_view(&ctx))),
        ("tab", texts(tab_view(&ctx))),
    ] {
        for wanted in [
            "DDR_DQ",
            "DDR_DQ0",
            "DDR_DQ6",
            "Rule: MatchedLengths_DDR_DQ",
        ] {
            assert!(
                shown.iter().any(|t| t == wanted),
                "{view}: {wanted} in {shown:?}"
            );
        }
        assert!(
            shown.iter().any(|t| t == "1 out of band"),
            "{view}: {shown:?}"
        );
    }
    let tab = texts(tab_view(&ctx));
    assert!(tab.iter().any(|t| t == "Add Group"), "{tab:?}");
    assert!(tab.iter().any(|t| t == "Show as panel"), "{tab:?}");
}

#[test]
fn the_tab_edits_the_picked_group_only() {
    let mut ctx = context(Some(facts()));
    let before = texts(tab_view(&ctx));
    assert!(!before.iter().any(|t| t == "Delete group"), "{before:?}");
    ctx.length_matching_ui
        .follow(std::path::Path::new("/b/board.snxpcb"));
    ctx.length_matching_ui
        .apply(&LengthMatchingMsg::GroupPicked(GROUP));
    let after = texts(tab_view(&ctx));
    assert!(after.iter().any(|t| t == "Delete group"), "{after:?}");
    assert!(after.iter().any(|t| t == "Edit DDR_DQ"), "{after:?}");
}

#[test]
fn without_a_board_the_tab_says_so() {
    let ctx = context(None);
    let shown = texts(tab_view(&ctx));
    assert!(shown.iter().any(|t| t == "No board"), "{shown:?}");
}
