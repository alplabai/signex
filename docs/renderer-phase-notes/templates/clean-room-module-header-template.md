# Clean-room Module Header Template

Use this header at the top of every new renderer source file.

//! Module purpose in one sentence.
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources:
//! - IPC-2612-1
//! - IEEE 315
//! - IEC 60617
//! - wgpu/WGSL public documentation

Guideline:

- Add per-constant derivation comments next to each numeric decision.
- Mark non-standard values as Signex design decisions with rationale.
