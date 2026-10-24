//! Recursive-descent VRML token-stream parser.

use super::defs::{Def, DefKind, DefRef, Defs, MAX_USE_EXPANSION, UseFailure};
use super::*;
use crate::vrml::lexer::Token;

/// Colour of a shape with no usable `Appearance`.
const DEFAULT_COLOR: [f32; 4] = [0.7, 0.7, 0.7, 1.0];

/// A node-valued field after its optional `DEF <name>` / `USE <name>` prefix.
enum FieldNode {
    /// A node follows: its type word, and the `DEF` name it is bound to.
    Typed { word: String, def: Option<String> },
    /// `USE <name>`: the copy it resolved to, or `None` (already warned).
    Use(Option<Def>),
}

/// Deepest node nesting accepted; bounds parser and `collect_meshes` recursion (#112).
pub(super) const MAX_NODE_DEPTH: usize = 256;

pub(super) struct Parser<'a> {
    tokens: &'a [Token],
    lines: &'a [usize],
    pos: usize,
    depth: usize,
    /// Recoverable findings, returned alongside the meshes (#805).
    pub(super) warnings: Vec<ParseWarning>,
    /// `DEF` bindings for `USE` to copy (#591).
    defs: Defs,
    /// The budget and depth refusals are reported once each, not per `USE`.
    use_budget_reported: bool,
    /// Prototype names declared so far; an instance of one is not imported
    /// and is reported once per name (#887).
    protos: std::collections::HashSet<String>,
    reported_protos: std::collections::HashSet<String>,
    use_depth_reported: bool,
}

impl<'a> Parser<'a> {
    pub(super) fn new(tokens: &'a [Token], lines: &'a [usize]) -> Self {
        Self {
            tokens,
            lines,
            pos: 0,
            depth: 0,
            warnings: Vec::new(),
            defs: Defs::default(),
            use_budget_reported: false,
            use_depth_reported: false,
            protos: Default::default(),
            reported_protos: Default::default(),
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

    /// A node-valued field taking a `kind` node: its type word with any
    /// `DEF <name>`, or the copy a `USE <name>` resolves to (#591).
    fn expect_node_type(&mut self, kind: DefKind) -> Result<FieldNode, ParseError> {
        let line = self.current_line();
        let word = self.expect_word()?;
        match word.as_str() {
            "DEF" => {
                let name = self.expect_word()?;
                Ok(FieldNode::Typed {
                    word: self.expect_word()?,
                    def: Some(name),
                })
            }
            "USE" => {
                let name = self.expect_word()?;
                Ok(FieldNode::Use(self.instance(&name, line, kind)))
            }
            _ => Ok(FieldNode::Typed { word, def: None }),
        }
    }

    /// A copy of what `name` is bound to, or `None` with a warning saying
    /// why the `USE` was dropped. A `USE` of an ignored node is dropped
    /// quietly, like the node.
    fn instance(&mut self, name: &str, line: usize, kind: DefKind) -> Option<Def> {
        // The copy replaces the `USE` node, which sits at `self.depth`.
        let room = MAX_NODE_DEPTH.saturating_sub(self.depth.saturating_sub(1));
        let reason = match self.defs.instance(name, kind, room) {
            Ok(def) => return Some(def),
            Err(UseFailure::Ignored) => return None,
            Err(UseFailure::OverBudget) if self.use_budget_reported => return None,
            Err(UseFailure::TooDeep) if self.use_depth_reported => return None,
            Err(UseFailure::Undefined) => {
                format!("USE {name} is not resolved: no earlier DEF {name}; node dropped")
            }
            Err(UseFailure::WrongKind) => {
                format!("USE {name} names a different kind of node here; ignored")
            }
            Err(UseFailure::OverBudget) => {
                self.use_budget_reported = true;
                format!(
                    "USE {name} dropped: instancing would copy more than {MAX_USE_EXPANSION} \
                     geometry entries (later ones dropped too)"
                )
            }
            Err(UseFailure::TooDeep) => {
                self.use_depth_reported = true;
                format!(
                    "USE {name} dropped: the copy would nest deeper than {MAX_NODE_DEPTH} \
                     levels (later ones dropped too)"
                )
            }
        };
        ParseWarning::push(&mut self.warnings, Some(line), reason);
        None
    }

    /// Bind `def`'s name, when the node carried one.
    fn bind(&mut self, def: Option<String>, value: DefRef<'_>) {
        if let Some(name) = def {
            self.defs.define(name, value);
        }
    }

    /// Skip a node the parser does not read, binding its `DEF` name (if
    /// any) as ignored.
    fn skip_field_node(&mut self, def: Option<String>) -> Result<(), ParseError> {
        self.bind(def, DefRef::Ignored);
        if let Some(Token::LBrace) = self.peek() {
            self.consume_block()?;
        }
        Ok(())
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
                    let line = self.current_line();
                    ParseWarning::push(
                        &mut self.warnings,
                        Some(line),
                        "unmatched '}' skipped".to_owned(),
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
                let name = self.expect_word()?;
                let node = self.parse_node()?;
                let value = node.as_ref().map_or(DefRef::Ignored, DefRef::Node);
                self.defs.define(name, value);
                Ok(node)
            }
            "USE" => {
                let line = self.current_line();
                let name = self.expect_word()?;
                match self.instance(&name, line, DefKind::Node) {
                    Some(Def::Node(node)) => Ok(Some(node)),
                    _ => Ok(None),
                }
            }
            "PROTO" | "EXTERNPROTO" => {
                self.skip_proto_declaration(&keyword)?;
                Ok(None)
            }
            "Transform" => self.parse_transform().map(Some),
            "Group" | "Collision" | "Anchor" => self.parse_group().map(Some),
            "Shape" => self.parse_shape().map(Some),
            _ => {
                if self.protos.contains(&keyword) && self.reported_protos.insert(keyword.clone()) {
                    let line = self.current_line();
                    ParseWarning::push(
                        &mut self.warnings,
                        Some(line),
                        format!("instances of prototype {keyword} are not imported"),
                    );
                }
                // Unknown node type: consume its body if it has one.
                if let Some(Token::LBrace) = self.peek() {
                    self.consume_block()?;
                }
                Ok(None)
            }
        }
    }

    /// Skip a `PROTO name [ interface ] { body }` or
    /// `EXTERNPROTO name [ interface ] url` declaration (#887). A
    /// declaration defines a node type and instantiates nothing, and names
    /// `DEF`d in its body belong to the prototype's own scope, so neither the
    /// body's geometry nor its `DEF` names may reach the scene. Instances are
    /// not expanded; the first one of each prototype is reported.
    fn skip_proto_declaration(&mut self, keyword: &str) -> Result<(), ParseError> {
        let name = self.expect_word()?;
        if let Some(Token::LBracket) = self.peek() {
            self.consume_block()?;
        }
        if keyword == "PROTO" {
            if let Some(Token::LBrace) = self.peek() {
                self.consume_block()?;
            }
        } else {
            // The URL: one string, or a list of them.
            self.skip_field_value()?;
        }
        self.protos.insert(name);
        Ok(())
    }

    fn parse_transform(&mut self) -> Result<Node, ParseError> {
        let line = self.current_line();
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
            local: Transform::from_fields(
                translation,
                rotation,
                scale,
                center,
                scale_orientation,
                line,
                &mut self.warnings,
            ),
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
        let line = self.current_line();
        self.expect_lbrace()?;
        let mut color = DEFAULT_COLOR;
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
                "geometry" => match self.expect_node_type(DefKind::Geometry)? {
                    FieldNode::Use(Some(Def::Geometry {
                        positions: p,
                        indices: i,
                    })) => (positions, indices) = (p, i),
                    FieldNode::Use(_) => {}
                    FieldNode::Typed { word, def } if word == "IndexedFaceSet" => {
                        (positions, indices) = self.parse_indexed_face_set()?;
                        self.bind(
                            def,
                            DefRef::Geometry {
                                positions: &positions,
                                indices: &indices,
                            },
                        );
                    }
                    FieldNode::Typed { def, .. } => self.skip_field_node(def)?,
                },
                _ => {
                    self.skip_field_value()?;
                }
            }
        }
        Ok(Node::Shape {
            color,
            positions,
            indices,
            line,
        })
    }

    fn parse_appearance_color(&mut self) -> Result<[f32; 4], ParseError> {
        // appearance Appearance { material Material { diffuseColor r g b ... } }
        // We skip all appearance fields except diffuseColor.
        let def = match self.expect_node_type(DefKind::Color)? {
            FieldNode::Use(Some(Def::Color(color))) => return Ok(color),
            FieldNode::Use(_) => return Ok(DEFAULT_COLOR),
            FieldNode::Typed { word, def } if word == "Appearance" => def,
            FieldNode::Typed { def, .. } => {
                self.skip_field_node(def)?;
                return Ok(DEFAULT_COLOR);
            }
        };
        self.expect_lbrace()?;
        let mut color = DEFAULT_COLOR;
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
                "material" => match self.expect_node_type(DefKind::Color)? {
                    FieldNode::Use(Some(Def::Color(c))) => color = c,
                    FieldNode::Use(_) => {}
                    FieldNode::Typed { word, def } if word == "Material" => {
                        color = self.parse_material_color()?;
                        self.bind(def, DefRef::Color(color));
                    }
                    FieldNode::Typed { def, .. } => self.skip_field_node(def)?,
                },
                _ => {
                    self.skip_field_value()?;
                }
            }
        }
        self.bind(def, DefRef::Color(color));
        Ok(color)
    }

    fn parse_material_color(&mut self) -> Result<[f32; 4], ParseError> {
        self.expect_lbrace()?;
        let mut color = DEFAULT_COLOR;
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
                "coord" => match self.expect_node_type(DefKind::Coords)? {
                    FieldNode::Use(Some(Def::Coords(points))) => positions = points,
                    FieldNode::Use(_) => {}
                    FieldNode::Typed { word, def } if word == "Coordinate" => {
                        positions = self.parse_coordinate_point()?;
                        self.bind(def, DefRef::Coords(&positions));
                    }
                    FieldNode::Typed { def, .. } => self.skip_field_node(def)?,
                },
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
                    // Bound as ignored, so a later `USE` of it is dropped as
                    // quietly as this field, not reported as undefined (#591).
                    let name = self.expect_word()?;
                    self.defs.define(name, DefRef::Ignored);
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
                        let line = self.lines.get(self.pos - 1).copied().unwrap_or(0);
                        let closer = if t == Token::RBrace { '}' } else { ']' };
                        ParseWarning::push(
                            &mut self.warnings,
                            Some(line),
                            format!("mismatched closer '{closer}' skipped"),
                        );
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}
