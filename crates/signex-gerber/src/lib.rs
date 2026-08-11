//! Gerber and Excellon viewer-domain support.
//!
//! File parsing and editing are delegated to `lib_gerber_edit`. This crate
//! translates the parsed representation into stable, renderer-independent
//! geometry consumed by the Signex application.

mod archive;
mod geometry;
mod job;
mod loading;
mod metadata;
mod object_attributes;

pub use archive::{load_zip_archive, load_zip_reader};
pub use geometry::{
    ApertureShape, Bounds, GerberGeometry, GerberPrimitive, Point, PrimitivePolarity,
};
pub use job::{GerberJobContext, JobFileAttributes, load_gerber_job_file};
pub use lib_gerber_edit::layer::LayerType;
pub use loading::{
    GerberLoadBatch, GerberLoadFailure, GerberReloadBatch, LoadedLayer, load_autodetected_file,
    load_autodetected_files, load_autodetected_reader, load_excellon_file, load_excellon_files,
    load_excellon_reader, load_fabrication_files, load_gerber_file, load_gerber_files,
    load_gerber_reader, reload_layers,
};
pub use metadata::{LayerDefinition, LayerDefinitionGroup, LayerMetadata};
pub use object_attributes::{GerberAttributeValue, GerberObjectAttributes};

#[cfg(test)]
mod geometry_tests;
#[cfg(test)]
mod loading_tests;
