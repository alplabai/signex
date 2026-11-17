//! Reading a component model file into meshes, through
//! `signex-3d-model-importer`. Blocking IO: callers run it off the update
//! thread.

use std::path::Path;

use signex_3d_model_importer::vrml::VrmlMesh;
use signex_3d_model_importer::{step, vrml};

use super::ModelMesh;

/// The meshes of the model at `path`, in the file's own units (taken as
/// mm). `name` is the file's name as the user attached it: a `.wrl` or
/// `.vrml` name reads VRML, any other STEP, since the library stores every
/// attachment under its content hash.
pub fn read_model_file(path: &Path, name: &str) -> Result<Vec<ModelMesh>, String> {
    let extension = Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    let meshes = match extension.as_str() {
        "wrl" | "vrml" => vrml::load(path).map(|(meshes, _warnings)| meshes),
        _ => step::load(path).map(|result| result.meshes),
    }
    .map_err(|e| e.to_string())?;
    Ok(meshes.into_iter().filter_map(model_mesh).collect())
}

fn model_mesh(mesh: VrmlMesh) -> Option<ModelMesh> {
    let positions: Vec<[f32; 3]> = mesh
        .positions
        .chunks_exact(3)
        .map(|p| [p[0], p[1], p[2]])
        .collect();
    let triangles: Vec<[u32; 3]> = mesh
        .indices
        .chunks_exact(3)
        .map(|t| [t[0], t[1], t[2]])
        .collect();
    (!triangles.is_empty()).then_some(ModelMesh {
        positions,
        triangles,
        color: mesh.color,
    })
}
