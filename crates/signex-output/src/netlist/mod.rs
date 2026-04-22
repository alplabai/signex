//! KiCad S-expression `.net` netlist export.
//!
//! See `OUTPUT_PLAN.md` §7. Emits byte-compatible KiCad netlist format so
//! downstream tools (KiCad CvPcb, kinet2pcb, legacy CAM) don't know Signex
//! was the emitter.

use thiserror::Error;

use crate::{ExportContext, Exporter};

mod kicad_sexpr;

pub struct NetlistExporter;

#[derive(Debug, Clone, Default)]
pub struct NetlistOptions {
    pub include_timestamps: bool,
}

#[derive(Debug, Clone)]
pub struct NetlistOutput {
    pub bytes: Vec<u8>,
}

#[derive(Debug, Error)]
pub enum NetlistError {
    #[error("net graph construction failed")]
    GraphConstruction,
}

impl Exporter for NetlistExporter {
    type Options = NetlistOptions;
    type Output = NetlistOutput;
    type Error = NetlistError;

    fn export(
        &self,
        _ctx: &ExportContext,
        _opts: &Self::Options,
    ) -> Result<Self::Output, Self::Error> {
        todo!("netlist emitter — implemented in a follow-up PR on feature/v0.8-output")
    }
}
