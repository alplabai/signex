//! The worker half of Compare Board with Schematic: Update PCB's design read
//! and Back Annotate's read, one after the other on one blocking thread, at
//! the same board revision. The report is built on the update thread once
//! the read lands (the shared-sheet check needs the open projects), and its
//! net rows are computed back on a worker.

use std::sync::Arc;

use crate::app::back_annotate::BackRead;
use crate::app::back_annotate::read::BackRequest;
use crate::app::update_pcb::Computed;
use crate::app::update_pcb::design::DesignRequest;

/// Everything the worker reads, taken on the update thread.
pub(crate) struct CompareRequest {
    pub design: DesignRequest,
    pub back: BackRequest,
}

/// What one read produced: Update PCB's design and change list, and Back
/// Annotate's read and change list.
pub struct CompareRead {
    pub(crate) pcb: Arc<Computed>,
    pub(crate) back: Arc<BackRead>,
}

/// Terse: every message is formatted for the frame probe.
impl std::fmt::Debug for CompareRead {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompareRead")
            .field("pcb", &self.pcb)
            .field("back_rows", &self.back.list.rows.len())
            .finish_non_exhaustive()
    }
}

/// One read is equal only to itself: messages compare reads by identity.
impl PartialEq for CompareRead {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

/// Read the design both ways. `Err` says why there is no report.
pub(crate) fn compute(request: CompareRequest) -> Result<CompareRead, String> {
    let pcb = crate::app::update_pcb::design::compute(request.design)?;
    let back = crate::app::back_annotate::read::compute(request.back)?;
    Ok(CompareRead {
        pcb,
        back: Arc::new(back),
    })
}
