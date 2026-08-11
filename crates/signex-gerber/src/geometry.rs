use std::f64::consts::TAU;

use lib_gerber_edit::LayerCorners;
use lib_gerber_edit::excellon_format::{
    Command as ExcellonCommand, ExcellonLayerData, GeometricCode, InputMode,
};
use lib_gerber_edit::gerber::GerberLayerData;
use lib_gerber_edit::gerber_types::{
    Aperture, AttributeDeletionCriterion, Command, CommentContent, CoordinateMode,
    CoordinateOffset, Coordinates, DCode, ExtendedCode, FunctionCode, GCode, InterpolationMode,
    Net, ObjectAttribute, Operation, Polarity, StandardComment,
};

use crate::object_attributes::{GerberObjectAttributes, normalize_object_attribute};

/// A point in Gerber world space, expressed in millimetres.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

/// Axis-aligned Gerber ink bounds in millimetres.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Bounds {
    pub min: Point,
    pub max: Point,
}

impl Bounds {
    pub fn width(self) -> f64 {
        self.max.x - self.min.x
    }

    pub fn height(self) -> f64 {
        self.max.y - self.min.y
    }

    pub fn is_finite(self) -> bool {
        self.min.x.is_finite()
            && self.min.y.is_finite()
            && self.max.x.is_finite()
            && self.max.y.is_finite()
    }
}

/// Dark artwork adds material; clear artwork removes it from the current layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimitivePolarity {
    Dark,
    Clear,
}

/// Standard RS-274X aperture shapes used by flashed items.
#[derive(Debug, Clone, PartialEq)]
pub enum ApertureShape {
    Circle {
        diameter: f64,
    },
    Rectangle {
        width: f64,
        height: f64,
    },
    Obround {
        width: f64,
        height: f64,
    },
    Polygon {
        diameter: f64,
        vertices: u8,
        rotation_degrees: f64,
    },
    Macro {
        name: String,
    },
}

impl ApertureShape {
    pub fn maximum_extent(&self) -> f64 {
        match self {
            Self::Circle { diameter } | Self::Polygon { diameter, .. } => *diameter,
            Self::Rectangle { width, height } | Self::Obround { width, height } => {
                (*width).max(*height)
            }
            Self::Macro { .. } => 0.15,
        }
    }
}

/// Renderer-independent Gerber drawing primitive.
#[derive(Debug, Clone, PartialEq)]
pub enum GerberPrimitive {
    Stroke {
        start: Point,
        end: Point,
        width: f64,
        d_code: Option<i32>,
        polarity: PrimitivePolarity,
    },
    Flash {
        position: Point,
        aperture: ApertureShape,
        d_code: Option<i32>,
        polarity: PrimitivePolarity,
    },
    Region {
        points: Vec<Point>,
        polarity: PrimitivePolarity,
    },
    DrillHit {
        position: Point,
        diameter: f64,
        tool: Option<u32>,
    },
    DrillSlot {
        start: Point,
        end: Point,
        width: f64,
        tool: Option<u32>,
    },
}

/// Render-ready geometry extracted from a parsed Gerber layer.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GerberGeometry {
    pub primitives: Vec<GerberPrimitive>,
    pub primitive_attributes: Vec<GerberObjectAttributes>,
    pub bounds: Option<Bounds>,
    pub warnings: Vec<String>,
}

impl GerberGeometry {
    fn push_primitive(&mut self, primitive: GerberPrimitive, attributes: GerberObjectAttributes) {
        self.primitives.push(primitive);
        self.primitive_attributes.push(attributes);
    }

    pub fn from_layer(layer: &GerberLayerData) -> Self {
        let mut geometry = Self {
            warnings: layer.parse_errors.clone(),
            ..Self::default()
        };
        let (min, max) = layer.get_corners();
        let bounds = Bounds {
            min: Point { x: min.x, y: min.y },
            max: Point { x: max.x, y: max.y },
        };
        if bounds.is_finite() && bounds.width() >= 0.0 && bounds.height() >= 0.0 {
            geometry.bounds = Some(bounds);
        }

        let mut state = GeometryState::default();
        for command in &layer.commands {
            state.apply(command, layer, &mut geometry);
        }
        state.finish_region(&mut geometry);
        geometry
    }

    pub fn from_excellon(layer: &ExcellonLayerData) -> Self {
        let (min, max) = layer.get_corners();
        let bounds = Bounds {
            min: Point { x: min.x, y: min.y },
            max: Point { x: max.x, y: max.y },
        };
        let mut geometry = Self {
            bounds: (bounds.is_finite() && bounds.width() >= 0.0 && bounds.height() >= 0.0)
                .then_some(bounds),
            ..Self::default()
        };
        let mut current = Point::default();
        let mut tool = None;
        let mut incremental = layer
            .header
            .iter()
            .rev()
            .find_map(|command| match command {
                Ok(ExcellonCommand::Incremental(value)) => Some(*value),
                _ => None,
            })
            .unwrap_or(false);

        for command in &layer.commands {
            let command = match command {
                Ok(command) => command,
                Err(error) => {
                    geometry.warnings.push(error.to_string());
                    continue;
                }
            };
            match command {
                ExcellonCommand::Tool(id) => {
                    tool = Some(*id);
                }
                ExcellonCommand::Geometric(GeometricCode::InputMode(mode)) => {
                    incremental = *mode == InputMode::Incremental;
                }
                ExcellonCommand::Coordinate(x, y, format) => {
                    current = resolve_excellon_point(*x, *y, current, incremental, format.unit);
                    let diameter = drill_diameter(layer, tool, &mut geometry);
                    geometry.push_primitive(
                        GerberPrimitive::DrillHit {
                            position: current,
                            diameter,
                            tool,
                        },
                        GerberObjectAttributes::default(),
                    );
                }
                ExcellonCommand::Slot {
                    from_x,
                    from_y,
                    to_x,
                    to_y,
                    fmt,
                } => {
                    let start =
                        resolve_excellon_point(*from_x, *from_y, current, incremental, fmt.unit);
                    let end = resolve_excellon_point(*to_x, *to_y, start, incremental, fmt.unit);
                    let width = drill_diameter(layer, tool, &mut geometry);
                    geometry.push_primitive(
                        GerberPrimitive::DrillSlot {
                            start,
                            end,
                            width,
                            tool,
                        },
                        GerberObjectAttributes::default(),
                    );
                    current = end;
                }
                _ => {}
            }
        }
        geometry
    }
}

#[derive(Debug)]
struct GeometryState {
    current: Point,
    aperture: Option<i32>,
    interpolation: InterpolationMode,
    polarity: PrimitivePolarity,
    region_points: Option<Vec<Point>>,
    region_attributes: GerberObjectAttributes,
    object_attributes: GerberObjectAttributes,
}

impl Default for GeometryState {
    fn default() -> Self {
        Self {
            current: Point::default(),
            aperture: None,
            interpolation: InterpolationMode::Linear,
            polarity: PrimitivePolarity::Dark,
            region_points: None,
            region_attributes: GerberObjectAttributes::default(),
            object_attributes: GerberObjectAttributes::default(),
        }
    }
}

impl GeometryState {
    fn apply(&mut self, command: &Command, layer: &GerberLayerData, geometry: &mut GerberGeometry) {
        match command {
            Command::FunctionCode(FunctionCode::DCode(DCode::SelectAperture(code))) => {
                self.aperture = Some(*code);
            }
            Command::FunctionCode(FunctionCode::DCode(DCode::Operation(operation))) => {
                self.apply_operation(operation, layer, geometry);
            }
            Command::FunctionCode(FunctionCode::GCode(GCode::InterpolationMode(mode))) => {
                self.interpolation = *mode;
            }
            Command::FunctionCode(FunctionCode::GCode(GCode::RegionMode(true))) => {
                self.finish_region(geometry);
                self.region_points = Some(vec![self.current]);
                self.region_attributes = self.object_attributes.clone();
            }
            Command::FunctionCode(FunctionCode::GCode(GCode::RegionMode(false))) => {
                self.finish_region(geometry);
            }
            Command::ExtendedCode(ExtendedCode::LoadPolarity(polarity)) => {
                self.polarity = match polarity {
                    Polarity::Dark => PrimitivePolarity::Dark,
                    Polarity::Clear => PrimitivePolarity::Clear,
                };
            }
            Command::ExtendedCode(ExtendedCode::ObjectAttribute(attribute))
            | Command::FunctionCode(FunctionCode::GCode(GCode::Comment(
                CommentContent::Standard(StandardComment::ObjectAttribute(attribute)),
            ))) => {
                self.apply_object_attribute(attribute);
            }
            Command::ExtendedCode(ExtendedCode::DeleteAttribute(criterion))
            | Command::FunctionCode(FunctionCode::GCode(GCode::Comment(
                CommentContent::Standard(StandardComment::DeleteAttribute(criterion)),
            ))) => {
                self.delete_object_attribute(criterion);
            }
            _ => {}
        }
    }

    fn apply_object_attribute(&mut self, attribute: &ObjectAttribute) {
        match attribute {
            ObjectAttribute::Component(component) => {
                self.object_attributes.component = Some(component.clone());
            }
            ObjectAttribute::Net(net) => {
                self.object_attributes.nets = match net {
                    Net::None => Vec::new(),
                    Net::NotConnected => vec!["N/C".into()],
                    Net::Connected(nets) => nets.clone(),
                };
            }
            _ => {}
        }

        let attribute = normalize_object_attribute(attribute);
        self.object_attributes
            .attributes
            .retain(|active| active.name != attribute.name);
        self.object_attributes.attributes.push(attribute);
    }

    fn delete_object_attribute(&mut self, criterion: &AttributeDeletionCriterion) {
        match criterion {
            AttributeDeletionCriterion::AllApertureAndObjectAttributes => {
                self.object_attributes = GerberObjectAttributes::default();
            }
            AttributeDeletionCriterion::SingleObjectAttribute(name) if name == ".C" => {
                self.object_attributes.component = None;
                self.object_attributes
                    .attributes
                    .retain(|attribute| attribute.name != *name);
            }
            AttributeDeletionCriterion::SingleObjectAttribute(name) if name == ".N" => {
                self.object_attributes.nets.clear();
                self.object_attributes
                    .attributes
                    .retain(|attribute| attribute.name != *name);
            }
            AttributeDeletionCriterion::SingleObjectAttribute(name) => {
                self.object_attributes
                    .attributes
                    .retain(|attribute| attribute.name != *name);
            }
            _ => {}
        }
    }

    fn apply_operation(
        &mut self,
        operation: &Operation,
        layer: &GerberLayerData,
        geometry: &mut GerberGeometry,
    ) {
        match operation {
            Operation::Move(coordinates) => {
                self.current = resolve_coordinates(coordinates.as_ref(), self.current);
                let should_finish_region = self
                    .region_points
                    .as_ref()
                    .is_some_and(|points| points.len() > 1);
                if should_finish_region {
                    self.finish_region(geometry);
                    self.region_points = Some(vec![self.current]);
                    self.region_attributes = self.object_attributes.clone();
                } else if let Some(points) = &mut self.region_points {
                    points[0] = self.current;
                }
            }
            Operation::Interpolate(coordinates, offset) => {
                let end = resolve_coordinates(coordinates.as_ref(), self.current);
                let width = aperture_width(layer, self.aperture, geometry);
                let points =
                    interpolation_points(self.current, end, offset.as_ref(), self.interpolation);
                if let Some(region_points) = &mut self.region_points {
                    region_points.extend(points.into_iter().skip(1));
                } else {
                    for pair in points.windows(2) {
                        geometry.push_primitive(
                            GerberPrimitive::Stroke {
                                start: pair[0],
                                end: pair[1],
                                width,
                                d_code: self.aperture,
                                polarity: self.polarity,
                            },
                            self.object_attributes.clone(),
                        );
                    }
                }
                self.current = end;
            }
            Operation::Flash(coordinates) => {
                self.current = resolve_coordinates(coordinates.as_ref(), self.current);
                let Some(code) = self.aperture else {
                    geometry
                        .warnings
                        .push("flash encountered before an aperture was selected".into());
                    return;
                };
                let Some(aperture) = layer.apertures.get(&code) else {
                    geometry
                        .warnings
                        .push(format!("flash references undefined D-code D{code}"));
                    return;
                };
                let aperture = aperture_shape(aperture);
                if let ApertureShape::Macro { name } = &aperture {
                    geometry.warnings.push(format!(
                        "macro aperture '{name}' is represented by a prototype marker"
                    ));
                }
                geometry.push_primitive(
                    GerberPrimitive::Flash {
                        position: self.current,
                        aperture,
                        d_code: Some(code),
                        polarity: self.polarity,
                    },
                    self.object_attributes.clone(),
                );
            }
        }
    }

    fn finish_region(&mut self, geometry: &mut GerberGeometry) {
        if let Some(points) = self.region_points.take()
            && points.len() >= 3
        {
            geometry.push_primitive(
                GerberPrimitive::Region {
                    points,
                    polarity: self.polarity,
                },
                self.region_attributes.clone(),
            );
        }
    }
}

fn resolve_coordinates(coordinates: Option<&Coordinates>, current: Point) -> Point {
    let Some(coordinates) = coordinates else {
        return current;
    };
    Point {
        x: resolve_axis(
            coordinates.x.map(f64::from),
            current.x,
            coordinates.format.coordinate_mode,
        ),
        y: resolve_axis(
            coordinates.y.map(f64::from),
            current.y,
            coordinates.format.coordinate_mode,
        ),
    }
}

fn resolve_axis(value: Option<f64>, current: f64, mode: CoordinateMode) -> f64 {
    match (value, mode) {
        (Some(value), CoordinateMode::Absolute) => value,
        (Some(value), CoordinateMode::Incremental) => current + value,
        (None, _) => current,
    }
}

fn aperture_shape(aperture: &Aperture) -> ApertureShape {
    match aperture {
        Aperture::Circle(circle) => ApertureShape::Circle {
            diameter: circle.diameter,
        },
        Aperture::Rectangle(rectangle) => ApertureShape::Rectangle {
            width: rectangle.x,
            height: rectangle.y,
        },
        Aperture::Obround(rectangle) => ApertureShape::Obround {
            width: rectangle.x,
            height: rectangle.y,
        },
        Aperture::Polygon(polygon) => ApertureShape::Polygon {
            diameter: polygon.diameter,
            vertices: polygon.vertices,
            rotation_degrees: polygon.rotation.unwrap_or_default(),
        },
        Aperture::Macro(name, _) => ApertureShape::Macro { name: name.clone() },
    }
}

fn aperture_width(
    layer: &GerberLayerData,
    code: Option<i32>,
    geometry: &mut GerberGeometry,
) -> f64 {
    let Some(code) = code else {
        geometry
            .warnings
            .push("interpolation encountered before an aperture was selected".into());
        return 0.1;
    };
    let Some(aperture) = layer.apertures.get(&code) else {
        geometry
            .warnings
            .push(format!("interpolation references undefined D-code D{code}"));
        return 0.1;
    };
    aperture_shape(aperture).maximum_extent()
}

fn interpolation_points(
    start: Point,
    end: Point,
    offset: Option<&CoordinateOffset>,
    mode: InterpolationMode,
) -> Vec<Point> {
    if mode == InterpolationMode::Linear {
        return vec![start, end];
    }
    let Some(offset) = offset else {
        return vec![start, end];
    };
    let center = Point {
        x: start.x + offset.x.map(f64::from).unwrap_or_default(),
        y: start.y + offset.y.map(f64::from).unwrap_or_default(),
    };
    let radius = ((start.x - center.x).powi(2) + (start.y - center.y).powi(2)).sqrt();
    if radius <= f64::EPSILON {
        return vec![start, end];
    }

    let start_angle = (start.y - center.y).atan2(start.x - center.x);
    let end_angle = (end.y - center.y).atan2(end.x - center.x);
    let clockwise = mode == InterpolationMode::ClockwiseCircular;
    let mut sweep = end_angle - start_angle;
    if clockwise {
        if sweep >= 0.0 {
            sweep -= TAU;
        }
    } else if sweep <= 0.0 {
        sweep += TAU;
    }
    if (end.x - start.x).abs() < 1e-9 && (end.y - start.y).abs() < 1e-9 {
        sweep = if clockwise { -TAU } else { TAU };
    }

    let segments = ((sweep.abs() / (TAU / 72.0)).ceil() as usize).clamp(4, 144);
    let mut points = Vec::with_capacity(segments + 1);
    for index in 0..=segments {
        let t = index as f64 / segments as f64;
        let angle = start_angle + sweep * t;
        points.push(Point {
            x: center.x + radius * angle.cos(),
            y: center.y + radius * angle.sin(),
        });
    }
    if let Some(last) = points.last_mut() {
        *last = end;
    }
    points
}

fn resolve_excellon_point(
    x: Option<f64>,
    y: Option<f64>,
    current: Point,
    incremental: bool,
    unit: lib_gerber_edit::gerber_types::Unit,
) -> Point {
    let x = x.map(|value| excellon_to_mm(value, unit));
    let y = y.map(|value| excellon_to_mm(value, unit));
    if incremental {
        Point {
            x: current.x + x.unwrap_or_default(),
            y: current.y + y.unwrap_or_default(),
        }
    } else {
        Point {
            x: x.unwrap_or(current.x),
            y: y.unwrap_or(current.y),
        }
    }
}

fn drill_diameter(
    layer: &ExcellonLayerData,
    tool: Option<u32>,
    geometry: &mut GerberGeometry,
) -> f64 {
    let Some(tool) = tool else {
        geometry
            .warnings
            .push("drill hit encountered before a tool was selected".into());
        return 0.1;
    };
    let Some(diameter) = layer.tools.get(&tool) else {
        geometry
            .warnings
            .push(format!("drill hit references undefined tool T{tool}"));
        return 0.1;
    };
    excellon_to_mm(*diameter, layer.unit.unit)
}

fn excellon_to_mm(value: f64, unit: lib_gerber_edit::gerber_types::Unit) -> f64 {
    match unit {
        lib_gerber_edit::gerber_types::Unit::Inches => value * 25.4,
        lib_gerber_edit::gerber_types::Unit::Millimeters => value,
    }
}
