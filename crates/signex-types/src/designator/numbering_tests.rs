use std::collections::HashMap;

use super::*;

#[test]
fn tokens_parse_strictly_and_round_trip() {
    for scheme in NumberingScheme::ALL {
        assert_eq!(NumberingScheme::parse(scheme.token()), Some(*scheme));
    }
    assert_eq!(
        NumberingScheme::parse(" PAGE_HUNDREDS "),
        Some(NumberingScheme::PageHundreds)
    );
    assert_eq!(NumberingScheme::parse("per_sheet"), None);
    assert_eq!(NumberingScheme::default(), NumberingScheme::Continuous);
    let json = serde_json::to_string(&NumberingScheme::PageThousands).unwrap();
    assert_eq!(json, "\"page_thousands\"");
}

#[test]
fn the_continuous_scheme_has_no_blocks() {
    let blocks = NumberBlocks::new(NumberingScheme::Continuous, &[Some(1)], ["R1"]);
    assert_eq!(blocks, Ok(None));
}

#[test]
fn each_page_numbers_inside_its_own_hundred() {
    let blocks = NumberBlocks::new(NumberingScheme::PageHundreds, &[Some(1), Some(2)], [])
        .unwrap()
        .unwrap();
    assert_eq!(blocks.start(0), Some(101));
    assert_eq!(blocks.start(1), Some(201));
    let mut first = HashMap::new();
    let mut second = HashMap::new();
    assert_eq!(blocks.issue(0, "R", &mut first), Ok(101));
    assert_eq!(blocks.issue(0, "R", &mut first), Ok(102));
    assert_eq!(blocks.issue(0, "C", &mut first), Ok(101));
    assert_eq!(blocks.issue(1, "R", &mut second), Ok(201));
}

#[test]
fn thousands_start_at_page_times_a_thousand_plus_one() {
    let blocks = NumberBlocks::new(NumberingScheme::PageThousands, &[Some(3)], [])
        .unwrap()
        .unwrap();
    assert_eq!(blocks.start(0), Some(3001));
    assert_eq!(blocks.issue(0, "U", &mut HashMap::new()), Ok(3001));
}

/// A kept number inside a block, from any unit, is never handed out again;
/// one outside every block (R5 from the continuous scheme) holds nothing back.
#[test]
fn kept_numbers_inside_a_block_are_skipped_wherever_they_sit() {
    let blocks = NumberBlocks::new(
        NumberingScheme::PageHundreds,
        &[Some(1), Some(2)],
        ["R5", "R104", "R250", "C?", "#PWR101"],
    )
    .unwrap()
    .unwrap();
    assert_eq!(blocks.issue(0, "R", &mut HashMap::new()), Ok(105));
    assert_eq!(blocks.issue(1, "R", &mut HashMap::new()), Ok(251));
    assert_eq!(blocks.issue(0, "C", &mut HashMap::new()), Ok(101));
}

#[test]
fn a_full_block_refuses_rather_than_spill_into_the_next_page() {
    let blocks = NumberBlocks::new(NumberingScheme::PageHundreds, &[Some(1)], ["R199"])
        .unwrap()
        .unwrap();
    let full = blocks.issue(0, "R", &mut HashMap::new());
    assert_eq!(
        full,
        Err(BlockIssue::Full(NumberBlock {
            first: 101,
            last: 199
        }))
    );
}

#[test]
fn a_unit_without_a_page_gets_no_block() {
    let blocks = NumberBlocks::new(NumberingScheme::PageHundreds, &[None, Some(2)], [])
        .unwrap()
        .unwrap();
    assert_eq!(blocks.start(0), None);
    assert_eq!(
        blocks.issue(0, "R", &mut HashMap::new()),
        Err(BlockIssue::NoBlock)
    );
    assert_eq!(
        blocks.issue(9, "R", &mut HashMap::new()),
        Err(BlockIssue::NoBlock)
    );
}

#[test]
fn two_units_on_one_page_are_refused() {
    let problem = NumberBlocks::new(NumberingScheme::PageHundreds, &[Some(2), None, Some(2)], [])
        .unwrap_err();
    assert_eq!(
        problem,
        BlockProblem::SharedPage {
            page: 2,
            units: vec![0, 2]
        }
    );
}

#[test]
fn a_page_whose_block_passes_u32_max_is_refused() {
    let page = (u32::MAX / 1000 + 1) as usize;
    let problem = NumberBlocks::new(NumberingScheme::PageThousands, &[Some(page)], []).unwrap_err();
    assert_eq!(problem, BlockProblem::PageTooHigh { page });
}
