//! Notebook badges (mockup row 197): one badge per part or net, the block
//! count, the colour of the worst status.

use crate::{
    AnnotationKind, BadgeTarget, BadgeTone, BlockMention, Status, badge_of, badges, scan,
    worst_tone,
};

fn mention(kind: AnnotationKind, status: Option<Status>, failing: bool) -> BlockMention {
    BlockMention {
        kind,
        title: String::new(),
        status,
        date: None,
        file: "design.snxnbk".into(),
        line: 1,
        failing,
    }
}

#[test]
fn the_tone_is_the_worst_status_of_the_blocks() {
    use AnnotationKind::*;
    let note = mention(Note, None, false);
    let calc = mention(Calculation, Some(Status::Current), false);
    let passed = mention(Measurement, Some(Status::Done), false);
    let planned = mention(Measurement, Some(Status::Planned), false);
    let failed = mention(Measurement, Some(Status::Done), true);
    let open = mention(Issue, Some(Status::Open), false);
    let resolved = mention(Issue, Some(Status::Resolved), false);
    assert_eq!(worst_tone(&[note.clone(), calc.clone()]), BadgeTone::Notes);
    assert_eq!(worst_tone(&[note.clone(), passed.clone()]), BadgeTone::Pass);
    assert_eq!(
        worst_tone(&[passed.clone(), planned]),
        BadgeTone::Notes,
        "green only when every measurement passes"
    );
    assert_eq!(
        worst_tone(&[passed.clone(), open.clone()]),
        BadgeTone::OpenIssue
    );
    assert_eq!(worst_tone(&[open, failed]), BadgeTone::Failing);
    assert_eq!(worst_tone(&[resolved, passed]), BadgeTone::Pass);
    assert!(BadgeTone::Notes < BadgeTone::Pass);
    assert!(BadgeTone::OpenIssue < BadgeTone::Failing);
}

#[test]
fn a_block_puts_one_badge_on_each_part_and_net_it_names() {
    let (design, _) = scan(
        "#annotation(kind: \"issue\", status: \"open\", title: \"Warm\")[#component(\"U3\") and #pin(\"U3\", \"3\") on #net(\"VBUS\")]\n\
         #annotation(kind: \"measurement\", status: \"done\", title: \"Ripple\")[#component(\"C15\")]\n",
    );
    let (chapter, _) = scan("#annotation(kind: \"note\")[#component(\"C15\")]\n");
    let found = badges([
        ("design.snxnbk", design.as_slice()),
        ("chapters/power.snxnbk", chapter.as_slice()),
    ]);
    let targets: Vec<_> = found.iter().map(|b| b.target.clone()).collect();
    assert_eq!(
        targets,
        [
            BadgeTarget::Part("C15".into()),
            BadgeTarget::Part("U3".into()),
            BadgeTarget::Net("VBUS".into()),
        ]
    );
    let u3 = badge_of(&found, &BadgeTarget::Part("U3".into())).expect("U3");
    assert_eq!(u3.count(), 1, "a part named twice in one block counts once");
    assert_eq!(u3.tone, BadgeTone::OpenIssue);
    let c15 = badge_of(&found, &BadgeTarget::Part("C15".into())).expect("C15");
    assert_eq!(c15.count(), 2);
    assert_eq!(c15.tone, BadgeTone::Pass);
    assert_eq!(c15.blocks[0].title, "Ripple", "notebook order");
    assert_eq!(c15.blocks[1].file, "chapters/power.snxnbk");
    assert!(badges([("design.snxnbk", &[][..])]).is_empty());
}

#[test]
fn a_badge_reads_as_json_with_its_target_tone_and_blocks() {
    let (blocks, _) = scan(
        "#annotation(kind: \"measurement\", status: \"done\", date: \"2026-11-24\", title: \"Ripple\")[#component(\"C15\")]\n",
    );
    let found = badges([("design.snxnbk", blocks.as_slice())]);
    let json = serde_json::to_value(&found[0]).expect("json");
    assert_eq!(json["target"]["kind"], "part");
    assert_eq!(json["target"]["name"], "C15");
    assert_eq!(json["tone"], "pass");
    assert_eq!(json["blocks"][0]["kind"], "measurement");
    assert_eq!(json["blocks"][0]["status"], "done");
    assert_eq!(json["blocks"][0]["date"], "2026-11-24");
}
