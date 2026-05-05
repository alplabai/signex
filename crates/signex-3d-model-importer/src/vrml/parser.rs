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
        rotation: [f32; 4],   // axis-angle: [ax, ay, az, angle] — deferred to E3
        scale: [f32; 3],
        children: Vec<Node>,
    },
    Shape {
        color: [f32; 4],
        positions: Vec<[f32; 3]>,
        indices: Vec<i32>,   // VRML uses -1 as face separator
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
pub fn parse(tokens: &[super::lexer::Token], line_offsets: &[usize]) -> Result<Vec<VrmlMesh>, ParseError> {
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

    fn compose_translation(parent: &Transform, translation: [f32; 3], scale: [f32; 3]) -> Transform {
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
        Node::Transform { translation, scale, children, .. } => {
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
        Node::Shape { color, positions, indices } => {
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

    VrmlMesh { positions: out_positions, indices: out_indices, color }
}

// ─── Token-stream parser ───────────────────────────────────────────────────────

use super::lexer::Token;

struct Parser<'a> {
    tokens: &'a [Token],
    lines: &'a [usize],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(tokens: &'a [Token], lines: &'a [usize]) -> Self {
        Self { tokens, lines, pos: 0 }
    }

    fn current_line(&self) -> usize {
        self.lines.get(self.pos).copied().unwrap_or(0)
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn next(&mut self) -> Option<&Token> {
        let t = self.tokens.get(self.pos);
        if t.is_some() {
            self.pos += 1;
        }
        t
    }

    fn expect_word(&mut self) -> Result<String, ParseError> {
        match self.next() {
            Some(Token::Word(w)) => Ok(w.clone()),
            _ => Err(ParseError::UnexpectedEof { line: self.current_line() }),
        }
    }

    /// Skip the `#VRML V2.0 utf8` header line (already tokenized as words).
    fn skip_vrml_header(&mut self) {
        // The header appears as tokens: "#VRML" "V2.0" "utf8" — but since `#`
        // starts a comment and the lexer strips it, the first line comment is
        // already dropped. Nothing to skip here.
    }

    /// Parse zero or more top-level or child nodes until we hit RBrace / EOF.
    fn parse_node_list(&mut self) -> Result<Vec<Node>, ParseError> {
        let mut nodes = Vec::new();
        loop {
            match self.peek() {
                None | Some(Token::RBrace) => break,
                _ => {
                    if let Some(node) = self.parse_node()? {
                        nodes.push(node);
                    }
                }
            }
        }
        Ok(nodes)
    }

    /// Parse a single VRML node, returning `None` for ignored node types.
    fn parse_node(&mut self) -> Result<Option<Node>, ParseError> {
        let keyword = match self.next() {
            Some(Token::Word(w)) => w.clone(),
            Some(Token::RBrace | Token::RBracket | Token::Comma) => return Ok(None),
            _ => return Ok(None),
        };

        match keyword.as_str() {
            "DEF" => {
                let _name = self.expect_word()?;
                let node = self.parse_node()?;
                // DEF nodes: we don't track the def map at parse level in E1;
                // USE resolution is deferred (emit warning on USE).
                Ok(node)
            }
            "USE" => {
                let _name = self.expect_word()?;
                // In E1 we skip USE nodes (geometry approximation acceptable).
                Ok(None)
            }
            "Transform" => self.parse_transform().map(Some),
            "Group" | "Collision" | "Anchor" => self.parse_group().map(Some),
            "Shape" => self.parse_shape().map(Some),
            _ => {
                // Unknown node type: consume its body if it has one.
                if let Some(Token::LBrace) = self.peek() {
                    self.consume_block()?;
                }
                Ok(None)
            }
        }
    }

    fn parse_transform(&mut self) -> Result<Node, ParseError> {
        self.expect_lbrace()?;
        let mut translation = [0.0f32; 3];
        let mut rotation = [0.0f32, 1.0, 0.0, 0.0];
        let mut scale = [1.0f32; 3];
        let mut children = Vec::new();

        loop {
            match self.peek() {
                None | Some(Token::RBrace) => { self.next(); break; }
                Some(Token::Word(_)) => {}
                _ => { self.next(); continue; }
            }
            let field = self.expect_word()?;
            match field.as_str() {
                "translation" => {
                    translation[0] = self.parse_f32()?;
                    translation[1] = self.parse_f32()?;
                    translation[2] = self.parse_f32()?;
                }
                "rotation" => {
                    rotation[0] = self.parse_f32()?;
                    rotation[1] = self.parse_f32()?;
                    rotation[2] = self.parse_f32()?;
                    rotation[3] = self.parse_f32()?;
                }
                "scale" => {
                    scale[0] = self.parse_f32()?;
                    scale[1] = self.parse_f32()?;
                    scale[2] = self.parse_f32()?;
                }
                "children" => {
                    children = self.parse_children_list()?;
                }
                _ => {
                    self.skip_field_value()?;
                }
            }
        }
        Ok(Node::Transform { translation, rotation, scale, children })
    }

    fn parse_group(&mut self) -> Result<Node, ParseError> {
        self.expect_lbrace()?;
        let mut children = Vec::new();
        loop {
            match self.peek() {
                None | Some(Token::RBrace) => { self.next(); break; }
                Some(Token::Word(_)) => {}
                _ => { self.next(); continue; }
            }
            let field = self.expect_word()?;
            match field.as_str() {
                "children" => {
                    children = self.parse_children_list()?;
                }
                _ => {
                    self.skip_field_value()?;
                }
            }
        }
        Ok(Node::Group { children })
    }

    fn parse_shape(&mut self) -> Result<Node, ParseError> {
        self.expect_lbrace()?;
        let mut color = [0.7f32, 0.7, 0.7, 1.0];
        let mut positions: Vec<[f32; 3]> = Vec::new();
        let mut indices: Vec<i32> = Vec::new();

        loop {
            match self.peek() {
                None | Some(Token::RBrace) => { self.next(); break; }
                Some(Token::Word(_)) => {}
                _ => { self.next(); continue; }
            }
            let field = self.expect_word()?;
            match field.as_str() {
                "appearance" => {
                    color = self.parse_appearance_color()?;
                }
                "geometry" => {
                    let geom_type = self.expect_word()?;
                    if geom_type == "IndexedFaceSet" {
                        (positions, indices) = self.parse_indexed_face_set()?;
                    } else if let Some(Token::LBrace) = self.peek() {
                        self.consume_block()?;
                    }
                }
                _ => {
                    self.skip_field_value()?;
                }
            }
        }
        Ok(Node::Shape { color, positions, indices })
    }

    fn parse_appearance_color(&mut self) -> Result<[f32; 4], ParseError> {
        // appearance Appearance { material Material { diffuseColor r g b ... } }
        // We skip all appearance fields except diffuseColor.
        let node_type = self.expect_word()?;
        if node_type != "Appearance" {
            if let Some(Token::LBrace) = self.peek() { self.consume_block()?; }
            return Ok([0.7, 0.7, 0.7, 1.0]);
        }
        self.expect_lbrace()?;
        let mut color = [0.7f32, 0.7, 0.7, 1.0];
        loop {
            match self.peek() {
                None | Some(Token::RBrace) => { self.next(); break; }
                Some(Token::Word(_)) => {}
                _ => { self.next(); continue; }
            }
            let field = self.expect_word()?;
            match field.as_str() {
                "material" => {
                    let mat_type = self.expect_word()?;
                    if mat_type == "Material" {
                        color = self.parse_material_color()?;
                    } else if let Some(Token::LBrace) = self.peek() {
                        self.consume_block()?;
                    }
                }
                _ => {
                    self.skip_field_value()?;
                }
            }
        }
        Ok(color)
    }

    fn parse_material_color(&mut self) -> Result<[f32; 4], ParseError> {
        self.expect_lbrace()?;
        let mut color = [0.7f32, 0.7, 0.7, 1.0];
        loop {
            match self.peek() {
                None | Some(Token::RBrace) => { self.next(); break; }
                Some(Token::Word(_)) => {}
                _ => { self.next(); continue; }
            }
            let field = self.expect_word()?;
            match field.as_str() {
                "diffuseColor" => {
                    color[0] = self.parse_f32()?;
                    color[1] = self.parse_f32()?;
                    color[2] = self.parse_f32()?;
                }
                "transparency" => {
                    let t = self.parse_f32()?;
                    color[3] = 1.0 - t;
                }
                _ => {
                    self.skip_field_value()?;
                }
            }
        }
        Ok(color)
    }

    fn parse_indexed_face_set(&mut self) -> Result<(Vec<[f32; 3]>, Vec<i32>), ParseError> {
        self.expect_lbrace()?;
        let mut positions: Vec<[f32; 3]> = Vec::new();
        let mut indices: Vec<i32> = Vec::new();

        loop {
            match self.peek() {
                None | Some(Token::RBrace) => { self.next(); break; }
                Some(Token::Word(_)) => {}
                _ => { self.next(); continue; }
            }
            let field = self.expect_word()?;
            match field.as_str() {
                "coord" => {
                    let coord_type = self.expect_word()?;
                    if coord_type == "Coordinate" {
                        positions = self.parse_coordinate_point()?;
                    } else if let Some(Token::LBrace) = self.peek() {
                        self.consume_block()?;
                    }
                }
                "coordIndex" => {
                    indices = self.parse_int_array()?;
                }
                _ => {
                    self.skip_field_value()?;
                }
            }
        }
        Ok((positions, indices))
    }

    fn parse_coordinate_point(&mut self) -> Result<Vec<[f32; 3]>, ParseError> {
        self.expect_lbrace()?;
        let mut points = Vec::new();
        loop {
            match self.peek() {
                None | Some(Token::RBrace) => { self.next(); break; }
                Some(Token::Word(w)) if w == "point" => {
                    self.next();
                    self.expect_lbracket()?;
                    loop {
                        match self.peek() {
                            None | Some(Token::RBracket) => { self.next(); break; }
                            Some(Token::Comma) => { self.next(); }
                            Some(Token::Word(_)) => {
                                let x = self.parse_f32()?;
                                let y = self.parse_f32()?;
                                let z = self.parse_f32()?;
                                points.push([x, y, z]);
                            }
                            _ => { self.next(); }
                        }
                    }
                }
                _ => { self.next(); }
            }
        }
        Ok(points)
    }

    fn parse_int_array(&mut self) -> Result<Vec<i32>, ParseError> {
        self.expect_lbracket()?;
        let mut values = Vec::new();
        loop {
            match self.peek() {
                None | Some(Token::RBracket) => { self.next(); break; }
                Some(Token::Comma) => { self.next(); }
                Some(Token::Word(w)) => {
                    let s = w.clone();
                    let line = self.current_line();
                    self.next();
                    let v = s.parse::<i32>().map_err(|_| ParseError::MalformedNumber {
                        line,
                        value: s,
                    })?;
                    values.push(v);
                }
                _ => { self.next(); }
            }
        }
        Ok(values)
    }

    fn parse_children_list(&mut self) -> Result<Vec<Node>, ParseError> {
        self.expect_lbracket()?;
        let mut children = Vec::new();
        loop {
            match self.peek() {
                None | Some(Token::RBracket) => { self.next(); break; }
                Some(Token::Comma) => { self.next(); }
                _ => {
                    if let Some(node) = self.parse_node()? {
                        children.push(node);
                    }
                }
            }
        }
        Ok(children)
    }

    fn parse_f32(&mut self) -> Result<f32, ParseError> {
        match self.next() {
            Some(Token::Word(w)) => {
                let s = w.clone();
                let line = self.current_line();
                s.parse::<f32>().map_err(|_| ParseError::MalformedNumber { line, value: s })
            }
            _ => Err(ParseError::UnexpectedEof { line: self.current_line() }),
        }
    }

    fn expect_lbrace(&mut self) -> Result<(), ParseError> {
        match self.next() {
            Some(Token::LBrace) => Ok(()),
            _ => Err(ParseError::UnexpectedEof { line: self.current_line() }),
        }
    }

    fn expect_lbracket(&mut self) -> Result<(), ParseError> {
        match self.next() {
            Some(Token::LBracket) => Ok(()),
            _ => Err(ParseError::UnexpectedEof { line: self.current_line() }),
        }
    }

    /// Skip a field value: either a single word/number or a `{ ... }` / `[ ... ]` block.
    fn skip_field_value(&mut self) -> Result<(), ParseError> {
        match self.peek() {
            Some(Token::LBrace | Token::LBracket) => self.consume_block(),
            Some(Token::Word(_)) => { self.next(); Ok(()) }
            _ => Ok(()),
        }
    }

    /// Consume a `{ ... }` or `[ ... ]` block including all nested blocks.
    fn consume_block(&mut self) -> Result<(), ParseError> {
        let open = match self.next() {
            Some(Token::LBrace) => Token::LBrace,
            Some(Token::LBracket) => Token::LBracket,
            _ => return Ok(()),
        };
        let close = if open == Token::LBrace { Token::RBrace } else { Token::RBracket };
        let mut depth = 1usize;
        loop {
            match self.next() {
                None => return Err(ParseError::UnexpectedEof { line: self.current_line() }),
                Some(t) if *t == Token::LBrace || *t == Token::LBracket => depth += 1,
                Some(t) if *t == close => {
                    depth -= 1;
                    if depth == 0 { break; }
                }
                _ => {}
            }
        }
        Ok(())
    }
}

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
