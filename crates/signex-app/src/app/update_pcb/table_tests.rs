//! #2371 — the table rules: each row's action, the summary counts, the
//! filters and the default folds.

use signex_engine::pcb::eco::{EcoBlock, EcoChange, EcoChangeList, EcoGroup, EcoKey, EcoRow};
use signex_types::part::FootprintResolveError;
use signex_types::pcb::PadNet;
use uuid::Uuid;

use super::{RowAction, RowFilter, Summary, TableGroup, TableState, folded_by_default};

fn net_row(n: u32) -> EcoRow {
    EcoRow {
        key: EcoKey::AddNet(format!("N{n}")),
        group: EcoGroup::Nets,
        change: EcoChange::AddNet {
            number: n,
            name: format!("N{n}"),
        },
        default_on: true,
        on: true,
        blocked: None,
        needs: Vec::new(),
    }
}

fn pad_row(from: Option<&str>, to: Option<&str>) -> EcoRow {
    let net = |name: &str| PadNet {
        number: 1,
        name: name.to_string(),
    };
    EcoRow {
        key: EcoKey::PadNet {
            footprint: Uuid::nil(),
            pad: Uuid::nil(),
        },
        group: EcoGroup::PadNets,
        change: EcoChange::PadNet {
            footprint: Uuid::nil(),
            pad: Uuid::nil(),
            reference: "R1".into(),
            pad_number: "1".into(),
            from: from.map(net),
            to: to.map(net),
            locked: false,
        },
        default_on: true,
        on: true,
        blocked: None,
        needs: Vec::new(),
    }
}

fn blocked_net_row(n: u32) -> EcoRow {
    EcoRow {
        on: false,
        blocked: Some(EcoBlock::MissingFootprint(
            FootprintResolveError::NoFootprint,
        )),
        ..net_row(n)
    }
}

#[test]
fn a_pad_gaining_its_first_net_is_an_add_and_losing_it_a_removal() {
    let action = |r: EcoRow| RowAction::of(&r.change);
    assert_eq!(action(pad_row(None, Some("VCC"))), RowAction::Add);
    assert_eq!(action(pad_row(Some("VCC"), None)), RowAction::Remove);
    assert_eq!(action(pad_row(Some("VCC"), Some("GND"))), RowAction::Change);
}

#[test]
fn the_summary_counts_each_action_and_the_problems() {
    let list = EcoChangeList {
        rows: vec![net_row(1), blocked_net_row(2), pad_row(Some("A"), None)],
        ..Default::default()
    };
    assert_eq!(
        Summary::of(&list),
        Summary {
            add: 2,
            change: 0,
            remove: 1,
            problems: 1
        }
    );
}

#[test]
fn filters_narrow_the_rows() {
    let (ok, bad) = (net_row(1), blocked_net_row(2));
    let mut table = TableState::default();
    assert!(table.shows(&ok) && table.shows(&bad));
    table.filter = Some(RowFilter::Problems);
    assert!(!table.shows(&ok) && table.shows(&bad));
    table.filter = Some(RowFilter::Action(RowAction::Remove));
    assert!(!table.shows(&ok) && !table.shows(&bad));
    table.filter = None;
    table.query = "n2".into();
    assert!(!table.shows(&ok) && table.shows(&bad), "the text names N2");
    assert!(table.is_filtered());
}

#[test]
fn a_big_clean_group_starts_folded_and_a_problem_opens_it() {
    let mut list = EcoChangeList {
        rows: (0..11).map(net_row).collect(),
        ..Default::default()
    };
    assert!(folded_by_default(&list, TableGroup::Nets));
    assert!(!folded_by_default(&list, TableGroup::Components));
    list.rows[3] = blocked_net_row(3);
    assert!(!folded_by_default(&list, TableGroup::Nets));
    list.rows.truncate(10);
    list.rows[3] = net_row(3);
    assert!(
        !folded_by_default(&list, TableGroup::Nets),
        "ten rows stay open"
    );

    let mut table = TableState::default();
    table.folds.insert(TableGroup::Nets, true);
    assert!(
        table.is_folded(&list, TableGroup::Nets),
        "the user's fold wins"
    );
}

fn key_of(row: &EcoRow) -> EcoKey {
    row.key.clone()
}

fn pad_row_at(n: u128, from: Option<&str>, to: Option<&str>) -> EcoRow {
    let mut row = pad_row(from, to);
    let (footprint, pad) = (Uuid::from_u128(n), Uuid::from_u128(n + 1));
    row.key = EcoKey::PadNet { footprint, pad };
    if let EcoChange::PadNet {
        footprint: f,
        pad: p,
        ..
    } = &mut row.change
    {
        (*f, *p) = (footprint, pad);
    }
    row
}

/// #2533 — each tile lists exactly its own rows, and a second click (or
/// Clear filter) lists every row again.
#[test]
fn each_tile_lists_exactly_its_rows() {
    let rows = vec![
        net_row(1),
        blocked_net_row(2),
        pad_row_at(10, Some("A"), Some("B")),
        pad_row_at(20, Some("A"), None),
    ];
    let list = EcoChangeList {
        rows: rows.clone(),
        ..Default::default()
    };
    let shown = |table: &TableState| -> Vec<EcoKey> {
        list.rows
            .iter()
            .filter(|r| table.shows(r))
            .map(key_of)
            .collect()
    };
    let mut table = TableState::default();
    assert_eq!(shown(&table).len(), 4, "no tile is selected on open");
    assert!(!table.is_filtered());
    let cases = [
        (RowFilter::Action(RowAction::Add), vec![0, 1]),
        (RowFilter::Action(RowAction::Change), vec![2]),
        (RowFilter::Action(RowAction::Remove), vec![3]),
        (RowFilter::Problems, vec![1]),
    ];
    for (filter, want) in cases {
        table.filter = Some(filter);
        let want: Vec<EcoKey> = want.into_iter().map(|i| key_of(&rows[i])).collect();
        assert_eq!(shown(&table), want, "{filter:?}");
    }
    let summary = Summary::of(&list);
    assert_eq!((summary.add, summary.change, summary.remove), (2, 1, 1));
}

/// The filter text names a row by its object or by a net its pad moves
/// between, in any case.
#[test]
fn the_filter_text_names_designators_and_nets() {
    let pad = pad_row(Some("VCC"), Some("GND"));
    let mut table = TableState::default();
    for (query, hit) in [("r1.1", true), ("gnd", true), ("Vc", true), ("R9", false)] {
        table.query = query.into();
        assert_eq!(table.shows(&pad), hit, "{query}");
    }
    table.query = "  ".into();
    assert!(
        table.shows(&pad) && !table.is_filtered(),
        "blank text is no filter"
    );
}

/// #2533 — every blocked kind is lifted into the Problems card, in list
/// order, with the fix the card offers for it.
#[test]
fn the_problems_card_lifts_every_blocked_kind() {
    use super::{ProblemFix, problems};
    let kinds = [
        (
            EcoBlock::MissingFootprint(FootprintResolveError::NoFootprint),
            ProblemFix::ChooseFootprint,
        ),
        (EcoBlock::NeedsAnnotation, ProblemFix::Annotate),
        (
            EcoBlock::DuplicateDesignator("C14".into()),
            ProblemFix::Annotate,
        ),
        (
            EcoBlock::PadsCannotCarry { number: "3".into() },
            ProblemFix::None,
        ),
        (EcoBlock::Unplaceable("NaN".into()), ProblemFix::None),
        (EcoBlock::PartNotRead, ProblemFix::None),
    ];
    let mut rows = vec![net_row(100)];
    for (n, (why, _)) in kinds.iter().enumerate() {
        rows.push(EcoRow {
            on: false,
            blocked: Some(why.clone()),
            ..net_row(n as u32)
        });
    }
    let list = EcoChangeList {
        rows,
        ..Default::default()
    };
    let lifted = problems(&list);
    assert_eq!(lifted.len(), kinds.len(), "the clean row stays out");
    for (row, (why, fix)) in lifted.iter().zip(&kinds) {
        assert_eq!(row.blocked.as_ref(), Some(why));
        assert_eq!(ProblemFix::of(why), *fix, "{why:?}");
        assert!(!row.on, "a blocked row stays unticked in its group");
    }
    assert_eq!(Summary::of(&list).problems, kinds.len());
}

/// #2533 — the group tick counts its open rows only: a blocked row is in
/// neither count, an unticked one makes the tick mixed.
#[test]
fn a_group_tick_counts_its_open_rows() {
    use super::{group_ticks, set_group_on};
    let mut list = EcoChangeList {
        rows: vec![net_row(1), net_row(2), blocked_net_row(3)],
        ..Default::default()
    };
    let ticks = group_ticks(&list, TableGroup::Nets);
    assert_eq!((ticks.on, ticks.open), (2, 2));
    assert!(ticks.all() && !ticks.mixed());
    list.set_on(&EcoKey::AddNet("N2".into()), false);
    let ticks = group_ticks(&list, TableGroup::Nets);
    assert_eq!((ticks.on, ticks.open), (1, 2));
    assert!(ticks.mixed());
    set_group_on(&mut list, TableGroup::Nets, true);
    let ticks = group_ticks(&list, TableGroup::Nets);
    assert!(ticks.all(), "the group tick ticks every open row");
    assert!(list.rows[2].blocked.is_some() && !list.rows[2].on);
    let empty = group_ticks(&list, TableGroup::Components);
    assert_eq!((empty.on, empty.open), (0, 0));
}

/// #2533 — Execute lowers exactly the ticked rows: an unticked row and a
/// blocked row reach the board as nothing.
#[test]
fn execute_applies_exactly_the_ticked_rows() {
    use signex_engine::pcb::BoardCommand;
    use signex_engine::pcb::eco::lower;
    let mut list = EcoChangeList {
        rows: vec![net_row(1), net_row(2), blocked_net_row(3)],
        ..Default::default()
    };
    list.set_on(&EcoKey::AddNet("N2".into()), false);
    assert_eq!(super::applied_count(&list), 1);
    let commands = lower(&list, &signex_types::pcb::PcbBoard::default());
    let nets: Vec<String> = commands
        .iter()
        .filter_map(|c| match c {
            BoardCommand::SetNets(edit) => Some(edit.nets.iter().map(|n| n.name.clone())),
            _ => None,
        })
        .flatten()
        .collect();
    assert_eq!(nets, ["N1"]);
}

/// #2533 — Pad nets start folded unless one of their rows has a problem.
#[test]
fn pad_nets_start_folded_and_a_problem_opens_them() {
    let mut list = EcoChangeList {
        rows: vec![pad_row(Some("A"), Some("B"))],
        ..Default::default()
    };
    assert!(folded_by_default(&list, TableGroup::PadNets));
    list.rows[0].blocked = Some(EcoBlock::PartNotRead);
    assert!(!folded_by_default(&list, TableGroup::PadNets));
}
