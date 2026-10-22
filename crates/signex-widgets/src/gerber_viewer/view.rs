use super::viewport::*;
use super::*;
use iced::widget::column;

pub fn view<'a>(
    workspace: &'a GerberWorkspaceState,
    shortcut_resolver: &'a dyn GerberShortcutResolver,
    tokens: &'a ThemeTokens,
) -> Element<'a, (GerberDocumentId, GerberViewerMessage)> {
    let Some(state) = workspace.active_viewer() else {
        return Space::new().width(Length::Fill).height(Length::Fill).into();
    };
    let active_document = workspace.active_document_id();
    let text_muted = styles::ti(tokens.text_secondary);
    let menu_bar = menu::view(state, tokens).map(move |message| (active_document, message));
    let dock_tokens = *tokens;
    let content = iced_dock::dock()
        .state(workspace.dock.session().state())
        .on_event(move |event| (active_document, GerberViewerMessage::DockEvent(event)))
        .content(move |panel_kind| {
            view_dock_panel(panel_kind, workspace, shortcut_resolver, tokens)
        })
        .style(move |theme| gerber_dock_style(theme, &dock_tokens))
        .min_pane_width(210.0)
        .min_pane_height(120.0)
        .tab_bar_height(28.0)
        .pane_padding(0.0)
        .splitter_size(5.0)
        .build();

    let (cartesian_label, polar_label) = cursor_coordinate_labels(state);
    let status_color = if state.status.contains("could not") {
        Color::from_rgb8(239, 83, 80)
    } else {
        text_muted
    };
    let status = container(
        row![
            text(cartesian_label).size(10).color(text_muted),
            text(polar_label).size(10).color(text_muted),
            text("|").size(10).color(text_muted),
            text(&state.status).size(10).color(status_color),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center),
    )
    .padding([4, 10])
    .width(Length::Fill)
    .style(styles::status_bar(tokens));

    column![
        menu_bar,
        row![
            toolbar::view(state, tokens).map(move |message| (active_document, message)),
            content,
        ]
        .width(Length::Fill)
        .height(Length::Fill),
        status,
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn view_dock_panel<'a>(
    panel_kind: GerberDockPanel,
    workspace: &'a GerberWorkspaceState,
    shortcut_resolver: &'a dyn GerberShortcutResolver,
    tokens: &'a ThemeTokens,
) -> Element<'a, (GerberDocumentId, GerberViewerMessage)> {
    match panel_kind {
        GerberDockPanel::Document(document_id) => workspace
            .viewer(document_id)
            .map(|state| {
                view_canvas(state, shortcut_resolver, tokens)
                    .map(move |message| (document_id, message))
            })
            .unwrap_or_else(|| Space::new().into()),
        GerberDockPanel::Layers => workspace
            .active_viewer()
            .map(|state| {
                view_layers(state, tokens)
                    .map(move |message| (workspace.active_document_id(), message))
            })
            .unwrap_or_else(|| Space::new().into()),
        GerberDockPanel::Highlight => workspace
            .active_viewer()
            .map(|state| {
                view_highlight(state, tokens)
                    .map(move |message| (workspace.active_document_id(), message))
            })
            .unwrap_or_else(|| Space::new().into()),
        GerberDockPanel::Grid => workspace
            .active_viewer()
            .map(|state| {
                view_grid(state, tokens)
                    .map(move |message| (workspace.active_document_id(), message))
            })
            .unwrap_or_else(|| Space::new().into()),
        GerberDockPanel::LayerInformation => workspace
            .active_viewer()
            .map(|state| {
                view_layer_information(state, tokens)
                    .map(move |message| (workspace.active_document_id(), message))
            })
            .unwrap_or_else(|| Space::new().into()),
        GerberDockPanel::DCodes => workspace
            .active_viewer()
            .map(|state| {
                view_d_codes(state, tokens)
                    .map(move |message| (workspace.active_document_id(), message))
            })
            .unwrap_or_else(|| Space::new().into()),
        GerberDockPanel::Source => workspace
            .active_viewer()
            .map(|state| {
                view_source(state, tokens)
                    .map(move |message| (workspace.active_document_id(), message))
            })
            .unwrap_or_else(|| Space::new().into()),
    }
}

fn view_canvas<'a>(
    state: &'a GerberViewerState,
    shortcut_resolver: &'a dyn GerberShortcutResolver,
    tokens: &ThemeTokens,
) -> Element<'a, GerberViewerMessage> {
    let canvas_bg = styles::ti(tokens.bg);
    let canvas_widget: Element<'_, GerberViewerMessage> = canvas(GerberCanvas {
        layers: &state.layers,
        background: canvas_bg,
        grid: state.grid_color,
        grid_visible: state.grid_visible,
        grid_style: state.grid_style,
        crosshair_mode: state.crosshair_mode,
        page_size: state.page_size,
        zoom_selection_active: state.zoom_selection_active,
        selection_active: state.selection_tool_active(),
        measurement_active: state.measurement_active(),
        measurement: state.measurement(),
        measurement_annotation: state.measurement_annotation(),
        sketch_flashes: state.sketch_flashes,
        sketch_lines: state.sketch_lines,
        sketch_polygons: state.sketch_polygons,
        ghost_negative_objects: state.ghost_negative_objects,
        negative_ghost_color: state.negative_ghost_color,
        show_d_code_labels: state.show_d_code_labels,
        d_code_color: state.d_code_color,
        compare_mode: state.compare_mode,
        compare_palette: &state.compare_palette,
        forced_opacity_mode: state.forced_opacity_mode,
        forced_opacity: state.forced_opacity,
        dim_inactive_layers: state.dim_inactive_layers,
        inactive_layer_opacity: state.inactive_layer_opacity,
        mirrored: state.mirrored,
        active_layer: state.active_layer,
        highlighted_component: state.highlighted_component(),
        highlighted_net: state.highlighted_net(),
        highlighted_attribute: state.highlighted_attribute(),
        highlighted_d_code: state.highlighted_d_code(),
        selected_item: state.selected_item(),
        selected_items: state.selected_items(),
        redraw_generation: state.redraw_generation,
        zoom: state.zoom,
        pan: state.pan,
        grid_size: state.active_grid(),
        shortcut_resolver,
    })
    .width(Length::Fill)
    .height(Length::Fill)
    .into();

    container(canvas_widget)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(canvas_bg)),
            ..container::Style::default()
        })
        .into()
}

fn view_highlight<'a>(
    state: &'a GerberViewerState,
    tokens: &ThemeTokens,
) -> Element<'a, GerberViewerMessage> {
    let text_primary = styles::ti(tokens.text);
    let text_muted = styles::ti(tokens.text_secondary);
    let component_picker = pick_list(
        state.component_choices(),
        state.highlighted_component().map(str::to_owned),
        GerberViewerMessage::SetHighlightedComponent,
    )
    .placeholder("Component")
    .width(Length::Fill);
    let net_picker = pick_list(
        state.net_choices(),
        state.highlighted_net().map(str::to_owned),
        GerberViewerMessage::SetHighlightedNet,
    )
    .placeholder("Net")
    .width(Length::Fill);
    let attribute_picker = pick_list(
        state.attribute_choices(),
        state.highlighted_attribute().cloned(),
        GerberViewerMessage::SetHighlightedAttribute,
    )
    .placeholder("Attribute")
    .width(Length::Fill);
    let d_code_choices = state.d_code_choices();
    let selected_d_code = state.highlighted_d_code().and_then(|selected| {
        d_code_choices
            .iter()
            .find(|choice| choice.code == selected)
            .cloned()
    });
    let d_code_picker = pick_list(d_code_choices, selected_d_code, |choice| {
        GerberViewerMessage::SetHighlightedDCode(choice.code)
    })
    .placeholder("D-code")
    .width(Length::Fill);
    let active_layer = state
        .active_layer
        .and_then(|index| state.layers.get(index))
        .map(|layer| layer.layer.name.as_str())
        .unwrap_or("No active layer");
    let content = column![
        row![
            column![
                text("Active-layer highlight").size(13).color(text_primary),
                text(active_layer).size(10).color(text_muted),
            ]
            .spacing(2)
            .width(Length::Fill),
            highlight_controls::clear_button(state.has_active_highlight(), tokens,),
        ]
        .align_y(iced::Alignment::Center),
        text("Component").size(11).color(text_muted),
        component_picker,
        text("Net").size(11).color(text_muted),
        net_picker,
        text("Attribute").size(11).color(text_muted),
        attribute_picker,
        text("D-code").size(11).color(text_muted),
        d_code_picker,
        text(format!(
            "{} / {MAX_VIEWER_LAYERS} layers",
            state.layers.len()
        ))
        .size(10)
        .color(text_muted),
    ]
    .spacing(6)
    .padding(8);

    panel_container(scrollable(content), tokens)
}

fn view_grid<'a>(
    state: &'a GerberViewerState,
    tokens: &ThemeTokens,
) -> Element<'a, GerberViewerMessage> {
    let text_primary = styles::ti(tokens.text);
    let text_muted = styles::ti(tokens.text_secondary);
    let grid_choices = grid_size_choices(&state.grid_catalog, &state.decimal_separator);
    let selected_grid = grid_choices.get(state.active_grid_index).cloned();
    let grid_picker = pick_list(grid_choices, selected_grid, |choice| {
        GerberViewerMessage::SelectGridSize(choice.index)
    })
    .width(Length::Fill);
    let display_unit_picker = pick_list(
        GerberDisplayUnit::ALL,
        Some(state.display_unit),
        GerberViewerMessage::SetDisplayUnit,
    )
    .width(Length::Fill);
    let page_size_picker = pick_list(
        GerberPageSize::ALL,
        Some(state.page_size),
        GerberViewerMessage::SetPageSize,
    )
    .width(Length::Fill);
    let bounds_label = visible_bounds(&state.layers)
        .map(|bounds| format_bounds_in_unit(bounds, state.display_unit, &state.decimal_separator))
        .unwrap_or_else(|| "Bounds: —".to_owned());
    let measurement_label = state
        .measurement_summary()
        .unwrap_or_else(|| "Measurement: —".to_owned());
    let content = column![
        text("Grid settings").size(13).color(text_primary),
        text("Spacing").size(11).color(text_muted),
        grid_picker,
        button(text("Edit grids ..."))
            .width(Length::Fill)
            .on_press(GerberViewerMessage::OpenGridEditor),
        row![
            column![
                text("Display unit").size(11).color(text_muted),
                display_unit_picker,
            ]
            .spacing(4)
            .width(Length::Fill),
            column![text("Page").size(11).color(text_muted), page_size_picker,]
                .spacing(4)
                .width(Length::Fill),
        ]
        .spacing(6),
        horizontal_rule(tokens),
        text(bounds_label).size(10).color(text_muted),
        text(measurement_label).size(10).color(text_muted),
    ]
    .spacing(6)
    .padding(8);

    panel_container(scrollable(content), tokens)
}

fn cursor_coordinate_labels(state: &GerberViewerState) -> (String, String) {
    state
        .cursor_world_position
        .map(|position| {
            (
                format_cartesian_coordinate_in_unit(
                    position,
                    state.display_unit,
                    &state.decimal_separator,
                ),
                format_polar_coordinate_in_unit(
                    position,
                    state.display_unit,
                    &state.decimal_separator,
                ),
            )
        })
        .unwrap_or_else(|| ("X: —  Y: —".to_owned(), "R: —  θ: —° / — rad".to_owned()))
}

fn view_layers<'a>(
    state: &'a GerberViewerState,
    tokens: &ThemeTokens,
) -> Element<'a, GerberViewerMessage> {
    let text_primary = styles::ti(tokens.text);
    let text_muted = styles::ti(tokens.text_secondary);
    let mut layer_list = column![text("Layers").size(13).color(text_primary)].spacing(6);

    if state.layers.is_empty() {
        layer_list = layer_list.push(text("No loaded layers").size(11).color(text_muted));
    } else {
        let color_choices = state.layer_color_choices();
        for (index, viewer_layer) in state.layers.iter().enumerate().rev() {
            let active = state.active_layer == Some(index);
            let color = viewer_layer.color;
            let color_chip =
                container(Space::new())
                    .width(12)
                    .height(12)
                    .style(move |_: &Theme| container::Style {
                        background: Some(Background::Color(color)),
                        border: Border {
                            width: 1.0,
                            radius: 2.0.into(),
                            color,
                        },
                        ..container::Style::default()
                    });
            let visible = checkbox(viewer_layer.visible)
                .size(14)
                .on_toggle(move |visible| GerberViewerMessage::SetLayerVisible(index, visible));
            let label = button(
                row![
                    color_chip,
                    text(&viewer_layer.layer.name)
                        .size(11)
                        .color(text_primary)
                        .wrapping(text::Wrapping::None),
                ]
                .spacing(7)
                .align_y(iced::Alignment::Center),
            )
            .width(Length::Fill)
            .padding([5, 6])
            .on_press(GerberViewerMessage::SelectLayer(index))
            .style(styles::rail_tab(tokens, active));
            let selected_color = state.selected_layer_color_choice(index);
            let target = GerberColorTarget::Layer(index);
            let color_picker = color_dropdown(
                color_choices.clone(),
                selected_color,
                target,
                state.color_picker_open(target),
                Length::Fixed(128.0),
                move |choice| GerberViewerMessage::SetLayerColor(index, choice),
                tokens,
            );
            layer_list = layer_list.push(
                row![visible, label, color_picker]
                    .spacing(5)
                    .align_y(iced::Alignment::Center),
            );
        }
    }

    let item_color_choices = state.layer_color_choices();
    let grid_color = state.selected_color_choice(state.grid_color);
    let d_code_color = state.selected_color_choice(state.d_code_color);
    let negative_color = state.selected_color_choice(state.negative_ghost_color);
    layer_list = layer_list
        .push(horizontal_rule(tokens))
        .push(layer_controls::view(state, tokens))
        .push(horizontal_rule(tokens))
        .push(text("Item colors").size(12).color(text_primary))
        .push(
            row![
                text("Grid").size(11).width(70),
                color_dropdown(
                    item_color_choices.clone(),
                    grid_color,
                    GerberColorTarget::Grid,
                    state.color_picker_open(GerberColorTarget::Grid),
                    Length::Fill,
                    GerberViewerMessage::SetGridColor,
                    tokens,
                )
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center),
        )
        .push(
            row![
                text("D-codes").size(11).width(70),
                color_dropdown(
                    item_color_choices.clone(),
                    d_code_color,
                    GerberColorTarget::DCode,
                    state.color_picker_open(GerberColorTarget::DCode),
                    Length::Fill,
                    GerberViewerMessage::SetDCodeColor,
                    tokens,
                )
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center),
        )
        .push(
            row![
                text("Negative").size(11).width(70),
                color_dropdown(
                    item_color_choices,
                    negative_color,
                    GerberColorTarget::NegativeObject,
                    state.color_picker_open(GerberColorTarget::NegativeObject),
                    Length::Fill,
                    GerberViewerMessage::SetNegativeObjectColor,
                    tokens,
                )
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center),
        );

    panel_container(scrollable(layer_list.padding(8)), tokens)
}

fn view_layer_information<'a>(
    state: &'a GerberViewerState,
    tokens: &ThemeTokens,
) -> Element<'a, GerberViewerMessage> {
    let text_primary = styles::ti(tokens.text);
    let text_muted = styles::ti(tokens.text_secondary);
    let content: Element<'_, GerberViewerMessage> =
        if let Some(metadata) = state.active_layer_metadata() {
            let bounds = metadata
                .bounds
                .map(|bounds| {
                    format_bounds_in_unit(bounds, state.display_unit, &state.decimal_separator)
                })
                .unwrap_or_else(|| "Bounds: unavailable".to_owned());
            let coordinate_format = metadata
                .coordinate_format
                .unwrap_or_else(|| "Unavailable".to_owned());
            let definitions = list_or_none(&metadata.definitions);
            let attributes = list_or_none(&metadata.attributes);
            let warnings = list_or_none(&metadata.warnings);
            column![
                text("Active Layer Information")
                    .size(13)
                    .color(text_primary),
                text(format!("File: {}", metadata.file_name))
                    .size(10)
                    .color(text_muted),
                text(format!("Source: {}", metadata.source))
                    .size(10)
                    .color(text_muted),
                text(format!("Format: {}", metadata.format))
                    .size(10)
                    .color(text_muted),
                text(format!("Role: {}", metadata.layer_role))
                    .size(10)
                    .color(text_muted),
                text(format!("Units: {}", metadata.units))
                    .size(10)
                    .color(text_muted),
                text(format!("Coordinate format: {coordinate_format}"))
                    .size(10)
                    .color(text_muted),
                text(bounds).size(10).color(text_muted),
                text(format!("Rendered primitives: {}", metadata.primitive_count))
                    .size(10)
                    .color(text_muted),
                text(format!("{}:\n{definitions}", metadata.definition_label))
                    .size(10)
                    .color(text_muted),
                text(format!("Attributes:\n{attributes}"))
                    .size(10)
                    .color(text_muted),
                text(format!("Warnings:\n{warnings}"))
                    .size(10)
                    .color(text_muted),
            ]
            .spacing(4)
            .padding(8)
            .into()
        } else {
            container(text("No active layer").size(10).color(text_muted))
                .center(Length::Fill)
                .into()
        };

    panel_container(scrollable(content), tokens)
}

fn view_d_codes<'a>(
    state: &'a GerberViewerState,
    tokens: &ThemeTokens,
) -> Element<'a, GerberViewerMessage> {
    let text_primary = styles::ti(tokens.text);
    let text_muted = styles::ti(tokens.text_secondary);
    let mut content =
        column![text("D-Codes and Drill Tools").size(13).color(text_primary),].spacing(6);

    for group in state.definition_groups() {
        let definitions = if group.definitions.is_empty() {
            format!("No {} defined", group.definition_label.to_lowercase())
        } else {
            group
                .definitions
                .iter()
                .map(|definition| {
                    format!(
                        "{} — {} — {} use(s)",
                        definition.code, definition.description, definition.usage_count,
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        };
        content = content.push(
            column![
                text(format!("{} · {}", group.layer_name, group.definition_label))
                    .size(11)
                    .color(text_primary),
                text(definitions).size(10).color(text_muted),
            ]
            .spacing(2),
        );
    }

    panel_container(scrollable(content.padding(8)), tokens)
}

fn view_source<'a>(
    state: &'a GerberViewerState,
    tokens: &ThemeTokens,
) -> Element<'a, GerberViewerMessage> {
    let text_primary = styles::ti(tokens.text);
    let text_muted = styles::ti(tokens.text_secondary);
    let content: Element<'_, GerberViewerMessage> = match state.active_gerber_source() {
        Ok((name, source)) => column![
            text(format!("Original Gerber source — {name}"))
                .size(13)
                .color(text_primary),
            scrollable(
                container(
                    text(source)
                        .size(11)
                        .font(iced::Font::MONOSPACE)
                        .color(text_primary),
                )
                .padding(12)
                .width(Length::Fill),
            )
            .height(Length::Fill),
        ]
        .spacing(8)
        .padding(8)
        .into(),
        Err(message) => container(text(message).size(12).color(text_muted))
            .center(Length::Fill)
            .into(),
    };

    panel_container(content, tokens)
}

fn panel_container<'a>(
    content: impl Into<Element<'a, GerberViewerMessage>>,
    tokens: &ThemeTokens,
) -> Element<'a, GerberViewerMessage> {
    let panel_bg = styles::ti(tokens.panel_bg);
    let text_primary = styles::ti(tokens.text);
    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(panel_bg)),
            text_color: Some(text_primary),
            ..container::Style::default()
        })
        .into()
}

fn horizontal_rule(tokens: &ThemeTokens) -> Element<'static, GerberViewerMessage> {
    container(Space::new())
        .width(Length::Fill)
        .height(1)
        .style(styles::chrome_separator(tokens))
        .into()
}

fn list_or_none(values: &[String]) -> String {
    if values.is_empty() {
        "None".to_owned()
    } else {
        values.join("\n")
    }
}

fn gerber_dock_style(theme: &Theme, tokens: &ThemeTokens) -> iced_dock::DockStyle {
    let mut style = iced_dock::default(theme);
    style.background.color = styles::ti(tokens.bg);
    style.window.background = styles::ti(tokens.panel_bg);
    style.window.border = Border {
        color: styles::ti(tokens.border),
        width: 1.0,
        radius: 0.0.into(),
    };
    style.window.focused_border = Some(Border {
        color: styles::ti(tokens.accent),
        ..style.window.border
    });
    style.tab_bar.background = styles::ti(tokens.toolbar_bg);
    style.tab_bar.separator = Some(styles::ti(tokens.border));
    style.tab.inactive_background = styles::ti(tokens.toolbar_bg);
    style.tab.inactive_text = styles::ti(tokens.text_secondary);
    style.tab.hovered_background = styles::ti(tokens.hover);
    style.tab.hovered_text = styles::ti(tokens.text);
    style.tab.pressed_background = styles::ti(tokens.selection);
    style.tab.pressed_text = styles::ti(tokens.text);
    style.tab.active_background = styles::ti(tokens.panel_bg);
    style.tab.active_text = styles::ti(tokens.text);
    style.tab.active_accent = styles::ti(tokens.accent);
    style.splitter.hover_color = styles::ti(tokens.accent);
    style.splitter.drag_color = styles::ti(tokens.accent);
    style.drop_overlay.color = styles::ti(tokens.selection);
    style
}
