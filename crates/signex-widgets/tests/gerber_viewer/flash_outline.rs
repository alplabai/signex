// Shared private unit-test definitions for flashed-item outline mode.
#![allow(unused_imports, unused_macros)]

macro_rules! gerber_flash_outline_tests {
    () => {
        #[cfg(test)]
        mod flash_outline_tests {
            use super::*;

            #[test]
            fn toggle_switches_only_flashed_item_render_mode() {
                let mut state = GerberViewerState::default();
                let generation = state.redraw_generation;

                assert_eq!(
                    flash_render_mode(state.sketch_flashes),
                    FlashRenderMode::Filled
                );

                state.toggle_sketch_flashes();

                assert!(state.sketch_flashes);
                assert_eq!(
                    flash_render_mode(state.sketch_flashes),
                    FlashRenderMode::Outline
                );
                assert_eq!(state.redraw_generation, generation + 1);

                state.toggle_sketch_flashes();

                assert!(!state.sketch_flashes);
                assert_eq!(
                    flash_render_mode(state.sketch_flashes),
                    FlashRenderMode::Filled
                );
            }
        }
    };
}

pub(crate) use gerber_flash_outline_tests;
