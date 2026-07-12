use std::collections::HashMap;

/// A flat triangle mesh produced from a VRML IndexedFaceSet.
#[derive(Debug, Default, Clone)]
pub struct VrmlMesh {
    /// Interleaved XYZ positions (each group of 3 floats = one vertex).
    pub positions: Vec<f32>,
    /// Flat triangle index list (each group of 3 = one triangle).
    pub indices: Vec<u32>,
    /// Per-mesh base color [R, G, B, A] in 0..=1 range.
    pub color: [f32; 4],
}

/// Minimal scene-graph node types we care about.
#[derive(Debug, Clone)]
enum Node {
    Transform {
        translation: [f32; 3],
        #[allow(dead_code)]
        rotation: [f32; 4], // axis-angle: [ax, ay, az, angle] — deferred to E3
        scale: [f32; 3],
        children: Vec<Node>,
    },
    Shape {
        color: [f32; 4],
        positions: Vec<[f32; 3]>,
        indices: Vec<i32>, // VRML uses -1 as face separator
    },
    Group {
        children: Vec<Node>,
    },
}

/// Parse a VRML97 source into a flat list of meshes.
///
/// # Errors
///
/// Returns `(meshes, warnings)` on success; panics are converted to
/// `VrmlParseError` by the caller layer in `vrml/mod.rs`.
pub fn parse(
    tokens: &[super::lexer::Token],
    line_offsets: &[usize],
) -> Result<Vec<VrmlMesh>, ParseError> {
    let mut parser = Parser::new(tokens, line_offsets);
    parser.skip_vrml_header();
    let nodes = parser.parse_node_list()?;
    let mut meshes = Vec::new();
    let identity = Transform::identity();
    for node in &nodes {
        collect_meshes(node, &identity, &HashMap::new(), &mut meshes);
    }
    Ok(meshes)
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("unexpected end of token stream at line {line}")]
    UnexpectedEof { line: usize },
    #[error("unresolved USE node: {name}")]
    UnresolvedUse { name: String },
    #[error("malformed number at line {line}: {value:?}")]
    MalformedNumber { line: usize, value: String },
}

// ─── Transform accumulator ────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct Transform {
    /// column-major 4×4 matrix (row-major layout for simplicity)
    m: [[f32; 4]; 4],
}

impl Transform {
    fn identity() -> Self {
        Self {
            m: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    fn apply_point(&self, p: [f32; 3]) -> [f32; 3] {
        let m = &self.m;
        let x = m[0][0] * p[0] + m[0][1] * p[1] + m[0][2] * p[2] + m[0][3];
        let y = m[1][0] * p[0] + m[1][1] * p[1] + m[1][2] * p[2] + m[1][3];
        let z = m[2][0] * p[0] + m[2][1] * p[1] + m[2][2] * p[2] + m[2][3];
        [x, y, z]
    }

    fn compose_translation(
        parent: &Transform,
        translation: [f32; 3],
        scale: [f32; 3],
    ) -> Transform {
        // Build a simple TRS matrix (no rotation for Milestone D E1 - WRL
        // root transforms are axis-aligned for most components).
        // T * S applied on top of parent.
        let mut m = parent.m;
        // Apply scale then translate (column-major multiply simplified for TRS only)
        for row in 0..3 {
            m[row][0] *= scale[0];
            m[row][1] *= scale[1];
            m[row][2] *= scale[2];
        }
        m[0][3] += translation[0];
        m[1][3] += translation[1];
        m[2][3] += translation[2];
        Transform { m }
    }
}

// ─── Mesh collection ──────────────────────────────────────────────────────────

fn collect_meshes(
    node: &Node,
    parent_transform: &Transform,
    def_map: &HashMap<String, Node>,
    out: &mut Vec<VrmlMesh>,
) {
    match node {
        Node::Transform {
            translation,
            scale,
            children,
            ..
        } => {
            let xf = Transform::compose_translation(parent_transform, *translation, *scale);
            for child in children {
                collect_meshes(child, &xf, def_map, out);
            }
        }
        Node::Group { children } => {
            for child in children {
                collect_meshes(child, parent_transform, def_map, out);
            }
        }
        Node::Shape {
            color,
            positions,
            indices,
        } => {
            let mesh = build_mesh(positions, indices, *color, parent_transform);
            if !mesh.indices.is_empty() {
                out.push(mesh);
            }
        }
    }
}

/// Convert an IndexedFaceSet (positions + face-separated indices) to a triangle mesh.
fn build_mesh(
    positions: &[[f32; 3]],
    face_indices: &[i32],
    color: [f32; 4],
    xf: &Transform,
) -> VrmlMesh {
    let mut out_positions: Vec<f32> = Vec::new();
    let mut out_indices: Vec<u32> = Vec::new();
    let mut face: Vec<i32> = Vec::new();

    for &idx in face_indices {
        if idx == -1 {
            // face separator: triangulate using fan
            if face.len() >= 3 {
                let base = face[0] as usize;
                for i in 1..face.len() - 1 {
                    for &vi in &[base, face[i] as usize, face[i + 1] as usize] {
                        if vi < positions.len() {
                            let p = xf.apply_point(positions[vi]);
                            let vertex_index = out_positions.len() as u32 / 3;
                            out_positions.extend_from_slice(&p);
                            out_indices.push(vertex_index);
                        }
                    }
                }
            }
            face.clear();
        } else if idx >= 0 {
            face.push(idx);
        }
    }

    // Handle missing trailing -1
    if face.len() >= 3 {
        let base = face[0] as usize;
        for i in 1..face.len() - 1 {
            for &vi in &[base, face[i] as usize, face[i + 1] as usize] {
                if vi < positions.len() {
                    let p = xf.apply_point(positions[vi]);
                    let vertex_index = out_positions.len() as u32 / 3;
                    out_positions.extend_from_slice(&p);
                    out_indices.push(vertex_index);
                }
            }
        }
    }

    VrmlMesh {
        positions: out_positions,
        indices: out_indices,
        color,
    }
}

mod descent;
use descent::Parser;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vrml::lexer::tokenize;

    fn parse_src(src: &str) -> Vec<VrmlMesh> {
        let (tokens, lines) = tokenize(src);
        parse(&tokens, &lines).expect("parse failed")
    }

    #[test]
    fn parse_empty_is_empty() {
        let meshes = parse_src("#VRML V2.0 utf8\n");
        assert!(meshes.is_empty());
    }

    #[test]
    fn parse_single_triangle() {
        let src = r#"
        Shape {
          geometry IndexedFaceSet {
            coord Coordinate {
              point [ 0 0 0, 1 0 0, 0 1 0 ]
            }
            coordIndex [ 0 1 2 -1 ]
          }
        }
        "#;
        let meshes = parse_src(src);
        assert_eq!(meshes.len(), 1);
        assert_eq!(meshes[0].indices.len(), 3);
    }

    #[test]
    fn parse_transform_applies_translation() {
        let src = r#"
        Transform {
          translation 1.0 2.0 3.0
          children [
            Shape {
              geometry IndexedFaceSet {
                coord Coordinate {
                  point [ 0 0 0, 1 0 0, 0 1 0 ]
                }
                coordIndex [ 0 1 2 -1 ]
              }
            }
          ]
        }
        "#;
        let meshes = parse_src(src);
        assert_eq!(meshes.len(), 1);
        // first vertex should be translated by (1, 2, 3)
        let p = &meshes[0].positions;
        assert!((p[0] - 1.0).abs() < 1e-5, "x should be ~1.0, got {}", p[0]);
        assert!((p[1] - 2.0).abs() < 1e-5, "y should be ~2.0, got {}", p[1]);
        assert!((p[2] - 3.0).abs() < 1e-5, "z should be ~3.0, got {}", p[2]);
    }

    #[test]
    fn parse_diffuse_color() {
        let src = r#"
        Shape {
          appearance Appearance {
            material Material {
              diffuseColor 0.2 0.4 0.6
            }
          }
          geometry IndexedFaceSet {
            coord Coordinate {
              point [ 0 0 0, 1 0 0, 0 1 0 ]
            }
            coordIndex [ 0 1 2 -1 ]
          }
        }
        "#;
        let meshes = parse_src(src);
        assert_eq!(meshes.len(), 1);
        let c = meshes[0].color;
        assert!((c[0] - 0.2).abs() < 1e-5);
        assert!((c[1] - 0.4).abs() < 1e-5);
        assert!((c[2] - 0.6).abs() < 1e-5);
        assert!((c[3] - 1.0).abs() < 1e-5);
    }
}
