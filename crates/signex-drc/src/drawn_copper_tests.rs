//! #2883: a string or drawing on copper is copper of no net; the check
//! measures it against every net's copper.

use pcb_geom::{ItemRef, PickKind};
use signex_types::pcb::{BoardGraphic, BoardText, Point};

use super::tests::{board, id, track};
use super::*;

const MM: i64 = 1_000_000;

fn string(layer: &str, y: f64) -> BoardText {
    BoardText {
        uuid: id(50),
        text: "A1".into(),
        position: Point::new(5.0, y),
        layer: layer.into(),
        font_size: 1.0,
        rotation: 0.0,
    }
}

fn text_findings(report: &DrcReport) -> Vec<&Violation> {
    report
        .violations
        .iter()
        .filter(|v| v.items.contains(&ItemRef::Text(id(50))))
        .collect()
}

#[test]
fn copper_text_too_near_a_track_is_a_clearance_violation() {
    let rules = DrcRules::new(MM);
    let mut b = board(
        vec![],
        vec![track(1, (0.0, 0.0), (10.0, 0.0), 1, "TopCopper")],
        vec![],
    );
    b.texts.push(string("TopCopper", 1.0));
    let report = run(&b, &rules);
    let found = text_findings(&report);
    assert_eq!(found.len(), 1, "one finding for the pair: {found:?}");
    let v = found[0];
    assert_eq!(v.kind, ViolationKind::Clearance);
    assert_eq!(v.items, [ItemRef::Track(id(2001)), ItemRef::Text(id(50))]);
    assert_eq!(v.nets, (1, 0), "the text has no net");
    assert!(v.measured_nm < MM && v.required_nm == MM);

    b.texts[0].position.y = 0.0;
    let report = run(&b, &rules);
    assert_eq!(text_findings(&report)[0].kind, ViolationKind::Short);

    b.texts[0].position.y = 10.0;
    assert!(text_findings(&run(&b, &rules)).is_empty(), "far enough off");

    b.texts[0] = string("TopSilkscreen", 1.0);
    let report = run(&b, &rules);
    assert!(
        !report
            .violations
            .iter()
            .any(|v| v.kind == ViolationKind::Clearance && v.items.contains(&ItemRef::Text(id(50)))),
        "silkscreen text is not copper"
    );
}

#[test]
fn a_copper_rectangle_across_a_track_is_a_short() {
    let mut b = board(
        vec![],
        vec![track(1, (0.0, 0.0), (10.0, 0.0), 1, "TopCopper")],
        vec![],
    );
    b.graphics.push(BoardGraphic {
        uuid: id(60),
        graphic_type: "rect".into(),
        layer: "TopCopper".into(),
        width: 0.2,
        start: Some(Point::new(4.0, -1.0)),
        end: Some(Point::new(6.0, 1.0)),
        center: None,
        radius: 0.0,
        points: Vec::new(),
        text: String::new(),
        font_size: 0.0,
    });
    let report = run(&b, &DrcRules::new(200_000));
    let rect = ItemRef::Graphic {
        graphic: id(60),
        kind: PickKind::Other,
    };
    let found: Vec<_> = report
        .violations
        .iter()
        .filter(|v| v.items.contains(&rect))
        .collect();
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].kind, ViolationKind::Short);
    assert_eq!(found[0].nets, (1, 0));
}
