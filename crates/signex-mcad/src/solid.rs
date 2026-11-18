//! A solid body and what can be read from it.

use std::fmt;

use solidcore::geom::attribute::{AttributeStore, ObjectKey};
use solidcore::mass::integrator::properties::{PropertyAttributes, brep_volume};
use solidcore::topology::Brep;

use crate::error::{McadError, contained, kernel_call};
use crate::naming::{EdgeName, FaceName, NameTable};
use crate::topology::edge_faces;

/// A solid body: an exact B-rep with persistent face names.
pub struct Solid {
    pub(crate) brep: Brep,
    pub(crate) names: NameTable,
}

/// One face of a [`Solid`], in the solid's face order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaceInfo {
    /// Position in the solid's face order (not persistent).
    pub index: usize,
    /// The face's persistent names, sorted. A face made from several named
    /// faces carries all their names; an unnamed face carries none.
    pub names: Vec<FaceName>,
}

/// The kernel's relative accuracy for mass properties.
const VOLUME_ACCURACY: f64 = 1e-6;

impl Solid {
    /// Number of faces.
    #[must_use]
    pub fn face_count(&self) -> usize {
        self.brep.face_list().len()
    }

    /// Number of edges.
    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.brep.edge_list().len()
    }

    /// The faces with their persistent names.
    #[must_use]
    pub fn faces(&self) -> Vec<FaceInfo> {
        self.brep
            .face_list()
            .iter()
            .enumerate()
            .map(|(index, &f)| FaceInfo {
                index,
                names: self.names.names(&self.brep, f),
            })
            .collect()
    }

    /// The edges with their persistent names, in the solid's edge order.
    ///
    /// # Errors
    ///
    /// [`McadError`] when the kernel cannot read an edge's faces.
    pub fn edges(&self) -> Result<Vec<EdgeName>, McadError> {
        contained("read edges", || {
            self.brep
                .edge_list()
                .iter()
                .map(|&e| {
                    let mut faces: Vec<FaceName> = edge_faces(&self.brep, e)?
                        .into_iter()
                        .filter_map(|f| self.names.names(&self.brep, f).first().copied())
                        .collect();
                    faces.sort_unstable();
                    faces.dedup();
                    Ok(EdgeName { faces })
                })
                .collect()
        })
    }

    /// The enclosed volume, mm³, from the exact geometry.
    ///
    /// # Errors
    ///
    /// [`McadError`] when the kernel's mass-property integration fails.
    pub fn volume(&mut self) -> Result<f64, McadError> {
        let kernel = crate::board::new_kernel();
        let store = AttributeStore::default();
        let attributes = PropertyAttributes {
            store: &store,
            key_of: &|_| ObjectKey(0),
        };
        kernel_call("volume", || {
            brep_volume(&kernel, &mut self.brep, VOLUME_ACCURACY, &attributes)
        })
    }
}

impl fmt::Debug for Solid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Solid")
            .field("faces", &self.face_count())
            .field("edges", &self.edge_count())
            .finish_non_exhaustive()
    }
}
