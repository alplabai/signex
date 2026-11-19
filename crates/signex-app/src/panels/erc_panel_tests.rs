//! #85: the ERC panel's grouping by rule, its tiles and its clean-run state.

use super::*;
use crate::layout_probe::{lay_out_texts, messages_on_click};

fn entry(index: usize, rule: &str, severity: ErcSeverityLite, message: &str) -> ErcDiagnosticEntry {
    ErcDiagnosticEntry {
        global_index: index,
        sheet_name: "power".into(),
        sheet_path: "power.snxsch".into(),
        severity,
        rule_label: rule.into(),
        rule_kind: None,
        message: message.into(),
        world_x: 120.65,
        world_y: 88.9,
        shown_y: 88.9,
        select: None,
        locatable: true,
        net: None,
        rule_id: format!("builtin::{rule}"),
        waiver_key: None,
    }
}

fn findings() -> Vec<ErcDiagnosticEntry> {
    vec![
        entry(
            0,
            "Unused pin",
            ErcSeverityLite::Warning,
            "U2.PA8 is not connected",
        ),
        entry(
            1,
            "Pin type conflict",
            ErcSeverityLite::Error,
            "U1.5 drives PGOOD",
        ),
        entry(
            2,
            "Unused pin",
            ErcSeverityLite::Warning,
            "U2.PA9 is not connected",
        ),
        entry(
            3,
            "Off-grid pin",
            ErcSeverityLite::Info,
            "R3.1 is off the grid",
        ),
        entry(
            4,
            "Pin type conflict",
            ErcSeverityLite::Error,
            "U3.1 and U7.2 on +3V3",
        ),
    ]
}

fn context() -> PanelContext {
    let (app, _boot) = crate::app::Signex::new();
    app.document_state.panel_ctx.clone()
}

fn texts(ctx: &PanelContext) -> Vec<String> {
    lay_out_texts(view_erc(ctx), iced::Size::new(300.0, 4000.0))
        .into_iter()
        .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect()
}

#[test]
fn findings_group_by_rule_most_severe_first() {
    let entries = findings();
    let shape: Vec<(&str, Vec<usize>)> = erc_groups(&entries, None)
        .iter()
        .map(|g| (g.rule, g.rows.iter().map(|r| r.global_index).collect()))
        .collect();
    assert_eq!(
        shape,
        [
            ("Pin type conflict", vec![1, 4]),
            ("Unused pin", vec![0, 2]),
            ("Off-grid pin", vec![3]),
        ]
    );
}

#[test]
fn a_tile_lists_only_its_severity_and_hides_the_other_groups() {
    let entries = findings();
    let only_warnings: Vec<&str> = erc_groups(&entries, Some(ErcSeverityLite::Warning))
        .iter()
        .map(|g| g.rule)
        .collect();
    assert_eq!(only_warnings, ["Unused pin"]);

    let mut view = ErcPanelView::default();
    view.apply(ErcPanelMsg::TileToggled(ErcSeverityLite::Warning), true);
    assert_eq!(view.only, Some(ErcSeverityLite::Warning));
    view.apply(ErcPanelMsg::TileToggled(ErcSeverityLite::Warning), true);
    assert_eq!(view.only, None, "a second press lists everything again");
}

#[test]
fn the_panel_heads_each_group_with_its_rule_and_count() {
    let mut ctx = context();
    ctx.erc_ran = true;
    ctx.erc_diagnostics = findings();
    let shown = texts(&ctx);
    for rule in ["Pin type conflict", "Unused pin", "Off-grid pin"] {
        assert!(shown.iter().any(|t| t == rule), "{rule}: {shown:?}");
    }
    assert!(
        shown.iter().any(|t| t == "power · 120.65, 88.90"),
        "{shown:?}"
    );
    // A folded group keeps its heading and hides its rows.
    ctx.validation
        .erc
        .collapsed
        .insert("Unused pin".to_string());
    let shown = texts(&ctx);
    assert!(shown.iter().any(|t| t == "Unused pin"));
    assert!(!shown.iter().any(|t| t == "U2.PA8 is not connected"));
}

#[test]
fn row_actions_show_on_hover_or_selection_only() {
    let mut ctx = context();
    ctx.erc_ran = true;
    ctx.erc_diagnostics = findings();
    assert!(!texts(&ctx).iter().any(|t| t == "Show"));
    ctx.validation.erc.hovered = Some(3);
    let window = iced::Size::new(300.0, 4000.0);
    let pressed = messages_on_click(view_erc(&ctx), window, "Show");
    assert!(
        matches!(pressed.as_slice(), [PanelMsg::FocusErcViolation(3)]),
        "{pressed:?}"
    );
}

#[test]
fn a_clean_run_says_what_it_checked_with_neutral_tiles_and_the_waived_group_shut() {
    let mut ctx = context();
    ctx.erc_ran = true;
    ctx.validation.erc_checked = Some(ErcChecked {
        sheets: 6,
        symbols: 42,
        nets: 118,
        at: chrono::Utc::now(),
    });
    ctx.erc_waivers = vec![ErcWaiverEntry {
        rule_id: "builtin::no-driver".into(),
        net: "VREF".into(),
        hidden: 1,
    }];
    let shown = texts(&ctx);
    assert!(shown.iter().any(|t| t == "No problems found."), "{shown:?}");
    assert!(
        shown
            .iter()
            .any(|t| t.starts_with("6 sheets, 42 symbols and 118 nets checked at ")),
        "{shown:?}"
    );
    assert_eq!(shown.iter().filter(|t| *t == "0").count(), 3, "{shown:?}");
    assert!(shown.iter().any(|t| t == "Waived"), "{shown:?}");
    assert!(
        !shown.iter().any(|t| t == "builtin::no-driver · net VREF"),
        "the Waived group is shut on a clean run: {shown:?}"
    );
    // A zero tile filters nothing: it is a summary, not a button.
    let window = iced::Size::new(300.0, 4000.0);
    assert!(messages_on_click(view_erc(&ctx), window, "errors").is_empty());
}

#[test]
fn run_erc_reads_running_while_it_runs() {
    let mut ctx = context();
    ctx.validation.run_erc_key = "F8".into();
    let idle = texts(&ctx);
    assert!(idle.iter().any(|t| t == "Run ERC"), "{idle:?}");
    assert!(idle.iter().any(|t| t == "F8"), "{idle:?}");
    let window = iced::Size::new(300.0, 4000.0);
    let pressed = messages_on_click(view_erc(&ctx), window, "Run ERC");
    assert!(
        matches!(pressed.as_slice(), [PanelMsg::RunErc]),
        "{pressed:?}"
    );
    ctx.erc_running = true;
    let busy = texts(&ctx);
    assert!(busy.iter().any(|t| t == "Running…"), "{busy:?}");
    assert!(messages_on_click(view_erc(&ctx), window, "Running…").is_empty());
}
