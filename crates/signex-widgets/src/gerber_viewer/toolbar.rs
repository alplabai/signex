use super::*;
use iced::widget::{Column, button, container, svg, text, tooltip};

const ICON_SIZE: f32 = 20.0;
const BUTTON_SIZE: f32 = 32.0;

const SELECT_ICON: &[u8] = include_bytes!("../../assets/gerber-viewer/select.svg");
const MEASURE_ICON: &[u8] = include_bytes!("../../assets/gerber-viewer/measure.svg");
const GRID_ICON: &[u8] = include_bytes!("../../assets/gerber-viewer/grid.svg");
const UNITS_MM_ICON: &[u8] = include_bytes!("../../assets/gerber-viewer/units_mm.svg");
const UNITS_MIL_ICON: &[u8] = include_bytes!("../../assets/gerber-viewer/units_mil.svg");
const UNITS_IN_ICON: &[u8] = include_bytes!("../../assets/gerber-viewer/units_in.svg");
const CROSSHAIR_ICON: &[u8] = include_bytes!("../../assets/gerber-viewer/crosshair.svg");
const SKETCH_ITEM_ICON: &[u8] = include_bytes!("../../assets/gerber-viewer/sketch_item.svg");
const SKETCH_LINE_ICON: &[u8] = include_bytes!("../../assets/gerber-viewer/sketch_line.svg");
const SKETCH_POLYGON_ICON: &[u8] = include_bytes!("../../assets/gerber-viewer/sketch_polygon.svg");
const GHOST_NEGATIVE_ICON: &[u8] = include_bytes!("../../assets/gerber-viewer/ghost_negative.svg");
const D_CODES_ICON: &[u8] = include_bytes!("../../assets/gerber-viewer/dcodes.svg");
const FORCED_OPACITY_ICON: &[u8] = include_bytes!("../../assets/gerber-viewer/forced_opacity.svg");
const XOR_ICON: &[u8] = include_bytes!("../../assets/gerber-viewer/xor.svg");
const INTERACTIVE_LAYER_ICON: &[u8] =
    include_bytes!("../../assets/gerber-viewer/interactive_layer.svg");
const FLIP_ICON: &[u8] = include_bytes!("../../assets/gerber-viewer/flip.svg");
const LAYERS_MANAGER_ICON: &[u8] = include_bytes!("../../assets/gerber-viewer/layers_manager.svg");

pub(super) fn view<'a>(
    state: &GerberViewerState,
    tokens: &ThemeTokens,
) -> Element<'a, GerberViewerMessage> {
    let units_icon = match state.display_unit {
        GerberDisplayUnit::Millimetres => UNITS_MM_ICON,
        GerberDisplayUnit::Mils => UNITS_MIL_ICON,
        GerberDisplayUnit::Inches => UNITS_IN_ICON,
    };
    let next_unit = state.display_unit.next();
    let next_crosshair = state.crosshair_mode.next();

    let mut rail = Column::new()
        .width(BUTTON_SIZE)
        .height(Length::Fill)
        .spacing(1)
        .push(icon_button(
            SELECT_ICON,
            "Select — click an item or drag a rectangular region".into(),
            GerberViewerMessage::ActivateSelectionTool,
            state.selection_tool_active(),
            tokens,
        ))
        .push(icon_button(
            MEASURE_ICON,
            "Measure — drag between two points".into(),
            GerberViewerMessage::ActivateMeasurementTool,
            state.measurement_active(),
            tokens,
        ))
        .push(separator(tokens))
        .push(icon_button(
            GRID_ICON,
            if state.grid_visible {
                "Show Grid — grid is visible"
            } else {
                "Show Grid — grid is hidden"
            }
            .into(),
            GerberViewerMessage::ToggleGridVisibility(!state.grid_visible),
            state.grid_visible,
            tokens,
        ))
        .push(icon_button(
            units_icon,
            format!("Units — {} (click for {next_unit})", state.display_unit),
            GerberViewerMessage::CycleDisplayUnit,
            false,
            tokens,
        ))
        .push(icon_button(
            CROSSHAIR_ICON,
            format!(
                "Crosshair — {} (click for {next_crosshair})",
                state.crosshair_mode
            ),
            GerberViewerMessage::CycleCrosshairMode,
            state.crosshair_mode != GerberCrosshairMode::None,
            tokens,
        ))
        .push(separator(tokens))
        .push(icon_button(
            SKETCH_ITEM_ICON,
            "Sketch Item — toggle flashed items filled/outline".into(),
            GerberViewerMessage::ToggleSketchFlashes,
            state.sketch_flashes,
            tokens,
        ))
        .push(icon_button(
            SKETCH_LINE_ICON,
            "Sketch Line — toggle lines filled/outline".into(),
            GerberViewerMessage::ToggleSketchLines,
            state.sketch_lines,
            tokens,
        ))
        .push(icon_button(
            SKETCH_POLYGON_ICON,
            "Sketch Polygons — toggle polygons filled/outline".into(),
            GerberViewerMessage::ToggleSketchPolygons,
            state.sketch_polygons,
            tokens,
        ))
        .push(icon_button(
            GHOST_NEGATIVE_ICON,
            "Ghost negative objects".into(),
            GerberViewerMessage::ToggleGhostNegativeObjects,
            state.ghost_negative_objects,
            tokens,
        ))
        .push(icon_button(
            D_CODES_ICON,
            "Show DCodes".into(),
            GerberViewerMessage::ToggleDCodeLabels,
            state.show_d_code_labels,
            tokens,
        ))
        .push(separator(tokens))
        .push(icon_button(
            FORCED_OPACITY_ICON,
            "Show with forced Opacity Mode".into(),
            GerberViewerMessage::ToggleForcedOpacityMode,
            state.forced_opacity_mode,
            tokens,
        ))
        .push(icon_button(
            XOR_ICON,
            "Show in XOR Mode".into(),
            GerberViewerMessage::ToggleCompareMode,
            state.compare_mode,
            tokens,
        ))
        .push(icon_button(
            INTERACTIVE_LAYER_ICON,
            "Interactive Layer View Mode".into(),
            GerberViewerMessage::ToggleDimInactiveLayers,
            state.dim_inactive_layers,
            tokens,
        ))
        .push(icon_button(
            FLIP_ICON,
            "Flip Gerber View".into(),
            GerberViewerMessage::ToggleMirrored,
            state.mirrored,
            tokens,
        ))
        .push(separator(tokens))
        .push(icon_button(
            LAYERS_MANAGER_ICON,
            "Show Layers Manager".into(),
            GerberViewerMessage::ToggleLayerManager,
            state.layer_manager_visible,
            tokens,
        ));

    rail = rail.push(Space::new().height(Length::Fill));

    container(rail)
        .width(BUTTON_SIZE)
        .height(Length::Fill)
        .style(styles::left_toolbar(tokens))
        .into()
}

fn icon_button<'a>(
    icon: &'static [u8],
    hint: String,
    on_press: GerberViewerMessage,
    selected: bool,
    tokens: &ThemeTokens,
) -> Element<'a, GerberViewerMessage> {
    let text_color = if selected {
        styles::ti(tokens.text)
    } else {
        styles::ti(tokens.text_secondary)
    };
    let hover = styles::ti(tokens.hover);
    let icon = svg(svg::Handle::from_memory(icon))
        .width(ICON_SIZE)
        .height(ICON_SIZE)
        .style(move |_: &Theme, _| iced::widget::svg::Style {
            color: Some(text_color),
        });
    let button = button(
        container(icon)
            .width(BUTTON_SIZE)
            .height(BUTTON_SIZE)
            .center_x(BUTTON_SIZE)
            .center_y(BUTTON_SIZE),
    )
    .padding(0)
    .on_press(on_press)
    .style(move |_: &Theme, status: button::Status| {
        let background = if selected {
            Some(Background::Color(hover))
        } else if status == button::Status::Hovered {
            Some(Background::Color(Color::from_rgba8(255, 255, 255, 0.06)))
        } else {
            None
        };
        button::Style {
            background,
            border: Border {
                width: 0.0,
                radius: 2.0.into(),
                color: Color::TRANSPARENT,
            },
            ..button::Style::default()
        }
    });
    let panel = styles::ti(tokens.panel_bg);
    let border = styles::ti(tokens.border);
    tooltip(
        button,
        container(text(hint).size(11).color(styles::ti(tokens.text)))
            .padding([4, 7])
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(panel)),
                border: Border {
                    width: 1.0,
                    radius: 3.0.into(),
                    color: border,
                },
                ..container::Style::default()
            }),
        tooltip::Position::Right,
    )
    .gap(5)
    .into()
}

fn separator<'a>(tokens: &ThemeTokens) -> Element<'a, GerberViewerMessage> {
    container(Space::new())
        .width(Length::Fill)
        .height(1)
        .style(styles::chrome_separator(tokens))
        .into()
}

#[cfg(test)]
pub(super) const TOOLBAR_ICON_COUNT: usize = 15;

#[cfg(test)]
pub(super) const TOOLBAR_ICON_ASSETS: [&[u8]; 17] = [
    SELECT_ICON,
    MEASURE_ICON,
    GRID_ICON,
    UNITS_MM_ICON,
    UNITS_MIL_ICON,
    UNITS_IN_ICON,
    CROSSHAIR_ICON,
    SKETCH_ITEM_ICON,
    SKETCH_LINE_ICON,
    SKETCH_POLYGON_ICON,
    GHOST_NEGATIVE_ICON,
    D_CODES_ICON,
    FORCED_OPACITY_ICON,
    XOR_ICON,
    INTERACTIVE_LAYER_ICON,
    FLIP_ICON,
    LAYERS_MANAGER_ICON,
];
