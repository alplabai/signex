// Shared private unit-test definitions for Gerber grid persistence.
#![allow(unused_imports, unused_macros)]

macro_rules! gerber_grid_tests {
    () => {
        #[cfg(test)]
        mod tests {
            use super::*;

            #[test]
            fn bundled_catalog_has_the_required_order_and_labels() {
                let catalog = default_grid_catalog();
                let labels = catalog
                    .iter()
                    .map(|grid| grid.display_label("."))
                    .collect::<Vec<_>>();

                assert_eq!(catalog.len(), 22);
                assert_eq!(
                    labels,
                    [
                        "100.00 mils (2.5400 mm)",
                        "50.00 mils (1.2700 mm)",
                        "25.00 mils (0.6350 mm)",
                        "20.00 mils (0.5080 mm)",
                        "10.00 mils (0.2540 mm)",
                        "5.00 mils (0.1270 mm)",
                        "2.50 mils (0.0635 mm)",
                        "2.00 mils (0.0508 mm)",
                        "1.00 mils (0.0254 mm)",
                        "0.50 mils (0.0127 mm)",
                        "0.20 mils (0.0051 mm)",
                        "0.10 mils (0.0025 mm)",
                        "5.0000 mm (196.85 mils)",
                        "1.5000 mm ⨯ 2.5000 mm (59.06 mils ⨯ 98.43 mils)",
                        "1.0000 mm (39.37 mils)",
                        "0.5000 mm (19.69 mils)",
                        "0.2500 mm (9.84 mils)",
                        "0.2000 mm (7.87 mils)",
                        "0.1000 mm (3.94 mils)",
                        "0.0500 mm ⨯ 0.0000 mm (1.97 mils ⨯ 0.00 mils)",
                        "0.0250 mm ⨯ 0.0000 mm (0.98 mils ⨯ 0.00 mils)",
                        "0.0100 mm ⨯ 0.0000 mm (0.39 mils ⨯ 0.00 mils)",
                    ]
                );
            }

            #[test]
            fn metric_presets_keep_exact_metric_spacing() {
                let catalog = default_grid_catalog();
                let rectangular = &catalog[13];
                let x_only = &catalog[19];

                assert_eq!(rectangular.x_millimetres(), 1.5);
                assert_eq!(rectangular.y_millimetres(), 2.5);
                assert_eq!(x_only.x_millimetres(), 0.05);
                assert_eq!(x_only.y_millimetres(), 0.0);
            }

            #[test]
            fn labels_use_the_supplied_decimal_separator() {
                let catalog = default_grid_catalog();

                assert_eq!(catalog[0].display_label("."), "100.00 mils (2.5400 mm)");
                assert_eq!(
                    catalog[21].display_label("."),
                    "0.0100 mm ⨯ 0.0000 mm (0.39 mils ⨯ 0.00 mils)"
                );
                assert_eq!(
                    catalog[21].display_label(","),
                    "0,0100 mm ⨯ 0,0000 mm (0,39 mils ⨯ 0,00 mils)"
                );
            }

            #[test]
            fn operating_system_decimal_separator_is_available() {
                assert_eq!(DEFAULT_DECIMAL_SEPARATOR, ".");
                assert!(!system_decimal_separator().is_empty());
            }

            #[test]
            fn parses_linux_and_macos_locale_decimal_point_output() {
                assert_eq!(parse_posix_decimal_separator(b".\n"), Some(".".to_owned()));
                assert_eq!(
                    parse_posix_decimal_separator(b"\",\"\n"),
                    Some(",".to_owned())
                );
                assert_eq!(
                    parse_posix_decimal_separator(b"decimal_point=\",\" \n"),
                    Some(",".to_owned())
                );
            }

            #[test]
            fn rejects_unusable_locale_decimal_point_output() {
                assert_eq!(parse_posix_decimal_separator(b"\n"), None);
                assert_eq!(parse_posix_decimal_separator(b"\"\"\n"), None);
                assert_eq!(parse_posix_decimal_separator(&[0xff]), None);
            }

            #[test]
            fn creates_named_and_unnamed_grid_definitions() {
                let named = create_grid_definition(" Fine metric ", "0.05", "0", GridUnit::Mm, ".")
                    .expect("valid named metric grid");
                let unnamed = create_grid_definition("  ", "2.5", "2.5", GridUnit::Mil, ".")
                    .expect("valid unnamed mil grid");

                assert_eq!(named.name.as_deref(), Some("Fine metric"));
                assert_eq!(named.x, 0.05);
                assert_eq!(named.y, 0.0);
                assert_eq!(named.unit, GridUnit::Mm);
                assert_eq!(unnamed.name, None);
            }

            #[test]
            fn rejects_invalid_grid_distances() {
                for x in ["", "0", "-1", "NaN", "inf"] {
                    assert!(create_grid_definition("", x, "1", GridUnit::Mm, ".").is_err());
                }
                for y in ["", "-1", "NaN", "inf"] {
                    assert!(create_grid_definition("", "1", y, GridUnit::Mm, ".").is_err());
                }
            }

            #[test]
            fn persists_and_loads_the_ordered_grid_catalog() {
                let directory = tempfile::tempdir().expect("temporary settings directory");
                let path = directory.path().join("gerber_viewer.toml");
                let mut catalog = default_grid_catalog();
                catalog.push(
                    create_grid_definition("Assembly", "0.25", "0.5", GridUnit::Mm, ".")
                        .expect("valid custom grid"),
                );

                persist_grid_catalog_to(&path, &catalog).expect("grid catalog must persist");
                let loaded =
                    load_grid_catalog_from(&path).expect("persisted grid catalog must load");

                assert_eq!(loaded, catalog);
                assert_eq!(
                    loaded.last().and_then(|grid| grid.name.as_deref()),
                    Some("Assembly")
                );
            }

            #[test]
            fn persists_page_size_in_the_shared_gerber_settings_file() {
                let directory = tempfile::tempdir().expect("temporary settings directory");
                let path = directory.path().join("gerber_viewer.toml");
                let catalog = default_grid_catalog();

                persist_settings_to(&path, &catalog, GerberPageSize::A3, GerberGridStyle::Lines)
                    .expect("Gerber settings must persist");
                let loaded =
                    load_settings_from(&path).expect("persisted Gerber settings must load");

                assert_eq!(loaded.page_size.size, GerberPageSize::A3);
                assert_eq!(loaded.grid_display.style, GerberGridStyle::Lines);
                assert_eq!(loaded.grid_sizes, catalog);
            }
        }
    };
}

pub(crate) use gerber_grid_tests;
