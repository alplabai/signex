//! The one store of half-typed Properties-panel text (#2027): title-block
//! fields and custom paper edges.
//!
//! A draft belongs to a (sheet, field) or (sheet, edge), not to the window it
//! was typed in, so there is exactly one of each whichever window typed it.
//! `DocumentState` owns the store; every `PanelContext` (the main window's and
//! each undocked window's) holds a clone of the same handle only so the
//! Properties view can read it. Nothing keeps a copy of a draft, so a context
//! that is cloned, swapped out for a window action or dropped can neither hide
//! a draft from a global action nor resurrect a stale one.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use super::{PaperEdge, PaperEdgeDraft, TitleBlockDraft, TitleBlockField};

#[derive(Default)]
struct Drafts {
    title_block: Vec<TitleBlockDraft>,
    paper_edge: Vec<PaperEdgeDraft>,
}

/// A handle on the single draft store; clones share it.
#[derive(Clone, Default)]
pub struct PanelDrafts(Arc<Mutex<Drafts>>);

impl PanelDrafts {
    fn lock(&self) -> MutexGuard<'_, Drafts> {
        // A panic while the lock is held leaves plain vectors, still valid.
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Whether no draft of either kind is pending.
    pub fn is_empty(&self) -> bool {
        let d = self.lock();
        d.title_block.is_empty() && d.paper_edge.is_empty()
    }

    /// The typed text of a sheet's title-block field, while one is pending.
    pub fn title_block_text(&self, sheet: &Path, field: TitleBlockField) -> Option<String> {
        let d = self.lock();
        let found = d
            .title_block
            .iter()
            .find(|d| d.field == field && d.sheet == sheet);
        found.map(|d| d.text.clone())
    }

    /// Record the text typed into a field, replacing that field's draft.
    pub fn set_title_block(&self, sheet: &Path, field: TitleBlockField, text: &str) {
        let mut d = self.lock();
        d.title_block
            .retain(|d| !(d.field == field && d.sheet == sheet));
        d.title_block.push(TitleBlockDraft {
            sheet: sheet.to_path_buf(),
            field,
            text: text.to_string(),
        });
    }

    /// Remove and return a field's draft: the commit of that field.
    pub fn take_title_block(
        &self,
        sheet: &Path,
        field: TitleBlockField,
    ) -> Option<TitleBlockDraft> {
        let mut d = self.lock();
        let at = d
            .title_block
            .iter()
            .position(|d| d.field == field && d.sheet == sheet)?;
        Some(d.title_block.remove(at))
    }

    /// Remove and return the title-block drafts of the sheets `matches` picks;
    /// the rest stay pending.
    pub fn take_title_blocks_where(&self, matches: impl Fn(&Path) -> bool) -> Vec<TitleBlockDraft> {
        let mut d = self.lock();
        let all = std::mem::take(&mut d.title_block);
        let (taken, kept) = all.into_iter().partition(|d| matches(&d.sheet));
        d.title_block = kept;
        taken
    }

    /// Every pending title-block draft, as a copy.
    pub fn title_blocks(&self) -> Vec<TitleBlockDraft> {
        self.lock().title_block.clone()
    }

    /// The sheets with a pending title-block draft, one entry per draft.
    pub fn title_block_sheets(&self) -> Vec<PathBuf> {
        let d = self.lock();
        d.title_block.iter().map(|d| d.sheet.clone()).collect()
    }

    /// The typed text of a sheet's custom paper edge, while one is pending.
    pub fn paper_edge_text(&self, sheet: &Path, edge: PaperEdge) -> Option<String> {
        let d = self.lock();
        let found = d
            .paper_edge
            .iter()
            .find(|d| d.edge == edge && d.sheet == sheet);
        found.map(|d| d.text.clone())
    }

    /// Record the text typed into a paper edge, replacing that edge's draft.
    pub fn set_paper_edge(&self, sheet: &Path, edge: PaperEdge, text: &str) {
        let mut d = self.lock();
        d.paper_edge
            .retain(|d| !(d.edge == edge && d.sheet == sheet));
        d.paper_edge.push(PaperEdgeDraft {
            sheet: sheet.to_path_buf(),
            edge,
            text: text.to_string(),
        });
    }

    /// Remove and return a paper edge's draft: the commit of that edge.
    pub fn take_paper_edge(&self, sheet: &Path, edge: PaperEdge) -> Option<PaperEdgeDraft> {
        let mut d = self.lock();
        let at = d
            .paper_edge
            .iter()
            .position(|d| d.edge == edge && d.sheet == sheet)?;
        Some(d.paper_edge.remove(at))
    }

    /// Every pending paper-edge draft, as a copy.
    pub fn paper_edges(&self) -> Vec<PaperEdgeDraft> {
        self.lock().paper_edge.clone()
    }

    /// Drop every draft of the sheet at `sheet`: a discard must not write
    /// what it was asked to throw away.
    pub fn drop_sheet(&self, sheet: &Path) {
        let mut d = self.lock();
        d.title_block.retain(|d| d.sheet != sheet);
        d.paper_edge.retain(|d| d.sheet != sheet);
    }

    /// Keep the drafts of the sheets `keep` accepts; a closed sheet's go.
    pub fn retain_sheets(&self, keep: impl Fn(&Path) -> bool) {
        let mut d = self.lock();
        d.title_block.retain(|d| keep(&d.sheet));
        d.paper_edge.retain(|d| keep(&d.sheet));
    }

    /// A sheet was renamed or moved: its drafts follow it.
    pub fn rekey(&self, old: &Path, new: &Path) {
        let mut d = self.lock();
        for draft in d.title_block.iter_mut().filter(|d| d.sheet == old) {
            draft.sheet = new.to_path_buf();
        }
        for draft in d.paper_edge.iter_mut().filter(|d| d.sheet == old) {
            draft.sheet = new.to_path_buf();
        }
    }
}
