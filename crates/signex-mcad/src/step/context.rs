//! The parts of a STEP file every model shares: the HEADER section, the
//! application context of the protocol, the product the shape belongs to,
//! and the geometric context (millimetres, radians, the length
//! uncertainty).

use super::StepSchema;
use super::p21::{Data, Ref, refs, string, triple};

/// The length below which two points are the same point, mm.
pub(crate) const LENGTH_UNCERTAINTY_MM: f64 = 1e-6;

impl StepSchema {
    /// The schema name of `FILE_SCHEMA`.
    fn schema_name(self) -> &'static str {
        match self {
            Self::Ap214 => "AUTOMOTIVE_DESIGN { 1 0 10303 214 1 1 1 1 }",
            Self::Ap242 => {
                "AP242_MANAGED_MODEL_BASED_3D_ENGINEERING_MIM_LF { 1 0 10303 442 1 1 4 }"
            }
        }
    }

    /// `APPLICATION_CONTEXT` application, protocol name and protocol year.
    fn protocol(self) -> (&'static str, &'static str, u32) {
        match self {
            Self::Ap214 => ("automotive design", "automotive_design", 2000),
            Self::Ap242 => (
                "managed model based 3d engineering",
                "ap242_managed_model_based_3d_engineering",
                2011,
            ),
        }
    }
}

/// The HEADER section.
pub(crate) fn header(
    schema: StepSchema,
    file_name: &str,
    timestamp: &str,
    system: &str,
    description: &str,
) -> String {
    format!(
        "HEADER;\nFILE_DESCRIPTION(({description}),'2;1');\n\
         FILE_NAME({file_name},{timestamp},(''),(''),{system},{system},'');\n\
         FILE_SCHEMA(('{schema}'));\nENDSEC;\n",
        description = string(description),
        file_name = string(file_name),
        timestamp = string(timestamp),
        system = string(system),
        schema = schema.schema_name(),
    )
}

/// What the shape representations hang off.
pub(crate) struct ProductFrame {
    /// The product's `PRODUCT_DEFINITION_SHAPE`.
    pub(crate) shape: Ref,
    /// The geometric representation context every representation uses.
    pub(crate) context: Ref,
    /// The world placement at the origin, z up.
    pub(crate) origin: Ref,
}

/// The application context, the product `name` and its definition, and the
/// unit and geometric contexts.
pub(crate) fn product(data: &mut Data, schema: StepSchema, name: &str) -> ProductFrame {
    let (application, protocol, year) = schema.protocol();
    let app = data.add(format!("APPLICATION_CONTEXT({})", string(application)));
    data.add(format!(
        "APPLICATION_PROTOCOL_DEFINITION('international standard',{},{year},{app})",
        string(protocol)
    ));
    let product_context = data.add(format!("PRODUCT_CONTEXT('',{app},'mechanical')"));
    let name = string(name);
    let product = data.add(format!(
        "PRODUCT({name},{name},'',{})",
        refs(&[product_context])
    ));
    data.add(format!(
        "PRODUCT_RELATED_PRODUCT_CATEGORY('part',$,{})",
        refs(&[product])
    ));
    let formation = data.add(format!("PRODUCT_DEFINITION_FORMATION('','',{product})"));
    let definition_context = data.add(format!(
        "PRODUCT_DEFINITION_CONTEXT('part definition',{app},'design')"
    ));
    let definition = data.add(format!(
        "PRODUCT_DEFINITION('design','',{formation},{definition_context})"
    ));
    let shape = data.add(format!("PRODUCT_DEFINITION_SHAPE('','',{definition})"));

    let length = data.add("(LENGTH_UNIT()NAMED_UNIT(*)SI_UNIT(.MILLI.,.METRE.))");
    let angle = data.add("(NAMED_UNIT(*)PLANE_ANGLE_UNIT()SI_UNIT($,.RADIAN.))");
    let solid_angle = data.add("(NAMED_UNIT(*)SI_UNIT($,.STERADIAN.)SOLID_ANGLE_UNIT())");
    let uncertainty = data.add(format!(
        "UNCERTAINTY_MEASURE_WITH_UNIT(LENGTH_MEASURE({}),{length},'distance_accuracy_value','confusion accuracy')",
        super::p21::real(LENGTH_UNCERTAINTY_MM)
    ));
    let context = data.add(format!(
        "(GEOMETRIC_REPRESENTATION_CONTEXT(3)GLOBAL_UNCERTAINTY_ASSIGNED_CONTEXT({})\
         GLOBAL_UNIT_ASSIGNED_CONTEXT({})REPRESENTATION_CONTEXT('',''))",
        refs(&[uncertainty]),
        refs(&[length, angle, solid_angle])
    ));
    let origin = placement(data, [0.0; 3], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]);
    ProductFrame {
        shape,
        context,
        origin,
    }
}

/// An `AXIS2_PLACEMENT_3D` at `location` with `axis` (z) and `reference`
/// (x) directions, both unit length.
pub(crate) fn placement(
    data: &mut Data,
    location: [f64; 3],
    axis: [f64; 3],
    reference: [f64; 3],
) -> Ref {
    let point = data.add(format!("CARTESIAN_POINT('',{})", triple(location)));
    placement_at(data, point, axis, reference)
}

/// An `AXIS2_PLACEMENT_3D` at the existing point `point`.
pub(crate) fn placement_at(
    data: &mut Data,
    point: Ref,
    axis: [f64; 3],
    reference: [f64; 3],
) -> Ref {
    let axis = data.add(format!("DIRECTION('',{})", triple(axis)));
    let reference = data.add(format!("DIRECTION('',{})", triple(reference)));
    data.add(format!("AXIS2_PLACEMENT_3D('',{point},{axis},{reference})"))
}
