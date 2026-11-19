//! The Footprint preview column of Assign Footprints (Graphite #80): the
//! footprints of the clicked row, read off the UI thread through the library
//! adapters (as the Properties footprint card reads them), with the facts the
//! column states: pad count, pad 1, courtyard and the pin map of a change.

use std::sync::Arc;

use iced::Task;
use signex_library::primitive::footprint::{PadKind, PadShape};
use signex_library::primitive::{Footprint, PrimitiveRef};

use super::AssignFootprintsMsg;
use crate::app::{Message, Signex};
use crate::library::editor::footprint::canvas::PreviewFootprint;

/// The size of pad 1, mm.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PadSize {
    Round(f64),
    Rect(f64, f64),
}

/// What the preview shows of one footprint.
#[derive(Debug, Clone)]
pub struct FootprintFacts {
    pub name: String,
    /// The copper pads' numbers, in footprint order.
    pub pads: Vec<String>,
    pub pad_one: Option<PadSize>,
    /// Courtyard width and height, mm; `None` without a courtyard.
    pub courtyard_mm: Option<(f64, f64)>,
    /// Width of what the preview draws, mm: side by side, each footprint gets
    /// room in proportion to it, so both are drawn near one scale.
    pub width_mm: f64,
    pub preview: PreviewFootprint,
}

impl FootprintFacts {
    pub fn of(footprint: &Footprint) -> Self {
        let copper: Vec<_> = footprint
            .pads
            .iter()
            .filter(|pad| pad.kind != PadKind::NptHole)
            .collect();
        let first = copper
            .iter()
            .find(|pad| pad.number.trim() == "1")
            .or(copper.first());
        let pad_one = first.map(|pad| {
            let [w, h] = pad.size;
            if matches!(pad.shape, PadShape::Round) && (w - h).abs() < 1e-9 {
                PadSize::Round(w)
            } else {
                PadSize::Rect(w, h)
            }
        });
        let preview = PreviewFootprint::build(footprint);
        let width_mm = preview
            .bbox_mm()
            .map_or(1.0, |(x0, _, x1, _)| (x1 - x0).max(0.1));
        Self {
            name: footprint.name.clone(),
            pads: copper
                .iter()
                .map(|pad| pad.number.trim().to_string())
                .filter(|number| !number.is_empty())
                .collect(),
            pad_one,
            courtyard_mm: extent(&footprint.courtyard.points),
            width_mm,
            preview,
        }
    }
}

/// Width and height of a polygon of at least three points.
fn extent(points: &[[f64; 2]]) -> Option<(f64, f64)> {
    if points.len() < 3 {
        return None;
    }
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for [x, y] in points {
        x0 = x0.min(*x);
        y0 = y0.min(*y);
        x1 = x1.max(*x);
        y1 = y1.max(*y);
    }
    Some((x1 - x0, y1 - y0))
}

/// Where a preview footprint stands.
#[derive(Debug, Clone)]
pub enum PreviewLoad {
    Loading,
    Ready(Arc<FootprintFacts>),
    Failed(String),
}

/// How the pins of a part land on the new footprint: pins connect to pads
/// by number, so every pad number of the current footprint must be on the
/// new one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PinMap {
    /// Every current pad number is on the new footprint, in this order.
    Matches(Vec<String>),
    /// These current pad numbers are not on the new footprint.
    Missing(Vec<String>),
}

/// The pin map from `current` to `new` pad numbers.
pub fn pin_map(current: &[String], new: &[String]) -> PinMap {
    let mut missing: Vec<String> = Vec::new();
    for number in current {
        if !new.contains(number) && !missing.contains(number) {
            missing.push(number.clone());
        }
    }
    if missing.is_empty() {
        PinMap::Matches(current.to_vec())
    } else {
        PinMap::Missing(missing)
    }
}

impl Signex {
    /// Start the reads the previewed row still needs: its current footprint
    /// and, for a change, the new one.
    pub(super) fn assign_footprints_preview_reads(&mut self) -> Task<Message> {
        let Some(state) = self.ui_state.assign_footprints.as_mut() else {
            return Task::none();
        };
        let generation = state.generation;
        let missing: Vec<String> = state
            .preview_bindings()
            .into_iter()
            .filter(|binding| !state.previews.contains_key(binding))
            .collect();
        for binding in &missing {
            state.previews.insert(binding.clone(), PreviewLoad::Loading);
        }
        let mut reads = Vec::with_capacity(missing.len());
        for binding in missing {
            match self.preview_read(generation, binding.clone()) {
                Ok(task) => reads.push(task),
                Err(why) => self.finish_preview_read(generation, binding, Err(why)),
            }
        }
        Task::batch(reads)
    }

    /// One footprint read on a worker, behind every write of its library
    /// started so far (#893).
    fn preview_read(&mut self, generation: u64, binding: String) -> Result<Task<Message>, String> {
        let reference = PrimitiveRef::parse_key(&binding)
            .ok_or_else(|| "the part names no library footprint".to_string())?;
        let adapter = self
            .library
            .set
            .get_shared(reference.library_id)
            .ok_or_else(|| "its library is not open".to_string())?;
        let root = self
            .library
            .open_libraries
            .iter()
            .find(|lib| lib.library_id == reference.library_id)
            .map(|lib| lib.root.clone());
        let turn = root.map(|root| self.library.writes.read_turn(&root));
        Ok(Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    if let Some(turn) = turn {
                        turn.wait();
                    }
                    adapter
                        .get_footprint(reference.uuid)
                        .map(|footprint| Arc::new(FootprintFacts::of(&footprint)))
                        .map_err(|e| e.to_string())
                })
                .await
                .unwrap_or_else(|e| Err(format!("footprint worker failed: {e}")))
            },
            move |result| {
                Message::from(AssignFootprintsMsg::PreviewLoaded {
                    generation,
                    binding: binding.clone(),
                    result,
                })
            },
        ))
    }

    /// A preview read finished: keep it, unless the dialog closed or was
    /// opened again since.
    pub(super) fn finish_preview_read(
        &mut self,
        generation: u64,
        binding: String,
        result: Result<Arc<FootprintFacts>, String>,
    ) {
        let Some(state) = self.ui_state.assign_footprints.as_mut() else {
            return;
        };
        if state.generation != generation {
            return;
        }
        let load = match result {
            Ok(facts) => PreviewLoad::Ready(facts),
            Err(why) => {
                crate::diagnostics::log_warning(format!(
                    "Assign Footprints: could not read footprint {binding} for the preview: {why}."
                ));
                PreviewLoad::Failed(why)
            }
        };
        state.previews.insert(binding, load);
    }
}

impl super::AssignFootprintsState {
    /// The bindings the preview draws for the previewed row: the current
    /// footprint, then the picked one when it changes.
    pub(crate) fn preview_bindings(&self) -> Vec<String> {
        let Some(index) = self.preview_row else {
            return Vec::new();
        };
        let Some(part) = self.parts().get(index) else {
            return Vec::new();
        };
        let mut bindings: Vec<String> = part.current_binding.iter().cloned().collect();
        if let Some(Some(pick)) = self.picks.get(index) {
            bindings.push(pick.clone());
        }
        bindings
    }
}
