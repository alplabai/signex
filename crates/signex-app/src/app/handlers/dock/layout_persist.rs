//! Debounced, off-thread persistence of the dock layout (#692).
//!
//! The decisions (changed? still current? flush?) live in
//! `crate::dock::DockLayoutSync`; this file only turns them into `Task`s
//! and reports failures to the Messages panel.

use iced::Task;

use crate::dock::{DOCK_LAYOUT_SAVE_DEBOUNCE, DockLayoutMsg, DockLayoutSnapshot, DockLayoutWrite};

use crate::app::{Message, Signex};

impl Signex {
    /// Call after any change to `document_state.dock`. Starts the
    /// debounce timer when the persisted layout really changed; hover and
    /// other transient messages leave it unchanged and cost nothing.
    pub(crate) fn persist_dock_layout_soon(&mut self) -> Task<Message> {
        let current = DockLayoutSnapshot::capture(&self.document_state.dock);
        let Some(generation) = self.document_state.dock_layout_sync.observe(current) else {
            return Task::none();
        };
        // The `async` block keeps the timer lazy: `tokio::time::sleep`
        // registers with the reactor when called, and `update` runs outside
        // the runtime's context.
        Task::perform(
            async { tokio::time::sleep(DOCK_LAYOUT_SAVE_DEBOUNCE).await },
            move |()| Message::DockLayout(DockLayoutMsg::SaveDebounceElapsed(generation)),
        )
    }

    pub(crate) fn dispatch_dock_layout_message(&mut self, msg: DockLayoutMsg) -> Task<Message> {
        match msg {
            DockLayoutMsg::SaveDebounceElapsed(generation) => {
                match self
                    .document_state
                    .dock_layout_sync
                    .take_if_current(generation)
                {
                    Some(write) => write_dock_layout_task(write),
                    None => Task::none(),
                }
            }
            DockLayoutMsg::Saved(Ok(())) => Task::none(),
            DockLayoutMsg::Saved(Err(reason)) => {
                self.document_state.dock_layout_sync.write_failed();
                crate::diagnostics::log_error(
                    "Dock layout was not saved, it will be lost at the next launch",
                    &anyhow::anyhow!(reason),
                );
                self.sync_diagnostics_panel_ctx();
                Task::none()
            }
        }
    }

    /// Write any pending layout now and wait for a write that is already
    /// running, then run `next` (the quit path).
    pub(crate) fn flush_dock_layout_then(&mut self, next: Task<Message>) -> Task<Message> {
        let sync = &mut self.document_state.dock_layout_sync;
        let barrier = sync.barrier();
        let pending = sync.flush().map(write_dock_layout_task);
        let drained = Task::perform(
            async move {
                tokio::task::spawn_blocking(move || barrier.wait())
                    .await
                    .map_err(|join_error| {
                        format!("waiting for the dock write failed: {join_error}")
                    })
            },
            |result| Message::DockLayout(DockLayoutMsg::Saved(result)),
        );
        match pending {
            Some(write) => write.chain(drained).chain(next),
            None => drained.chain(next),
        }
    }
}

fn write_dock_layout_task(write: DockLayoutWrite) -> Task<Message> {
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || write.run())
                .await
                .map_err(|join_error| format!("the write task failed: {join_error}"))
                .and_then(|result| result)
        },
        |result| Message::DockLayout(DockLayoutMsg::Saved(result)),
    )
}
