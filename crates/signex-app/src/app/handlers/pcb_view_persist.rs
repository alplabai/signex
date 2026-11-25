//! The board views kept across launches (#2480). A board that opens takes
//! the view kept for it; a changed view is written to `prefs.json` after a
//! quiet period, off the UI thread, and at quit. The decisions are
//! `crate::persist::DebouncedSync`'s; this file turns them into `Task`s.
//! The codec: `crate::board_view_prefs`.

use std::path::PathBuf;

use iced::Task;

use crate::app::{Message, PcbMsg, Signex, TabKind};
use crate::board_view_prefs::{
    BOARD_VIEWS_SAVE_DEBOUNCE, BoardViewSaveMsg, BoardViewWrite, BoardViews,
};

fn save_msg(msg: BoardViewSaveMsg) -> Message {
    Message::Pcb(PcbMsg::ViewSave(msg))
}

impl Signex {
    /// After every message: an open board with a kept view takes it, and a
    /// changed view arms the save timer.
    pub(crate) fn sync_pcb_layer_views(&mut self) -> Task<Message> {
        self.restore_kept_pcb_layer_views();
        let current: BoardViews = self
            .ui_state
            .pcb_layer_views
            .iter()
            .map(|(path, view)| (path.clone(), view.kept()))
            .collect();
        let Some(generation) = self.ui_state.pcb_layer_view_sync.observe(current) else {
            return Task::none();
        };
        // The `async` block keeps the timer lazy: `update` runs outside the
        // runtime's context.
        Task::perform(
            async { tokio::time::sleep(BOARD_VIEWS_SAVE_DEBOUNCE).await },
            move |()| save_msg(BoardViewSaveMsg::SaveDebounceElapsed(generation)),
        )
    }

    /// Each open board this session has no view for yet takes the one an
    /// earlier launch kept, settled on its stackup and redrawn.
    fn restore_kept_pcb_layer_views(&mut self) {
        if self.ui_state.saved_pcb_layer_views.is_empty() {
            return;
        }
        let mut opened: Vec<PathBuf> = Vec::new();
        for tab in &self.document_state.tabs {
            let waiting = matches!(tab.kind, TabKind::Pcb)
                && self.ui_state.saved_pcb_layer_views.contains_key(&tab.path)
                && !self.ui_state.pcb_layer_views.contains_key(&tab.path)
                && self.document_state.pcb_board(&tab.path).is_some();
            if waiting {
                opened.push(tab.path.clone());
            }
        }
        for path in opened {
            if let Some(kept) = self.ui_state.saved_pcb_layer_views.remove(&path) {
                self.edit_pcb_view_at(&path, move |view, _| *view = kept);
            }
        }
    }

    pub(crate) fn dispatch_board_view_save_message(
        &mut self,
        msg: BoardViewSaveMsg,
    ) -> Task<Message> {
        match msg {
            BoardViewSaveMsg::SaveDebounceElapsed(generation) => {
                match self
                    .ui_state
                    .pcb_layer_view_sync
                    .take_if_current(generation)
                {
                    Some(write) => write_board_views_task(write),
                    None => Task::none(),
                }
            }
            BoardViewSaveMsg::Saved(Ok(())) => Task::none(),
            BoardViewSaveMsg::Saved(Err(reason)) => {
                self.ui_state.pcb_layer_view_sync.write_failed();
                crate::diagnostics::log_error(
                    "The board view was not saved; retrying on the next change or at quit",
                    &anyhow::anyhow!(reason),
                );
                self.sync_diagnostics_panel_ctx();
                Task::none()
            }
        }
    }

    /// Write any pending board views now and wait for a write already
    /// running, then run `next` (the quit path).
    pub(crate) fn flush_pcb_layer_views_then(&mut self, next: Task<Message>) -> Task<Message> {
        let sync = &mut self.ui_state.pcb_layer_view_sync;
        let barrier = sync.barrier();
        let pending = sync.flush().map(write_board_views_task);
        let drained = Task::perform(
            async move {
                tokio::task::spawn_blocking(move || barrier.wait())
                    .await
                    .map_err(|join_error| {
                        format!("waiting for the board view write failed: {join_error}")
                    })
            },
            |result| save_msg(BoardViewSaveMsg::Saved(result)),
        );
        let flushed = match pending {
            Some(write) => write.chain(drained),
            None => drained,
        };
        flushed.chain(next)
    }
}

fn write_board_views_task(write: BoardViewWrite) -> Task<Message> {
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || write.run())
                .await
                .map_err(|join_error| format!("the write task failed: {join_error}"))
                .and_then(|result| result)
        },
        |result| save_msg(BoardViewSaveMsg::Saved(result)),
    )
}

#[cfg(test)]
#[path = "pcb_view_persist_tests.rs"]
mod tests;
