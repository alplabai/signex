//! Flow progress samples: the output-run pipeline mid-run, finished with
//! warnings, and stopped at Validate. Static samples, so no click handler.

use iced::widget::column;
use iced::{Element, Widget};
use signex_types::theme::ThemeTokens;
use signex_widgets::flow_progress::{
    FlowProgress, FlowProgressStyle, FlowStage, StageCounts, StageState,
};
use signex_widgets::{Label, raw};

use crate::catalog::Message;

/// The app's default body size (`styles::typography::DEFAULT_BASE_SIZE`).
const BODY: f32 = 12.0;

/// Sample text; the real stages take catalog messages.
fn sample(text: &'static str) -> Label<'static> {
    raw(text)
}

/// One pipeline sample: (name, detail, counts) per stage.
fn pipeline(rows: [(&'static str, &'static str, StageCounts); 4]) -> Vec<FlowStage<'static>> {
    rows.into_iter()
        .map(|(name, detail, counts)| FlowStage::from_counts(sample(name), sample(detail), counts))
        .collect()
}

fn counts(total: u32, finished: u32, running: u32, warnings: u32, errors: u32) -> StageCounts {
    StageCounts {
        total,
        finished,
        running,
        warnings,
        errors,
    }
}

fn samples() -> [Vec<FlowStage<'static>>; 3] {
    let running = pipeline([
        ("VALIDATE", "3 of 3 · 2 warnings", counts(3, 3, 0, 2, 0)),
        ("GENERATE", "5 of 11 · running…", counts(11, 5, 1, 0, 0)),
        ("PACKAGE", "pending", counts(2, 0, 0, 0, 0)),
        ("SUMMARY", "pending", counts(1, 0, 0, 0, 0)),
    ]);
    let finished = pipeline([
        ("VALIDATE", "3 of 3", counts(3, 3, 0, 0, 0)),
        ("GENERATE", "11 of 11 · 1 warning", counts(11, 11, 0, 1, 0)),
        ("PACKAGE", "2 of 2", counts(2, 2, 0, 0, 0)),
        ("SUMMARY", "22 files · 0:18", counts(1, 1, 0, 0, 0)),
    ]);
    let stopped = pipeline([
        ("VALIDATE", "1 error", counts(3, 2, 0, 1, 1)),
        ("GENERATE", "not run", counts(11, 0, 0, 0, 0)),
        ("PACKAGE", "not run", counts(2, 0, 0, 0, 0)),
        ("SUMMARY", "not run", counts(1, 0, 0, 0, 0)),
    ]);
    [running, finished, stopped]
}

pub(crate) fn view<'a>(tokens: &ThemeTokens) -> Element<'a, Message> {
    let style = FlowProgressStyle::from_tokens(tokens, BODY);
    let [running, finished, stopped] = samples();
    column![
        FlowProgress::<Message>::new(running, style)
            .selected(Some(1))
            .view(),
        FlowProgress::<Message>::new(finished, style).view(),
        FlowProgress::<Message>::new(stopped, style)
            .selected(Some(0))
            .view(),
    ]
    .spacing(BODY)
    .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_cover_every_stage_state() {
        let mut seen: Vec<StageState> = samples()
            .iter()
            .flat_map(|p| p.iter().map(|s| s.state))
            .collect();
        seen.dedup();
        for state in [
            StageState::Pending,
            StageState::Running,
            StageState::Done,
            StageState::Warning,
            StageState::Failed,
        ] {
            assert!(seen.contains(&state), "no sample shows {state:?}");
        }
    }
}
