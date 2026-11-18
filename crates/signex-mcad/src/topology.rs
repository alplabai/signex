//! Read-only walks over the kernel's topology.

use solidcore::nurbs::b_spline_surface_n_lib::p2::OrientType;
use solidcore::topology::edgeuse::p1::{edge, face, faceuse_loopuses, loopuse_edgeuses};
use solidcore::topology::{Brep, EdgeId, EdgeuseId, FaceId};

use crate::error::McadError;

/// The loops of `f`, each as its edge uses in traversal order, read from the
/// face's primary (upward) face use.
pub(crate) fn face_loops(brep: &Brep, f: FaceId) -> Result<Vec<Vec<EdgeuseId>>, McadError> {
    const OP: &str = "read face loops";
    let faceuse = brep
        .face(f)
        .map_err(McadError::kernel(OP))?
        .faceuse
        .ok_or_else(|| McadError::invalid(OP, "face has no face use"))?;
    let loopuses = faceuse_loopuses(brep, faceuse, false, None).map_err(McadError::kernel(OP))?;
    loopuses
        .into_iter()
        .map(|lu| loopuse_edgeuses(brep, lu).map_err(McadError::kernel(OP)))
        .collect()
}

/// The edge an edge use runs along.
pub(crate) fn edgeuse_edge(brep: &Brep, eu: EdgeuseId) -> Result<EdgeId, McadError> {
    const OP: &str = "read edge use";
    edge(brep, eu)
        .map_err(McadError::kernel(OP))?
        .ok_or_else(|| McadError::invalid(OP, "edge use has no edge"))
}

/// True when the edge use runs against its edge's curve.
pub(crate) fn runs_backward(brep: &Brep, eu: EdgeuseId) -> Result<bool, McadError> {
    let record = brep
        .edgeuse(eu)
        .map_err(McadError::kernel("read edge use"))?;
    Ok(record.orientation == OrientType::Opposite)
}

/// The faces an edge bounds, without repeats (a seam edge bounds one face
/// twice).
pub(crate) fn edge_faces(brep: &Brep, e: EdgeId) -> Result<Vec<FaceId>, McadError> {
    const OP: &str = "read edge faces";
    let record = brep.edge(e).map_err(McadError::kernel(OP))?;
    let mut faces = Vec::new();
    for &eu in &record.edgeuses {
        if let Some(f) = face(brep, eu).map_err(McadError::kernel(OP))?
            && !faces.contains(&f)
        {
            faces.push(f);
        }
    }
    Ok(faces)
}

/// The corner points of `f`: the end vertices of every edge on its loops.
pub(crate) fn face_vertex_points(brep: &Brep, f: FaceId) -> Result<Vec<[f64; 3]>, McadError> {
    const OP: &str = "read face vertices";
    let mut points = Vec::new();
    for edgeuses in face_loops(brep, f)? {
        for eu in edgeuses {
            let record = brep
                .edge(edgeuse_edge(brep, eu)?)
                .map_err(McadError::kernel(OP))?;
            for v in record.vertices {
                let p = brep.vertex(v).map_err(McadError::kernel(OP))?.point;
                points.push([p.x, p.y, p.z]);
            }
        }
    }
    Ok(points)
}
