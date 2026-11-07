//! Whether each minted junction dot of a sheet is still justified, kept so a
//! sheet that differs from it by a few wires revalidates only the dots near
//! those wires (#2077).
//!
//! A dot's verdict ([`wire_meeting_justifies_junction`]) reads only the wires
//! within the junction tolerance of its point. So in a later sheet a dot that
//! kept its id and position, and that no added, removed or changed wire comes
//! near (old or new geometry), has the verdict it had here. The others are
//! judged again.

use std::collections::{HashMap, HashSet};

use signex_types::schematic::{Junction, Point, SchematicSheet, Wire};
use uuid::Uuid;

use super::autoplace::wire_meeting_justifies_junction;
use super::wire_index::WireIndex;

/// An axis-aligned box in mm: `(min x, min y, max x, max y)`.
type Area = (f64, f64, f64, f64);

/// The minted dots of a sheet with their verdicts, and the wires they were
/// judged against.
#[derive(Debug)]
pub(crate) struct DotVerdicts {
    wires: HashMap<Uuid, (Point, Point)>,
    dots: HashMap<Uuid, (Point, bool)>,
}

/// The ids in `ids` when no two are equal.
fn unique<I: IntoIterator<Item = Uuid>>(ids: I) -> Option<HashSet<Uuid>> {
    let mut seen = HashSet::new();
    ids.into_iter().all(|id| seen.insert(id)).then_some(seen)
}

/// The wire's bounding box grown by twice the tolerance: every point the
/// verdict of a dot could read the wire from. `None` for a non-finite wire.
fn area_of((a, b): (Point, Point)) -> Option<Area> {
    if ![a.x, a.y, b.x, b.y].iter().all(|v| v.is_finite()) {
        return None;
    }
    let margin = 2.0 * crate::JUNCTION_TOLERANCE_MM;
    Some((
        a.x.min(b.x) - margin,
        a.y.min(b.y) - margin,
        a.x.max(b.x) + margin,
        a.y.max(b.y) + margin,
    ))
}

fn inside(p: Point, (x0, y0, x1, y1): Area) -> bool {
    p.x >= x0 && p.x <= x1 && p.y >= y0 && p.y <= y1
}

impl DotVerdicts {
    /// Judge every minted dot of `sheet`; `None` when two wires share an id,
    /// so a later sheet's wires could not be matched against these. (A
    /// verdict depends only on a dot's position, so a repeated dot id is
    /// harmless: [`kept`](Self::kept) also matches the position.)
    #[must_use]
    pub(crate) fn new(sheet: &SchematicSheet) -> Option<Self> {
        unique(sheet.wires.iter().map(|w| w.uuid))?;
        let minted = || sheet.junctions.iter().filter(|j| j.minted);
        let index = WireIndex::new(&sheet.wires, crate::JUNCTION_TOLERANCE_MM);
        Some(Self {
            wires: sheet
                .wires
                .iter()
                .map(|w| (w.uuid, (w.start, w.end)))
                .collect(),
            dots: minted()
                .map(|j| {
                    let ok = wire_meeting_justifies_junction(j.position, &index);
                    (j.uuid, (j.position, ok))
                })
                .collect(),
        })
    }

    /// Whether the sheet had no minted dot, so there is no verdict to keep.
    pub(crate) fn is_empty(&self) -> bool {
        self.dots.is_empty()
    }

    /// Where `wires` differ from the judged wires: the areas of every wire
    /// added, removed or changed, as it was and as it is. `None` when two of
    /// `wires` share an id, or a changed wire has a non-finite coordinate (its
    /// key-space extent need not lie in its area): then every dot is judged.
    pub(crate) fn changed_areas(&self, wires: &[Wire]) -> Option<Vec<Area>> {
        let now = unique(wires.iter().map(|w| w.uuid))?;
        let mut areas = Vec::new();
        for wire in wires {
            let geom = (wire.start, wire.end);
            match self.wires.get(&wire.uuid) {
                Some(&was) if was == geom => {}
                Some(&was) => areas.extend([area_of(was)?, area_of(geom)?]),
                None => areas.push(area_of(geom)?),
            }
        }
        areas.extend(
            self.wires
                .iter()
                .filter(|(id, _)| !now.contains(id))
                .map(|(_, &was)| area_of(was))
                .collect::<Option<Vec<_>>>()?,
        );
        Some(areas)
    }

    /// The verdict `junction` had, when it still holds: same id and position,
    /// and outside every changed area.
    pub(crate) fn kept(&self, junction: &Junction, changed: &[Area]) -> Option<bool> {
        let &(at, ok) = self.dots.get(&junction.uuid)?;
        (at == junction.position && !changed.iter().any(|&area| inside(at, area))).then_some(ok)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Lcg(u64);

    impl Lcg {
        fn next(&mut self, bound: u64) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (self.0 >> 33) % bound
        }

        fn spot(&mut self) -> Point {
            Point::new(5.0 * self.next(8) as f64, 5.0 * self.next(8) as f64)
        }
    }

    fn wire(a: Point, b: Point) -> Wire {
        Wire {
            uuid: Uuid::now_v7(),
            start: a,
            end: b,
            ..Default::default()
        }
    }

    /// An axis-aligned wire from a random grid point.
    fn random_wire(rng: &mut Lcg) -> Wire {
        let a = rng.spot();
        let len = 5.0 * (1 + rng.next(4)) as f64;
        let b = if rng.next(2) == 0 {
            Point::new(a.x + len, a.y)
        } else {
            Point::new(a.x, a.y + len)
        };
        wire(a, b)
    }

    fn judge(at: Point, wires: &[Wire], lookups: usize) -> bool {
        let index = WireIndex::for_lookups(wires, crate::JUNCTION_TOLERANCE_MM, lookups);
        wire_meeting_justifies_junction(at, &index)
    }

    /// Every verdict `kept` reuses after random wire edits is the one judging
    /// the dot again gives, and the scanning index judges as the gridded one.
    #[test]
    fn a_kept_verdict_is_the_verdict_of_the_edited_sheet() {
        let mut reused = 0;
        for seed in 1..=200u64 {
            let mut rng = Lcg(seed);
            let mut sheet = SchematicSheet::default();
            for _ in 0..12 {
                sheet.wires.push(random_wire(&mut rng));
            }
            // Dots on wire ends, and just off them inside the tolerance (on
            // either axis, and a few key units off in the 1 um key space), so
            // an area smaller than the tolerance would reuse a stale verdict.
            const NEAR: [(f64, f64); 5] = [
                (0.0, 0.0),
                (0.009, 0.0),
                (0.0, -0.009),
                (-0.003, 0.003),
                (0.0, 0.0004),
            ];
            let dots: Vec<Point> = (sheet.wires.iter())
                .flat_map(|w| [w.start, w.end])
                .flat_map(|p| NEAR.map(|(dx, dy)| Point::new(p.x + dx, p.y + dy)))
                .chain((0..6).map(|_| rng.spot()))
                .collect();
            sheet.junctions = dots
                .iter()
                .map(|&position| Junction {
                    uuid: Uuid::now_v7(),
                    position,
                    minted: true,
                    ..Default::default()
                })
                .collect();
            let verdicts = DotVerdicts::new(&sheet).expect("unique wire ids");
            let mut edited = sheet.wires.clone();
            for _ in 0..1 + rng.next(3) {
                let at = rng.next(edited.len() as u64) as usize;
                match rng.next(3) {
                    0 => {
                        edited.remove(at);
                    }
                    1 => edited.push(random_wire(&mut rng)),
                    _ => {
                        let moved = random_wire(&mut rng);
                        edited[at].start = moved.start;
                        edited[at].end = moved.end;
                    }
                }
            }
            let areas = verdicts.changed_areas(&edited).expect("unique wire ids");
            for dot in &sheet.junctions {
                let fresh = judge(dot.position, &edited, usize::MAX);
                assert_eq!(judge(dot.position, &edited, 0), fresh, "seed {seed}");
                if let Some(kept) = verdicts.kept(dot, &areas) {
                    assert_eq!(kept, fresh, "seed {seed}, dot at {:?}", dot.position);
                    reused += 1;
                }
            }
        }
        assert!(reused > 1000, "only {reused} verdicts reused");
    }

    #[test]
    fn a_changed_wire_with_a_non_finite_end_judges_every_dot() {
        let mut sheet = SchematicSheet::default();
        sheet
            .wires
            .push(wire(Point::new(0.0, 0.0), Point::new(10.0, 0.0)));
        let verdicts = DotVerdicts::new(&sheet).expect("unique");
        let mut edited = sheet.wires.clone();
        edited[0].start.x = f64::NAN;
        assert!(verdicts.changed_areas(&edited).is_none());
        assert!(verdicts.changed_areas(&sheet.wires).is_some());
    }

    #[test]
    fn repeated_wire_ids_are_not_matched() {
        let mut sheet = SchematicSheet::default();
        let w = wire(Point::new(0.0, 0.0), Point::new(10.0, 0.0));
        sheet.wires = vec![w.clone(), w.clone()];
        assert!(DotVerdicts::new(&sheet).is_none());
        sheet.wires.pop();
        let verdicts = DotVerdicts::new(&sheet).expect("unique");
        assert!(verdicts.changed_areas(&[w.clone(), w]).is_none());
    }
}
