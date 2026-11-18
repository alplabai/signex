//! A small reader of the STEP files the writer makes, for checks the
//! importer does not do: instance lookup, the edge uses of every face, and
//! the polylines its straight-edged bounds trace.

use std::collections::HashMap;

/// The DATA section: instance number to (type, parameters).
pub struct StepFile {
    pub entities: HashMap<u64, (String, String)>,
}

impl StepFile {
    pub fn parse(bytes: &[u8]) -> Self {
        let text = std::str::from_utf8(bytes).expect("STEP is ASCII");
        let data = text
            .split_once("DATA;\n")
            .and_then(|(_, rest)| rest.split_once("ENDSEC;\nEND-ISO-10303-21;"))
            .map(|(data, _)| data)
            .expect("DATA section");
        let entities = data
            .lines()
            .map(|line| {
                let (name, record) = line.split_once('=').expect("#n=record");
                let id = name.trim_start_matches('#').parse().expect("id");
                let record = record.strip_suffix(';').expect("ends with ;");
                let (kind, params) = match record.split_once('(') {
                    Some((kind, params)) if !kind.is_empty() => {
                        (kind.to_owned(), params[..params.len() - 1].to_owned())
                    }
                    _ => (String::new(), record.to_owned()),
                };
                (id, (kind, params))
            })
            .collect();
        Self { entities }
    }

    pub fn of_kind(&self, kind: &str) -> Vec<u64> {
        let mut ids: Vec<u64> = self
            .entities
            .iter()
            .filter(|(_, (k, _))| k == kind)
            .map(|(&id, _)| id)
            .collect();
        ids.sort_unstable();
        ids
    }

    pub fn count(&self, kind: &str) -> usize {
        self.of_kind(kind).len()
    }

    pub fn kind(&self, id: u64) -> &str {
        &self.entities[&id].0
    }

    /// The `#n` references in the parameters of `id`, in order.
    pub fn refs(&self, id: u64) -> Vec<u64> {
        let params = &self.entities[&id].1;
        params
            .split('#')
            .skip(1)
            .map(|s| {
                let digits: String = s.chars().take_while(char::is_ascii_digit).collect();
                digits.parse().expect("ref")
            })
            .collect()
    }

    /// The trailing `.T.` / `.F.` of `id`.
    pub fn last_flag(&self, id: u64) -> bool {
        let params = &self.entities[&id].1;
        if params.ends_with(".T.") {
            true
        } else if params.ends_with(".F.") {
            false
        } else {
            panic!("#{id} has no trailing flag: {params}")
        }
    }

    /// The quoted name of `id`.
    pub fn name(&self, id: u64) -> String {
        let params = &self.entities[&id].1;
        let rest = params.strip_prefix('\'').expect("name first");
        rest[..rest.find('\'').expect("closing quote")].to_owned()
    }

    pub fn point(&self, id: u64) -> [f64; 3] {
        assert_eq!(self.kind(id), "CARTESIAN_POINT");
        self.triple(id)
    }

    pub fn direction(&self, id: u64) -> [f64; 3] {
        assert_eq!(self.kind(id), "DIRECTION");
        self.triple(id)
    }

    /// The `(x,y,z)` list of a point or direction.
    fn triple(&self, id: u64) -> [f64; 3] {
        let params = &self.entities[&id].1;
        let open = params.rfind('(').expect("list");
        let close = open + params[open..].find(')').expect("list end");
        let v: Vec<f64> = params[open + 1..close]
            .split(',')
            .map(|s| s.trim().parse().expect("real"))
            .collect();
        [v[0], v[1], v[2]]
    }

    /// Per bound of face `face`: whether it is the outer bound, and its
    /// edge uses as (edge, runs along it), the bound's flag applied.
    pub fn bounds(&self, face: u64) -> Vec<(bool, Vec<(u64, bool)>)> {
        let refs = self.refs(face);
        refs[..refs.len() - 1]
            .iter()
            .map(|&bound| {
                let outer = self.kind(bound) == "FACE_OUTER_BOUND";
                let keep = self.last_flag(bound);
                let edge_loop = self.refs(bound)[0];
                let mut uses: Vec<(u64, bool)> = self
                    .refs(edge_loop)
                    .into_iter()
                    .map(|oriented| (self.refs(oriented)[0], self.last_flag(oriented)))
                    .collect();
                if !keep {
                    uses.reverse();
                    for u in &mut uses {
                        u.1 = !u.1;
                    }
                }
                (outer, uses)
            })
            .collect()
    }

    /// The start and end points of edge `edge`.
    pub fn edge_ends(&self, edge: u64) -> ([f64; 3], [f64; 3]) {
        let r = self.refs(edge);
        let at = |vertex: u64| self.point(self.refs(vertex)[0]);
        (at(r[0]), at(r[1]))
    }

    /// The outward normal of a face on a plane: the plane's axis, reversed
    /// when the face's sense is false.
    pub fn plane_normal(&self, face: u64) -> Option<[f64; 3]> {
        let refs = self.refs(face);
        let surface = *refs.last()?;
        if self.kind(surface) != "PLANE" {
            return None;
        }
        let axis = self.refs(surface)[0];
        let n = self.direction(self.refs(axis)[1]);
        Some(if self.last_flag(face) {
            n
        } else {
            n.map(|x| -x)
        })
    }
}

pub fn newell(ring: &[[f64; 3]]) -> [f64; 3] {
    let mut n = [0.0; 3];
    for (i, a) in ring.iter().enumerate() {
        let b = ring[(i + 1) % ring.len()];
        n[0] += (a[1] - b[1]) * (a[2] + b[2]);
        n[1] += (a[2] - b[2]) * (a[0] + b[0]);
        n[2] += (a[0] - b[0]) * (a[1] + b[1]);
    }
    n
}

pub fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
