//! Live ERC in `update`: the wait after an edit, the background check, and
//! the merge of its findings into the per-sheet cache.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use iced::{Subscription, Task};

use super::job::{self, LiveErcInputs, LiveErcOutcome};
use super::{LIVE_ERC_DEBOUNCE, LiveErcMsg};
use crate::app::{ErcMsg, Message, Signex};

impl Signex {
    /// After every update: a new sheet edit starts the wait, an open sheet
    /// gets a baseline the first time it is seen, a closed one loses its own.
    pub(crate) fn sync_live_erc(&mut self) {
        let epoch = self.document_state.sheet_edit_epoch;
        let live = &mut self.ui_state.live_erc;
        let edited = live.observe(epoch);
        if !live.is_active() {
            return;
        }
        let engines = &self.document_state.engines;
        live.baselines.retain(|path, _| engines.contains_key(path));
        for (path, engine) in engines {
            // Seen first in the update that edited it: the edit is already
            // in the document, so the next check takes the whole sheet.
            live.baselines
                .entry(path.clone())
                .or_insert_with(|| (!edited).then(|| Arc::new(engine.document().clone())));
        }
    }

    /// The debounce timer: one tick [`LIVE_ERC_DEBOUNCE`] after the newest
    /// edit, none while nothing is owed. Each wait has its own identity, so a
    /// later edit restarts the timer.
    pub(crate) fn live_erc_subscription(&self) -> Subscription<Message> {
        match self.ui_state.live_erc.pending() {
            Some(token) => iced::time::every(LIVE_ERC_DEBOUNCE)
                .with(token)
                .map(|(token, _)| Message::Erc(ErcMsg::Live(LiveErcMsg::DebounceElapsed(token)))),
            None => Subscription::none(),
        }
    }

    pub(crate) fn dispatch_live_erc_message(&mut self, msg: LiveErcMsg) -> Task<Message> {
        match msg {
            LiveErcMsg::DebounceElapsed(token) => self.handle_live_erc_due(token),
            LiveErcMsg::Finished { generation, result } => {
                self.apply_live_erc_result(generation, &result);
                Task::none()
            }
            LiveErcMsg::Toggled => {
                let enabled = !self.ui_state.live_erc.enabled;
                self.set_live_erc(enabled)
            }
        }
    }

    /// Set the `live_erc` preference and save it; returns the write.
    pub(crate) fn set_live_erc(&mut self, enabled: bool) -> Task<Message> {
        self.ui_state.live_erc.set_enabled(enabled);
        // Seed the baselines now, so the next edit is scoped against the
        // sheets as they are when the preference turned on.
        self.sync_live_erc();
        crate::fonts::background_pref_write(crate::fonts::LIVE_ERC_KEY, move || {
            crate::fonts::write_live_erc_pref(enabled)
        })
    }

    /// The wait `token` ended: start a check unless a newer edit restarted
    /// the wait, live ERC is off, or a check or a Run ERC is in flight (then
    /// wait again).
    fn handle_live_erc_due(&mut self, token: u64) -> Task<Message> {
        let live = &mut self.ui_state.live_erc;
        if !live.take_elapsed(token) || !live.is_active() {
            return Task::none();
        }
        // One check at a time: after the one out, or the Run ERC, wait again.
        if self.ui_state.erc_running || live.is_running() {
            live.rearm();
            return Task::none();
        }
        if self.document_state.engines.is_empty() {
            return Task::none();
        }
        let (generation, inputs) = self.begin_live_erc();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || job::run_live_erc_job(inputs))
                    .await
                    .map_err(|e| format!("Live ERC worker failed: {e}"))
            },
            move |result| {
                Message::Erc(ErcMsg::Live(LiveErcMsg::Finished {
                    generation,
                    result: Arc::new(result),
                }))
            },
        )
    }

    /// Capture a live check's inputs (memory only, no IO, no Messages line)
    /// and claim its token. The ERC panel does not show it as running.
    pub(super) fn begin_live_erc(&mut self) -> (u64, LiveErcInputs) {
        let rules = self.active_erc_rule_settings();
        let run = self.erc_run_inputs(rules);
        let live = &mut self.ui_state.live_erc;
        let generation = live.begin();
        let baselines = live.baselines.clone();
        (generation, LiveErcInputs { run, baselines })
    }

    /// Run a live check to completion on the calling thread - tests only.
    #[cfg(test)]
    pub(crate) fn run_live_erc_blocking(&mut self) -> bool {
        let (generation, inputs) = self.begin_live_erc();
        let outcome = Ok(job::run_live_erc_job(inputs));
        self.apply_live_erc_result(generation, &outcome)
    }

    /// Apply a finished live check, unless a newer check or a Run ERC started
    /// since: on each sheet, the findings on what the edit touched are
    /// replaced, the markers and the panel counts follow. Says nothing in
    /// Messages but a failed worker. Returns whether it applied.
    pub(crate) fn apply_live_erc_result(
        &mut self,
        generation: u64,
        result: &Result<LiveErcOutcome, String>,
    ) -> bool {
        if !self.ui_state.live_erc.finish(generation) {
            return false;
        }
        let outcome = match result {
            Ok(outcome) => outcome,
            Err(why) => {
                crate::diagnostics::log_warning(why);
                return false;
            }
        };
        let mut paths: BTreeSet<PathBuf> = outcome.by_path.keys().cloned().collect();
        paths.extend(self.ui_state.erc_violations_by_path.keys().cloned());
        let mut changed = false;
        for path in paths {
            let scope = outcome.scope_of(&path);
            if scope.is_empty() {
                continue;
            }
            let cache = &mut self.ui_state.erc_violations_by_path;
            let cached = cache.get(&path).map(Vec::as_slice).unwrap_or_default();
            let fresh = outcome
                .by_path
                .get(&path)
                .map(Vec::as_slice)
                .unwrap_or_default();
            let merged = signex_erc::live::merge_live(cached, fresh, &scope);
            // A sheet nothing was ever found on gets no entry, so the panel
            // still reads "not run" until something is found.
            if merged.is_empty() && !cache.contains_key(&path) {
                continue;
            }
            cache.insert(path, merged);
            changed = true;
        }
        for (path, frame) in &outcome.page_frames {
            self.ui_state.erc_page_frames.insert(path.clone(), *frame);
        }
        let live = &mut self.ui_state.live_erc;
        for (path, sheet) in &outcome.read {
            live.baselines.insert(path.clone(), Some(sheet.clone()));
        }
        if changed {
            // The lists moved: no row stays selected under another finding.
            self.ui_state.erc_focus_global_index = None;
        }
        let active = self.document_state.active_path.clone();
        self.refresh_active_erc_from_cache(active.as_ref());
        self.refresh_panel_ctx();
        true
    }
}
