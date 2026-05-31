//! Shared test fixtures for the constraint-residual integration tests.
//!
//! Test files (`constraints_*.rs`) compile as separate crates, but they
//! all share the same `signex-sketch` API. Concrete sketch builders
//! live here so each test file can import them via
//! `mod common;` and avoid drifting fixtures.

#![allow(dead_code)] // tests pull in only the helpers they use

use signex_sketch::SketchData;
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};

pub struct Sketch {
    pub data: SketchData,
    pub plane: PlaneId,
}

impl Sketch {
    pub fn new() -> Self {
        let plane = Plane {
            id: PlaneId::new(),
            kind: PlaneKind::BoardTop,
        };
        let mut data = SketchData::default();
        let plane_id = plane.id;
        data.planes.push(plane);
        Self {
            data,
            plane: plane_id,
        }
    }

    pub fn add_point(&mut self, x: f64, y: f64) -> SketchEntityId {
        let id = SketchEntityId::new();
        self.data
            .entities
            .push(Entity::new(id, self.plane, EntityKind::Point { x, y }));
        id
    }

    pub fn add_line(&mut self, start: SketchEntityId, end: SketchEntityId) -> SketchEntityId {
        let id = SketchEntityId::new();
        self.data
            .entities
            .push(Entity::new(id, self.plane, EntityKind::Line { start, end }));
        id
    }

    pub fn add_arc(
        &mut self,
        center: SketchEntityId,
        start: SketchEntityId,
        end: SketchEntityId,
        sweep_ccw: bool,
    ) -> SketchEntityId {
        let id = SketchEntityId::new();
        self.data.entities.push(Entity::new(
            id,
            self.plane,
            EntityKind::Arc {
                center,
                start,
                end,
                sweep_ccw,
            },
        ));
        id
    }

    pub fn add_circle(&mut self, center: SketchEntityId, radius: f64) -> SketchEntityId {
        let id = SketchEntityId::new();
        self.data.entities.push(Entity::new(
            id,
            self.plane,
            EntityKind::Circle { center, radius },
        ));
        id
    }
}
