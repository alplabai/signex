//! Tests for symbol-editor interaction state.
use super::*;
use signex_library::Symbol;

#[test]
fn add_pin_assigns_next_number() {
    let mut s = Symbol::empty("test");
    add_pin(&mut s, 0.0, 0.0, 1); // seed first pin so numbering starts at "1"
    let idx = add_pin(&mut s, 1.0, 1.0, 1);
    assert_eq!(idx, 1);
    assert_eq!(s.pins[1].number, "2");
}

#[test]
fn add_pin_records_active_part() {
    let mut s = Symbol::empty("test");
    let idx = add_pin(&mut s, 0.0, 0.0, 3);
    assert_eq!(s.pins[idx].part_number, 3);
}

#[test]
fn max_part_number_ignores_part_zero() {
    let mut s = Symbol::empty("test");
    add_pin(&mut s, 0.0, 0.0, 0); // shared
    add_pin(&mut s, 0.0, 0.0, 2);
    add_pin(&mut s, 0.0, 0.0, 4);
    assert_eq!(max_part_number(&s), 4);
}

#[test]
fn max_part_number_defaults_to_one() {
    let s = Symbol::empty("test");
    assert_eq!(max_part_number(&s), 1);
}

#[test]
fn delete_unit_removes_and_renumbers() {
    let mut s = Symbol::empty("test");
    add_pin(&mut s, 0.0, 0.0, 1); // pin "1" on part 1
    add_pin(&mut s, 1.0, 0.0, 2); // pin "2" on part 2 (deleted)
    add_pin(&mut s, 2.0, 0.0, 3); // pin "3" on part 3 (renumbers to 2)
    s.part_count = 3;

    let active = delete_unit(&mut s, 2);

    // Part 3 collapsed down — nothing sits on part 3 any more.
    assert!(s.pins.iter().all(|p| p.part_number != 3));
    // The pin originally scoped to the deleted part 2 is gone.
    assert!(!s.pins.iter().any(|p| p.number == "2"));
    // The pin that WAS on part 3 renumbered down to part 2.
    let renumbered = s
        .pins
        .iter()
        .find(|p| p.number == "3")
        .expect("pin 3 survives");
    assert_eq!(renumbered.part_number, 2);
    assert_eq!(s.part_count, 2);
    assert_eq!(active, 2);
}

#[test]
fn delete_unit_out_of_range_leaves_count_unchanged() {
    // Regression: a stale active_part (e.g. after undoing a New
    // Part) must not silently drop a unit. delete_unit with a part
    // greater than the count is a no-op on part_count.
    let mut s = Symbol::empty("test");
    add_pin(&mut s, 0.0, 0.0, 1); // populated part 1
    s.part_count = 2; // an empty part 2 exists (no pins on it)

    let active = delete_unit(&mut s, 3); // 3 > count (2)

    assert_eq!(s.part_count, 2, "empty unit must not be dropped");
    assert_eq!(active, 2, "active clamps to the top valid unit");
}

#[test]
fn delete_pin_clears_selection_via_return() {
    let mut s = Symbol::empty("test");
    add_pin(&mut s, 0.0, 0.0, 1); // first pin
    add_pin(&mut s, 1.0, 1.0, 1); // second pin
    let new_sel = delete_selected(&mut s, Some(SymbolSelection::Pin(0)));
    assert_eq!(new_sel, Some(None));
    assert_eq!(s.pins.len(), 1);
}

#[test]
fn move_selected_updates_position() {
    let mut s = Symbol::empty("test");
    add_pin(&mut s, 0.0, 0.0, 1);
    move_selected(&mut s, Some(SymbolSelection::Pin(0)), 5.5, -2.0);
    assert_eq!(s.pins[0].position, [5.5, -2.0]);
}

#[test]
fn hit_test_returns_pin() {
    let mut s = Symbol::empty("test");
    add_pin(&mut s, 3.0, 4.0, 1);
    let sel = hit_test(&s, 3.0, 4.0, 1);
    assert_eq!(sel, Some(SymbolSelection::Pin(0)));
}

#[test]
fn graphic_handle_position_returns_rectangle_corners() {
    let mut s = Symbol::empty("test");
    s.graphics.push(signex_library::SymbolGraphic {
        kind: SymbolGraphicKind::Rectangle {
            from: [-2.0, -1.0],
            to: [2.0, 1.0],
        },
        stroke_width: 0.15,
        part_number: 0,
        fill: None,
    });
    // TL = (from.x, to.y), BR = (to.x, from.y)
    assert_eq!(
        graphic_handle_position(&s, 0, GraphicHandle::RectCorner(0)),
        Some([-2.0, 1.0])
    );
    assert_eq!(
        graphic_handle_position(&s, 0, GraphicHandle::RectCorner(2)),
        Some([2.0, -1.0])
    );
}

#[test]
fn hit_test_graphic_handle_finds_rectangle_corner() {
    let mut s = Symbol::empty("test");
    s.graphics.push(signex_library::SymbolGraphic {
        kind: SymbolGraphicKind::Rectangle {
            from: [0.0, 0.0],
            to: [10.0, 5.0],
        },
        stroke_width: 0.15,
        part_number: 0,
        fill: None,
    });
    // BR corner is at (to.x, from.y) = (10.0, 0.0).
    let hit = hit_test_graphic_handle(&s, 10.0, 0.0, 1.5, 1);
    assert_eq!(hit, Some((0, GraphicHandle::RectCorner(2))));
}

#[test]
fn move_graphic_handle_moves_line_endpoint() {
    let mut s = Symbol::empty("test");
    s.graphics.push(signex_library::SymbolGraphic {
        kind: SymbolGraphicKind::Line {
            from: [0.0, 0.0],
            to: [5.0, 0.0],
        },
        stroke_width: 0.15,
        part_number: 0,
        fill: None,
    });
    move_graphic_handle(&mut s, 0, GraphicHandle::LineEndpoint(1), 7.0, 3.0);
    match &s.graphics[0].kind {
        SymbolGraphicKind::Line { to, .. } => assert_eq!(*to, [7.0, 3.0]),
        _ => panic!("expected Line"),
    }
}

#[test]
fn move_graphic_handle_resizes_circle_radius() {
    let mut s = Symbol::empty("test");
    s.graphics.push(signex_library::SymbolGraphic {
        kind: SymbolGraphicKind::Circle {
            center: [0.0, 0.0],
            radius: 1.0,
        },
        stroke_width: 0.15,
        part_number: 0,
        fill: None,
    });
    move_graphic_handle(&mut s, 0, GraphicHandle::CircleRadius, 3.0, 4.0);
    match &s.graphics[0].kind {
        SymbolGraphicKind::Circle { radius, .. } => assert!((*radius - 5.0).abs() < 1e-9),
        _ => panic!("expected Circle"),
    }
}

#[test]
fn hit_test_returns_graphic_inside_rectangle() {
    let mut s = Symbol::empty("test");
    s.graphics.push(signex_library::SymbolGraphic {
        kind: SymbolGraphicKind::Rectangle {
            from: [0.0, 0.0],
            to: [10.0, 5.0],
        },
        stroke_width: 0.15,
        part_number: 0,
        fill: None,
    });
    // No pins in empty symbol — graphic hit is unambiguous.
    let hit = hit_test(&s, 5.0, 2.5, 1);
    assert_eq!(hit, Some(SymbolSelection::Graphic(0)));
}

#[test]
fn move_selected_translates_rectangle_by_anchor_delta() {
    let mut s = Symbol::empty("test");
    s.graphics.push(signex_library::SymbolGraphic {
        kind: SymbolGraphicKind::Rectangle {
            from: [0.0, 0.0],
            to: [10.0, 5.0],
        },
        stroke_width: 0.15,
        part_number: 0,
        fill: None,
    });
    move_selected(&mut s, Some(SymbolSelection::Graphic(0)), 3.0, 7.0);
    match &s.graphics[0].kind {
        SymbolGraphicKind::Rectangle { from, to } => {
            assert_eq!(*from, [3.0, 7.0]);
            assert_eq!(*to, [13.0, 12.0]);
        }
        _ => panic!("expected Rectangle"),
    }
}

#[test]
fn rotate_selected_rotates_rectangle_clockwise_around_origin() {
    let mut s = Symbol::empty("test");
    s.graphics.push(signex_library::SymbolGraphic {
        kind: SymbolGraphicKind::Rectangle {
            from: [1.0, 2.0],
            to: [3.0, 4.0],
        },
        stroke_width: 0.15,
        part_number: 0,
        fill: None,
    });

    rotate_selected(&mut s, Some(SymbolSelection::Graphic(0)), true);

    match &s.graphics[0].kind {
        SymbolGraphicKind::Rectangle { from, to } => {
            assert_eq!(*from, [2.0, -1.0]);
            assert_eq!(*to, [4.0, -3.0]);
        }
        _ => panic!("expected Rectangle"),
    }
}

#[test]
fn rotate_selected_rotates_pin_orientation_in_place() {
    let mut s = Symbol::empty("test");
    let idx = add_pin(&mut s, 2.0, 1.0, 1);
    s.pins[idx].orientation = PinOrientation::Right;

    rotate_selected(&mut s, Some(SymbolSelection::Pin(idx)), false);

    // Body-end (pivot) was at (2.0 + 2.54, 1.0) = (4.54, 1.0).
    // Tip orbits around it CCW by 90°: new tip = (4.54, -1.54).
    assert_eq!(s.pins[idx].position, [4.54, -1.54]);
    assert_eq!(s.pins[idx].orientation, PinOrientation::Up);
}

#[test]
fn rotate_selected_about_geometry_center_keeps_rectangle_center() {
    let mut s = Symbol::empty("test");
    s.graphics.push(signex_library::SymbolGraphic {
        kind: SymbolGraphicKind::Rectangle {
            from: [1.0, 2.0],
            to: [3.0, 4.0],
        },
        stroke_width: 0.15,
        part_number: 0,
        fill: None,
    });

    rotate_selected_with_pivot(
        &mut s,
        Some(SymbolSelection::Graphic(0)),
        true,
        GraphicRotationPivotMode::GeometryCenter,
    );

    match &s.graphics[0].kind {
        SymbolGraphicKind::Rectangle { from, to } => {
            assert_eq!(*from, [1.0, 4.0]);
            assert_eq!(*to, [3.0, 2.0]);
        }
        _ => panic!("expected Rectangle"),
    }
}

#[test]
fn rotate_selected_about_geometry_center_keeps_text_anchor_fixed() {
    let mut s = Symbol::empty("test");
    s.graphics.push(signex_library::SymbolGraphic {
        kind: SymbolGraphicKind::Text {
            position: [5.0, -7.0],
            content: "R".into(),
            size: 1.0,
        },
        stroke_width: 0.15,
        part_number: 0,
        fill: None,
    });

    rotate_selected_about_geometry_center(&mut s, Some(SymbolSelection::Graphic(0)), false);

    match &s.graphics[0].kind {
        SymbolGraphicKind::Text { position, .. } => {
            assert_eq!(*position, [5.0, -7.0]);
        }
        _ => panic!("expected Text"),
    }
}

#[test]
fn delete_selected_removes_graphic() {
    let mut s = Symbol::empty("test");
    s.graphics.push(signex_library::SymbolGraphic {
        kind: SymbolGraphicKind::Circle {
            center: [0.0, 0.0],
            radius: 1.0,
        },
        stroke_width: 0.15,
        part_number: 0,
        fill: None,
    });
    let new_sel = delete_selected(&mut s, Some(SymbolSelection::Graphic(0)));
    assert_eq!(new_sel, Some(None));
    assert!(s.graphics.is_empty());
}

#[test]
fn move_graphic_handle_no_op_for_mismatched_variant() {
    let mut s = Symbol::empty("test");
    s.graphics.push(signex_library::SymbolGraphic {
        kind: SymbolGraphicKind::Line {
            from: [0.0, 0.0],
            to: [5.0, 0.0],
        },
        stroke_width: 0.15,
        part_number: 0,
        fill: None,
    });
    // Asking to move a rectangle corner on a Line — should silently no-op.
    move_graphic_handle(&mut s, 0, GraphicHandle::RectCorner(0), 99.0, 99.0);
    match &s.graphics[0].kind {
        SymbolGraphicKind::Line { from, to } => {
            assert_eq!(*from, [0.0, 0.0]);
            assert_eq!(*to, [5.0, 0.0]);
        }
        _ => panic!("expected Line"),
    }
}

#[test]
fn graphic_on_part_shared_and_scoped() {
    let mut s = Symbol::empty("test");
    // Shared graphic (part 0) — visible on every unit.
    s.graphics.push(signex_library::SymbolGraphic {
        kind: SymbolGraphicKind::Rectangle {
            from: [0.0, 0.0],
            to: [1.0, 1.0],
        },
        stroke_width: 0.15,
        part_number: 0,
        fill: None,
    });
    // Graphic scoped to unit 2.
    s.graphics.push(signex_library::SymbolGraphic {
        kind: SymbolGraphicKind::Rectangle {
            from: [0.0, 0.0],
            to: [1.0, 1.0],
        },
        stroke_width: 0.15,
        part_number: 2,
        fill: None,
    });

    let shared = &s.graphics[0];
    let scoped = &s.graphics[1];

    // Part 0 is visible regardless of the active unit.
    assert!(graphic_on_part(shared, 1));
    assert!(graphic_on_part(shared, 2));
    assert!(graphic_on_part(shared, 5));

    // A scoped graphic is visible only on its own unit.
    assert!(graphic_on_part(scoped, 2));
    assert!(!graphic_on_part(scoped, 1));
    assert!(!graphic_on_part(scoped, 3));
}

#[test]
fn hit_test_respects_active_part() {
    let mut s = Symbol::empty("test");
    // A rectangle scoped to unit 2, covering the point (5.0, 2.5).
    s.graphics.push(signex_library::SymbolGraphic {
        kind: SymbolGraphicKind::Rectangle {
            from: [0.0, 0.0],
            to: [10.0, 5.0],
        },
        stroke_width: 0.15,
        part_number: 2,
        fill: None,
    });
    // Hidden on unit 1 — nothing under the point.
    assert_eq!(hit_test(&s, 5.0, 2.5, 1), None);
    // Visible on unit 2 — the graphic is picked up.
    assert!(matches!(
        hit_test(&s, 5.0, 2.5, 2),
        Some(SymbolSelection::Graphic(_))
    ));

    // A shared (part 0) rectangle is hittable on any active unit.
    let mut shared = Symbol::empty("test");
    shared.graphics.push(signex_library::SymbolGraphic {
        kind: SymbolGraphicKind::Rectangle {
            from: [0.0, 0.0],
            to: [10.0, 5.0],
        },
        stroke_width: 0.15,
        part_number: 0,
        fill: None,
    });
    assert!(matches!(
        hit_test(&shared, 5.0, 2.5, 1),
        Some(SymbolSelection::Graphic(_))
    ));
    assert!(matches!(
        hit_test(&shared, 5.0, 2.5, 7),
        Some(SymbolSelection::Graphic(_))
    ));
}

// --- Phase C2 regression tests ------------------------------------------

#[test]
fn delete_unit_prunes_and_renumbers_graphics() {
    let mut s = Symbol::empty("test");
    // Shared body geometry (part 0) — must survive untouched.
    s.graphics.push(signex_library::SymbolGraphic {
        kind: SymbolGraphicKind::Rectangle {
            from: [0.0, 0.0],
            to: [1.0, 1.0],
        },
        stroke_width: 0.15,
        part_number: 0,
        fill: None,
    });
    // Distinct `from` per unit so we can identify which rectangle survived.
    s.graphics.push(signex_library::SymbolGraphic {
        kind: SymbolGraphicKind::Rectangle {
            from: [10.0, 10.0],
            to: [11.0, 11.0],
        },
        stroke_width: 0.15,
        part_number: 1,
        fill: None,
    });
    s.graphics.push(signex_library::SymbolGraphic {
        kind: SymbolGraphicKind::Rectangle {
            from: [20.0, 20.0],
            to: [21.0, 21.0],
        },
        stroke_width: 0.15,
        part_number: 2,
        fill: None,
    });
    s.graphics.push(signex_library::SymbolGraphic {
        kind: SymbolGraphicKind::Rectangle {
            from: [30.0, 30.0],
            to: [31.0, 31.0],
        },
        stroke_width: 0.15,
        part_number: 3,
        fill: None,
    });
    s.part_count = 3;

    delete_unit(&mut s, 2);

    // Part 3 collapsed down — no graphic sits on part 3 any more.
    assert!(s.graphics.iter().all(|g| g.part_number != 3));
    // The shared graphic (part 0) is left alone — exactly one remains.
    assert_eq!(s.graphics.iter().filter(|g| g.part_number == 0).count(), 1);
    // The graphic that WAS on part 3 renumbered down to part 2 — identify
    // it by the `from` coordinate that was unique to the old part 3.
    let renumbered = s
        .graphics
        .iter()
        .find(|g| g.part_number == 2)
        .expect("part 3 graphic renumbers down to part 2");
    match &renumbered.kind {
        SymbolGraphicKind::Rectangle { from, .. } => assert_eq!(*from, [30.0, 30.0]),
        _ => panic!("expected Rectangle"),
    }
    // The graphic originally scoped to the deleted part 2 is gone.
    assert!(!s.graphics.iter().any(|g| matches!(
        &g.kind,
        SymbolGraphicKind::Rectangle { from, .. } if *from == [20.0, 20.0]
    )));
    assert_eq!(s.part_count, 2);
}

#[test]
fn hit_test_ignores_other_unit_pin() {
    let mut s = Symbol::empty("test");
    add_pin(&mut s, 3.0, 3.0, 2); // pin scoped to unit 2

    // Hidden while unit 1 is active — nothing under the cursor.
    assert_eq!(hit_test(&s, 3.0, 3.0, 1), None);
    // Visible on its own unit — the pin is picked up.
    assert!(matches!(
        hit_test(&s, 3.0, 3.0, 2),
        Some(SymbolSelection::Pin(_))
    ));

    // A Part-Zero pin is hittable on any active unit.
    add_pin(&mut s, -3.0, -3.0, 0);
    assert!(matches!(
        hit_test(&s, -3.0, -3.0, 1),
        Some(SymbolSelection::Pin(_))
    ));
}

#[test]
fn select_in_box_all_uses_visible_counts() {
    let mut s = Symbol::empty("test");
    // One pin and one rectangle visible on unit 1.
    add_pin(&mut s, 5.0, 5.0, 1);
    s.graphics.push(signex_library::SymbolGraphic {
        kind: SymbolGraphicKind::Rectangle {
            from: [0.0, 0.0],
            to: [10.0, 10.0],
        },
        stroke_width: 0.15,
        part_number: 1,
        fill: None,
    });
    // A unit-2 rectangle far away — invisible while unit 1 is active.
    s.graphics.push(signex_library::SymbolGraphic {
        kind: SymbolGraphicKind::Rectangle {
            from: [100.0, 100.0],
            to: [101.0, 101.0],
        },
        stroke_width: 0.15,
        part_number: 2,
        fill: None,
    });
    s.part_count = 2;

    // A box enclosing everything VISIBLE on unit 1 (but not the unit-2
    // rectangle) resolves to `All` — the count is against visible items,
    // not the unfiltered whole-symbol total.
    assert_eq!(
        select_in_box(&s, -1.0, -1.0, 11.0, 11.0, BoxSelectKind::Window, 1),
        Some(SymbolSelection::All)
    );

    // A tight box that fully contains only the pin (not the whole
    // rectangle) is a partial selection — `Multiple`, never `All`.
    match select_in_box(&s, 4.0, 4.0, 6.0, 6.0, BoxSelectKind::Window, 1) {
        Some(SymbolSelection::Multiple {
            pin_indices,
            graphic_indices,
        }) => {
            assert_eq!(pin_indices, vec![0]);
            assert!(graphic_indices.is_empty());
        }
        other => panic!("expected Multiple, got {other:?}"),
    }
}

