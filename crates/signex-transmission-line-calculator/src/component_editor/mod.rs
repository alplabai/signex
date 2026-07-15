use super::*;
use iced::widget::column;

mod circuit_component_field;
mod circuit_component_kind;
mod circuit_editor_component;

pub use circuit_component_field::CircuitComponentField;
pub use circuit_component_kind::CircuitComponentKind;
pub use circuit_editor_component::CircuitEditorComponent;

#[cfg(test)]
mod tests;

impl CircuitEditorComponent {
    pub fn new(kind: CircuitComponentKind) -> Self {
        let mut component = Self {
            kind,
            primary: String::new(),
            secondary: String::new(),
            tertiary: String::new(),
            tolerance: "0".to_string(),
            transformer_model: TransformerModel::CoupledInductor,
            interpolation: CustomInterpolation::Linear,
        };
        match kind {
            CircuitComponentKind::BlackBox => {
                component.primary = "50".to_string();
                component.secondary = "0".to_string();
            }
            CircuitComponentKind::ShortedCapacitor | CircuitComponentKind::SeriesCapacitor => {
                component.primary = "1".to_string();
                component.secondary = "0".to_string();
                component.tertiary = "0".to_string();
            }
            CircuitComponentKind::ShortedInductor | CircuitComponentKind::SeriesInductor => {
                component.primary = "1".to_string();
                component.secondary = "0".to_string();
            }
            CircuitComponentKind::ShortedResistor | CircuitComponentKind::SeriesResistor => {
                component.primary = "50".to_string();
                component.secondary = "0".to_string();
            }
            CircuitComponentKind::ParallelRlc => {
                component.primary = "50".to_string();
                component.secondary = "1".to_string();
                component.tertiary = "1".to_string();
            }
            CircuitComponentKind::Custom => {
                component.primary = "900,50,0;1000,40,10;1100,55,-5".to_string();
            }
            CircuitComponentKind::TransmissionLine
            | CircuitComponentKind::Stub
            | CircuitComponentKind::ShortedStub => {
                component.primary = "1".to_string();
                component.secondary = "50".to_string();
                component.tertiary = "1".to_string();
            }
            CircuitComponentKind::Transformer => {
                component.primary = "1".to_string();
                component.secondary = "1".to_string();
                component.tertiary = "1".to_string();
            }
            CircuitComponentKind::SParameters => {
                component.primary = DEFAULT_S_PARAMETER_TEXT.replace('\n', "|");
            }
        }
        component
    }

    pub fn set_field(&mut self, field: CircuitComponentField, value: String) {
        match field {
            CircuitComponentField::Primary => self.primary = value,
            CircuitComponentField::Secondary => self.secondary = value,
            CircuitComponentField::Tertiary => self.tertiary = value,
            CircuitComponentField::Tolerance => self.tolerance = value,
        }
    }

    pub fn to_element(&self) -> Result<SmithChartElement, String> {
        let tolerance_percent = || optional_tolerance("component tolerance", &self.tolerance);
        Ok(match self.kind {
            CircuitComponentKind::BlackBox => SmithChartElement::BlackBox {
                impedance: Complex::new(
                    parse_field("black box resistance", &self.primary)?,
                    parse_field("black box reactance", &self.secondary)?,
                ),
                tolerance_percent: tolerance_percent()?,
            },
            CircuitComponentKind::ShortedCapacitor => SmithChartElement::ShuntCapacitor {
                capacitance_f: parse_field("shorted capacitance", &self.primary)? * 1.0e-12,
                esr_ohm: parse_optional("shorted capacitor ESR", &self.secondary)?,
                esl_h: parse_optional("shorted capacitor ESL", &self.tertiary)? * 1.0e-9,
                tolerance_percent: tolerance_percent()?,
            },
            CircuitComponentKind::SeriesCapacitor => SmithChartElement::SeriesCapacitor {
                capacitance_f: parse_field("series capacitance", &self.primary)? * 1.0e-12,
                esr_ohm: parse_optional("series capacitor ESR", &self.secondary)?,
                esl_h: parse_optional("series capacitor ESL", &self.tertiary)? * 1.0e-9,
                tolerance_percent: tolerance_percent()?,
            },
            CircuitComponentKind::ShortedInductor => SmithChartElement::ShuntInductor {
                inductance_h: parse_field("shorted inductance", &self.primary)? * 1.0e-9,
                esr_ohm: parse_optional("shorted inductor ESR", &self.secondary)?,
                tolerance_percent: tolerance_percent()?,
            },
            CircuitComponentKind::SeriesInductor => SmithChartElement::SeriesInductor {
                inductance_h: parse_field("series inductance", &self.primary)? * 1.0e-9,
                esr_ohm: parse_optional("series inductor ESR", &self.secondary)?,
                tolerance_percent: tolerance_percent()?,
            },
            CircuitComponentKind::ShortedResistor => SmithChartElement::ShuntResistor {
                resistance_ohm: parse_field("shorted resistance", &self.primary)?,
                esl_h: parse_optional("shorted resistor ESL", &self.secondary)? * 1.0e-9,
                tolerance_percent: tolerance_percent()?,
            },
            CircuitComponentKind::SeriesResistor => SmithChartElement::SeriesResistor {
                resistance_ohm: parse_field("series resistance", &self.primary)?,
                esl_h: parse_optional("series resistor ESL", &self.secondary)? * 1.0e-9,
                tolerance_percent: tolerance_percent()?,
            },
            CircuitComponentKind::ParallelRlc => SmithChartElement::SeriesParallelRlc {
                resistance_ohm: parse_field("parallel RLC resistance", &self.primary)?,
                inductance_h: parse_field("parallel RLC inductance", &self.secondary)? * 1.0e-9,
                capacitance_f: parse_field("parallel RLC capacitance", &self.tertiary)? * 1.0e-12,
            },
            CircuitComponentKind::Custom => SmithChartElement::Custom {
                points: parse_custom_points(&self.primary)?,
                interpolation: self.interpolation,
            },
            CircuitComponentKind::TransmissionLine => SmithChartElement::TransmissionLine {
                length_m: parse_field("transmission line length", &self.primary)? * 1.0e-3,
                characteristic_impedance_ohm: parse_field(
                    "transmission line impedance",
                    &self.secondary,
                )?,
                effective_dielectric: parse_field(
                    "transmission line effective dielectric",
                    &self.tertiary,
                )?,
                tolerance_percent: tolerance_percent()?,
            },
            CircuitComponentKind::Stub => SmithChartElement::OpenStub {
                length_m: parse_field("stub length", &self.primary)? * 1.0e-3,
                characteristic_impedance_ohm: parse_field("stub impedance", &self.secondary)?,
                effective_dielectric: parse_field("stub effective dielectric", &self.tertiary)?,
                tolerance_percent: tolerance_percent()?,
            },
            CircuitComponentKind::ShortedStub => SmithChartElement::ShortedStub {
                length_m: parse_field("shorted stub length", &self.primary)? * 1.0e-3,
                characteristic_impedance_ohm: parse_field(
                    "shorted stub impedance",
                    &self.secondary,
                )?,
                effective_dielectric: parse_field(
                    "shorted stub effective dielectric",
                    &self.tertiary,
                )?,
                tolerance_percent: tolerance_percent()?,
            },
            CircuitComponentKind::Transformer => SmithChartElement::Transformer {
                model: self.transformer_model,
                l1_h: parse_field("transformer L1", &self.primary)? * 1.0e-9,
                l2_h: parse_field("transformer L2", &self.secondary)? * 1.0e-9,
                coupling_or_turns_ratio: parse_field(
                    match self.transformer_model {
                        TransformerModel::Ideal => "transformer turns ratio",
                        TransformerModel::CoupledInductor => "transformer coupling",
                    },
                    &self.tertiary,
                )?,
            },
            CircuitComponentKind::SParameters => {
                SmithChartElement::SParameter(parse_touchstone_input(&self.primary)?)
            }
        })
    }

    pub fn from_element(element: &SmithChartElement) -> Self {
        match element {
            SmithChartElement::BlackBox {
                impedance,
                tolerance_percent,
            }
            | SmithChartElement::LoadTermination {
                impedance,
                tolerance_percent,
            } => Self {
                primary: format_number(impedance.re),
                secondary: format_number(impedance.im),
                tolerance: format_optional(*tolerance_percent),
                ..Self::new(CircuitComponentKind::BlackBox)
            },
            SmithChartElement::SeriesCapacitor {
                capacitance_f,
                esr_ohm,
                esl_h,
                tolerance_percent,
            } => Self {
                primary: format_number(*capacitance_f / 1.0e-12),
                secondary: format_number(*esr_ohm),
                tertiary: format_number(*esl_h / 1.0e-9),
                tolerance: format_optional(*tolerance_percent),
                ..Self::new(CircuitComponentKind::SeriesCapacitor)
            },
            SmithChartElement::ShuntCapacitor {
                capacitance_f,
                esr_ohm,
                esl_h,
                tolerance_percent,
            } => Self {
                primary: format_number(*capacitance_f / 1.0e-12),
                secondary: format_number(*esr_ohm),
                tertiary: format_number(*esl_h / 1.0e-9),
                tolerance: format_optional(*tolerance_percent),
                ..Self::new(CircuitComponentKind::ShortedCapacitor)
            },
            SmithChartElement::SeriesInductor {
                inductance_h,
                esr_ohm,
                tolerance_percent,
            } => Self {
                primary: format_number(*inductance_h / 1.0e-9),
                secondary: format_number(*esr_ohm),
                tolerance: format_optional(*tolerance_percent),
                ..Self::new(CircuitComponentKind::SeriesInductor)
            },
            SmithChartElement::ShuntInductor {
                inductance_h,
                esr_ohm,
                tolerance_percent,
            } => Self {
                primary: format_number(*inductance_h / 1.0e-9),
                secondary: format_number(*esr_ohm),
                tolerance: format_optional(*tolerance_percent),
                ..Self::new(CircuitComponentKind::ShortedInductor)
            },
            SmithChartElement::SeriesResistor {
                resistance_ohm,
                esl_h,
                tolerance_percent,
            } => Self {
                primary: format_number(*resistance_ohm),
                secondary: format_number(*esl_h / 1.0e-9),
                tolerance: format_optional(*tolerance_percent),
                ..Self::new(CircuitComponentKind::SeriesResistor)
            },
            SmithChartElement::ShuntResistor {
                resistance_ohm,
                esl_h,
                tolerance_percent,
            } => Self {
                primary: format_number(*resistance_ohm),
                secondary: format_number(*esl_h / 1.0e-9),
                tolerance: format_optional(*tolerance_percent),
                ..Self::new(CircuitComponentKind::ShortedResistor)
            },
            SmithChartElement::SeriesParallelRlc {
                resistance_ohm,
                inductance_h,
                capacitance_f,
            } => Self {
                primary: format_number(*resistance_ohm),
                secondary: format_number(*inductance_h / 1.0e-9),
                tertiary: format_number(*capacitance_f / 1.0e-12),
                ..Self::new(CircuitComponentKind::ParallelRlc)
            },
            SmithChartElement::Custom {
                points,
                interpolation,
            } => Self {
                primary: points
                    .iter()
                    .map(|point| {
                        format!(
                            "{},{},{}",
                            format_number(point.frequency_hz / 1.0e6),
                            format_number(point.impedance.re),
                            format_number(point.impedance.im)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(";"),
                interpolation: *interpolation,
                ..Self::new(CircuitComponentKind::Custom)
            },
            SmithChartElement::TransmissionLine {
                length_m,
                characteristic_impedance_ohm,
                effective_dielectric,
                tolerance_percent,
            } => Self {
                primary: format_number(*length_m / 1.0e-3),
                secondary: format_number(*characteristic_impedance_ohm),
                tertiary: format_number(*effective_dielectric),
                tolerance: format_optional(*tolerance_percent),
                ..Self::new(CircuitComponentKind::TransmissionLine)
            },
            SmithChartElement::OpenStub {
                length_m,
                characteristic_impedance_ohm,
                effective_dielectric,
                tolerance_percent,
            } => Self {
                primary: format_number(*length_m / 1.0e-3),
                secondary: format_number(*characteristic_impedance_ohm),
                tertiary: format_number(*effective_dielectric),
                tolerance: format_optional(*tolerance_percent),
                ..Self::new(CircuitComponentKind::Stub)
            },
            SmithChartElement::ShortedStub {
                length_m,
                characteristic_impedance_ohm,
                effective_dielectric,
                tolerance_percent,
            } => Self {
                primary: format_number(*length_m / 1.0e-3),
                secondary: format_number(*characteristic_impedance_ohm),
                tertiary: format_number(*effective_dielectric),
                tolerance: format_optional(*tolerance_percent),
                ..Self::new(CircuitComponentKind::ShortedStub)
            },
            SmithChartElement::Transformer {
                model,
                l1_h,
                l2_h,
                coupling_or_turns_ratio,
            } => Self {
                primary: format_number(*l1_h / 1.0e-9),
                secondary: format_number(*l2_h / 1.0e-9),
                tertiary: format_number(*coupling_or_turns_ratio),
                transformer_model: *model,
                ..Self::new(CircuitComponentKind::Transformer)
            },
            SmithChartElement::SParameter(block) => Self {
                primary: block.raw.replace('\n', "|"),
                ..Self::new(CircuitComponentKind::SParameters)
            },
        }
    }
}

pub(super) fn component_editor_section(state: &SmithChartState) -> Element<'_, SmithChartMessage> {
    let picker = pick_list(
        CircuitComponentKind::PICKER_OPTIONS,
        None::<CircuitComponentKind>,
        SmithChartMessage::AddCircuitComponent,
    )
    .placeholder("Add component…")
    .width(Length::Fixed(240.0));

    let mut component_strip = row![].spacing(10).align_y(Alignment::Start);
    for (index, component) in state.circuit_components.iter().enumerate() {
        component_strip = component_strip.push(component_card(
            index,
            state.circuit_components.len(),
            component,
        ));
    }

    let strip = scrollable(component_strip)
        .direction(scrollable::Direction::Horizontal(
            scrollable::Scrollbar::new(),
        ))
        .width(Length::Fill);

    section(
        "Component Editor",
        vec![
            row![
                text("Circuit is evaluated from left to right."),
                Space::new().width(Length::Fill),
                picker,
            ]
            .align_y(Alignment::Center)
            .spacing(12)
            .into(),
            strip.into(),
        ],
    )
}

fn component_card(
    index: usize,
    component_count: usize,
    component: &CircuitEditorComponent,
) -> Element<'_, SmithChartMessage> {
    let mut heading = row![
        text(format!("DP{index}")).size(15),
        text(component.kind.to_string()).size(13),
    ]
    .spacing(8)
    .align_y(Alignment::Center);
    if index > 0 {
        heading = heading.push(Space::new().width(Length::Fill));
        if index > 1 {
            heading = heading.push(
                button(text("←")).on_press(SmithChartMessage::MoveCircuitComponentLeft(index)),
            );
        }
        if index + 1 < component_count {
            heading = heading.push(
                button(text("→")).on_press(SmithChartMessage::MoveCircuitComponentRight(index)),
            );
        }
        heading = heading.push(
            button(text("Delete")).on_press(SmithChartMessage::RemoveCircuitComponent(index)),
        );
    }

    let mut editor = column![heading].spacing(6);
    match component.kind {
        CircuitComponentKind::BlackBox => {
            editor = editor
                .push(component_input(
                    index,
                    "Re",
                    "Ω",
                    &component.primary,
                    CircuitComponentField::Primary,
                ))
                .push(component_input(
                    index,
                    "Im",
                    "Ω",
                    &component.secondary,
                    CircuitComponentField::Secondary,
                ))
                .push(component_input(
                    index,
                    "Tolerance",
                    "%",
                    &component.tolerance,
                    CircuitComponentField::Tolerance,
                ));
        }
        CircuitComponentKind::ShortedCapacitor | CircuitComponentKind::SeriesCapacitor => {
            editor = editor
                .push(component_input(
                    index,
                    "C",
                    "pF",
                    &component.primary,
                    CircuitComponentField::Primary,
                ))
                .push(component_input(
                    index,
                    "ESR",
                    "Ω",
                    &component.secondary,
                    CircuitComponentField::Secondary,
                ))
                .push(component_input(
                    index,
                    "ESL",
                    "nH",
                    &component.tertiary,
                    CircuitComponentField::Tertiary,
                ))
                .push(component_input(
                    index,
                    "Tolerance",
                    "%",
                    &component.tolerance,
                    CircuitComponentField::Tolerance,
                ));
        }
        CircuitComponentKind::ShortedInductor | CircuitComponentKind::SeriesInductor => {
            editor = editor
                .push(component_input(
                    index,
                    "L",
                    "nH",
                    &component.primary,
                    CircuitComponentField::Primary,
                ))
                .push(component_input(
                    index,
                    "ESR",
                    "Ω",
                    &component.secondary,
                    CircuitComponentField::Secondary,
                ))
                .push(component_input(
                    index,
                    "Tolerance",
                    "%",
                    &component.tolerance,
                    CircuitComponentField::Tolerance,
                ));
        }
        CircuitComponentKind::ShortedResistor | CircuitComponentKind::SeriesResistor => {
            editor = editor
                .push(component_input(
                    index,
                    "R",
                    "Ω",
                    &component.primary,
                    CircuitComponentField::Primary,
                ))
                .push(component_input(
                    index,
                    "ESL",
                    "nH",
                    &component.secondary,
                    CircuitComponentField::Secondary,
                ))
                .push(component_input(
                    index,
                    "Tolerance",
                    "%",
                    &component.tolerance,
                    CircuitComponentField::Tolerance,
                ));
        }
        CircuitComponentKind::ParallelRlc => {
            editor = editor
                .push(component_input(
                    index,
                    "R",
                    "Ω",
                    &component.primary,
                    CircuitComponentField::Primary,
                ))
                .push(component_input(
                    index,
                    "L",
                    "nH",
                    &component.secondary,
                    CircuitComponentField::Secondary,
                ))
                .push(component_input(
                    index,
                    "C",
                    "pF",
                    &component.tertiary,
                    CircuitComponentField::Tertiary,
                ));
        }
        CircuitComponentKind::Custom => {
            editor = editor
                .push(component_input(
                    index,
                    "MHz,Re,Im",
                    "",
                    &component.primary,
                    CircuitComponentField::Primary,
                ))
                .push(pick_list(
                    CUSTOM_INTERPOLATIONS,
                    Some(component.interpolation),
                    move |value| SmithChartMessage::CircuitInterpolationChanged(index, value),
                ));
        }
        CircuitComponentKind::TransmissionLine
        | CircuitComponentKind::Stub
        | CircuitComponentKind::ShortedStub => {
            editor = editor
                .push(component_input(
                    index,
                    "Length",
                    "mm",
                    &component.primary,
                    CircuitComponentField::Primary,
                ))
                .push(component_input(
                    index,
                    "Z₀",
                    "Ω",
                    &component.secondary,
                    CircuitComponentField::Secondary,
                ))
                .push(component_input(
                    index,
                    "εeff",
                    "",
                    &component.tertiary,
                    CircuitComponentField::Tertiary,
                ))
                .push(component_input(
                    index,
                    "Tolerance",
                    "%",
                    &component.tolerance,
                    CircuitComponentField::Tolerance,
                ));
        }
        CircuitComponentKind::Transformer => {
            editor = editor
                .push(component_input(
                    index,
                    "L1",
                    "nH",
                    &component.primary,
                    CircuitComponentField::Primary,
                ))
                .push(component_input(
                    index,
                    "L2",
                    "nH",
                    &component.secondary,
                    CircuitComponentField::Secondary,
                ))
                .push(component_input(
                    index,
                    if component.transformer_model == TransformerModel::Ideal {
                        "Ratio"
                    } else {
                        "k"
                    },
                    "",
                    &component.tertiary,
                    CircuitComponentField::Tertiary,
                ))
                .push(pick_list(
                    TRANSFORMER_MODELS,
                    Some(component.transformer_model),
                    move |value| SmithChartMessage::CircuitTransformerModelChanged(index, value),
                ));
        }
        CircuitComponentKind::SParameters => {
            editor = editor
                .push(component_input(
                    index,
                    "Touchstone",
                    "",
                    &component.primary,
                    CircuitComponentField::Primary,
                ))
                .push(
                    button(text("Import .s1p/.s2p"))
                        .on_press(SmithChartMessage::ImportSParameterFile),
                );
        }
    }

    container(editor)
        .padding(10)
        .width(Length::Fixed(330.0))
        .style(container::rounded_box)
        .into()
}

fn component_input<'a>(
    index: usize,
    label: &'static str,
    unit: &'static str,
    value: &'a str,
    field: CircuitComponentField,
) -> Element<'a, SmithChartMessage> {
    row![
        text(label).width(Length::Fixed(72.0)),
        text_input("", value)
            .on_input(
                move |value| SmithChartMessage::CircuitComponentFieldChanged {
                    index,
                    field,
                    value,
                }
            )
            .width(Length::Fill),
        text(unit).width(Length::Fixed(34.0)),
    ]
    .align_y(Alignment::Center)
    .spacing(5)
    .into()
}
