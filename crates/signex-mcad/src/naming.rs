//! Persistent face and edge names.
//!
//! A face is named when the feature that makes it runs. The name rides on
//! the face as a kernel attribute, and the kernel carries attributes through
//! its operations (a split face keeps the name on both pieces, a merged face
//! keeps every name), so the name outlives the face's kernel id. Each
//! distinct name gets its own attribute id, which is what keeps two names
//! apart when faces merge.

use solidcore::EntityRef;
use solidcore::geom::attribute::{Attribute, Behavior};
use solidcore::topology::{Brep, FaceId};

use crate::error::{McadError, kernel_call};

/// The persistent name of a face, given by the feature that made it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FaceName {
    /// The board's top face (the one at its thickness).
    Top,
    /// The board's bottom face (the one at z = 0).
    Bottom,
    /// The side wall along outline segment `n`, the segment from outline
    /// point `n` to point `n + 1`.
    OutlineSide(u32),
    /// The wall of round hole `n` of the board's hole list.
    HoleWall(u32),
    /// A wall of polygon cut-out `n` of the board's hole list.
    CutoutWall(u32),
}

/// The persistent name of an edge: the primary names of the faces it joins,
/// sorted.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EdgeName {
    pub faces: Vec<FaceName>,
}

/// The first attribute id this crate uses; name `n` of a solid is id
/// `NAME_ATTRIBUTE_BASE + n`. Far above the ids the kernel itself uses.
const NAME_ATTRIBUTE_BASE: u64 = 0x5347_4E00_0000;

/// A solid's face names, indexed by attribute id.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct NameTable {
    names: Vec<FaceName>,
}

impl NameTable {
    /// The attribute id of `name`, added to the table when new.
    fn id_of(&mut self, name: FaceName) -> u64 {
        let index = match self.names.iter().position(|n| *n == name) {
            Some(index) => index,
            None => {
                self.names.push(name);
                self.names.len() - 1
            }
        };
        NAME_ATTRIBUTE_BASE + index as u64
    }

    fn name_of(&self, attribute_id: u64) -> Option<FaceName> {
        let index = attribute_id.checked_sub(NAME_ATTRIBUTE_BASE)?;
        self.names.get(usize::try_from(index).ok()?).copied()
    }

    /// Tags face `f` of `brep` with `name`.
    pub(crate) fn tag(
        &mut self,
        brep: &mut Brep,
        f: FaceId,
        name: FaceName,
    ) -> Result<(), McadError> {
        let id = self.id_of(name);
        let attribute = Attribute::long(id, 0, Behavior::Copy);
        kernel_call("name face", || {
            brep.add_attribute(EntityRef::Face(f), attribute)
                .map(|_| ())
        })
    }

    /// The names `f` carries, sorted, without repeats.
    pub(crate) fn names(&self, brep: &Brep, f: FaceId) -> Vec<FaceName> {
        let mut names: Vec<FaceName> = brep
            .entity_attributes(EntityRef::Face(f))
            .into_iter()
            .filter_map(|(_, attribute)| self.name_of(attribute.attribute_id()))
            .collect();
        names.sort_unstable();
        names.dedup();
        names
    }
}
