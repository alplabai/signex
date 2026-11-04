//! Every panel context the app keeps: the main window's and one per undocked
//! document window (#1966). A title-block or paper-edge draft is typed into
//! the context of the window it was typed in, so the global actions (save,
//! close, remove, rename, quit) must see all of them (#2027).

use crate::panels::{PanelContext, TitleBlockDraft};

use super::DocumentState;

impl DocumentState {
    /// The main panel context, then each undocked window's.
    pub(crate) fn panel_ctxs(&self) -> impl Iterator<Item = &PanelContext> {
        std::iter::once(&self.panel_ctx).chain(self.window_panel_ctx.values())
    }

    /// [`Self::panel_ctxs`], mutable.
    pub(crate) fn panel_ctxs_mut(&mut self) -> impl Iterator<Item = &mut PanelContext> {
        std::iter::once(&mut self.panel_ctx).chain(self.window_panel_ctx.values_mut())
    }

    /// Every pending title-block draft of every window.
    pub(crate) fn title_block_drafts_all(&self) -> impl Iterator<Item = &TitleBlockDraft> {
        self.panel_ctxs()
            .flat_map(|ctx| ctx.title_block_drafts.iter())
    }
}
