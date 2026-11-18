//! STEP export (#2488): solids and meshes written as an ISO 10303-21 file
//! in the AP214 (automotive design) or AP242 (managed model based 3D
//! engineering) schema. Written from the published standards' structure on
//! the kernel's topology, bytes out; the host writes the file.
//!
//! The file holds one product. Its shape gathers an
//! `ADVANCED_BREP_SHAPE_REPRESENTATION` of the solids (exact geometry,
//! [`brep`]) and a `MANIFOLD_SURFACE_SHAPE_REPRESENTATION` of the meshes
//! (faceted, [`mesh`]), each body named. Rules: docs/BOARD_OUTPUT.md,
//! "STEP export".

mod brep;
mod context;
mod mesh;
mod p21;
mod shape;

use crate::error::McadError;
use crate::solid::Solid;

use p21::{Data, Ref, refs, string};

/// The application protocol a file is written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StepSchema {
    /// ISO 10303-214, automotive design: read by nearly every MCAD tool.
    #[default]
    Ap214,
    /// ISO 10303-242, managed model based 3D engineering.
    Ap242,
}

impl StepSchema {
    pub const ALL: [Self; 2] = [Self::Ap214, Self::Ap242];

    /// The id a command operand names the schema by.
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::Ap214 => "ap214",
            Self::Ap242 => "ap242",
        }
    }

    /// The schema named `id`, any case.
    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|s| s.id().eq_ignore_ascii_case(id.trim()))
    }
}

/// What the HEADER section says about the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepHeader {
    /// The file's name as `FILE_NAME` records it.
    pub file_name: String,
    /// ISO 8601 time the file was made, e.g. `2026-11-18T10:00:00`. A
    /// value, so the bytes are the same for the same input.
    pub timestamp: String,
    /// The program that wrote it, e.g. `Signex 1.0.0`.
    pub system: String,
}

/// A solid to write, by name.
#[derive(Debug, Clone, Copy)]
pub struct StepSolid<'a> {
    pub name: &'a str,
    pub solid: &'a Solid,
}

/// A triangle mesh to write as a surface body, mm, z up, triangles
/// counter-clockwise seen from outside.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct StepMesh {
    pub name: String,
    pub positions: Vec<[f64; 3]>,
    pub triangles: Vec<[u32; 3]>,
}

/// One STEP file: a product named `product` holding `solids` and
/// `meshes`, all in one frame.
#[derive(Debug, Clone)]
pub struct StepModel<'a> {
    pub schema: StepSchema,
    pub header: StepHeader,
    pub product: String,
    pub solids: Vec<StepSolid<'a>>,
    pub meshes: Vec<StepMesh>,
}

/// The STEP file of `model`.
///
/// # Errors
///
/// [`McadError::StepExport`] when a solid holds geometry the writer cannot
/// describe exactly or a mesh holds a point that is not a number;
/// [`McadError::Kernel`] / [`McadError::KernelPanicked`] when reading a
/// solid fails in the kernel.
pub fn write_step(model: &StepModel<'_>) -> Result<Vec<u8>, McadError> {
    let mut data = Data::default();
    let frame = context::product(&mut data, model.schema, &model.product);
    let shape = data.add(format!(
        "SHAPE_REPRESENTATION({},{},{})",
        string(&model.product),
        refs(&[frame.origin]),
        frame.context
    ));
    data.add(format!(
        "SHAPE_DEFINITION_REPRESENTATION({},{shape})",
        frame.shape
    ));

    let mut solids: Vec<Ref> = Vec::new();
    for item in &model.solids {
        solids.extend(brep::write_solid(&mut data, item.solid, item.name)?);
    }
    if !solids.is_empty() {
        solids.push(frame.origin);
        let rep = data.add(format!(
            "ADVANCED_BREP_SHAPE_REPRESENTATION('',{},{})",
            refs(&solids),
            frame.context
        ));
        data.add(format!(
            "SHAPE_REPRESENTATION_RELATIONSHIP('','',{shape},{rep})"
        ));
    }

    let mut surfaces: Vec<Ref> = Vec::new();
    for item in &model.meshes {
        surfaces.extend(mesh::write_mesh(&mut data, item)?);
    }
    if !surfaces.is_empty() {
        surfaces.push(frame.origin);
        let rep = data.add(format!(
            "MANIFOLD_SURFACE_SHAPE_REPRESENTATION('',{},{})",
            refs(&surfaces),
            frame.context
        ));
        data.add(format!(
            "SHAPE_REPRESENTATION_RELATIONSHIP('','',{shape},{rep})"
        ));
    }

    let header = context::header(
        model.schema,
        &model.header.file_name,
        &model.header.timestamp,
        &model.header.system,
        &model.product,
    );
    let text = format!(
        "ISO-10303-21;\n{header}DATA;\n{}ENDSEC;\nEND-ISO-10303-21;\n",
        data.text()
    );
    Ok(text.into_bytes())
}
