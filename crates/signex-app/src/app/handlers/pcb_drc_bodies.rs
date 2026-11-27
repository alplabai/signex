//! The parts' 3D bodies a DRC run and `query_part_heights` measure (#2939):
//! the library models of the footprints a board's parts came from, read
//! with the 3D view's reader (blocking IO, so only in a worker) and turned
//! into `signex_drc` body models.

use std::collections::HashSet;
use std::path::PathBuf;

use signex_drc::{BodyModel, BodyModels};
use signex_renderer::board3d::{ComponentModel, model_point_on_footprint};
use signex_types::part::FootprintSourceRef;
use signex_types::pcb::PcbBoard;
use uuid::Uuid;

use crate::app::Signex;

impl Signex {
    /// The open libraries by id and root, for a worker that reads models.
    pub(crate) fn open_library_roots(&self) -> Vec<(Uuid, PathBuf)> {
        self.library
            .open_libraries
            .iter()
            .map(|l| (l.library_id, l.root.clone()))
            .collect()
    }
}

/// The models of `board`'s parts from the open `libraries`, and a line per
/// model that could not be read (that part's stored height is used).
/// Nothing is read when no library is open. Blocking IO.
pub(crate) fn body_models(
    board: &PcbBoard,
    libraries: &[(Uuid, PathBuf)],
) -> (BodyModels, Vec<String>) {
    if libraries.is_empty() {
        return (BodyModels::new(), Vec::new());
    }
    let mut seen = HashSet::new();
    let sources: Vec<FootprintSourceRef> = board
        .footprints
        .iter()
        .filter_map(|f| f.library_ref.clone())
        .filter(|source| seen.insert(source.clone()))
        .collect();
    let read = super::board_3d::read_models(&sources, libraries);
    let models = read
        .models
        .iter()
        .map(|(source, model)| (source.clone(), body_model(model)))
        .collect();
    let unread = read
        .failures
        .iter()
        .map(|(source, why)| {
            format!(
                "DRC: the 3D model of {source} could not be read ({why}); its stored height is used"
            )
        })
        .collect();
    (models, unread)
}

/// A model as the 3D view reads it, as DRC measures it.
fn body_model(model: &ComponentModel) -> BodyModel {
    match model {
        ComponentModel::Model(attached) => BodyModel::Points(
            attached
                .meshes
                .iter()
                .flat_map(|mesh| mesh.positions.iter())
                .map(|&p| model_point_on_footprint(attached, p))
                .collect(),
        ),
        ComponentModel::Body(body) => BodyModel::Extrusion {
            outline: body.outline.clone(),
            offset_z_mm: body.offset_z_mm,
            height_mm: body.height_mm,
        },
    }
}

#[cfg(test)]
mod tests {
    use signex_renderer::board3d::{AttachedModel, BodyExtrusion, ModelMesh};

    use super::*;

    /// An attached model is measured where the 3D view draws it: its
    /// offset applied and its y turned to point down the board.
    #[test]
    fn a_model_is_measured_where_the_3d_view_draws_it() {
        let model = ComponentModel::Model(AttachedModel {
            meshes: vec![ModelMesh {
                positions: vec![[1.0, 2.0, 3.0]],
                triangles: Vec::new(),
                color: [1.0; 4],
            }],
            offset_mm: [0.5, 0.5, 1.0],
            rotation_deg: [0.0; 3],
        });
        assert_eq!(
            body_model(&model),
            BodyModel::Points(vec![[1.5, -2.5, 4.0]])
        );
        let body = ComponentModel::Body(BodyExtrusion {
            outline: None,
            height_mm: 2.0,
            offset_z_mm: 0.25,
            colors: None,
        });
        assert_eq!(
            body_model(&body),
            BodyModel::Extrusion {
                outline: None,
                offset_z_mm: 0.25,
                height_mm: 2.0,
            }
        );
    }
}
