//! The indexed `dangling_wire` must report exactly what the original
//! all-pairs scan reported (issue #1366), including at the `same()` epsilon.
use super::*;
use crate::context::{ErcBusEntry, ErcNoConnect};

/// The pre-index implementation, kept verbatim as the oracle.
fn dangling_wire_scan(ctx: &ErcContext, out: &mut Vec<Diagnostic>) {
    let touched_non_wire = |p: &Point| -> bool {
        ctx.symbols
            .iter()
            .flat_map(|s| s.pins.iter())
            .any(|pin| same(&pin.world_pos, p))
            || ctx.junctions.iter().any(|j| same(&j.position, p))
            || ctx.labels.iter().any(|l| same(&l.position, p))
            || ctx.no_connects.iter().any(|nc| same(&nc.position, p))
            || ctx.bus_entries.iter().any(|be| {
                same(&be.position, p)
                    || same(
                        &pt_of(be.position.x + be.size.0, be.position.y + be.size.1),
                        p,
                    )
            })
    };
    for wire in &ctx.wires {
        for endpoint in [wire.start, wire.end] {
            let other = ctx
                .wires
                .iter()
                .filter(|w| w.uuid != wire.uuid)
                .filter(|w| same(&w.start, &endpoint) || same(&w.end, &endpoint))
                .count();
            if other > 0 || touched_non_wire(&endpoint) {
                continue;
            }
            out.push(
                Diagnostic::new(
                    RuleKind::DanglingWire,
                    "Wire endpoint is not connected",
                    endpoint,
                )
                .with_primary(sel(wire.uuid, SelectedKind::Wire)),
            );
        }
    }
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    /// A lattice point, sometimes nudged by sub- and just-over-epsilon jitter.
    fn point(&mut self, span: u64) -> Point {
        let jitter = [0.0, 0.0, 5e-5, -5e-5, 1.5e-4, -1.5e-4];
        let j = |r: &mut Rng| jitter[r.below(jitter.len() as u64) as usize];
        Point {
            x: self.below(span) as f64 + j(self),
            y: self.below(span) as f64 + j(self),
        }
    }
}

fn random_ctx(seed: u64, n_wires: usize, span: u64) -> ErcContext {
    let mut r = Rng(seed);
    let wires = (0..n_wires)
        .map(|i| ErcWire {
            // Occasionally reuse a uuid to exercise the `uuid !=` exclusion.
            uuid: Uuid::from_u128(if r.below(20) == 0 { 7 } else { i as u128 + 100 }),
            start: r.point(span),
            end: r.point(span),
        })
        .collect();
    let n_other = n_wires / 3;
    let mut c = ctx(wires, Vec::new(), Vec::new(), Vec::new());
    for _ in 0..n_other {
        c.symbols.push(symbol_with_pin("R1", r.point(span), true));
        c.junctions.push(ErcJunction {
            position: r.point(span),
        });
        c.labels.push(ErcLabel {
            uuid: Uuid::nil(),
            text: "N".into(),
            position: r.point(span),
            label_type: LabelType::Net,
        });
        c.no_connects.push(ErcNoConnect {
            position: r.point(span),
        });
        c.bus_entries.push(ErcBusEntry {
            uuid: Uuid::nil(),
            position: r.point(span),
            size: (2.54, -2.54),
        });
    }
    c
}

fn summary(out: &[Diagnostic]) -> Vec<String> {
    out.iter().map(|d| format!("{d:?}")).collect()
}

#[test]
fn indexed_dangling_wire_matches_the_all_pairs_scan() {
    for seed in 1..=60u64 {
        let c = random_ctx(seed * 0x9E37_79B9, 40 + (seed as usize % 5) * 20, 12);
        let (mut new, mut old) = (Vec::new(), Vec::new());
        dangling_wire(&c, &mut new);
        dangling_wire_scan(&c, &mut old);
        assert_eq!(summary(&new), summary(&old), "seed {seed}");
        assert!(
            seed > 5 || !old.is_empty(),
            "generator must produce findings"
        );
    }
}

/// The pre-index `on_any_wire` linear scan, kept verbatim as the oracle.
fn on_any_wire_scan(pos: &Point, wires: &[(Point, Point)]) -> bool {
    let pk = pt_key(pos);
    wires
        .iter()
        .any(|(a, b)| signex_net::point_on_segment(pk, pt_key(a), pt_key(b)))
}

#[test]
fn indexed_on_any_wire_matches_the_linear_scan() {
    let mut hits = 0;
    for seed in 1..=60u64 {
        let mut r = Rng(seed * 0x2545_F491);
        let c = random_ctx(seed * 0x9E37_79B9, 30 + (seed as usize % 5) * 20, 12);
        let pairs = wire_pairs(&c);
        let index = SegmentIndex::new(&pairs);
        // Lattice and jittered probes: endpoints, interiors and misses.
        let probes = c
            .labels
            .iter()
            .map(|l| l.position)
            .chain(c.wires.iter().flat_map(|w| [w.start, w.end]))
            .chain((0..200).map(|_| r.point(12)));
        for p in probes {
            let (new, old) = (on_any_wire(&p, &index), on_any_wire_scan(&p, &pairs));
            assert_eq!(new, old, "seed {seed} at {p:?}");
            hits += usize::from(old);
        }
    }
    assert!(hits > 100, "generator must produce on-wire probes");
}

/// Timing evidence for #1366: `cargo test -p signex-erc --release
/// on_any_wire_timing -- --ignored --nocapture`.
#[test]
#[ignore = "timing probe, not a gate"]
fn on_any_wire_timing() {
    let c = random_ctx(7, 6000, 400);
    let pairs = wire_pairs(&c);
    let labels: Vec<Point> = (0..6000).map(|i| Rng(i + 1).point(400)).collect();
    let t = std::time::Instant::now();
    let old = labels
        .iter()
        .filter(|p| on_any_wire_scan(p, &pairs))
        .count();
    let scan = t.elapsed();
    let t = std::time::Instant::now();
    let index = SegmentIndex::new(&pairs);
    let new = labels.iter().filter(|p| on_any_wire(p, &index)).count();
    let indexed = t.elapsed();
    assert_eq!(old, new);
    println!("6000 labels x 6000 wires: scan {scan:?}, indexed {indexed:?}");
}
