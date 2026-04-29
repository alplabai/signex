//! Netlist export.
//!
//! The Standard-format `.net` S-expression emitter that previously lived
//! here was split out as part of the issue #62 Apache-clean cutover.
//! It moves to the optional `signex-standard-import` GPL-3.0 companion
//! repository alongside the rest of the Standard I/O codepaths.
//!
//! Future Signex-native netlist formats (XML, Spice, etc.) will land
//! here as separate `Exporter` impls. The exported types
//! (`NetlistExporter`, `NetlistOptions`, `NetlistOutput`) stay so the
//! app-layer wiring keeps compiling; the exporter currently returns
//! `NetlistError::NotImplemented` to surface the migration to the user.

use thiserror::Error;

use crate::{ExportContext, Exporter};

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
    #[error("netlist export is not yet available in Signex Community; the Standard-format emitter has moved to the signex-standard-import companion repo (issue #62)")]
    NotImplemented,
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
        Err(NetlistError::NotImplemented)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn netlist_export_returns_not_implemented() {
        let ctx = ExportContext {
            sheets: Vec::new(),
            metadata: Default::default(),
        };
        let result = NetlistExporter.export(&ctx, &NetlistOptions::default());
        assert!(matches!(result, Err(NetlistError::NotImplemented)));
    }
}
