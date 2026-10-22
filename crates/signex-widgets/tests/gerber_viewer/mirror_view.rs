// Shared private unit-test definitions for mirrored Gerber display.
#![allow(unused_imports, unused_macros)]

macro_rules! gerber_mirror_view_tests {
    () => {
        #[cfg(test)]
        mod mirror_view_tests {
            use super::*;

            #[test]
            fn mirror_reflects_horizontal_projection_about_view_center() {
                let world = signex_gerber::Point { x: 12.0, y: 23.0 };
                let world_center = Point::new(10.0, 20.0);
                let screen_center = Point::new(100.0, 200.0);

                assert_eq!(
                    world_to_screen_point(world, world_center, screen_center, 10.0, false,),
                    Point::new(120.0, 170.0)
                );
                assert_eq!(
                    world_to_screen_point(world, world_center, screen_center, 10.0, true,),
                    Point::new(80.0, 170.0)
                );
            }

            #[test]
            fn mirrored_projection_round_trips_for_picking_and_measurement() {
                let world = signex_gerber::Point { x: -4.5, y: 8.25 };
                let world_center = Point::new(1.0, 2.0);
                let screen_center = Point::new(320.0, 240.0);
                let screen = world_to_screen_point(world, world_center, screen_center, 17.0, true);

                let projected =
                    screen_to_world_point(screen, world_center, screen_center, 17.0, true);

                assert!((projected.x - world.x).abs() < 0.000_01);
                assert!((projected.y - world.y).abs() < 0.000_01);
            }

            #[test]
            fn toggle_preserves_navigation_and_requests_redraw() {
                let mut state = GerberViewerState::default();
                state.zoom = 2.5;
                state.pan = iced::Vector::new(12.0, -7.0);
                let generation = state.redraw_generation;

                state.toggle_mirrored();

                assert!(state.mirrored);
                assert_eq!(state.zoom, 2.5);
                assert_eq!(state.pan, iced::Vector::new(12.0, -7.0));
                assert_eq!(state.redraw_generation, generation + 1);
            }
        }
    };
}

pub(crate) use gerber_mirror_view_tests;
