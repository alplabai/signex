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
}

impl Def {
    fn kind(&self) -> Option<DefKind> {
        match self {
            Def::Node(_) => Some(DefKind::Node),
            Def::Geometry { .. } => Some(DefKind::Geometry),
            Def::Coords(_) => Some(DefKind::Coords),
            Def::Color(_) => Some(DefKind::Color),
            Def::Ignored => None,
        }
    }

    fn weight(&self) -> usize {
        match self {
            Def::Node(node) => node_weight(node),
            Def::Geometry { positions, indices } => 1 + positions.len() + indices.len(),
            Def::Coords(points) => 1 + points.len(),
            Def::Color(_) | Def::Ignored => 1,
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

/// The `DEF` bindings seen so far, and how much `USE` has copied.
#[derive(Debug, Default)]
pub(super) struct Defs {
    bound: HashMap<String, Def>,
    expanded: usize,
}

impl Defs {
    /// Bind `name`. A later `DEF` of the same name rebinds it, as in VRML.
    pub(super) fn define(&mut self, name: String, def: Def) {
        self.bound.insert(name, def);
    }

    /// A copy of what `name` is bound to, for a `USE` in a `kind` slot.
    pub(super) fn instance(&mut self, name: &str, kind: DefKind) -> Result<Def, UseFailure> {
        let def = self.bound.get(name).ok_or(UseFailure::Undefined)?;
        match def.kind() {
            None => return Err(UseFailure::Ignored),
            Some(bound) if bound != kind => return Err(UseFailure::WrongKind),
            Some(_) => {}
        }
        let weight = def.weight();
        if self.expanded.saturating_add(weight) > MAX_USE_EXPANSION {
            return Err(UseFailure::OverBudget);
        }
        self.expanded += weight;
        Ok(def.clone())
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
        assert!(copied <= 2 * MAX_USE_EXPANSION, "{copied} vertices copied");
        assert!(
            warnings
                .iter()
                .any(|w| w.reason.contains("instancing would copy more than")),
            "{warnings:?}"
        );
    }
}
