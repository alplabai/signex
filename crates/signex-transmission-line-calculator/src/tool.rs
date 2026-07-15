use crate::{
    Complex, CustomInterpolation, DEFAULT_REFERENCE_IMPEDANCE_OHM, GainCirclePort, ScalarUnit,
    SmithChartAnalysis, SmithChartElement, TransformerModel, chart_point_from_normalized_impedance,
    normalized_impedance_from_chart_point,
};
use iced::widget::{
    Space, button, canvas, checkbox, column, container, pick_list, row, scrollable, text,
    text_input,
};
use iced::{Alignment, Color, Element, Length, Point, Rectangle, Renderer, Theme, mouse};

#[path = "component_editor/mod.rs"]
mod component_editor;
#[path = "equations/mod.rs"]
mod equations;
#[path = "results/mod.rs"]
mod results;
#[path = "settings_and_features/mod.rs"]
mod settings_and_features;
#[path = "shared/mod.rs"]
mod shared;
#[path = "smith_chart_2d/mod.rs"]
mod smith_chart_2d;
#[path = "smith_chart_3d/mod.rs"]
mod smith_chart_3d;
#[path = "smith_chart_y/mod.rs"]
mod smith_chart_y;
#[path = "tutorials/mod.rs"]
mod tutorials;

use component_editor::*;
use equations::*;
pub use results::ResultDiagramKind;
use results::{frequency_plot_panel, result_panel};
use settings_and_features::*;
use shared::{
    CUSTOM_INTERPOLATIONS, DEFAULT_S_PARAMETER_TEXT, FORMULA_REFERENCE_LINKS, FORMULA_REFERENCES,
    FREQUENCY_UNITS, SmithChartDiagramMode, TRANSFORMER_MODELS, TUTORIAL_REFERENCE_LINKS,
    format_complex, format_complex_and_polar, format_db, format_finite, format_frequency,
    format_number, format_optional, optional_tolerance, parse_custom_points, parse_field,
    parse_marker_list, parse_optional, parse_scalar_list, parse_touchstone_input,
    parse_vswr_circle_list, quality_factor,
};
pub use shared::{ReferenceLink, SmithChartMessage, SmithChartState};
use smith_chart_2d::*;
use smith_chart_3d::*;
use smith_chart_y::*;
use tutorials::*;

pub fn view(state: &SmithChartState) -> Element<'_, SmithChartMessage> {
    let result = state.solve();
    let chart_point = result
        .as_ref()
        .ok()
        .map(|result| result.nominal.reflection_coefficient);
    let frequency_results = result
        .as_ref()
        .ok()
        .map(|result| result.frequency_results.clone())
        .unwrap_or_default();
    let active_frequency_hz = result
        .as_ref()
        .ok()
        .map(|result| result.active_frequency_hz)
        .unwrap_or_default();
    let stability_circles = result
        .as_ref()
        .ok()
        .map(|result| result.stability_circles.clone())
        .unwrap_or_default();
    let (gain_circles, noise_figure_circles) = state.overlay_circles().unwrap_or_default();
    let markers = parse_marker_list(&state.marker_list).unwrap_or_default();
    let q_circles = parse_scalar_list(&state.q_circle_list).unwrap_or_default();
    let vswr_circles = parse_vswr_circle_list(&state.vswr_circle_list, state.vswr_circle_input_db)
        .unwrap_or_default();
    let resistance_labels = parse_scalar_list(&state.resistance_label_list)
        .unwrap_or_else(|_| vec![0.0, 0.2, 0.5, 1.0, 2.0, 4.0, 10.0]);
    let reactance_labels = parse_scalar_list(&state.reactance_label_list).unwrap_or_else(|_| {
        vec![
            0.2, 0.5, 1.0, 2.0, 4.0, 10.0, -0.2, -0.5, -1.0, -2.0, -4.0, -10.0,
        ]
    });
    let s_parameter_traces = s_parameter_chart_traces(state);
    let reference_impedance_ohm = parse_field("reference impedance", &state.reference_ohm)
        .ok()
        .filter(|value| *value > f64::EPSILON)
        .unwrap_or(DEFAULT_REFERENCE_IMPEDANCE_OHM);
    let impedance_arc_traces = result
        .as_ref()
        .ok()
        .map(|result| impedance_arc_chart_traces(result, reference_impedance_ohm))
        .unwrap_or_default();
    let smith_chart_canvas = || SmithChartCanvas {
        point: chart_point,
        frequency_results: frequency_results.clone(),
        active_frequency_hz,
        frequency_unit: state.frequency_unit,
        show_grid: state.show_grid,
        show_admittance: state.show_admittance,
        admittance_chart: false,
        show_vswr: state.show_vswr,
        show_q: state.show_q,
        resistance_labels: resistance_labels.clone(),
        reactance_labels: reactance_labels.clone(),
        markers: markers.clone(),
        q_circles: q_circles.clone(),
        vswr_circles: vswr_circles.clone(),
        reference_impedance_ohm,
        stability_circles: if state.show_stability_circles {
            stability_circles.clone()
        } else {
            Vec::new()
        },
        gain_circles: gain_circles.clone(),
        noise_figure_circles: noise_figure_circles.clone(),
        impedance_arc_traces: impedance_arc_traces.clone(),
        s_parameter_traces: s_parameter_traces.clone(),
    };
    let chart = || -> Element<'_, SmithChartMessage> {
        match state.diagram_mode {
            SmithChartDiagramMode::TwoDimensional => canvas(smith_chart_canvas())
                .width(Length::Fill)
                .height(Length::Fixed(840.0))
                .into(),
            SmithChartDiagramMode::AdmittanceTwoDimensional => {
                canvas(SmithChartYCanvas::new(smith_chart_canvas()))
                    .width(Length::Fill)
                    .height(Length::Fixed(840.0))
                    .into()
            }
            SmithChartDiagramMode::ThreeDimensional => canvas(SmithChart3dCanvas {
                point: chart_point,
                show_grid: state.show_grid,
                resistance_labels: resistance_labels.clone(),
                reactance_labels: reactance_labels.clone(),
                markers: markers.clone(),
                reference_impedance_ohm,
                impedance_arc_traces: impedance_arc_traces.clone(),
                s_parameter_traces: s_parameter_traces.clone(),
                yaw: state.smith_sphere_yaw,
                pitch: state.smith_sphere_pitch,
            })
            .width(Length::Fill)
            .height(Length::Fixed(840.0))
            .into(),
        }
    };
    let body = column![
        row![
            text("Transmission Line Calculator").size(22),
            Space::new().width(Length::Fill),
            button(text("Reset")).on_press(SmithChartMessage::Reset),
        ]
        .align_y(Alignment::Center)
        .spacing(12),
        component_editor_section(state),
        section(
            "Smith Chart",
            vec![
                row![
                    text("View"),
                    pick_list(
                        SmithChartDiagramMode::ALL,
                        Some(state.diagram_mode),
                        SmithChartMessage::DiagramModeChanged,
                    )
                    .width(Length::Fixed(180.0)),
                    button(text("Reset 3D")).on_press(SmithChartMessage::ResetSmithSphereRotation),
                ]
                .align_y(Alignment::Center)
                .spacing(10)
                .into(),
                chart(),
            ],
        ),
        result_panel(state, result),
        frequency_plot_panel(state, &state.solve()),
        settings_and_features_section(state),
        tutorial_references_section(),
        formula_references_section(),
        section(
            "Import & Export",
            vec![url_section(state), svg_export_section(state)],
        ),
    ]
    .spacing(16)
    .padding(20)
    .width(Length::Fill);

    container(scrollable(body))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn input_row<'a>(
    label: &'static str,
    unit: &'static str,
    value: &'a str,
    on_change: fn(String) -> SmithChartMessage,
) -> Element<'a, SmithChartMessage> {
    row![
        text(label).width(Length::Fixed(118.0)),
        text_input("", value)
            .on_input(on_change)
            .width(Length::FillPortion(2)),
        text(unit).width(Length::Fixed(72.0)),
    ]
    .align_y(Alignment::Center)
    .spacing(8)
    .into()
}

fn unit_input_row<'a>(
    label: &'static str,
    value: &'a str,
    unit: ScalarUnit,
    on_change: fn(String) -> SmithChartMessage,
    on_unit_change: fn(ScalarUnit) -> SmithChartMessage,
) -> Element<'a, SmithChartMessage> {
    row![
        text(label).width(Length::Fixed(118.0)),
        text_input("", value)
            .on_input(on_change)
            .width(Length::FillPortion(2)),
        pick_list(FREQUENCY_UNITS, Some(unit), on_unit_change)
            .width(Length::Fixed(92.0))
            .padding(4),
    ]
    .align_y(Alignment::Center)
    .spacing(8)
    .into()
}

fn checkbox_row<'a>(
    label: &'static str,
    checked: bool,
    on_toggle: fn(bool) -> SmithChartMessage,
) -> Element<'a, SmithChartMessage> {
    checkbox(checked)
        .on_toggle(on_toggle)
        .label(label)
        .size(14)
        .spacing(8)
        .into()
}

fn section<'a>(
    title: &'static str,
    rows: Vec<Element<'a, SmithChartMessage>>,
) -> Element<'a, SmithChartMessage> {
    let mut content = column![text(title).size(14)].spacing(8);
    for row in rows {
        content = content.push(row);
    }
    container(content)
        .padding(12)
        .width(Length::Fill)
        .style(container::rounded_box)
        .into()
}
