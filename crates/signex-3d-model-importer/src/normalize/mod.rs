use crate::vrml::parser::VrmlMesh;

/// Build a minimal glTF 2.0 JSON and binary buffer from a flat list of meshes.
///
/// Output:
///   - `json_bytes`: UTF-8 encoded glTF JSON
///   - `bin_bytes`:  interleaved float32 position data
pub fn meshes_to_gltf(
    meshes: &[VrmlMesh],
    source_format: &str,
    source_path: &str,
    converter_version: &str,
) -> (Vec<u8>, Vec<u8>) {
    use serde_json::{json, Value};

    if meshes.is_empty() {
        let json = json!({
            "asset": {
                "version": "2.0",
                "generator": "signex-3d-model-importer",
                "extras": {
                    "source_format": source_format,
                    "source_path": source_path,
                    "converter_version": converter_version
                }
            },
            "scene": 0,
            "scenes": [{ "nodes": [] }],
            "nodes": [],
            "meshes": [],
            "accessors": [],
            "bufferViews": [],
            "buffers": [],
            "materials": []
        });
        return (json.to_string().into_bytes(), vec![]);
    }

    let mut bin: Vec<u8> = Vec::new();
    let mut buffer_views: Vec<Value> = Vec::new();
    let mut accessors: Vec<Value> = Vec::new();
    let mut gltf_meshes: Vec<Value> = Vec::new();
    let mut materials: Vec<Value> = Vec::new();
    let mut nodes: Vec<Value> = Vec::new();

    for (mesh_index, mesh) in meshes.iter().enumerate() {
        // ── position buffer view ──────────────────────────────────────────────
        let pos_bv_index = buffer_views.len();
        let pos_byte_offset = bin.len();
        for &f in &mesh.positions {
            bin.extend_from_slice(&f.to_le_bytes());
        }
        let pos_byte_len = bin.len() - pos_byte_offset;

        buffer_views.push(json!({
            "buffer": 0,
            "byteOffset": pos_byte_offset,
            "byteLength": pos_byte_len,
            "target": 34962  // ARRAY_BUFFER
        }));

        let vertex_count = mesh.positions.len() / 3;

        // Compute POSITION min/max for the accessor
        let (min_pos, max_pos) = compute_min_max(&mesh.positions);

        let pos_acc_index = accessors.len();
        accessors.push(json!({
            "bufferView": pos_bv_index,
            "byteOffset": 0,
            "componentType": 5126,   // FLOAT
            "count": vertex_count,
            "type": "VEC3",
            "min": min_pos,
            "max": max_pos
        }));

        // ── index buffer view ─────────────────────────────────────────────────
        // Align to 4-byte boundary before writing u32 indices
        while bin.len() % 4 != 0 {
            bin.push(0u8);
        }
        let idx_bv_index = buffer_views.len();
        let idx_byte_offset = bin.len();
        for &i in &mesh.indices {
            bin.extend_from_slice(&i.to_le_bytes());
        }
        let idx_byte_len = bin.len() - idx_byte_offset;

        buffer_views.push(json!({
            "buffer": 0,
            "byteOffset": idx_byte_offset,
            "byteLength": idx_byte_len,
            "target": 34963  // ELEMENT_ARRAY_BUFFER
        }));

        let idx_acc_index = accessors.len();
        accessors.push(json!({
            "bufferView": idx_bv_index,
            "byteOffset": 0,
            "componentType": 5125,   // UNSIGNED_INT
            "count": mesh.indices.len(),
            "type": "SCALAR"
        }));

        // ── material ──────────────────────────────────────────────────────────
        let mat_index = materials.len();
        let [r, g, b, a] = mesh.color;
        materials.push(json!({
            "pbrMetallicRoughness": {
                "baseColorFactor": [r, g, b, a],
                "metallicFactor": 0.0,
                "roughnessFactor": 0.8
            }
        }));

        // ── mesh primitive ────────────────────────────────────────────────────
        let mesh_gltf_index = gltf_meshes.len();
        gltf_meshes.push(json!({
            "primitives": [{
                "attributes": { "POSITION": pos_acc_index },
                "indices": idx_acc_index,
                "material": mat_index,
                "mode": 4  // TRIANGLES
            }]
        }));

        nodes.push(json!({ "mesh": mesh_gltf_index, "name": format!("mesh_{mesh_index}") }));
    }

    // Root node with Y-up identity transform (VRML is already Y-up)
    let child_indices: Vec<usize> = (0..nodes.len()).collect();
    let root_node_index = nodes.len();
    nodes.push(json!({
        "name": "root",
        "children": child_indices
    }));

    let bin_total = bin.len();
    let json = json!({
        "asset": {
            "version": "2.0",
            "generator": "signex-3d-model-importer",
            "extras": {
                "source_format": source_format,
                "source_path": source_path,
                "converter_version": converter_version,
                "mesh_count": meshes.len()
            }
        },
        "scene": 0,
        "scenes": [{ "nodes": [root_node_index] }],
        "nodes": nodes,
        "meshes": gltf_meshes,
        "accessors": accessors,
        "bufferViews": buffer_views,
        "buffers": [{ "byteLength": bin_total }],
        "materials": materials
    });

    (json.to_string().into_bytes(), bin)
}

fn compute_min_max(positions: &[f32]) -> ([f32; 3], [f32; 3]) {
    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];
    for chunk in positions.chunks(3) {
        for i in 0..3 {
            min[i] = min[i].min(chunk[i]);
            max[i] = max[i].max(chunk[i]);
        }
    }
    if positions.is_empty() {
        min = [0.0; 3];
        max = [0.0; 3];
    }
    (min, max)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vrml::parser::VrmlMesh;

    #[test]
    fn empty_mesh_list_produces_valid_json() {
        let (json, bin) = meshes_to_gltf(&[], "vrml", "/dev/null", "0.0.0");
        assert!(bin.is_empty());
        let v: serde_json::Value = serde_json::from_slice(&json).expect("not valid json");
        assert_eq!(v["asset"]["version"], "2.0");
    }

    #[test]
    fn single_triangle_produces_one_mesh() {
        let mesh = VrmlMesh {
            positions: vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            indices: vec![0, 1, 2],
            color: [1.0, 0.0, 0.0, 1.0],
        };
        let (json, bin) = meshes_to_gltf(&[mesh], "vrml", "/test.wrl", "0.1.0");
        assert!(!bin.is_empty());
        let v: serde_json::Value = serde_json::from_slice(&json).expect("not valid json");
        assert_eq!(v["meshes"].as_array().unwrap().len(), 1);
        assert_eq!(v["materials"][0]["pbrMetallicRoughness"]["baseColorFactor"][0], 1.0);
    }
}
