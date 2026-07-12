//! Recursive-descent VRML token-stream parser.

use super::*;
use crate::vrml::lexer::Token;

pub(super) struct Parser<'a> {
    tokens: &'a [Token],
    lines: &'a [usize],
    pos: usize,
}

impl<'a> Parser<'a> {
    pub(super) fn new(tokens: &'a [Token], lines: &'a [usize]) -> Self {
        Self {
            tokens,
            lines,
            pos: 0,
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

    /// Skip the `#VRML V2.0 utf8` header line (already tokenized as words).
    pub(super) fn skip_vrml_header(&mut self) {
        // The header appears as tokens: "#VRML" "V2.0" "utf8" — but since `#`
        // starts a comment and the lexer strips it, the first line comment is
        // already dropped. Nothing to skip here.
    }

    /// Parse zero or more top-level or child nodes until we hit RBrace / EOF.
    pub(super) fn parse_node_list(&mut self) -> Result<Vec<Node>, ParseError> {
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
        Ok(Node::Transform {
            translation,
            rotation,
            scale,
            children,
        })
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
        Ok(Node::Shape {
            color,
            positions,
            indices,
        })
    }

    fn parse_appearance_color(&mut self) -> Result<[f32; 4], ParseError> {
        // appearance Appearance { material Material { diffuseColor r g b ... } }
        // We skip all appearance fields except diffuseColor.
        let node_type = self.expect_word()?;
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
                self.next();
                Ok(())
            }
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
        let close = if open == Token::LBrace {
            Token::RBrace
        } else {
            Token::RBracket
        };
        let mut depth = 1usize;
        loop {
            match self.next() {
                None => {
                    return Err(ParseError::UnexpectedEof {
                        line: self.current_line(),
                    });
                }
                Some(t) if *t == Token::LBrace || *t == Token::LBracket => depth += 1,
                Some(t) if *t == close => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}
