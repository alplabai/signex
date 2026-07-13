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
    fn demote_part_pins_collapses_target_part() {
        let mut s = Symbol::empty("test");
        add_pin(&mut s, 0.0, 0.0, 2);
        add_pin(&mut s, 0.0, 0.0, 2);
        add_pin(&mut s, 0.0, 0.0, 3);
        demote_part_pins_to_part_one(&mut s, 2);
        let twos = s.pins.iter().filter(|p| p.part_number == 2).count();
        let ones = s.pins.iter().filter(|p| p.part_number == 1).count();
        assert_eq!(twos, 0);
        // 2 demoted from part 2, no default pin
        assert_eq!(ones, 2);
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
        let sel = hit_test(&s, 3.0, 4.0);
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
        });
        // BR corner is at (to.x, from.y) = (10.0, 0.0).
        let hit = hit_test_graphic_handle(&s, 10.0, 0.0, 1.5);
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
        });
        // No pins in empty symbol — graphic hit is unambiguous.
        let hit = hit_test(&s, 5.0, 2.5);
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
