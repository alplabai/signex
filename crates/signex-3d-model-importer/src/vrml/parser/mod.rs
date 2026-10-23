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
        /// The node's own matrix (T * C * R * SR * S * -SR * -C).
        local: Transform,
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
        collect_meshes(node, &identity, &mut meshes);
    }
    Ok(meshes)
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("unexpected end of token stream at line {line}")]
    UnexpectedEof { line: usize },
    #[error("unresolved USE node: {name}")]
    UnresolvedUse { name: String },
    #[error("node nesting deeper than {limit} at line {line}")]
    NestingTooDeep { line: usize, limit: usize },
    #[error("malformed number at line {line}: {value:?}")]
    MalformedNumber { line: usize, value: String },
}

// ─── Transform accumulator ────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct Transform {
    /// 4×4 matrix stored row-major (`m[row][col]`), applied to column vectors.
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

    fn translation(t: [f32; 3]) -> Self {
        let mut r = Self::identity();
        for (row, v) in r.m.iter_mut().zip(t) {
            row[3] = v;
        }
        r
    }

    fn scaling(s: [f32; 3]) -> Self {
        let mut r = Self::identity();
        for (i, v) in s.into_iter().enumerate() {
            r.m[i][i] = v;
        }
        r
    }

    /// Axis-angle rotation `[ax, ay, az, angle]` (Rodrigues). A zero-length
    /// axis is a no-op.
    fn rotation(r: [f32; 4]) -> Self {
        let len = (r[0] * r[0] + r[1] * r[1] + r[2] * r[2]).sqrt();
        if len == 0.0 {
            log::debug!("VRML: zero-length rotation axis; using identity");
            return Self::identity();
        }
        let (x, y, z) = (r[0] / len, r[1] / len, r[2] / len);
        let (s, c) = r[3].sin_cos();
        let t = 1.0 - c;
        let mut out = Self::identity();
        out.m[0][..3].copy_from_slice(&[t * x * x + c, t * x * y - s * z, t * x * z + s * y]);
        out.m[1][..3].copy_from_slice(&[t * x * y + s * z, t * y * y + c, t * y * z - s * x]);
        out.m[2][..3].copy_from_slice(&[t * x * z - s * y, t * y * z + s * x, t * z * z + c]);
        out
    }

    /// `self * rhs`: `rhs` is applied to a point first.
    fn mul(&self, rhs: &Transform) -> Transform {
        let mut m = [[0.0f32; 4]; 4];
        for (i, row) in m.iter_mut().enumerate() {
            for (j, cell) in row.iter_mut().enumerate() {
                *cell = (0..4).map(|k| self.m[i][k] * rhs.m[k][j]).sum();
            }
        }
        Transform { m }
    }

    /// True when the linear part flips handedness (negative determinant),
    /// which reverses triangle winding.
    fn is_mirroring(&self) -> bool {
        let m = &self.m;
        let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
            - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
        det < 0.0
    }

    /// VRML97 Transform node matrix: P' = T * C * R * SR * S * -SR * -C * P.
    /// A field holding a non-finite value (`inf` / `NaN` parse as f32) is
    /// warned about and replaced by its identity value.
    fn from_fields(
        translation: [f32; 3],
        rotation: [f32; 4],
        scale: [f32; 3],
        center: [f32; 3],
        scale_orientation: [f32; 4],
    ) -> Transform {
        fn finite<const N: usize>(v: [f32; N], identity: [f32; N], name: &str) -> [f32; N] {
            if v.iter().all(|x| x.is_finite()) {
                v
            } else {
                log::warn!("VRML: non-finite Transform {name} {v:?}; using identity");
                identity
            }
        }
        let translation = finite(translation, [0.0; 3], "translation");
        let rotation = finite(rotation, [0.0, 0.0, 1.0, 0.0], "rotation");
        let scale = finite(scale, [1.0; 3], "scale");
        let center = finite(center, [0.0; 3], "center");
        let scale_orientation = finite(scale_orientation, [0.0, 0.0, 1.0, 0.0], "scaleOrientation");
        let inv_sr = Transform::rotation([
            scale_orientation[0],
            scale_orientation[1],
            scale_orientation[2],
            -scale_orientation[3],
        ]);
        Transform::translation(translation)
            .mul(&Transform::translation(center))
            .mul(&Transform::rotation(rotation))
            .mul(&Transform::rotation(scale_orientation))
            .mul(&Transform::scaling(scale))
            .mul(&inv_sr)
            .mul(&Transform::translation(center.map(|v| -v)))
    }
}

// ─── Mesh collection ──────────────────────────────────────────────────────────

fn collect_meshes(node: &Node, parent_transform: &Transform, out: &mut Vec<VrmlMesh>) {
    match node {
        Node::Transform { local, children } => {
            let xf = parent_transform.mul(local);
            for child in children {
                collect_meshes(child, &xf, out);
            }
        }
        Node::Group { children } => {
            for child in children {
                collect_meshes(child, parent_transform, out);
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
    let mut face: Vec<usize> = Vec::new();
    let mirrored = xf.is_mirroring();

    for &idx in face_indices {
        if idx == -1 {
            // face separator: triangulate using fan
            fan_triangulate(
                &face,
                positions,
                xf,
                mirrored,
                &mut out_positions,
                &mut out_indices,
            );
            face.clear();
        } else if idx >= 0 {
            face.push(idx as usize);
        }
    }

    // Handle missing trailing -1
    fan_triangulate(
        &face,
        positions,
        xf,
        mirrored,
        &mut out_positions,
        &mut out_indices,
    );

    debug_assert_eq!(out_indices.len() % 3, 0);
    VrmlMesh {
        positions: out_positions,
        indices: out_indices,
        color,
    }
}

/// Fan-triangulate one polygon. A triangle is emitted only when all three of
/// its corners are in range, so the index buffer is always a whole number of
/// triangles; a face with a bad index loses just the triangles that use it (#147).
fn fan_triangulate(
    face: &[usize],
    positions: &[[f32; 3]],
    xf: &Transform,
    mirrored: bool,
    out_positions: &mut Vec<f32>,
    out_indices: &mut Vec<u32>,
) {
    if face.len() < 3 {
        return;
    }
    let base = face[0];
    for pair in face[1..].windows(2) {
        let mut corners = [base, pair[0], pair[1]];
        if corners.iter().any(|&vi| vi >= positions.len()) {
            continue;
        }
        // A mirroring transform reverses winding; swap two corners to compensate.
        if mirrored {
            corners.swap(1, 2);
        }
        let points = corners.map(|vi| xf.apply_point(positions[vi]));
        if points.iter().flatten().any(|v| !v.is_finite()) {
            log::warn!("VRML: transform overflowed to a non-finite vertex; triangle dropped");
            continue;
        }
        for p in points {
            let vertex_index = out_positions.len() as u32 / 3;
            out_positions.extend_from_slice(&p);
            out_indices.push(vertex_index);
        }
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

    fn first_vertices(meshes: &[VrmlMesh]) -> Vec<f32> {
        meshes[0].positions.clone()
    }

    fn assert_close(got: &[f32], want: &[f32]) {
        assert_eq!(got.len(), want.len());
        for (g, w) in got.iter().zip(want) {
            assert!((g - w).abs() < 1e-5, "got {got:?}, want {want:?}");
        }
    }

    const TRI: &str = "Shape { geometry IndexedFaceSet { coord Coordinate { point [ 0 0 0, 1 0 0, 0 1 0 ] } coordIndex [ 0 1 2 -1 ] } }";

    #[test]
    fn nested_transform_applies_parent_scale_to_child_translation() {
        let src = format!(
            "Transform {{ scale 2 2 2 children [ Transform {{ translation 1 0 0 children [ {TRI} ] }} ] }}"
        );
        assert_close(
            &first_vertices(&parse_src(&src)),
            &[2.0, 0.0, 0.0, 4.0, 0.0, 0.0, 2.0, 2.0, 0.0],
        );
    }

    #[test]
    fn transform_rotation_is_applied() {
        // 90 degrees about Z: (0,0,0)->(0,0,0), (1,0,0)->(0,1,0), (0,1,0)->(-1,0,0).
        let src = format!("Transform {{ rotation 0 0 1 1.5707964 children [ {TRI} ] }}");
        assert_close(
            &first_vertices(&parse_src(&src)),
            &[0.0, 0.0, 0.0, 0.0, 1.0, 0.0, -1.0, 0.0, 0.0],
        );
    }

    #[test]
    fn nested_rotation_then_child_translation() {
        // Child moves +X, parent rotates 90 degrees about Z: origin lands on +Y.
        let src = format!(
            "Transform {{ rotation 0 0 1 1.5707964 children [ Transform {{ translation 1 0 0 children [ {TRI} ] }} ] }}"
        );
        assert_close(
            &first_vertices(&parse_src(&src)),
            &[0.0, 1.0, 0.0, 0.0, 2.0, 0.0, -1.0, 1.0, 0.0],
        );
    }

    #[test]
    fn transform_scale_about_center() {
        // scale 2 about (1,0,0): (0,0,0)->(-1,0,0), (1,0,0) fixed, (0,1,0)->(-1,2,0).
        let src = format!("Transform {{ center 1 0 0 scale 2 2 2 children [ {TRI} ] }}");
        assert_close(
            &first_vertices(&parse_src(&src)),
            &[-1.0, 0.0, 0.0, 1.0, 0.0, 0.0, -1.0, 2.0, 0.0],
        );
    }

    #[test]
    fn unknown_node_with_list_does_not_eat_following_shape() {
        let src = format!("NavigationInfo {{ type [ \"EXAMINE\" ] }} {TRI}");
        assert_eq!(parse_src(&src).len(), 1);
    }

    #[test]
    fn stray_closer_inside_block_is_skipped() {
        let src = format!("Foo {{ v [ 1 2 ] ] }} {TRI}");
        assert_eq!(parse_src(&src).len(), 1);
    }

    #[test]
    fn stray_top_level_brace_keeps_following_shape() {
        let src = format!("}} {TRI}");
        assert_eq!(parse_src(&src).len(), 1);
    }

    #[test]
    fn eof_inside_block_still_errors() {
        let (tokens, lines) = tokenize("Foo { a [ 1 2 ]");
        assert!(matches!(
            parse(&tokens, &lines),
            Err(ParseError::UnexpectedEof { .. })
        ));
    }

    #[test]
    fn non_finite_transform_field_falls_back_to_identity() {
        let src =
            format!("Transform {{ rotation 0 0 1 NaN translation inf 0 0 children [ {TRI} ] }}");
        let v = first_vertices(&parse_src(&src));
        assert!(v.iter().all(|x| x.is_finite()), "{v:?}");
        assert_close(&v, &[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
    }

    #[test]
    fn mirroring_scale_keeps_triangle_winding() {
        // scale -1 1 1: points become (0,0,0), (-1,0,0), (0,1,0); original
        // order would now be clockwise, so corners 1 and 2 are swapped.
        let src = format!("Transform {{ scale -1 1 1 children [ {TRI} ] }}");
        assert_close(
            &first_vertices(&parse_src(&src)),
            &[0.0, 0.0, 0.0, 0.0, 1.0, 0.0, -1.0, 0.0, 0.0],
        );
    }

    #[test]
    fn scale_orientation_with_non_uniform_scale() {
        // SR 90 deg about Z, scale 2 1 1: the stretch runs along world Y.
        // (0,1,0): -SR -> (1,0,0); S -> (2,0,0); SR -> (0,2,0).
        // (1,0,0): -SR -> (0,-1,0); S -> (0,-1,0); SR -> (1,0,0).
        let src = format!(
            "Transform {{ scaleOrientation 0 0 1 1.5707964 scale 2 1 1 children [ {TRI} ] }}"
        );
        assert_close(
            &first_vertices(&parse_src(&src)),
            &[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 2.0, 0.0],
        );
    }

    #[test]
    fn def_use_and_texture_fields_are_skipped_whole() {
        let src = "Shape { appearance Appearance { texture ImageTexture { url [ \"a.png\" ] } material Material { diffuseColor 0.2 0.4 0.6 } } geometry IndexedFaceSet { color DEF c Color { color [ 1 0 0 ] } normal USE n coord Coordinate { point [ 0 0 0, 1 0 0, 0 1 0 ] } coordIndex [ 0 1 2 -1 ] } }";
        let meshes = parse_src(src);
        assert_eq!(meshes.len(), 1);
        assert_eq!(meshes[0].indices.len(), 3);
    }

    fn shape_with(appearance: &str, geometry: &str, coord: &str) -> String {
        format!(
            "Shape {{ {appearance} geometry {geometry} IndexedFaceSet {{ coord {coord} Coordinate {{ point [ 0 0 0, 1 0 0, 0 1 0 ] }} coordIndex [ 0 1 2 -1 ] }} }}"
        )
    }

    #[test]
    fn def_geometry_keeps_mesh() {
        assert_eq!(parse_src(&shape_with("", "DEF g", "")).len(), 1);
    }

    #[test]
    fn def_coord_keeps_mesh() {
        assert_eq!(parse_src(&shape_with("", "", "DEF co")).len(), 1);
    }

    #[test]
    fn use_coord_is_dropped_not_misparsed() {
        // Unresolved USE (#591): no coordinates, so no mesh, but no desync/panic.
        let src = "Shape { geometry IndexedFaceSet { coord USE co coordIndex [ 0 1 2 -1 ] } }";
        assert!(parse_src(src).is_empty());
    }

    #[test]
    fn def_appearance_and_material_keep_colour() {
        for appearance in [
            "appearance DEF a Appearance { material Material { diffuseColor 1 0 0 } }",
            "appearance Appearance { material DEF m Material { diffuseColor 1 0 0 } }",
        ] {
            let meshes = parse_src(&shape_with(appearance, "", ""));
            assert_eq!(meshes.len(), 1);
            assert_eq!(meshes[0].color, [1.0, 0.0, 0.0, 1.0], "{appearance}");
        }
    }

    #[test]
    fn composed_overflow_drops_triangle_instead_of_emitting_nan() {
        let src = format!(
            "Transform {{ scale 1e30 1e30 1e30 children [ Transform {{ scale 1e30 1 1 children [ {TRI} ] }} ] }}"
        );
        assert!(parse_src(&src).is_empty());
    }

    #[test]
    fn nested_list_inside_block_closes() {
        let src = format!("Foo {{ v [ 1 2 ] }} {TRI}");
        assert_eq!(parse_src(&src).len(), 1);
    }

    #[test]
    fn node_valued_field_before_coord_keeps_mesh() {
        let src = "Shape { geometry IndexedFaceSet { normal Normal { vector [ 0 0 1 ] } coord Coordinate { point [ 0 0 0, 1 0 0, 0 1 0 ] } coordIndex [ 0 1 2 -1 ] } }";
        assert_eq!(parse_src(src).len(), 1);
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

    #[test]
    fn out_of_range_vertex_drops_only_its_triangles() {
        // Quad 0 1 2 99: fan = (0,1,2) ok, (0,2,99) bad.
        let src = r#"
        Shape {
          geometry IndexedFaceSet {
            coord Coordinate { point [ 0 0 0, 1 0 0, 1 1 0 ] }
            coordIndex [ 0 1 2 99 -1 ]
          }
        }
        "#;
        let meshes = parse_src(src);
        assert_eq!(meshes.len(), 1);
        assert_eq!(meshes[0].indices.len(), 3);
        assert_eq!(meshes[0].positions.len(), 9);
    }

    #[test]
    fn index_buffer_is_always_whole_triangles() {
        let src = r#"
        Shape {
          geometry IndexedFaceSet {
            coord Coordinate { point [ 0 0 0, 1 0 0, 1 1 0, 0 1 0 ] }
            coordIndex [ 0 1 9 -1  9 8 7 -1  0 1 2 3 -1  0 2 5 3 ]
          }
        }
        "#;
        for mesh in parse_src(src) {
            assert_eq!(mesh.indices.len() % 3, 0);
            let verts = (mesh.positions.len() / 3) as u32;
            assert!(mesh.indices.iter().all(|&i| i < verts));
        }
    }
}
