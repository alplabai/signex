// Shared private unit-test definitions for the Gerber viewer icon rail.
#![allow(unused_imports, unused_macros)]

macro_rules! gerber_toolbar_tests {
    () => {
        #[cfg(test)]
        mod toolbar_tests {
            use super::*;

            #[test]
            fn toolbar_has_one_button_for_every_requested_control() {
                assert_eq!(toolbar::TOOLBAR_ICON_COUNT, 15);
            }

            #[test]
            fn toolbar_icons_are_black_paths_on_transparent_backgrounds() {
                for asset in toolbar::TOOLBAR_ICON_ASSETS {
                    let source = std::str::from_utf8(asset).expect("toolbar SVG must be UTF-8");

                    assert!(source.starts_with("<svg "));
                    assert!(source.contains("<path "));
                    assert!(source.contains("fill=\"#000000\""));
                    assert!(!source.contains("<rect"));
                    assert!(!source.contains("<circle"));
                    assert!(!source.contains("<image"));
                    assert!(!source.contains("background"));
                }
            }
        }
    };
}

pub(crate) use gerber_toolbar_tests;
