// Shared private unit-test definitions for layer comparison.
#![allow(unused_imports, unused_macros)]

macro_rules! gerber_compare_mode_tests {
    () => {
        #[cfg(test)]
        mod compare_mode_tests {
            use super::*;

            #[test]
            fn compare_colors_are_deterministic_and_make_overlap_distinct() {
                let palette = material_compare_palette();
                let original = Color::from_rgb8(211, 47, 47);
                let first = compare_layer_color(original, 0, 2, true, &palette);
                let second = compare_layer_color(original, 1, 2, true, &palette);
                let overlap = composite_compare_colors(second, first);

                assert_ne!(first, second);
                assert_ne!(overlap, first);
                assert_ne!(overlap, second);
                assert_eq!(first, compare_layer_color(original, 0, 2, true, &palette));
            }

            #[test]
            fn normal_or_single_visible_layer_preserves_original_color() {
                let palette = material_compare_palette();
                let original = Color::from_rgb8(211, 47, 47);

                assert_eq!(
                    compare_layer_color(original, 0, 3, false, &palette),
                    original
                );
                assert_eq!(
                    compare_layer_color(original, 0, 1, true, &palette),
                    original
                );
                assert_eq!(compare_layer_color(original, 0, 2, true, &[]), original);
            }

            #[test]
            fn toggle_changes_only_compare_state_and_redraw_generation() {
                let mut state = GerberViewerState::default();
                let generation = state.redraw_generation;
                let palette = state.palette.clone();

                state.toggle_compare_mode();

                assert!(state.compare_mode);
                assert_eq!(state.redraw_generation, generation + 1);
                assert_eq!(state.palette, palette);

                state.toggle_compare_mode();

                assert!(!state.compare_mode);
            }
        }
    };
}

pub(crate) use gerber_compare_mode_tests;
