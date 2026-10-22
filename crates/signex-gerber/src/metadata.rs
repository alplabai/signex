use lib_gerber_edit::gerber_types::{
    Aperture, Command, CommentContent, ExtendedCode, FunctionCode, GCode, StandardComment, Unit,
};
use lib_gerber_edit::layer::LayerData;

use crate::{Bounds, LoadedLayer};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerDefinition {
    pub code: String,
    pub description: String,
    pub usage_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerDefinitionGroup {
    pub layer_name: String,
    pub definition_label: &'static str,
    pub definitions: Vec<LayerDefinition>,
}

/// Display-ready summary of the parsed data available for one fabrication layer.
#[derive(Debug, Clone, PartialEq)]
pub struct LayerMetadata {
    pub file_name: String,
    pub source: String,
    pub format: &'static str,
    pub layer_role: String,
    pub units: String,
    pub coordinate_format: Option<String>,
    pub bounds: Option<Bounds>,
    pub primitive_count: usize,
    pub definition_label: &'static str,
    pub definitions: Vec<String>,
    pub attributes: Vec<String>,
    pub warnings: Vec<String>,
}

impl LoadedLayer {
    pub fn definition_group(&self) -> LayerDefinitionGroup {
        match &self.data {
            LayerData::Gerber(layer) => {
                let mut definitions = layer
                    .apertures
                    .iter()
                    .map(|(code, aperture)| LayerDefinition {
                        code: format!("D{code}"),
                        description: describe_aperture(aperture),
                        usage_count: self
                            .geometry
                            .primitives
                            .iter()
                            .filter(|primitive| {
                                matches!(
                                    primitive,
                                    crate::GerberPrimitive::Stroke {
                                        d_code: Some(value),
                                        ..
                                    } | crate::GerberPrimitive::Flash {
                                        d_code: Some(value),
                                        ..
                                    } if value == code
                                )
                            })
                            .count(),
                    })
                    .collect::<Vec<_>>();
                definitions.sort_by_key(|definition| {
                    definition
                        .code
                        .trim_start_matches('D')
                        .parse::<i32>()
                        .unwrap_or_default()
                });
                LayerDefinitionGroup {
                    layer_name: self.name.clone(),
                    definition_label: "D-codes",
                    definitions,
                }
            }
            LayerData::Excellon(layer) => {
                let mut definitions = layer
                    .tools
                    .iter()
                    .map(|(tool, diameter)| LayerDefinition {
                        code: format!("T{tool}"),
                        description: format!("⌀{diameter:.4} {}", unit_suffix(layer.unit.unit),),
                        usage_count: self
                            .geometry
                            .primitives
                            .iter()
                            .filter(|primitive| {
                                matches!(
                                    primitive,
                                    crate::GerberPrimitive::DrillHit {
                                        tool: Some(value),
                                        ..
                                    } | crate::GerberPrimitive::DrillSlot {
                                        tool: Some(value),
                                        ..
                                    } if value == tool
                                )
                            })
                            .count(),
                    })
                    .collect::<Vec<_>>();
                definitions.sort_by_key(|definition| {
                    definition
                        .code
                        .trim_start_matches('T')
                        .parse::<u32>()
                        .unwrap_or_default()
                });
                LayerDefinitionGroup {
                    layer_name: self.name.clone(),
                    definition_label: "Drill tools",
                    definitions,
                }
            }
            LayerData::Info(_) => LayerDefinitionGroup {
                layer_name: self.name.clone(),
                definition_label: "Definitions",
                definitions: Vec::new(),
            },
        }
    }

    pub fn metadata(&self) -> LayerMetadata {
        let source = self
            .source_path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "In-memory source".to_owned());
        let layer_role = self.layer_type.to_string().trim().to_owned();

        let mut metadata = match &self.data {
            LayerData::Gerber(layer) => {
                let format = &layer.coordinate_format;
                let definitions = self
                    .definition_group()
                    .definitions
                    .into_iter()
                    .map(|definition| format!("{}: {}", definition.code, definition.description))
                    .collect();
                let attributes = layer
                    .header
                    .iter()
                    .chain(&layer.commands)
                    .filter_map(attribute_description)
                    .collect();

                LayerMetadata {
                    file_name: self.name.clone(),
                    source,
                    format: "Gerber RS-274X",
                    layer_role,
                    units: "Millimetres (normalized)".to_owned(),
                    coordinate_format: Some(format!(
                        "{}.{} digits, {:?}, {:?}",
                        format.integer,
                        format.decimal,
                        format.zero_omission,
                        format.coordinate_mode,
                    )),
                    bounds: self.geometry.bounds,
                    primitive_count: self.geometry.primitives.len(),
                    definition_label: "Apertures",
                    definitions,
                    attributes,
                    warnings: self.geometry.warnings.clone(),
                }
            }
            LayerData::Excellon(layer) => {
                let definitions = self
                    .definition_group()
                    .definitions
                    .into_iter()
                    .map(|definition| {
                        format!(
                            "{}: {}",
                            definition.code,
                            definition.description.trim_start_matches('⌀'),
                        )
                    })
                    .collect();

                LayerMetadata {
                    file_name: self.name.clone(),
                    source,
                    format: "Excellon drill",
                    layer_role,
                    units: unit_name(layer.unit.unit).to_owned(),
                    coordinate_format: Some(layer.unit.to_string()),
                    bounds: self.geometry.bounds,
                    primitive_count: self.geometry.primitives.len(),
                    definition_label: "Tools",
                    definitions,
                    attributes: Vec::new(),
                    warnings: self.geometry.warnings.clone(),
                }
            }
            LayerData::Info(_) => LayerMetadata {
                file_name: self.name.clone(),
                source,
                format: "Text information",
                layer_role,
                units: "Not applicable".to_owned(),
                coordinate_format: None,
                bounds: None,
                primitive_count: 0,
                definition_label: "Definitions",
                definitions: Vec::new(),
                attributes: Vec::new(),
                warnings: Vec::new(),
            },
        };

        if let Some(context) = &self.job_context {
            metadata
                .attributes
                .push(format!("Job file: {}", context.job_path.display(),));
            if let Some(function) = &context.file_attributes.file_function {
                metadata
                    .attributes
                    .push(format!("Job file function: {function}"));
            }
            if let Some(polarity) = &context.file_attributes.file_polarity {
                metadata
                    .attributes
                    .push(format!("Job file polarity: {polarity}"));
            }
            if let Some(format) = &context.file_attributes.file_format {
                metadata
                    .attributes
                    .push(format!("Job file format: {format}"));
            }
        }

        metadata
    }
}

fn describe_aperture(aperture: &Aperture) -> String {
    match aperture {
        Aperture::Circle(circle) => format!("circle, ⌀{:.4} mm", circle.diameter),
        Aperture::Rectangle(rectangle) => {
            format!("rectangle, {:.4} ⨯ {:.4} mm", rectangle.x, rectangle.y)
        }
        Aperture::Obround(rectangle) => {
            format!("obround, {:.4} ⨯ {:.4} mm", rectangle.x, rectangle.y)
        }
        Aperture::Polygon(polygon) => format!(
            "polygon, ⌀{:.4} mm, {} vertices, {:.2}°",
            polygon.diameter,
            polygon.vertices,
            polygon.rotation.unwrap_or_default(),
        ),
        Aperture::Macro(name, _) => format!("macro, {name}"),
    }
}

fn attribute_description(command: &Command) -> Option<String> {
    match command {
        Command::ExtendedCode(ExtendedCode::FileAttribute(attribute)) => {
            Some(format!("File: {attribute:?}"))
        }
        Command::ExtendedCode(ExtendedCode::ApertureAttribute(attribute)) => {
            Some(format!("Aperture: {attribute:?}"))
        }
        Command::ExtendedCode(ExtendedCode::ObjectAttribute(attribute)) => {
            Some(format!("Object: {attribute:?}"))
        }
        Command::FunctionCode(FunctionCode::GCode(GCode::Comment(CommentContent::Standard(
            StandardComment::FileAttribute(attribute),
        )))) => Some(format!("File: {attribute:?}")),
        Command::FunctionCode(FunctionCode::GCode(GCode::Comment(CommentContent::Standard(
            StandardComment::ApertureAttribute(attribute),
        )))) => Some(format!("Aperture: {attribute:?}")),
        Command::FunctionCode(FunctionCode::GCode(GCode::Comment(CommentContent::Standard(
            StandardComment::ObjectAttribute(attribute),
        )))) => Some(format!("Object: {attribute:?}")),
        _ => None,
    }
}

fn unit_name(unit: Unit) -> &'static str {
    match unit {
        Unit::Inches => "Inches",
        Unit::Millimeters => "Millimetres",
    }
}

fn unit_suffix(unit: Unit) -> &'static str {
    match unit {
        Unit::Inches => "in",
        Unit::Millimeters => "mm",
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    #[test]
    fn gerber_metadata_reports_normalized_format_apertures_and_attributes() {
        let layer = crate::load_gerber_reader(
            "copper.gtl",
            Cursor::new(
                b"%FSLAX46Y46*%\n%MOIN*%\n%TF.GenerationSoftware,Signex,Fixture,1.0*%\n%ADD10C,0.010*%\nD10*\nX0Y0D03*\nM02*\n",
            ),
        )
        .expect("test Gerber must parse");

        let metadata = layer.metadata();

        assert_eq!(metadata.file_name, "copper.gtl");
        assert_eq!(metadata.format, "Gerber RS-274X");
        assert_eq!(metadata.units, "Millimetres (normalized)");
        assert_eq!(
            metadata.coordinate_format.as_deref(),
            Some("4.6 digits, Leading, Absolute"),
        );
        assert_eq!(metadata.definition_label, "Apertures");
        assert_eq!(metadata.definitions, vec!["D10: circle, ⌀0.2540 mm"]);
        assert_eq!(metadata.primitive_count, 1);
        assert!(
            metadata
                .attributes
                .iter()
                .any(|attribute| attribute.contains("GenerationSoftware")),
        );
    }

    #[test]
    fn excellon_metadata_reports_declared_unit_and_sorted_tools() {
        let layer = crate::load_excellon_reader(
            "holes.drl",
            Cursor::new(b"M48\nMETRIC\nT02C1.2\nT01C0.8\n%\nG05\nT01\nX1.0Y1.0\nM30\n"),
        )
        .expect("test Excellon must parse");

        let metadata = layer.metadata();

        assert_eq!(metadata.format, "Excellon drill");
        assert_eq!(metadata.units, "Millimetres");
        assert_eq!(metadata.definition_label, "Tools");
        assert_eq!(metadata.definitions, vec!["T1: 0.8000 mm", "T2: 1.2000 mm"],);
        assert_eq!(metadata.primitive_count, 1);
        assert!(metadata.attributes.is_empty());
    }

    #[test]
    fn definition_groups_report_sorted_codes_and_rendered_usage() {
        let gerber = crate::load_gerber_reader(
            "copper.gtl",
            Cursor::new(
                b"%FSLAX46Y46*%\n%MOMM*%\n%ADD11R,1.0X2.0*%\n%ADD10C,0.5*%\nD10*\nX0Y0D03*\nD11*\nX0Y0D02*\nX1000000Y0D01*\nM02*\n",
            ),
        )
        .expect("test Gerber must parse");
        let drill = crate::load_excellon_reader(
            "holes.drl",
            Cursor::new(b"M48\nMETRIC\nT02C1.2\nT01C0.8\n%\nG05\nT01\nX1.0Y1.0\nX2.0Y2.0\nM30\n"),
        )
        .expect("test Excellon must parse");

        let gerber_group = gerber.definition_group();
        assert_eq!(gerber_group.layer_name, "copper.gtl");
        assert_eq!(gerber_group.definition_label, "D-codes");
        assert_eq!(
            gerber_group
                .definitions
                .iter()
                .map(|definition| (definition.code.as_str(), definition.usage_count,))
                .collect::<Vec<_>>(),
            vec![("D10", 1), ("D11", 1)],
        );

        let drill_group = drill.definition_group();
        assert_eq!(drill_group.definition_label, "Drill tools");
        assert_eq!(
            drill_group
                .definitions
                .iter()
                .map(|definition| (definition.code.as_str(), definition.usage_count,))
                .collect::<Vec<_>>(),
            vec![("T1", 2), ("T2", 0)],
        );
    }
}
