//! #1312 review: Positional Re-Annotate leaves an unlinked board alone, so
//! the next Update PCB, which matches unlinked footprints by designator,
//! still finds each part on its own footprint.

use super::fixture::*;
use super::*;
use crate::pcb::BoardEngine;
use crate::pcb::designators::{KeptReason, ReannotateOptions, plan_reannotate};

#[test]
fn re_annotating_an_unlinked_board_does_not_re_match_parts() {
    // R2 sits left of R1, so by position R2 would become R1.
    let board = board(vec![
        board_fp(0xa1, "R1", "10k", &[], &r0603(), (25.0, 10.0)),
        board_fp(0xa2, "R2", "4k7", &[], &r0603(), (20.0, 10.0)),
        board_fp(0xa3, "U1", "LM", &[], &sot23(), (30.0, 10.0)),
    ]);
    let plan = plan_reannotate(&board, ReannotateOptions::default());
    assert!(plan.renames.is_empty(), "{:?}", plan.renames);
    assert!(plan.kept.iter().all(|k| k.reason == KeptReason::Unlinked));

    let mut engine = BoardEngine::new(board, 1);
    engine
        .execute_batch(plan.commands(engine.board()).unwrap())
        .unwrap();
    let parts = vec![
        part("R1", &[0x101], "10k", Ok(r0603())),
        part("R2", &[0x102], "4k7", Ok(r0603())),
        part("U1", &[0x201, 0x202], "LM", Ok(sot23())),
    ];
    let design = Design::new(engine.board().clone(), parts, netlist(&[]));
    let list = design.diff();
    let swaps: Vec<&EcoRow> = list
        .rows
        .iter()
        .filter(|r| {
            matches!(
                r.change,
                EcoChange::SetValue { .. } | EcoChange::SetDesignator { .. }
            )
        })
        .collect();
    assert!(swaps.is_empty(), "{swaps:?}");
    let link_of = |fp: u128| {
        list.rows.iter().find_map(|r| match &r.change {
            EcoChange::Link {
                footprint, link, ..
            } if *footprint == u(fp) => Some(link.units.clone()),
            _ => None,
        })
    };
    assert_eq!(
        link_of(0xa1),
        Some(vec![u(0x101)]),
        "R1 stays on R1's footprint"
    );
    assert_eq!(link_of(0xa2), Some(vec![u(0x102)]));
}
