// Shared private unit-test definitions for the Gerber viewer.
#![allow(unused_imports, unused_macros)]

macro_rules! gerber_layer_state_tests
{
    () => {
        #[cfg(test)]
        mod layer_state_tests
        {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn layer_limit_accepts_32_mixed_layers_and_reports_overflow()
    {
        let mut state = GerberViewerState::default();
        let gerber = signex_gerber::load_gerber_reader(
            "copper.gbr",
            Cursor::new(
                b"%FSLAX46Y46*%\n%MOMM*%\n%ADD10C,1.000*%\nD10*\nX0Y0D03*\nM02*\n",
            ),
        )
        .expect("test Gerber must parse");
        let drill = signex_gerber::load_excellon_reader(
            "holes.drl",
            Cursor::new(
                b"M48\nMETRIC\nT01C0.8\n%\nG05\nT01\nX1.0Y1.0\nM30\n",
            ),
        )
        .expect("test Excellon must parse");
        let mut mixed = Vec::new();
        for _ in 0..16
        {
            mixed.push(gerber.clone());
            mixed.push(drill.clone());
        }

        state.apply_load_batch(GerberLoadBatch {
            layers: mixed,
            failures: Vec::new(),
        });
        assert_eq!(state.layers.len(), MAX_VIEWER_LAYERS);
        assert_eq!(
            state
                .layers
                .iter()
                .filter(|layer| layer.layer.layer_type == signex_gerber::LayerType::Drill)
                .count(),
            16
        );

        state.apply_load_batch(GerberLoadBatch {
            layers: vec![gerber],
            failures: Vec::new(),
        });
        assert_eq!(state.layers.len(), MAX_VIEWER_LAYERS);
        assert!(state.status.contains("1 layer(s) were skipped"));
    }

    #[test]
    fn autodetected_mixed_batch_adds_each_successful_format_as_a_layer()
    {
        let gerber = signex_gerber::load_autodetected_reader(
            "artwork.data",
            Cursor::new(
                b"%FSLAX46Y46*%\n%MOMM*%\n%ADD10C,0.5*%\nD10*\nX0Y0D03*\nM02*\n",
            ),
        )
        .expect("Gerber signature");
        let drill = signex_gerber::load_autodetected_reader(
            "drill.data",
            Cursor::new(b"M48\nMETRIC\nT01C0.8\n%\nG05\nT01\nX1.0Y1.0\nM30\n"),
        )
        .expect("Excellon signature");
        let mut state = GerberViewerState::default();

        state.apply_load_batch(GerberLoadBatch {
            layers: vec![gerber, drill],
            failures: vec![signex_gerber::GerberLoadFailure {
                path: PathBuf::from("notes.txt"),
                message: "unsupported fabrication file".to_owned(),
            }],
        });

        assert_eq!(state.layers.len(), 2);
        assert_eq!(state.layers[0].layer.name, "artwork.data");
        assert_eq!(
            state.layers[1].layer.layer_type,
            signex_gerber::LayerType::Drill,
        );
        assert!(state.status.contains("Loaded 2 fabrication layer(s)."));
        assert!(state.status.contains("1 file(s) could not be loaded"));
    }

    #[test]
    fn toggling_layer_manager_preserves_viewer_state()
    {
        let mut state = GerberViewerState::default();
        state.zoom = 2.0;
        state.active_layer = None;

        state.toggle_layer_manager();

        assert!(!state.layer_manager_visible);
        assert_eq!(state.zoom, 2.0);
        assert!(state.layers.is_empty());

        state.toggle_layer_manager();
        assert!(state.layer_manager_visible);
    }

    #[test]
    fn default_dock_contains_document_and_all_tool_tabs()
    {
        let workspace = GerberWorkspaceState::default();
        let document_id = workspace.active_document_id();
        let panel_ids = workspace.dock.panel_ids();

        for expected in [
            GerberDockPanel::Document(document_id),
            GerberDockPanel::Layers,
            GerberDockPanel::Highlight,
            GerberDockPanel::Grid,
            GerberDockPanel::LayerInformation,
            GerberDockPanel::DCodes,
            GerberDockPanel::Source,
        ]
        {
            assert!(
                panel_ids.contains(&expected.id()),
                "missing dock panel {}",
                expected.id(),
            );
        }
    }

    #[test]
    fn closing_a_dock_tab_updates_menu_state_and_allows_reopening()
    {
        let mut workspace = GerberWorkspaceState::default();
        workspace.dock.close_tool(GerberDockPanel::DCodes);

        workspace.handle_dock_event(&iced_dock::DockEvent::TabClosed {
            panel: GerberDockPanel::DCodes,
        });

        assert!(!workspace
            .active_viewer()
            .expect("active Gerber document")
            .is_tool_panel_visible(GerberDockPanel::DCodes));
        workspace.set_tool_visible(GerberDockPanel::DCodes, true);
        assert!(workspace
            .active_viewer()
            .expect("active Gerber document")
            .is_tool_panel_visible(GerberDockPanel::DCodes));
        assert!(
            workspace
                .dock
                .panel_ids()
                .contains(&GerberDockPanel::DCodes.id()),
        );
    }

    #[test]
    fn highlight_and_grid_tabs_can_be_closed_and_reopened()
    {
        let mut workspace = GerberWorkspaceState::default();

        for panel in [GerberDockPanel::Highlight, GerberDockPanel::Grid]
        {
            workspace.dock.close_tool(panel);
            workspace.handle_dock_event(&iced_dock::DockEvent::TabClosed {
                panel,
            });

            assert!(!workspace
                .active_viewer()
                .expect("active Gerber document")
                .is_tool_panel_visible(panel));

            workspace.set_tool_visible(panel, true);

            assert!(workspace
                .active_viewer()
                .expect("active Gerber document")
                .is_tool_panel_visible(panel));
            assert!(workspace.dock.panel_ids().contains(&panel.id()));
        }
    }

    #[test]
    fn document_tabs_have_stable_identity_and_keep_one_document_open()
    {
        let mut workspace = GerberWorkspaceState::default();
        let first = workspace.active_document_id();
        let second = workspace.new_document();

        assert_ne!(first, second);
        assert_eq!(workspace.documents().len(), 2);
        assert_eq!(workspace.active_document_id(), second);

        workspace.handle_dock_event(&iced_dock::DockEvent::TabClosed {
            panel: GerberDockPanel::Document(second),
        });
        assert_eq!(workspace.documents().len(), 1);
        assert_eq!(workspace.active_document_id(), first);

        workspace.handle_dock_event(&iced_dock::DockEvent::TabClosed {
            panel: GerberDockPanel::Document(first),
        });
        assert_eq!(workspace.documents().len(), 1);
        assert_ne!(workspace.active_document_id(), first);
    }

    #[test]
    fn layer_information_handles_missing_and_active_layers()
    {
        let mut state = GerberViewerState::default();

        assert_eq!(state.active_layer_metadata(), None);
        assert!(state.layer_information_visible);
        state.toggle_layer_information();
        assert!(!state.layer_information_visible);
        state.toggle_layer_information();
        assert!(state.layer_information_visible);
        assert!(state.layer_manager_visible);

        let layer = signex_gerber::load_gerber_reader(
            "copper.gbr",
            Cursor::new(
                b"%FSLAX46Y46*%\n%MOMM*%\n%ADD10C,1.000*%\nD10*\nX0Y0D03*\nM02*\n",
            ),
        )
        .expect("test Gerber must parse");
        state.apply_load_batch(GerberLoadBatch {
            layers: vec![layer],
            failures: Vec::new(),
        });

        let metadata = state
            .active_layer_metadata()
            .expect("loaded active layer metadata");
        assert_eq!(metadata.file_name, "copper.gbr");
        assert_eq!(metadata.format, "Gerber RS-274X");
        assert_eq!(metadata.definition_label, "Apertures");
    }

    #[test]
    fn d_code_list_groups_all_loaded_layers_and_drill_tools()
    {
        let gerber = signex_gerber::load_gerber_reader(
            "copper.gbr",
            Cursor::new(
                b"%FSLAX46Y46*%\n%MOMM*%\n%ADD10C,1.000*%\nD10*\nX0Y0D03*\nM02*\n",
            ),
        )
        .expect("test Gerber must parse");
        let drill = signex_gerber::load_excellon_reader(
            "holes.drl",
            Cursor::new(
                b"M48\nMETRIC\nT01C0.8\n%\nG05\nT01\nX1.0Y1.0\nM30\n",
            ),
        )
        .expect("test Excellon must parse");
        let mut state = GerberViewerState::default();
        state.apply_load_batch(GerberLoadBatch {
            layers: vec![gerber, drill],
            failures: Vec::new(),
        });

        state.toggle_d_code_list();
        assert!(!state.d_code_list_visible);
        assert!(state.layer_information_visible);
        state.toggle_d_code_list();
        let groups = state.definition_groups();

        assert!(state.d_code_list_visible);
        assert!(state.layer_information_visible);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].definition_label, "D-codes");
        assert_eq!(groups[0].definitions[0].code, "D10");
        assert_eq!(groups[0].definitions[0].usage_count, 1);
        assert_eq!(groups[1].definition_label, "Drill tools");
        assert_eq!(groups[1].definitions[0].code, "T1");
        assert_eq!(groups[1].definitions[0].usage_count, 1);

        state.toggle_layer_information();
        assert!(!state.layer_information_visible);
        assert!(state.d_code_list_visible);
    }

    #[test]
    fn source_view_preserves_original_text_and_handles_drill_layers()
    {
        let source = "%FSLAX46Y46*%\r\n%MOMM*%\r\nG04 Keep me *\r\nM02*\r\n";
        let gerber = signex_gerber::load_gerber_reader(
            "source.gbr",
            Cursor::new(source.as_bytes()),
        )
        .expect("test Gerber must parse");
        let drill = signex_gerber::load_excellon_reader(
            "holes.drl",
            Cursor::new(b"M48\nMETRIC\nT01C0.8\n%\nM30\n"),
        )
        .expect("test Excellon must parse");
        let mut state = GerberViewerState::default();
        state.apply_load_batch(GerberLoadBatch {
            layers: vec![gerber, drill],
            failures: Vec::new(),
        });
        state.select_layer(0);

        state.toggle_source_view();
        assert!(!state.source_view_visible);
        state.toggle_source_view();
        assert!(state.source_view_visible);
        assert_eq!(state.active_gerber_source(), Ok(("source.gbr", source)));

        state.select_layer(1);
        assert_eq!(
            state.active_gerber_source(),
            Err("Source view is available only for Gerber layers."),
        );

        state.clear_all_layers();
        assert_eq!(state.active_gerber_source(), Err("No active layer."));
    }

    #[test]
    fn reload_replaces_only_successful_layer_data_and_preserves_view_state()
    {
        let first = signex_gerber::load_gerber_reader(
            "first.gbr",
            Cursor::new(
                b"%FSLAX46Y46*%\n%MOMM*%\n%ADD10C,0.5*%\nD10*\nX0Y0D03*\nM02*\n",
            ),
        )
        .expect("first Gerber");
        let second = signex_gerber::load_gerber_reader(
            "second.gbr",
            Cursor::new(
                b"%FSLAX46Y46*%\n%MOMM*%\n%ADD10C,0.5*%\nD10*\nX0Y0D03*\nM02*\n",
            ),
        )
        .expect("second Gerber");
        let replacement = signex_gerber::load_gerber_reader(
            "first.gbr",
            Cursor::new(
                b"%FSLAX46Y46*%\n%MOMM*%\n%ADD10C,0.5*%\nD10*\nX0Y0D03*\nX1000000Y0D03*\nM02*\n",
            ),
        )
        .expect("replacement Gerber");
        let mut state = GerberViewerState::default();
        state.apply_load_batch(GerberLoadBatch {
            layers: vec![first, second.clone()],
            failures: Vec::new(),
        });
        state.select_layer(0);
        state.set_layer_visible(0, false);
        let color = state.layers[0].color;
        let generation = state.redraw_generation;

        state.apply_reload_batch(signex_gerber::GerberReloadBatch {
            layers: vec![(0, replacement)],
            failures: vec![signex_gerber::GerberLoadFailure {
                path: PathBuf::from("second.gbr"),
                message: "could not open file".to_owned(),
            }],
        });

        assert_eq!(state.layers[0].layer.geometry.primitives.len(), 2);
        assert_eq!(state.layers[1].layer, second);
        assert!(!state.layers[0].visible);
        assert_eq!(state.layers[0].color, color);
        assert_eq!(state.active_layer, Some(0));
        assert_eq!(state.redraw_generation, generation + 1);
        assert!(state.status.contains("Reloaded 1 layer(s)."));
        assert!(state.status.contains("1 layer(s) could not be reloaded"));
    }

    #[test]
    fn selecting_active_layer_does_not_change_visibility()
    {
        let layer = signex_gerber::load_gerber_reader(
            "copper.gbr",
            Cursor::new(
                b"%FSLAX46Y46*%\n%MOMM*%\n%ADD10C,1.000*%\nD10*\nX0Y0D03*\nM02*\n",
            ),
        )
        .expect("test Gerber must parse");
        let mut state = GerberViewerState::default();
        state.apply_load_batch(GerberLoadBatch {
            layers: vec![layer.clone(), layer],
            failures: Vec::new(),
        });
        state.set_layer_visible(0, false);

        state.select_layer(0);
        assert_eq!(state.active_layer, Some(0));
        assert!(!state.layers[0].visible);
        assert!(state.layers[1].visible);

        state.select_layer(10);
        assert_eq!(state.active_layer, Some(0));
    }

    #[test]
    fn layer_navigation_uses_loaded_order_without_wrapping()
    {
        let layer = signex_gerber::load_gerber_reader(
            "layer.gbr",
            Cursor::new(
                b"%FSLAX46Y46*%\n%MOMM*%\n%ADD10C,1.000*%\nD10*\nX0Y0D03*\nM02*\n",
            ),
        )
        .expect("test Gerber must parse");
        let mut first = layer.clone();
        first.name = "first.gbr".into();
        let mut second = layer.clone();
        second.name = "second.gbr".into();
        let mut third = layer;
        third.name = "third.gbr".into();
        let mut state = GerberViewerState::default();
        state.apply_load_batch(GerberLoadBatch {
            layers: vec![first, second, third],
            failures: Vec::new(),
        });
        state.set_layer_visible(1, false);
        state.select_layer(0);

        state.select_next_layer();
        assert_eq!(state.active_layer, Some(1));
        assert_eq!(state.status, "Active layer: second.gbr");

        state.select_next_layer();
        assert_eq!(state.active_layer, Some(2));
        state.select_next_layer();
        assert_eq!(state.active_layer, Some(2));

        state.select_previous_layer();
        assert_eq!(state.active_layer, Some(1));
        state.select_previous_layer();
        assert_eq!(state.active_layer, Some(0));
        state.select_previous_layer();
        assert_eq!(state.active_layer, Some(0));
    }

    #[test]
    fn layer_navigation_from_no_selection_uses_nearest_boundary()
    {
        assert_eq!(next_layer_index(None, 3), Some(0));
        assert_eq!(previous_layer_index(None, 3), Some(2));
        assert_eq!(next_layer_index(None, 0), None);
        assert_eq!(previous_layer_index(None, 0), None);
    }

    #[test]
    fn clearing_current_layer_preserves_siblings_and_selects_next_layer()
    {
        let layer = signex_gerber::load_gerber_reader(
            "layer.gbr",
            Cursor::new(
                b"%FSLAX46Y46*%\n%MOMM*%\n%ADD10C,1.000*%\nD10*\nX0Y0D03*\nM02*\n",
            ),
        )
        .expect("test Gerber must parse");
        let mut first = layer.clone();
        first.name = "first.gbr".into();
        let mut second = layer.clone();
        second.name = "second.gbr".into();
        let mut third = layer;
        third.name = "third.gbr".into();
        let mut state = GerberViewerState::default();
        state.apply_load_batch(GerberLoadBatch {
            layers: vec![first, second, third],
            failures: Vec::new(),
        });
        state.select_layer(1);

        state.clear_current_layer();

        assert_eq!(state.layers.len(), 2);
        assert_eq!(state.layers[0].layer.name, "first.gbr");
        assert_eq!(state.layers[1].layer.name, "third.gbr");
        assert_eq!(state.active_layer, Some(1));
    }

    #[test]
    fn clearing_all_layers_restores_empty_view()
    {
        let layer = signex_gerber::load_gerber_reader(
            "copper.gbr",
            Cursor::new(
                b"%FSLAX46Y46*%\n%MOMM*%\n%ADD10C,1.000*%\nD10*\nX0Y0D03*\nM02*\n",
            ),
        )
        .expect("test Gerber must parse");
        let mut state = GerberViewerState::default();
        state.apply_load_batch(GerberLoadBatch {
            layers: vec![layer],
            failures: Vec::new(),
        });
        state.zoom = 3.0;
        state.pan = iced::Vector::new(50.0, 20.0);

        state.clear_all_layers();

        assert!(state.layers.is_empty());
        assert_eq!(state.active_layer, None);
        assert_eq!(state.zoom, 1.0);
        assert_eq!(state.pan, iced::Vector::default());
    }
        }
    };
}

pub(crate) use gerber_layer_state_tests;
