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
    table.problems_only = true;
    assert!(!table.shows(&ok) && table.shows(&bad));
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
