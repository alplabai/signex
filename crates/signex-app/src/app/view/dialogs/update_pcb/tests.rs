//! The Update PCB dialog builds in every phase, its marks are drawn glyphs
//! and its ticks the solid-accent tri-state box (mockup rows 81, 82); the
//! panels' mixed checkbox keeps the Graphite soft accent (#1312).

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
        pad_locks: Vec::new(),
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

/// The panels' form_look mixed checkbox keeps the Graphite soft accent
/// (#1312). This dialog's group box is `tick_box`, pinned below.
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

/// #2533 — the redesigned dialog builds with a blocked row of every kind,
/// the Notes group open and closed, a filter text and every tile, in both
/// Graphite themes.
#[test]
fn the_redesign_builds_with_every_problem_and_filter() {
    use signex_engine::pcb::eco::{
        EcoBlock, EcoChange, EcoChangeList, EcoGroup, EcoKey, EcoNote, EcoRow,
    };
    use signex_types::part::FootprintResolveError;

    use crate::app::update_pcb::{RowAction, RowFilter};

    let (mut app, _t) = crate::app::Signex::new();
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
    let blocks = [
        EcoBlock::MissingFootprint(FootprintResolveError::NoFootprint),
        EcoBlock::NeedsAnnotation,
        EcoBlock::DuplicateDesignator("C14".into()),
        EcoBlock::PadsCannotCarry { number: "3".into() },
        EcoBlock::Unplaceable("NaN".into()),
        EcoBlock::PartNotRead,
    ];
    let mut rows = vec![net("VCC", None)];
    rows.extend(
        blocks
            .into_iter()
            .enumerate()
            .map(|(n, b)| net(&format!("N{n}"), Some(b))),
    );
    let list = EcoChangeList {
        revision: 1,
        rows,
        notes: vec![EcoNote::KeptUnlinked {
            reference: "J4".into(),
            footprint: uuid::Uuid::nil(),
        }],
        pad_locks: Vec::new(),
    };
    let filters = [
        None,
        Some(RowFilter::Problems),
        Some(RowFilter::Action(RowAction::Add)),
        Some(RowFilter::Action(RowAction::Change)),
    ];
    for theme in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
        app.document_view.panel_ctx.tokens = theme_tokens(theme);
        for (n, filter) in filters.into_iter().enumerate() {
            let mut state = UpdatePcbState::new(
                project,
                "demo.snxprj".to_string(),
                Some(std::path::PathBuf::from("/w/demo.snxpcb")),
                false,
            );
            state.phase = Phase::Ready;
            state.list = Some(list.clone());
            state.table.filter = filter;
            state.table.notes_open = n % 2 == 0;
            state.table.query = if n == 3 { "vcc".into() } else { String::new() };
            state.validation = Some(if n == 1 { Err("stale".into()) } else { Ok(()) });
            app.ui_state.update_pcb = Some(Box::new(state));
            let _ = app.view_update_pcb_dialog();
        }
    }
}

/// #2533 — Back Annotate builds with a flagged rename (ticked and not), a
/// shared-sheet row, the footprint info row and folded groups; Positional
/// Re-Annotate builds with the in-order toggle on and off.
#[test]
fn back_annotate_and_reannotate_build_in_both_themes() {
    use signex_engine::pcb::designators::{
        Kept, KeptReason, ReannotateOptions, ReannotatePlan, Rename,
    };
    use signex_engine::pcb::eco::{
        BackBlock, BackChange, BackChangeList, BackGroup, BackKey, BackRow,
    };
    use signex_types::schematic::InstancePath;
    use uuid::Uuid;

    use crate::app::BoardReannotateState;
    use crate::app::back_annotate::BackAnnotateState;
    use crate::app::state::ProjectId;

    let id = Uuid::from_u128;
    let rename = BackRow {
        key: BackKey::Designator(id(1)),
        group: BackGroup::Designators,
        change: BackChange::Designator {
            footprint: id(1),
            sheet: InstancePath::root(),
            units: vec![id(11)],
            from: "R17".into(),
            to: "R12".into(),
        },
        on: false,
        blocked: None,
    };
    let footprint = BackRow {
        key: BackKey::Footprint(id(2)),
        group: BackGroup::Footprints,
        change: BackChange::Footprint {
            footprint: id(2),
            reference: "L1".into(),
            board: "IND_4040".into(),
            schematic: "IND_5050".into(),
        },
        on: false,
        blocked: Some(BackBlock::FootprintFromBoard),
    };
    let list = BackChangeList {
        revision: 1,
        rows: vec![rename, footprint],
        notes: Vec::new(),
        designators: vec![
            (InstancePath::root(), vec![id(11)], "R17".into()),
            (InstancePath::root(), vec![id(12)], "R12".into()),
        ],
    };
    let plan = ReannotatePlan {
        renames: vec![Rename {
            footprint: id(3),
            from: "R3".into(),
            to: "R2".into(),
        }],
        kept: vec![Kept {
            footprint: id(4),
            reference: "R9".into(),
            reason: KeptReason::Locked,
        }],
        unchanged: 1,
    };
    let (mut app, _t) = crate::app::Signex::new();
    for theme in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
        app.document_view.panel_ctx.tokens = theme_tokens(theme);
        for ticked in [false, true] {
            let mut state = BackAnnotateState::new(
                ProjectId::test_id(1),
                "p.snxprj".into(),
                Some("b.snxpcb".into()),
            );
            let mut l = list.clone();
            l.set_on(&BackKey::Designator(id(1)), ticked);
            state.list = Some(l);
            state.phase = Phase::Ready;
            state.folds.insert(BackGroup::Footprints, ticked);
            state.focused = ticked.then_some(BackKey::Designator(id(1)));
            assert!(
                state
                    .duplicate_renames()
                    .contains_key(&BackKey::Designator(id(1)))
            );
            app.ui_state.back_annotate = Some(Box::new(state));
            let _ = app.view_back_annotate_dialog();

            app.ui_state.board_reannotate = Some(BoardReannotateState {
                board: "b.snxpcb".into(),
                board_name: "b.snxpcb".into(),
                options: ReannotateOptions::default(),
                plan: plan.clone(),
                revision: 1,
                problem: None,
                in_order: vec!["R1".into()],
                show_in_order: ticked,
            });
            let _ = app.view_board_reannotate_dialog();
        }
    }
}

/// Mockup rows 81 and 82: every status mark of a row, a Notes header and a
/// banner is a drawn glyph, never a font character that falls back to a
/// missing-glyph box; the tick is green.
#[test]
fn every_row_status_is_a_drawn_glyph() {
    use super::change_rows::{RowStatus, StatusGlyph};

    assert_eq!(RowStatus::Quiet.glyph(), None);
    assert_eq!(RowStatus::Applies.glyph(), Some(StatusGlyph::Check));
    assert_eq!(RowStatus::Skipped.glyph(), Some(StatusGlyph::Dash));
    assert_eq!(RowStatus::Blocked.glyph(), Some(StatusGlyph::Blocked));
    assert_eq!(RowStatus::Info.glyph(), Some(StatusGlyph::Info));
    assert_eq!(RowStatus::Warn.glyph(), Some(StatusGlyph::Warn));
    for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
        let t = theme_tokens(id);
        assert_eq!(RowStatus::Applies.ink(&t), ti(t.success), "{id:?}");
        assert_eq!(RowStatus::Warn.ink(&t), ti(t.warning), "{id:?}");
    }
}

/// Mockup row 81: a ticked or partly ticked box is solid accent with the
/// white tick or dash; an empty one is the field with a strong hairline;
/// a box that cannot change sits on the sunken fill.
#[test]
fn a_ticked_or_mixed_box_is_solid_accent() {
    use signex_engine::pcb::eco::GroupTicks;

    use super::change_rows::{StatusGlyph, Tick, tick_look};

    assert_eq!(Tick::of_group(GroupTicks { on: 3, open: 3 }), Tick::On);
    assert_eq!(Tick::of_group(GroupTicks { on: 1, open: 3 }), Tick::Mixed);
    assert_eq!(Tick::of_group(GroupTicks { on: 0, open: 3 }), Tick::Off);
    assert_eq!(Tick::of_group(GroupTicks { on: 0, open: 0 }), Tick::Off);
    for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
        let t = theme_tokens(id);
        let r = tokens::resolve(&t);
        let accent = tokens::accent_fill(&t);
        let white = tokens::on_accent(&t);
        let on = tick_look(Tick::On, true, &t);
        assert_eq!(
            (on.fill, on.edge, on.mark),
            (accent, accent, Some((StatusGlyph::Check, white))),
            "{id:?}"
        );
        let mixed = tick_look(Tick::Mixed, true, &t);
        assert_eq!(
            (mixed.fill, mixed.edge, mixed.mark),
            (accent, accent, Some((StatusGlyph::Dash, white))),
            "{id:?}"
        );
        let off = tick_look(Tick::Off, true, &t);
        assert_eq!(
            (off.fill, off.edge, off.mark),
            (r.field, r.line_strong, None),
            "{id:?}"
        );
        let inert = tick_look(Tick::Off, false, &t);
        assert_eq!((inert.fill, inert.edge), (r.sunken, r.line_soft), "{id:?}");
        let inert_on = tick_look(Tick::On, false, &t);
        assert_eq!(
            (inert_on.fill, inert_on.mark),
            (r.line_strong, Some((StatusGlyph::Check, white))),
            "{id:?}"
        );
    }
}

/// Mockup row 81: after a passing Validate the footer says "Validated: ..."
/// beside Report... with the green tick; a failing one keeps the reason.
#[test]
fn the_footer_says_validated() {
    use super::change_rows::StatusGlyph;

    let i18n = signex_i18n::I18n::en_us_ref();
    let (mut app, _t) = crate::app::Signex::new();
    let project = app.document_state.mint_project_id();
    let mut state = UpdatePcbState::new(
        project,
        "demo.snxprj".to_string(),
        Some(std::path::PathBuf::from("/w/demo.snxpcb")),
        false,
    );
    assert!(super::validation_verdict(&state, i18n).is_none());
    state.validation = Some(Ok(()));
    let (glyph, words, ok) = super::validation_verdict(&state, i18n).expect("a verdict");
    assert_eq!(glyph, StatusGlyph::Check);
    assert!(ok);
    assert_eq!(
        words.into_string(),
        "Validated: every ticked change applies to the board."
    );
    state.validation = Some(Err("stale".into()));
    let (glyph, _, ok) = super::validation_verdict(&state, i18n).expect("a verdict");
    assert_eq!(glyph, StatusGlyph::Blocked);
    assert!(!ok);
}

/// Mockup row 82: a ticked Back Annotate row that applies shows the green
/// tick once Validate passed, and the footer shows the verdict beside the
/// `N of M changes ticked` pill.
#[test]
fn back_annotate_ticks_a_validated_row_and_says_so_beside_the_pill() {
    use signex_engine::pcb::eco::{BackChange, BackChangeList, BackGroup, BackKey, BackRow};
    use signex_types::schematic::InstancePath;
    use uuid::Uuid;

    use super::super::back_annotate::{footer_left, row_status};
    use super::change_rows::RowStatus;
    use crate::app::back_annotate::BackAnnotateState;
    use crate::app::state::ProjectId;

    let i18n = signex_i18n::I18n::en_us_ref();
    let t = theme_tokens(ThemeId::GraphiteLight);
    let id = Uuid::from_u128;
    let value = BackRow {
        key: BackKey::Value(id(1)),
        group: BackGroup::Values,
        change: BackChange::Value {
            footprint: id(1),
            sheet: InstancePath::root(),
            units: vec![id(11)],
            reference: "C1".into(),
            from: "10uF".into(),
            to: "22uF".into(),
        },
        on: true,
        blocked: None,
    };
    let mut state = BackAnnotateState::new(
        ProjectId::test_id(1),
        "p.snxprj".into(),
        Some("b.snxpcb".into()),
    );
    state.list = Some(BackChangeList {
        revision: 1,
        rows: vec![value.clone()],
        notes: Vec::new(),
        designators: Vec::new(),
    });
    state.phase = Phase::Ready;
    assert_eq!(footer_left(&state, i18n, &t).len(), 1, "the pill alone");
    assert_eq!(
        row_status(&state, &value, None, i18n, &t).0,
        RowStatus::Quiet
    );
    state.validation = Some(Ok(()));
    let (status, caption) = row_status(&state, &value, None, i18n, &t);
    assert_eq!(status, RowStatus::Applies);
    assert!(caption.is_none());
    assert_eq!(footer_left(&state, i18n, &t).len(), 2, "pill and verdict");
}

/// Mockup rows 81, 82: both dialogs stack the subtitle under the title, so
/// their header is the taller one.
#[test]
fn the_stacked_header_is_taller_than_the_one_line_header() {
    use super::super::header::{DIALOG_HEADER_H, STACKED_HEADER_H};
    const { assert!(STACKED_HEADER_H > DIALOG_HEADER_H) };
}

/// Exit criterion 3: the net classes from the schematic are one summary
/// line (no mockup row), worded by count, and nothing when there are none.
#[test]
fn the_classes_from_the_schematic_are_one_summary_line() {
    let i18n = signex_i18n::I18n::en_us();
    let tokens = theme_tokens(ThemeId::GraphiteLight);
    assert!(super::schematic_rules::classes_line(0, &i18n, &tokens).is_none());
    assert!(super::schematic_rules::classes_line(2, &i18n, &tokens).is_some());
    let line = |count: usize| {
        signex_i18n::t!(i18n, signex_i18n::ids::UPDATE_PCB_SCHEMATIC_CLASSES, "count" => count)
            .into_string()
    };
    assert!(
        line(1).starts_with("1 net class from the schematic"),
        "{}",
        line(1)
    );
    assert!(
        line(2).contains("2") && line(2).ends_with("net classes from the schematic"),
        "{}",
        line(2)
    );
}
