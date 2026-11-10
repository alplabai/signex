use super::*;
use signex_library::primitive::{Pad, PadKind, StepAttachment};

fn key(version: u64) -> FootprintCardKey {
    FootprintCardKey {
        footprint: PrimitiveRef::new(uuid::Uuid::from_u128(1), uuid::Uuid::from_u128(2)),
        version,
    }
}

fn sot23_5() -> Footprint {
    let mut fp = Footprint::empty("SOT-23-5");
    fp.pads = [
        (-0.95, 1.1),
        (0.0, 1.1),
        (0.95, 1.1),
        (0.95, -1.1),
        (-0.95, -1.1),
    ]
    .iter()
    .enumerate()
    .map(|(i, (x, y))| Pad {
        number: (i + 1).to_string(),
        kind: PadKind::Smd,
        position: [*x, *y],
        size: [0.6, 1.0],
        ..Pad::default()
    })
    .collect();
    fp
}

fn facts() -> Arc<FootprintCardFacts> {
    Arc::new(FootprintCardFacts::of(&sot23_5()))
}

#[test]
fn the_summary_states_pitch_and_pad_count_like_the_mock() {
    let facts = FootprintCardFacts::of(&sot23_5());
    assert_eq!(facts.name, "SOT-23-5");
    assert_eq!(facts.summary(), "0.95 mm pitch, 5 pads");
    assert!(!facts.has_3d_body, "no STEP model, no 3D line");
}

#[test]
fn a_part_without_a_pitch_states_only_its_pads() {
    let mut fp = sot23_5();
    fp.pads.truncate(2);
    fp.pads[1].position = [0.0, -1.1];
    assert_eq!(FootprintCardFacts::of(&fp).summary(), "2 pads");
    fp.pads.truncate(1);
    assert_eq!(FootprintCardFacts::of(&fp).summary(), "1 pad");
}

#[test]
fn a_step_attachment_is_the_attached_3d_body() {
    let mut fp = sot23_5();
    fp.step_attachment = Some(StepAttachment {
        content_hash: "ab".into(),
        filename: "SOT-23-5.step".into(),
        offset_xyz: [0.0; 3],
        rotation_xyz: [0.0; 3],
    });
    assert!(FootprintCardFacts::of(&fp).has_3d_body);
}

#[test]
fn the_first_ask_queues_one_read_and_a_repeat_ask_does_not() {
    let mut cards = FootprintCards::default();
    assert!(matches!(cards.status(key(1)), FootprintCardStatus::Loading));
    assert!(matches!(cards.status(key(1)), FootprintCardStatus::Loading));
    assert_eq!(cards.take_queued(), vec![key(1)]);
    assert!(cards.take_queued().is_empty());
}

#[test]
fn a_finished_read_is_served_from_the_cache() {
    let mut cards = FootprintCards::default();
    cards.status(key(1));
    cards.take_queued();
    assert!(matches!(
        cards.finish(key(1), Ok(facts())),
        Some(FootprintCardStatus::Ready(_))
    ));
    assert!(matches!(
        cards.status(key(1)),
        FootprintCardStatus::Ready(_)
    ));
    assert!(
        cards.take_queued().is_empty(),
        "a cached footprint is not read again"
    );
}

#[test]
fn a_newer_library_version_reads_the_footprint_again() {
    let mut cards = FootprintCards::default();
    cards.status(key(1));
    cards.take_queued();
    cards.finish(key(1), Ok(facts()));
    assert!(matches!(cards.status(key(2)), FootprintCardStatus::Loading));
    assert_eq!(cards.take_queued(), vec![key(2)]);
    assert!(
        cards.finish(key(1), Ok(facts())).is_none(),
        "the read of the older version is dropped"
    );
}

#[test]
fn a_failed_read_is_kept_until_the_version_moves() {
    let mut cards = FootprintCards::default();
    cards.status(key(1));
    cards.take_queued();
    cards.finish(key(1), Err("gone".into()));
    match cards.status(key(1)) {
        FootprintCardStatus::Failed(why) => assert_eq!(why, "gone"),
        other => panic!("expected the failure, got {other:?}"),
    }
    assert!(cards.take_queued().is_empty());
    assert!(
        cards.finish(key(1), Ok(facts())).is_none(),
        "only a pending read lands"
    );
}

#[test]
fn a_full_cache_drops_finished_reads_but_keeps_pending_ones() {
    let mut cards = FootprintCards::default();
    let at = |n: u128| FootprintCardKey {
        footprint: PrimitiveRef::new(uuid::Uuid::from_u128(1), uuid::Uuid::from_u128(n)),
        version: 1,
    };
    for n in 0..MAX_CACHED as u128 {
        cards.status(at(n));
        if n > 0 {
            cards.finish(at(n), Ok(facts()));
        }
    }
    cards.status(at(999));
    assert!(matches!(cards.status(at(0)), FootprintCardStatus::Loading));
    assert_eq!(cards.entries.len(), 2, "the pending read and the new one");
}
