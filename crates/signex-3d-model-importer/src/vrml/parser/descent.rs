//! Recursive-descent VRML token-stream parser.

use super::*;
use crate::vrml::lexer::Token;

/// Deepest node nesting accepted; bounds parser and `collect_meshes` recursion (#112).
pub(super) const MAX_NODE_DEPTH: usize = 256;

pub(super) struct Parser<'a> {
    tokens: &'a [Token],
    lines: &'a [usize],
    pos: usize,
    depth: usize,
}

impl<'a> Parser<'a> {
    pub(super) fn new(tokens: &'a [Token], lines: &'a [usize]) -> Self {
        Self {
            tokens,
            lines,
            pos: 0,
            depth: 0,
        }
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
            _ => Err(ParseError::UnexpectedEof {
                line: self.current_line(),
            }),
        }
    }

    /// The type word of a node-valued field: consumes an optional `DEF <name>`
    /// prefix. `USE <name>` is not resolved (#591): it is warned about and
    /// yields `"USE"`, which matches no node type, so the field is ignored.
    fn expect_node_type(&mut self) -> Result<String, ParseError> {
        let line = self.current_line();
        let mut word = self.expect_word()?;
        if word == "DEF" {
            self.expect_word()?;
            word = self.expect_word()?;
        } else if word == "USE" {
            let name = self.expect_word()?;
            log::warn!("VRML: USE {name} at line {line} is not resolved; node dropped");
        }
        Ok(word)
    }

    /// Skip the `#VRML V2.0 utf8` header line (already tokenized as words).
    pub(super) fn skip_vrml_header(&mut self) {
        // The header appears as tokens: "#VRML" "V2.0" "utf8" — but since `#`
        // starts a comment and the lexer strips it, the first line comment is
        // already dropped. Nothing to skip here.
    }

    /// Parse top-level nodes until EOF. A stray `}` is warned about and
    /// skipped so the rest of the file still parses.
    pub(super) fn parse_node_list(&mut self) -> Result<Vec<Node>, ParseError> {
        let mut nodes = Vec::new();
        loop {
            match self.peek() {
                None => break,
                Some(Token::RBrace) => {
                    log::warn!(
                        "VRML: unmatched '}}' at line {}; skipped",
                        self.current_line()
                    );
                    self.next();
                }
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
        if self.depth >= MAX_NODE_DEPTH {
            return Err(ParseError::NestingTooDeep {
                line: self.current_line(),
                limit: MAX_NODE_DEPTH,
            });
        }
        self.depth += 1;
        let result = self.parse_node_inner();
        self.depth -= 1;
        result
    }

    fn parse_node_inner(&mut self) -> Result<Option<Node>, ParseError> {
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
        let mut rotation = [0.0f32, 0.0, 1.0, 0.0];
        let mut scale = [1.0f32; 3];
        let mut center = [0.0f32; 3];
        let mut scale_orientation = [0.0f32, 0.0, 1.0, 0.0];
        let mut children = Vec::new();

        loop {
            match self.peek() {
                None | Some(Token::RBrace) => {
                    self.next();
                    break;
                }
                Some(Token::Word(_)) => {}
                _ => {
                    self.next();
                    continue;
                }
            }
            let field = self.expect_word()?;
            match field.as_str() {
                "translation" => {
                    translation[0] = self.parse_f32()?;
                    translation[1] = self.parse_f32()?;
                    translation[2] = self.parse_f32()?;
                }
                "rotation" => rotation = self.parse_rotation()?,
                "scaleOrientation" => scale_orientation = self.parse_rotation()?,
                "center" => {
                    center[0] = self.parse_f32()?;
                    center[1] = self.parse_f32()?;
                    center[2] = self.parse_f32()?;
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
        Ok(Node::Transform {
            local: Transform::from_fields(translation, rotation, scale, center, scale_orientation),
            children,
        })
    }

    fn parse_rotation(&mut self) -> Result<[f32; 4], ParseError> {
        Ok([
            self.parse_f32()?,
            self.parse_f32()?,
            self.parse_f32()?,
            self.parse_f32()?,
        ])
    }

    fn parse_group(&mut self) -> Result<Node, ParseError> {
        self.expect_lbrace()?;
        let mut children = Vec::new();
        loop {
            match self.peek() {
                None | Some(Token::RBrace) => {
                    self.next();
                    break;
                }
                Some(Token::Word(_)) => {}
                _ => {
                    self.next();
                    continue;
                }
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
                None | Some(Token::RBrace) => {
                    self.next();
                    break;
                }
                Some(Token::Word(_)) => {}
                _ => {
                    self.next();
                    continue;
                }
            }
            let field = self.expect_word()?;
            match field.as_str() {
                "appearance" => {
                    color = self.parse_appearance_color()?;
                }
                "geometry" => {
                    let geom_type = self.expect_node_type()?;
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
        Ok(Node::Shape {
            color,
            positions,
            indices,
        })
    }

    fn parse_appearance_color(&mut self) -> Result<[f32; 4], ParseError> {
        // appearance Appearance { material Material { diffuseColor r g b ... } }
        // We skip all appearance fields except diffuseColor.
        let node_type = self.expect_node_type()?;
        if node_type != "Appearance" {
            if let Some(Token::LBrace) = self.peek() {
                self.consume_block()?;
            }
            return Ok([0.7, 0.7, 0.7, 1.0]);
        }
        self.expect_lbrace()?;
        let mut color = [0.7f32, 0.7, 0.7, 1.0];
        loop {
            match self.peek() {
                None | Some(Token::RBrace) => {
                    self.next();
                    break;
                }
                Some(Token::Word(_)) => {}
                _ => {
                    self.next();
                    continue;
                }
            }
            let field = self.expect_word()?;
            match field.as_str() {
                "material" => {
                    let mat_type = self.expect_node_type()?;
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
                None | Some(Token::RBrace) => {
                    self.next();
                    break;
                }
                Some(Token::Word(_)) => {}
                _ => {
                    self.next();
                    continue;
                }
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
                None | Some(Token::RBrace) => {
                    self.next();
                    break;
                }
                Some(Token::Word(_)) => {}
                _ => {
                    self.next();
                    continue;
                }
            }
            let field = self.expect_word()?;
            match field.as_str() {
                "coord" => {
                    let coord_type = self.expect_node_type()?;
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
                None | Some(Token::RBrace) => {
                    self.next();
                    break;
                }
                Some(Token::Word(w)) if w == "point" => {
                    self.next();
                    self.expect_lbracket()?;
                    loop {
                        match self.peek() {
                            None | Some(Token::RBracket) => {
                                self.next();
                                break;
                            }
                            Some(Token::Comma) => {
                                self.next();
                            }
                            Some(Token::Word(_)) => {
                                let x = self.parse_f32()?;
                                let y = self.parse_f32()?;
                                let z = self.parse_f32()?;
                                points.push([x, y, z]);
                            }
                            _ => {
                                self.next();
                            }
                        }
                    }
                }
                _ => {
                    self.next();
                }
            }
        }
        Ok(points)
    }

    fn parse_int_array(&mut self) -> Result<Vec<i32>, ParseError> {
        self.expect_lbracket()?;
        let mut values = Vec::new();
        loop {
            match self.peek() {
                None | Some(Token::RBracket) => {
                    self.next();
                    break;
                }
                Some(Token::Comma) => {
                    self.next();
                }
                Some(Token::Word(w)) => {
                    let s = w.clone();
                    let line = self.current_line();
                    self.next();
                    let v = s
                        .parse::<i32>()
                        .map_err(|_| ParseError::MalformedNumber { line, value: s })?;
                    values.push(v);
                }
                _ => {
                    self.next();
                }
            }
        }
        Ok(values)
    }

    fn parse_children_list(&mut self) -> Result<Vec<Node>, ParseError> {
        self.expect_lbracket()?;
        let mut children = Vec::new();
        loop {
            match self.peek() {
                None | Some(Token::RBracket) => {
                    self.next();
                    break;
                }
                Some(Token::Comma) => {
                    self.next();
                }
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
                s.parse::<f32>()
                    .map_err(|_| ParseError::MalformedNumber { line, value: s })
            }
            _ => Err(ParseError::UnexpectedEof {
                line: self.current_line(),
            }),
        }
    }

    fn expect_lbrace(&mut self) -> Result<(), ParseError> {
        match self.next() {
            Some(Token::LBrace) => Ok(()),
            _ => Err(ParseError::UnexpectedEof {
                line: self.current_line(),
            }),
        }
    }

    fn expect_lbracket(&mut self) -> Result<(), ParseError> {
        match self.next() {
            Some(Token::LBracket) => Ok(()),
            _ => Err(ParseError::UnexpectedEof {
                line: self.current_line(),
            }),
        }
    }

    /// Skip a field value: either a single word/number or a `{ ... }` / `[ ... ]` block.
    fn skip_field_value(&mut self) -> Result<(), ParseError> {
        match self.peek() {
            Some(Token::LBrace | Token::LBracket) => self.consume_block(),
            Some(Token::Word(_)) => {
                // An SFNode value is `[DEF name] Type { ... }`: the words are
                // followed by a body that must be consumed with them, or its
                // closing brace ends the enclosing node early (#689).
                let word = self.expect_word()?;
                if word == "USE" {
                    self.expect_word()?;
                    return Ok(());
                }
                if word == "DEF" {
                    self.expect_word()?;
                    self.expect_word()?;
                }
                if let Some(Token::LBrace) = self.peek() {
                    self.consume_block()?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// Consume a `{ ... }` or `[ ... ]` block including all nested blocks.
    fn consume_block(&mut self) -> Result<(), ParseError> {
        if !matches!(self.next(), Some(Token::LBrace | Token::LBracket)) {
            return Ok(());
        }
        // Each opener ('{' or '[') pushes its own closer, so nested lists inside
        // a node body close correctly (#689). A closer that does not match the
        // innermost opener is warned about and skipped, never counted.
        let mut expected = vec![self.tokens[self.pos - 1].closer()];
        loop {
            match self.next().cloned() {
                None => {
                    return Err(ParseError::UnexpectedEof {
                        line: self.current_line(),
                    });
                }
                Some(t @ (Token::LBrace | Token::LBracket)) => expected.push(t.closer()),
                Some(t @ (Token::RBrace | Token::RBracket)) => {
                    if t == expected[expected.len() - 1] {
                        expected.pop();
                        if expected.is_empty() {
                            break;
                        }
                    } else {
                        log::warn!(
                            "VRML: mismatched closer {:?} at line {}; skipped",
                            t,
                            self.lines.get(self.pos - 1).copied().unwrap_or(0)
                        );
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}
