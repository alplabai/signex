//! `DEF` / `USE` instancing (#591).
//!
//! A `DEF <name>` binds the node that follows it; a later `USE <name>`
//! stands for a copy of that node. Only what the parser keeps is bound: a
//! scene node, an `IndexedFaceSet`'s geometry, a `Coordinate`'s points, or
//! an `Appearance`/`Material` colour. A `DEF` on a node the parser ignores
//! is bound as [`Def::Ignored`], so its `USE` is dropped as quietly as the
//! node itself.

use std::collections::HashMap;

use super::Node;

/// Most geometry (nodes plus vertex and index entries) that `USE` may copy
/// in one file. Instancing that nests (`DEF B` holding two `USE A`, `DEF C`
/// holding two `USE B`, ...) doubles at every level, so a small file could
/// otherwise expand without bound.
pub(super) const MAX_USE_EXPANSION: usize = 2_000_000;

/// What a `DEF` name is bound to.
#[derive(Debug, Clone)]
pub(super) enum Def {
    Node(Node),
    Geometry {
        positions: Vec<[f32; 3]>,
        indices: Vec<i32>,
    },
    Coords(Vec<[f32; 3]>),
    Color([f32; 4]),
    Ignored,
    /// Binding it would have passed [`MAX_USE_EXPANSION`], so nothing was
    /// kept; a `USE` of it is over budget too.
    Dropped,
}

/// A value about to be bound, borrowed so that nothing is copied until the
/// budget allows it.
#[derive(Debug, Clone, Copy)]
pub(super) enum DefRef<'a> {
    Node(&'a Node),
    Geometry {
        positions: &'a [[f32; 3]],
        indices: &'a [i32],
    },
    Coords(&'a [[f32; 3]]),
    Color([f32; 4]),
    Ignored,
}

/// The slot a `USE` appears in, which decides the binding it accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DefKind {
    Node,
    Geometry,
    Coords,
    Color,
}

/// Why a `USE` produced nothing.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum UseFailure {
    /// No earlier `DEF` has this name.
    Undefined,
    /// The name is bound to a node the parser ignores; nothing to report.
    Ignored,
    /// The name is bound to a different kind of node than the slot takes.
    WrongKind,
    /// Copying it would pass [`MAX_USE_EXPANSION`].
    OverBudget,
    /// The copy would nest deeper than the parser's depth limit.
    TooDeep,
}

impl Def {
    fn kind(&self) -> Option<DefKind> {
        match self {
            Def::Node(_) => Some(DefKind::Node),
            Def::Geometry { .. } => Some(DefKind::Geometry),
            Def::Coords(_) => Some(DefKind::Coords),
            Def::Color(_) => Some(DefKind::Color),
            Def::Ignored | Def::Dropped => None,
        }
    }
}

impl DefRef<'_> {
    fn weight(&self) -> usize {
        match self {
            DefRef::Node(node) => node_weight(node),
            DefRef::Geometry { positions, indices } => 1 + positions.len() + indices.len(),
            DefRef::Coords(points) => 1 + points.len(),
            DefRef::Color(_) | DefRef::Ignored => 1,
        }
    }

    fn depth(&self) -> usize {
        match self {
            DefRef::Node(node) => node_depth(node),
            _ => 0,
        }
    }

    fn to_def(self) -> Def {
        match self {
            DefRef::Node(node) => Def::Node(node.clone()),
            DefRef::Geometry { positions, indices } => Def::Geometry {
                positions: positions.to_vec(),
                indices: indices.to_vec(),
            },
            DefRef::Coords(points) => Def::Coords(points.to_vec()),
            DefRef::Color(color) => Def::Color(color),
            DefRef::Ignored => Def::Ignored,
        }
    }
}

fn node_weight(node: &Node) -> usize {
    match node {
        Node::Transform { children, .. } | Node::Group { children } => {
            1 + children.iter().map(node_weight).sum::<usize>()
        }
        Node::Shape {
            positions, indices, ..
        } => 1 + positions.len() + indices.len(),
    }
}

fn node_depth(node: &Node) -> usize {
    match node {
        Node::Transform { children, .. } | Node::Group { children } => {
            1 + children.iter().map(node_depth).max().unwrap_or(0)
        }
        Node::Shape { .. } => 1,
    }
}

/// A binding, with its size and nesting depth worked out once at `DEF` so a
/// `USE` checks both in constant time.
#[derive(Debug)]
struct Binding {
    def: Def,
    weight: usize,
    depth: usize,
}

/// The `DEF` bindings seen so far, and how much binding and `USE` have
/// copied. Both draw on the one budget, so neither a long `DEF a DEF b ...`
/// chain nor nested instancing can copy more than [`MAX_USE_EXPANSION`].
#[derive(Debug, Default)]
pub(super) struct Defs {
    bound: HashMap<String, Binding>,
    spent: usize,
}

impl Defs {
    /// Bind `name`. A later `DEF` of the same name rebinds it, as in VRML.
    pub(super) fn define(&mut self, name: String, value: DefRef<'_>) {
        let weight = value.weight();
        let binding = match self.spent.checked_add(weight) {
            Some(spent) if spent <= MAX_USE_EXPANSION => {
                self.spent = spent;
                Binding {
                    def: value.to_def(),
                    weight,
                    depth: value.depth(),
                }
            }
            _ => Binding {
                def: Def::Dropped,
                weight: 0,
                depth: 0,
            },
        };
        self.bound.insert(name, binding);
    }

    /// A copy of what `name` is bound to, for a `USE` in a `kind` slot whose
    /// copy may nest at most `room` levels deep.
    pub(super) fn instance(
        &mut self,
        name: &str,
        kind: DefKind,
        room: usize,
    ) -> Result<Def, UseFailure> {
        let binding = self.bound.get(name).ok_or(UseFailure::Undefined)?;
        match binding.def.kind() {
            None if matches!(binding.def, Def::Dropped) => return Err(UseFailure::OverBudget),
            None => return Err(UseFailure::Ignored),
            Some(bound) if bound != kind => return Err(UseFailure::WrongKind),
            Some(_) => {}
        }
        if binding.depth > room {
            return Err(UseFailure::TooDeep);
        }
        match self.spent.checked_add(binding.weight) {
            Some(spent) if spent <= MAX_USE_EXPANSION => self.spent = spent,
            _ => return Err(UseFailure::OverBudget),
        }
        Ok(binding.def.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::super::{ParseWarning, VrmlMesh, parse};
    use super::MAX_USE_EXPANSION;
    use crate::vrml::lexer::tokenize;

    const TRI: &str = "Shape { geometry IndexedFaceSet { coord Coordinate { point [ 0 0 0, 1 0 0, 0 1 0 ] } coordIndex [ 0 1 2 -1 ] } }";

    fn parse_src(src: &str) -> (Vec<VrmlMesh>, Vec<ParseWarning>) {
        let (tokens, lines) = tokenize(src);
        parse(&tokens, &lines).expect("parse failed")
    }

    fn min_x(mesh: &VrmlMesh) -> f32 {
        mesh.positions
            .chunks(3)
            .map(|p| p[0])
            .fold(f32::INFINITY, f32::min)
    }

    /// The pin-body case: one part `DEF`d, instanced with `USE` under another
    /// transform. Both instances import, each at its own place.
    #[test]
    fn a_used_node_is_instanced_under_its_own_transform() {
        let src = format!(
            "Transform {{ translation 1 0 0 children [ DEF pin {TRI} ] }}
             Transform {{ translation 5 0 0 children [ USE pin ] }}"
        );
        let (meshes, warnings) = parse_src(&src);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(meshes.len(), 2);
        assert_eq!(min_x(&meshes[0]), 1.0);
        assert_eq!(min_x(&meshes[1]), 5.0);
    }

    /// Field-level instancing: shared geometry, coordinates and colours.
    #[test]
    fn used_geometry_coordinates_and_colours_resolve() {
        let src = "Shape { appearance DEF red Appearance { material Material { diffuseColor 1 0 0 } } \
                   geometry DEF tri IndexedFaceSet { coord DEF pts Coordinate { point [ 0 0 0, 1 0 0, 0 1 0 ] } coordIndex [ 0 1 2 -1 ] } }
             Shape { appearance USE red geometry USE tri }
             Shape { appearance Appearance { material DEF blue Material { diffuseColor 0 0 1 } } \
                   geometry IndexedFaceSet { coord USE pts coordIndex [ 0 1 2 -1 ] } }
             Shape { appearance Appearance { material USE blue } geometry USE tri }";
        let (meshes, warnings) = parse_src(src);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(meshes.len(), 4);
        for (mesh, colour) in meshes.iter().zip([
            [1.0, 0.0, 0.0, 1.0],
            [1.0, 0.0, 0.0, 1.0],
            [0.0, 0.0, 1.0, 1.0],
            [0.0, 0.0, 1.0, 1.0],
        ]) {
            assert_eq!(mesh.color, colour);
            assert_eq!(mesh.indices.len(), 3);
        }
    }

    /// A `USE` of a node the parser ignores is dropped as quietly as the
    /// node itself, not reported as unresolved.
    #[test]
    fn a_used_ignored_node_is_dropped_quietly() {
        let src = format!(
            "DEF view Viewpoint {{ position 0 0 10 }} Group {{ children [ USE view {TRI} ] }}"
        );
        let (meshes, warnings) = parse_src(&src);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(meshes.len(), 1);
    }

    /// A `DEF` inside a field the parser skips is bound as ignored, so its
    /// `USE` is not misreported as having no earlier `DEF`.
    #[test]
    fn a_use_of_a_def_in_a_skipped_field_is_quiet() {
        let src = format!(
            "Shape {{ appearance Appearance {{ texture DEF tx ImageTexture {{ url \"a.png\" }} }} }} \
             Shape {{ appearance Appearance {{ texture USE tx }} }} Group {{ children [ USE tx {TRI} ] }}"
        );
        let (meshes, warnings) = parse_src(&src);
        assert_eq!(meshes.len(), 1);
        assert!(warnings.is_empty(), "{warnings:?}");
    }

    /// #887: a `PROTO` declaration instantiates nothing, and a name `DEF`d
    /// in its body belongs to the prototype, not to the file.
    #[test]
    fn a_proto_body_neither_imports_nor_leaks_its_defs() {
        let src = format!(
            "PROTO P [ field SFFloat r 1 ] {{ DEF inner {TRI} }} Group {{ children [ USE inner ] }}"
        );
        let (meshes, warnings) = parse_src(&src);
        assert!(meshes.is_empty(), "the body's shape was imported");
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].reason.contains("USE inner is not resolved"));
    }

    /// #887: prototype instances are not expanded; the first of each name is
    /// reported, and geometry around them still imports.
    #[test]
    fn prototype_instances_are_reported_once_per_name() {
        let src = format!(
            "PROTO P [ ] {{ {TRI} }} EXTERNPROTO E [ ] \"lib.wrl#E\" P {{ }} P {{ }} E {{ }} {TRI}"
        );
        let (meshes, warnings) = parse_src(&src);
        assert_eq!(meshes.len(), 1, "only the plain shape imports");
        let reasons: Vec<&str> = warnings.iter().map(|w| w.reason.as_str()).collect();
        assert_eq!(
            reasons,
            [
                "instances of prototype P are not imported",
                "instances of prototype E are not imported"
            ]
        );
    }

    /// #895 review: a prototype instance used as a field value (here the
    /// geometry and the material) is reported too, not silently dropped.
    #[test]
    fn a_prototype_instance_in_a_field_is_reported() {
        let src = "PROTO G [ ] { IndexedFaceSet { coord Coordinate { point [ 0 0 0, 1 0 0, 0 1 0 ] } coordIndex [ 0 1 2 -1 ] } } \
                   PROTO M [ ] { Material { diffuseColor 1 0 0 } } \
                   Shape { appearance Appearance { material M { } } geometry G { } }";
        let (_, warnings) = parse_src(src);
        let reasons: Vec<&str> = warnings.iter().map(|w| w.reason.as_str()).collect();
        assert_eq!(
            reasons,
            [
                "instances of prototype M are not imported",
                "instances of prototype G are not imported"
            ]
        );
    }

    /// #895 review: a `PROTO` statement among a node's fields is skipped
    /// whole, so it does not close the node early and lose its transform.
    #[test]
    fn a_proto_among_node_fields_does_not_close_the_node() {
        let src = format!(
            "Transform {{ translation 2 0 0 PROTO Q [ field SFFloat a 1 ] {{ Shape {{ }} }} children [ {TRI} ] }}"
        );
        let (meshes, warnings) = parse_src(&src);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(meshes.len(), 1);
        assert_eq!(
            min_x(&meshes[0]),
            2.0,
            "the child keeps the Transform's translation"
        );
    }

    /// A name bound to another kind of node is refused, with a warning.
    #[test]
    fn a_use_in_the_wrong_slot_is_ignored_with_a_warning() {
        let src = format!("DEF pin {TRI} Shape {{ geometry USE pin }}");
        let (meshes, warnings) = parse_src(&src);
        assert_eq!(meshes.len(), 1);
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(
            warnings[0]
                .reason
                .contains("USE pin names a different kind of node")
        );
    }

    /// Instancing that doubles at every level stops at the budget, with a
    /// warning, instead of expanding exponentially.
    #[test]
    fn nested_instancing_stops_at_the_expansion_budget() {
        let points: Vec<String> = (0..1000).map(|i| format!("{i} 0 0")).collect();
        let mut src = format!(
            "DEF l0 Shape {{ geometry IndexedFaceSet {{ coord Coordinate {{ point [ {} ] }} coordIndex [ 0 1 2 -1 ] }} }}\n",
            points.join(", ")
        );
        for level in 1..=40 {
            let below = level - 1;
            src.push_str(&format!(
                "DEF l{level} Group {{ children [ USE l{below} USE l{below} ] }}\n"
            ));
        }
        let (meshes, warnings) = parse_src(&src);
        let copied: usize = meshes.iter().map(|m| m.positions.len() / 3).sum();
        // Every copy draws on the budget; only the one parsed leaf is extra.
        assert!(copied <= MAX_USE_EXPANSION + 1000, "{copied} vertices");
        let over: Vec<_> = warnings
            .iter()
            .filter(|w| w.reason.contains("instancing would copy more than"))
            .collect();
        assert_eq!(over.len(), 1, "reported once, not per USE: {over:?}");
    }

    /// A binding is a copy too: a long `DEF a DEF b ...` chain on one large
    /// node draws on the same budget instead of cloning without bound.
    #[test]
    fn a_def_chain_draws_on_the_expansion_budget() {
        // 250 bindings (under the depth limit) of a 10 000-point shape would
        // copy 2.5M entries, past the budget.
        let points: Vec<String> = (0..10_000).map(|i| format!("{i} 0 0")).collect();
        let defs: String = (0..250).map(|i| format!("DEF x{i} ")).collect();
        let src = format!(
            "{defs}Shape {{ geometry IndexedFaceSet {{ coord Coordinate {{ point [ {} ] }} coordIndex [ 0 1 2 -1 ] }} }}\nGroup {{ children [ USE x0 ] }}",
            points.join(", ")
        );
        let (meshes, warnings) = parse_src(&src);
        // The outermost name, bound last, was refused, so its USE is too.
        assert_eq!(meshes.len(), 1);
        assert!(
            warnings
                .iter()
                .any(|w| w.reason.contains("instancing would copy more than")),
            "{warnings:?}"
        );
    }

    /// A copy placed deep in the tree must not nest past the parser's depth
    /// limit, which also bounds the mesh walk's recursion.
    #[test]
    fn a_use_that_would_nest_too_deep_is_dropped() {
        let wrap = |depth: usize, inner: &str| {
            let mut s = inner.to_string();
            for _ in 0..depth {
                s = format!("Group {{ children [ {s} ] }}");
            }
            s
        };
        let src = format!("DEF deep {}\n{}", wrap(200, TRI), wrap(100, "USE deep"));
        let (meshes, warnings) = parse_src(&src);
        assert_eq!(meshes.len(), 1, "the DEF itself imports; the copy does not");
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].reason.contains("nest deeper than"));
    }
}
