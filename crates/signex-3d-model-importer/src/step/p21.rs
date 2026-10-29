use std::collections::{BTreeMap, HashMap};

use super::holes::{MAX_FILE_CUT_VERTICES, face_vertex_count, triangulate_with_holes};
use crate::vrml::parser::VrmlMesh;

#[derive(Debug)]
pub struct StepMeshResult {
    pub meshes: Vec<VrmlMesh>,
    pub unsupported_entities: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("missing DATA section")]
    DataSectionMissing,
    #[error("malformed entity at line {line}: {reason}")]
    MalformedEntity { line: usize, reason: String },
    #[error("malformed number at line {line}: {value}")]
    MalformedNumber { line: usize, value: String },
    #[error("no tessellatable geometry found")]
    EmptyGeometry,
}

#[derive(Clone, Debug)]
struct Entity {
    kind: String,
    params: String,
    line: usize,
}

/// Parse a STEP ISO-10303-21 DATA section into tessellated triangle meshes.
///
/// Supported paths:
/// - `ADVANCED_FACE` + `FACE_OUTER_BOUND` + `POLY_LOOP` + `VERTEX_POINT`
/// - `ADVANCED_FACE` + `FACE_BOUND` + `EDGE_LOOP` + `ORIENTED_EDGE` + `EDGE_CURVE`
pub fn parse_to_meshes(source: &str) -> Result<StepMeshResult, ParseError> {
    let data = extract_data_section(source).ok_or(ParseError::DataSectionMissing)?;
    let entities = parse_entities(data)?;

    let mut points: HashMap<u32, [f32; 3]> = HashMap::new();
    let mut vertex_to_point: HashMap<u32, u32> = HashMap::new();
    let mut poly_loops: HashMap<u32, Vec<u32>> = HashMap::new();
    let mut edge_loops: HashMap<u32, Vec<u32>> = HashMap::new();
    let mut oriented_edges: HashMap<u32, u32> = HashMap::new();
    let mut edge_curves: HashMap<u32, (u32, u32)> = HashMap::new();
    // EDGE_CURVE id -> id of the curve geometry it runs along.
    let mut edge_geometry: HashMap<u32, u32> = HashMap::new();
    // bound id -> (loop id, is FACE_OUTER_BOUND)
    let mut face_bounds: HashMap<u32, (u32, bool)> = HashMap::new();
    let mut advanced_faces: HashMap<u32, Vec<u32>> = HashMap::new();
    let mut unsupported_entities: Vec<String> = Vec::new();

    for (&id, entity) in &entities {
        match entity.kind.as_str() {
            "CARTESIAN_POINT" => {
                let xyz = parse_cartesian_point(entity)?;
                points.insert(id, xyz);
            }
            "VERTEX_POINT" => {
                if let Some(point_ref) = first_ref(&entity.params) {
                    vertex_to_point.insert(id, point_ref);
                }
            }
            "POLY_LOOP" => {
                poly_loops.insert(id, parse_refs(&entity.params));
            }
            "EDGE_LOOP" => {
                edge_loops.insert(id, parse_refs(&entity.params));
            }
            "ORIENTED_EDGE" => {
                if let Some(curve_id) = parse_refs(&entity.params).last().copied() {
                    oriented_edges.insert(id, curve_id);
                }
            }
            "EDGE_CURVE" => {
                let refs = parse_refs(&entity.params);
                if refs.len() >= 2 {
                    edge_curves.insert(id, (refs[0], refs[1]));
                }
                if let Some(&geometry) = refs.get(2) {
                    edge_geometry.insert(id, geometry);
                }
            }
            "FACE_OUTER_BOUND" | "FACE_BOUND" => {
                if let Some(loop_ref) = first_ref(&entity.params) {
                    face_bounds.insert(id, (loop_ref, entity.kind == "FACE_OUTER_BOUND"));
                }
            }
            "ADVANCED_FACE" => {
                advanced_faces.insert(id, parse_refs(&entity.params));
            }
            "CLOSED_SHELL" | "MANIFOLD_SOLID_BREP" | "PLANE" | "AXIS2_PLACEMENT_3D" => {
                // Structural entities currently ignored in Phase A.
            }
            other => {
                // Keep an abbreviated list for diagnostics/warnings.
                // One entry per entity type, so a repeated type cannot fill the cap.
                if unsupported_entities.len() < 24
                    && !unsupported_entities.iter().any(|e| e == other)
                {
                    unsupported_entities.push(other.to_owned());
                }
            }
        }
    }

    let mut meshes = Vec::<VrmlMesh>::new();

    let loop_points_of = |loop_id: u32| -> Vec<[f32; 3]> {
        if let Some(vertex_refs) = poly_loops.get(&loop_id) {
            resolve_poly_loop_points(vertex_refs, &vertex_to_point, &points)
        } else if let Some(edge_refs) = edge_loops.get(&loop_id) {
            resolve_edge_loop_points(
                edge_refs,
                &oriented_edges,
                &edge_curves,
                &vertex_to_point,
                &points,
            )
        } else {
            Vec::new()
        }
    };

    // A loop built from EDGE_LOOP keeps only each edge's end vertices, which
    // is exact for a LINE and wrong for anything curved: a circle written as
    // two half-circle edges collapses to the two-point loop `[a, b, a]`. A
    // loop is `curved` when any edge runs along something other than a LINE.
    let loop_is_curved = |loop_id: u32| -> bool {
        let Some(edge_refs) = edge_loops.get(&loop_id) else {
            return false;
        };
        edge_refs.iter().any(|oriented| {
            let is_line = oriented_edges
                .get(oriented)
                .and_then(|edge| edge_geometry.get(edge))
                .and_then(|geometry| entities.get(geometry))
                .is_some_and(|e| e.kind == "LINE");
            !is_line
        })
    };

    // Holes left in the outline fill, per reason.
    let mut holes_not_cut: BTreeMap<String, usize> = BTreeMap::new();
    // Vertices still allowed through the hole cut for this file.
    let mut cut_budget = MAX_FILE_CUT_VERTICES;
    for bound_refs in advanced_faces.values() {
        // A bound listed twice is one loop, not a second hole.
        let mut seen = std::collections::HashSet::new();
        let bound_refs: Vec<_> = bound_refs.iter().filter(|r| seen.insert(**r)).collect();
        // The face outline is its FACE_OUTER_BOUND, or, when none is marked
        // (legal STEP), the loop with the largest area. Every other bound is a
        // hole, cut out of the outline by `triangulate_with_holes`. A face
        // that cannot be cut keeps the outline fill and is counted (#708).
        let mut loops: Vec<(Vec<[f32; 3]>, bool, bool)> = bound_refs
            .iter()
            .filter_map(|r| face_bounds.get(*r))
            .map(|&(loop_id, is_outer)| {
                (loop_points_of(loop_id), is_outer, loop_is_curved(loop_id))
            })
            .collect();
        let outline = match loops.iter().position(|l| l.1) {
            Some(i) => Some(i),
            None => (0..loops.len())
                .max_by(|&i, &j| loop_area(&loops[i].0).total_cmp(&loop_area(&loops[j].0))),
        };
        let Some(i) = outline else { continue };
        let loop_points = loops.swap_remove(i).0;
        if loop_points.len() < 3 {
            continue;
        }
        // A curved hole is only known by its end vertices, so cutting it would
        // cut the wrong shape (or nothing). Keep the outline fill and say so.
        let curved_holes = loops.iter().filter(|l| l.2).count();
        let holes: Vec<Vec<[f32; 3]>> = loops.into_iter().map(|l| l.0).collect();
        let needed = face_vertex_count(&loop_points, &holes);
        let mesh = if holes.is_empty() {
            triangulate_polygon(&loop_points)
        } else if curved_holes > 0 {
            *holes_not_cut
                .entry("hole has curved edges".to_owned())
                .or_default() += holes.len();
            triangulate_polygon(&loop_points)
        } else if needed > cut_budget {
            *holes_not_cut
                .entry(format!(
                    "file has more than {MAX_FILE_CUT_VERTICES} vertices to cut"
                ))
                .or_default() += holes.len();
            triangulate_polygon(&loop_points)
        } else {
            cut_budget -= needed;
            match triangulate_with_holes(&loop_points, &holes) {
                Ok(mesh) => mesh,
                Err(reason) => {
                    *holes_not_cut.entry(reason).or_default() += holes.len();
                    triangulate_polygon(&loop_points)
                }
            }
        };
        if !mesh.indices.is_empty() {
            meshes.push(mesh);
        }
    }
    for (reason, count) in holes_not_cut {
        // Outside the shared 24-entry cap, which real files fill first.
        unsupported_entities.push(format!("FACE_BOUND (hole not cut: {reason}) x{count}"));
    }

    if meshes.is_empty() {
        return Err(ParseError::EmptyGeometry);
    }

    Ok(StepMeshResult {
        meshes,
        unsupported_entities,
    })
}

/// Area of a closed 3D loop (Newell), so the biggest loop of a face can be
/// told from its holes in any plane.
fn loop_area(points: &[[f32; 3]]) -> f32 {
    let mut n = [0f32; 3];
    for (i, a) in points.iter().enumerate() {
        let b = &points[(i + 1) % points.len()];
        n[0] += (a[1] - b[1]) * (a[2] + b[2]);
        n[1] += (a[2] - b[2]) * (a[0] + b[0]);
        n[2] += (a[0] - b[0]) * (a[1] + b[1]);
    }
    (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt() / 2.0
}

fn extract_data_section(source: &str) -> Option<&str> {
    let upper = source.to_ascii_uppercase();
    let data_start = upper.find("DATA;")?;
    let data_body_start = data_start + "DATA;".len();
    let end_rel = upper[data_body_start..].find("ENDSEC;")?;
    Some(&source[data_body_start..data_body_start + end_rel])
}

fn parse_entities(data: &str) -> Result<HashMap<u32, Entity>, ParseError> {
    let mut out = HashMap::new();

    for (statement, line) in split_statements(data) {
        let trimmed = statement.trim();
        if !trimmed.starts_with('#') {
            continue;
        }

        let entity = parse_entity_statement(trimmed, line)?;
        out.insert(entity.0, entity.1);
    }

    Ok(out)
}

/// Split a DATA section into `;`-terminated statements with their 1-based
/// starting line, in one pass.
///
/// A `;` inside a quoted string literal does not end a statement. STEP escapes
/// a quote by doubling it (`''`), which toggles the quote state twice and so
/// needs no special case. The newline count is carried forward rather than
/// recomputed from the start of `data` per statement (#112).
fn split_statements(data: &str) -> Vec<(&str, usize)> {
    let mut statements = Vec::new();
    let mut start = 0usize;
    let mut line = 1usize;
    let mut start_line = 1usize;
    let mut seen_content = false;
    let mut in_quote = false;

    let bytes = data.as_bytes();
    let mut skip_until = 0usize;

    for (i, &b) in bytes.iter().enumerate() {
        if i < skip_until {
            if b == b'\n' {
                line += 1;
            }
            continue;
        }
        // A /* ... */ comment may hold apostrophes or semicolons; skip it
        // whole so they cannot flip the quote state or end a statement. One
        // that opens a statement is left out of it.
        if b == b'/' && !in_quote && bytes.get(i + 1) == Some(&b'*') {
            skip_until = match data[i + 2..].find("*/") {
                Some(end) => i + 2 + end + 2,
                None => bytes.len(),
            };
            if !seen_content {
                start = skip_until;
            }
            continue;
        }
        // A statement's line is where its first non-blank byte sits.
        if !seen_content && !b.is_ascii_whitespace() {
            seen_content = true;
            start_line = line;
        }
        match b {
            b'\'' => in_quote = !in_quote,
            b'\n' => line += 1,
            b';' if !in_quote => {
                statements.push((&data[start..=i], start_line));
                start = i + 1;
                seen_content = false;
            }
            _ => {}
        }
    }
    if start < data.len() {
        statements.push((&data[start..], start_line));
    }
    statements
}

fn parse_entity_statement(statement: &str, line: usize) -> Result<(u32, Entity), ParseError> {
    let content = statement.trim_end_matches(';').trim();

    let (id_part, rhs) = content.split_once('=').ok_or(ParseError::MalformedEntity {
        line,
        reason: "missing '='".to_owned(),
    })?;

    let id = id_part
        .trim()
        .trim_start_matches('#')
        .parse::<u32>()
        .map_err(|_| ParseError::MalformedEntity {
            line,
            reason: "invalid entity id".to_owned(),
        })?;

    let rhs = rhs.trim();
    let open = rhs.find('(').ok_or(ParseError::MalformedEntity {
        line,
        reason: "missing '('".to_owned(),
    })?;
    let close = rhs.rfind(')').ok_or(ParseError::MalformedEntity {
        line,
        reason: "missing ')'".to_owned(),
    })?;

    if close <= open {
        return Err(ParseError::MalformedEntity {
            line,
            reason: "malformed parameter list".to_owned(),
        });
    }

    let kind = rhs[..open].trim().to_ascii_uppercase();
    let params = rhs[open + 1..close].trim().to_owned();

    Ok((id, Entity { kind, params, line }))
}

fn parse_cartesian_point(entity: &Entity) -> Result<[f32; 3], ParseError> {
    let nums = parse_numbers(&entity.params, entity.line)?;
    if nums.len() < 3 {
        return Err(ParseError::MalformedEntity {
            line: entity.line,
            reason: "CARTESIAN_POINT requires at least 3 coordinates".to_owned(),
        });
    }
    Ok([nums[0], nums[1], nums[2]])
}

/// Blank out every `'...'` string literal (`''` is an escaped quote and stays
/// inside the literal), so digits and `#` in a name are never read as data.
fn strip_strings(params: &str) -> String {
    let mut out = String::with_capacity(params.len());
    let mut in_quote = false;
    for ch in params.chars() {
        if ch == '\'' {
            in_quote = !in_quote;
            out.push(' ');
        } else if !in_quote {
            out.push(ch);
        }
    }
    out
}

fn parse_numbers(params: &str, line: usize) -> Result<Vec<f32>, ParseError> {
    let params = strip_strings(params);
    let params = params.as_str();
    let mut out = Vec::new();
    let mut token = String::new();

    let flush = |token: &mut String, out: &mut Vec<f32>| -> Result<(), ParseError> {
        if token.is_empty() {
            return Ok(());
        }
        if !token.chars().any(|ch| ch.is_ascii_digit()) {
            token.clear();
            return Ok(());
        }
        let normalized = token.replace('D', "E").replace('d', "e");
        let parsed = normalized
            .parse::<f32>()
            .map_err(|_| ParseError::MalformedNumber {
                line,
                value: token.clone(),
            })?;
        out.push(parsed);
        token.clear();
        Ok(())
    };

    for ch in params.chars() {
        if ch.is_ascii_digit() || matches!(ch, '+' | '-' | '.' | 'E' | 'e' | 'D' | 'd') {
            token.push(ch);
        } else {
            flush(&mut token, &mut out)?;
        }
    }

    flush(&mut token, &mut out)?;
    Ok(out)
}

fn parse_refs(params: &str) -> Vec<u32> {
    let params = strip_strings(params);
    let params = params.as_str();
    let bytes = params.as_bytes();
    let mut refs = Vec::<u32>::new();
    let mut i = 0usize;

    while i < bytes.len() {
        if bytes[i] == b'#' {
            i += 1;
            let start = i;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            if start < i
                && let Ok(id) = params[start..i].parse::<u32>()
            {
                refs.push(id);
            }
        } else {
            i += 1;
        }
    }

    refs
}

fn first_ref(params: &str) -> Option<u32> {
    parse_refs(params).into_iter().next()
}

fn resolve_poly_loop_points(
    vertex_refs: &[u32],
    vertex_to_point: &HashMap<u32, u32>,
    points: &HashMap<u32, [f32; 3]>,
) -> Vec<[f32; 3]> {
    let mut out = Vec::new();

    for &vertex_id in vertex_refs {
        let point_id = vertex_to_point
            .get(&vertex_id)
            .copied()
            .unwrap_or(vertex_id);
        if let Some(point) = points.get(&point_id).copied() {
            out.push(point);
        }
    }

    out
}

fn resolve_edge_loop_points(
    oriented_edge_refs: &[u32],
    oriented_edges: &HashMap<u32, u32>,
    edge_curves: &HashMap<u32, (u32, u32)>,
    vertex_to_point: &HashMap<u32, u32>,
    points: &HashMap<u32, [f32; 3]>,
) -> Vec<[f32; 3]> {
    let mut vertex_chain = Vec::<u32>::new();

    for oriented_edge_id in oriented_edge_refs {
        let Some(curve_id) = oriented_edges.get(oriented_edge_id).copied() else {
            continue;
        };
        let Some((start, end)) = edge_curves.get(&curve_id).copied() else {
            continue;
        };

        if vertex_chain.is_empty() {
            vertex_chain.push(start);
            vertex_chain.push(end);
            continue;
        }

        let last = *vertex_chain.last().unwrap_or(&start);
        if last == start {
            vertex_chain.push(end);
        } else if last == end {
            vertex_chain.push(start);
        } else {
            vertex_chain.push(start);
            vertex_chain.push(end);
        }
    }

    let mut out = Vec::new();
    for vertex_id in vertex_chain {
        let point_id = vertex_to_point
            .get(&vertex_id)
            .copied()
            .unwrap_or(vertex_id);
        if let Some(point) = points.get(&point_id).copied() {
            out.push(point);
        }
    }

    out
}

fn triangulate_polygon(points: &[[f32; 3]]) -> VrmlMesh {
    let mut positions = Vec::<f32>::new();
    let mut indices = Vec::<u32>::new();

    if points.len() < 3 {
        return VrmlMesh {
            positions,
            indices,
            color: [0.65, 0.65, 0.68, 1.0],
        };
    }

    for i in 1..points.len() - 1 {
        let tri = [points[0], points[i], points[i + 1]];
        for p in tri {
            let idx = (positions.len() / 3) as u32;
            positions.extend_from_slice(&p);
            indices.push(idx);
        }
    }

    VrmlMesh {
        positions,
        indices,
        color: [0.65, 0.65, 0.68, 1.0],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semicolon_inside_string_does_not_split_statement() {
        let data = "#1 = PRODUCT('a;b','it''s; ok');
#2 = CARTESIAN_POINT('',(0.,0.,0.));";
        let stmts = split_statements(data);
        assert_eq!(stmts.len(), 2);
        assert_eq!(stmts[0].0, "#1 = PRODUCT('a;b','it''s; ok');");
        assert_eq!(stmts[1].1, 2);
        let entities = parse_entities(data).expect("parse");
        assert_eq!(entities.len(), 2);
        assert_eq!(entities[&1].kind, "PRODUCT");
    }

    #[test]
    fn statement_lines_are_one_based_and_track_newlines() {
        let data = "

#1 = A();

#2 = B(;
";
        let err = parse_entities(data).unwrap_err();
        match err {
            ParseError::MalformedEntity { line, .. } => assert_eq!(line, 5),
            other => panic!("unexpected error {other:?}"),
        }
    }

    fn synthetic_data(count: u32) -> String {
        let mut data = String::new();
        for i in 1..=count {
            data.push_str(&format!(
                "#{i} = CARTESIAN_POINT('',(0.,0.,0.));
"
            ));
        }
        data
    }

    #[test]
    fn many_entities_parse_in_linear_time() {
        // Guards the #112 fix (the old per-statement newline recount was
        // O(n^2)) without trusting wall-clock ratios of runs of very
        // different length (#851). Both samples cover the same amount of
        // input: 16 back-to-back parses of SMALL statements against one parse
        // of 16 x SMALL. Linear code makes the two take about as long;
        // quadratic makes the single large parse ~16x slower. A 4x ceiling
        // separates them, and load lands on both samples alike.
        //
        // The large parse runs on its own thread with a deadline of 4x the
        // small sample, so a quadratic regression fails in seconds instead of
        // hanging the suite. Three attempts; one quiet attempt passes.
        use std::sync::mpsc;
        use std::time::{Duration, Instant};

        const SMALL: u32 = 1_000;
        const REPEATS: u32 = 16;
        const CEILING: u32 = 4;
        let small = synthetic_data(SMALL);
        let large = std::sync::Arc::new(synthetic_data(SMALL * REPEATS));

        let mut ratios = Vec::new();
        for _ in 0..3 {
            let started = Instant::now();
            for _ in 0..REPEATS {
                assert_eq!(parse_entities(&small).expect("parse").len(), SMALL as usize);
            }
            let small_total = started.elapsed().max(Duration::from_millis(1));

            let (tx, rx) = mpsc::channel();
            let data = std::sync::Arc::clone(&large);
            // Detached on purpose: past the deadline nothing waits for it.
            // It runs on beside the next attempt, which only slows the
            // already-failing regression case, and ends with the test binary.
            std::thread::spawn(move || {
                let started = Instant::now();
                let parsed = parse_entities(&data).expect("parse").len();
                let _ = tx.send((parsed, started.elapsed()));
            });
            match rx.recv_timeout(small_total * CEILING) {
                Ok((parsed, large_time)) => {
                    assert_eq!(parsed, (SMALL * REPEATS) as usize);
                    let ratio = large_time.as_secs_f64() / small_total.as_secs_f64();
                    if ratio < f64::from(CEILING) {
                        return;
                    }
                    ratios.push(format!("{ratio:.1}x"));
                }
                Err(_) => ratios.push(format!(">{CEILING}x (deadline)")),
            }
        }
        panic!(
            "one parse of 16x the input took over {CEILING}x as long as 16 small parses: {ratios:?}"
        );
    }

    /// Carried-forward line numbers stay exact across a large input. This is
    /// a correctness check, not a guard on #112's speed: the old quadratic
    /// recount produced exact lines too, just slowly.
    #[test]
    fn statement_lines_stay_exact_across_a_large_input() {
        // Kept small: a quadratic regression would make a larger input take
        // minutes here, and the speed guard above already catches that.
        const COUNT: usize = 5_000;
        let data = synthetic_data(COUNT as u32);
        // The trailing newline leaves a blank tail, skipped like `parse_entities` does.
        let statements: Vec<_> = split_statements(&data)
            .into_iter()
            .filter(|(s, _)| s.trim().starts_with('#'))
            .collect();
        assert_eq!(statements.len(), COUNT);
        for (i, (_, line)) in statements.iter().enumerate().step_by(499) {
            assert_eq!(*line, i + 1);
        }
        assert_eq!(statements.last().unwrap().1, COUNT);
    }

    #[test]
    fn apostrophe_in_comment_does_not_flip_quote_state() {
        let data = "#1 = CARTESIAN_POINT('',(0.,0.,0.));
/* it's a comment; with semicolon */
#2 = CARTESIAN_POINT('',(1.,0.,0.));
#3 = CARTESIAN_POINT('',(2.,0.,0.));
";
        let entities = parse_entities(data).expect("parse");
        assert_eq!(entities.len(), 3);
    }

    #[test]
    fn parse_minimal_poly_loop_face() {
        let src = r#"
ISO-10303-21;
HEADER;
ENDSEC;
DATA;
#1 = CARTESIAN_POINT('',(0.,0.,0.));
#2 = CARTESIAN_POINT('',(1.,0.,0.));
#3 = CARTESIAN_POINT('',(0.,1.,0.));
#10 = VERTEX_POINT('',#1);
#11 = VERTEX_POINT('',#2);
#12 = VERTEX_POINT('',#3);
#20 = POLY_LOOP('',(#10,#11,#12));
#21 = FACE_OUTER_BOUND('',#20,.T.);
#30 = ADVANCED_FACE('',(#21),#999,.T.);
ENDSEC;
END-ISO-10303-21;
"#;

        let result = parse_to_meshes(src).expect("parse");
        assert_eq!(result.meshes.len(), 1);
        assert_eq!(result.meshes[0].indices.len(), 3);
    }

    #[test]
    fn digits_and_hashes_in_names_are_not_data() {
        let src = r#"
DATA;
#1 = CARTESIAN_POINT('P7 #99',(1.,2.,3.));
#2 = CARTESIAN_POINT('D1',(4.,5.,6.));
#3 = CARTESIAN_POINT('it''s 9',(7.,8.,9.));
ENDSEC;
"#;
        let entities = parse_entities(extract_data_section(src).unwrap()).unwrap();
        assert_eq!(parse_cartesian_point(&entities[&1]).unwrap(), [1., 2., 3.]);
        assert_eq!(parse_cartesian_point(&entities[&2]).unwrap(), [4., 5., 6.]);
        assert_eq!(parse_cartesian_point(&entities[&3]).unwrap(), [7., 8., 9.]);
        assert_eq!(parse_refs("'a #5',(#7)"), vec![7]);
    }

    /// A single ADVANCED_FACE over `loops` (outer flag, point indices), with
    /// every point written as a CARTESIAN_POINT and every loop as a POLY_LOOP.
    fn face_src(points: &[[f64; 3]], loops: &[(bool, &[usize])]) -> String {
        let mut src = String::from("DATA;\n");
        for (i, p) in points.iter().enumerate() {
            src += &format!(
                "#{} = CARTESIAN_POINT('',({:?},{:?},{:?}));\n",
                i + 1,
                p[0],
                p[1],
                p[2]
            );
        }
        let mut bounds = Vec::new();
        for (k, (outer, idx)) in loops.iter().enumerate() {
            let refs: Vec<String> = idx.iter().map(|i| format!("#{}", i + 1)).collect();
            src += &format!("#{} = POLY_LOOP('',({}));\n", 100 + k, refs.join(","));
            let kind = if *outer {
                "FACE_OUTER_BOUND"
            } else {
                "FACE_BOUND"
            };
            src += &format!("#{} = {kind}('',#{},.T.);\n", 200 + k, 100 + k);
            bounds.push(format!("#{}", 200 + k));
        }
        src += &format!(
            "#300 = ADVANCED_FACE('',({}),#999,.T.);\nENDSEC;",
            bounds.join(",")
        );
        src
    }

    fn corner(m: &VrmlMesh, tri: usize, k: usize) -> [f64; 3] {
        let i = m.indices[tri * 3 + k] as usize * 3;
        [
            f64::from(m.positions[i]),
            f64::from(m.positions[i + 1]),
            f64::from(m.positions[i + 2]),
        ]
    }

    fn triangle_count(m: &VrmlMesh) -> usize {
        m.indices.len() / 3
    }

    /// 3D area of triangle `tri`.
    fn area(m: &VrmlMesh, tri: usize) -> f64 {
        let (a, b, c) = (corner(m, tri, 0), corner(m, tri, 1), corner(m, tri, 2));
        let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let n = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt() / 2.0
    }

    fn total_area(m: &VrmlMesh) -> f64 {
        (0..triangle_count(m)).map(|t| area(m, t)).sum()
    }

    fn centroid(m: &VrmlMesh, tri: usize) -> [f64; 3] {
        let c: Vec<[f64; 3]> = (0..3).map(|k| corner(m, tri, k)).collect();
        [0, 1, 2].map(|d| (c[0][d] + c[1][d] + c[2][d]) / 3.0)
    }

    /// Unit normal of triangle `tri` (winding-sensitive).
    fn normal(m: &VrmlMesh, tri: usize) -> [f64; 3] {
        let (a, b, c) = (corner(m, tri, 0), corner(m, tri, 1), corner(m, tri, 2));
        let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let n = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        [n[0] / l, n[1] / l, n[2] / l]
    }

    fn has_hole_warning(r: &StepMeshResult) -> bool {
        r.unsupported_entities.iter().any(|e| e.contains("hole"))
    }

    /// Rotate about the x axis, to put a face in a non-axis plane.
    fn tilt(p: [f64; 3], deg: f64) -> [f64; 3] {
        let (s, c) = deg.to_radians().sin_cos();
        [p[0], p[1] * c - p[2] * s, p[1] * s + p[2] * c]
    }

    /// Undo [`tilt`].
    fn untilt(p: [f64; 3], deg: f64) -> [f64; 3] {
        tilt(p, -deg)
    }

    const SQUARE: [[f64; 3]; 4] = [[0., 0., 0.], [10., 0., 0.], [10., 10., 0.], [0., 10., 0.]];
    const HOLE_A: [[f64; 3]; 4] = [[2., 2., 0.], [2., 4., 0.], [4., 4., 0.], [4., 2., 0.]];
    const HOLE_B: [[f64; 3]; 4] = [[6., 6., 0.], [6., 8., 0.], [8., 8., 0.], [8., 6., 0.]];

    fn in_square(c: [f64; 3], h: &[[f64; 3]; 4]) -> bool {
        c[0] > h[0][0] && c[0] < h[2][0] && c[1] > h[0][1] && c[1] < h[2][1]
    }

    #[test]
    fn face_bound_hole_is_cut_not_fan_filled() {
        let mut pts = SQUARE.to_vec();
        pts.extend(HOLE_A);
        let src = face_src(&pts, &[(false, &[4, 5, 6, 7]), (true, &[0, 1, 2, 3])]);
        let result = parse_to_meshes(&src).expect("parse");
        assert_eq!(result.meshes.len(), 1);
        let m = &result.meshes[0];
        assert!(
            (total_area(m) - (100.0 - 4.0)).abs() < 1e-4,
            "{}",
            total_area(m)
        );
        for t in 0..triangle_count(m) {
            assert!(
                !in_square(centroid(m, t), &HOLE_A),
                "triangle {t} covers the hole"
            );
            // Same facing as the outline loop (counter-clockwise about +z).
            assert!(normal(m, t)[2] > 0.99);
        }
        assert!(
            !has_hole_warning(&result),
            "{:?}",
            result.unsupported_entities
        );
    }

    #[test]
    fn hole_is_cut_in_a_tilted_plane() {
        let mut pts: Vec<[f64; 3]> = SQUARE.to_vec();
        pts.extend(HOLE_A);
        let pts: Vec<[f64; 3]> = pts.into_iter().map(|p| tilt(p, 30.0)).collect();
        let src = face_src(&pts, &[(true, &[0, 1, 2, 3]), (false, &[4, 5, 6, 7])]);
        let result = parse_to_meshes(&src).expect("parse");
        let m = &result.meshes[0];
        assert!((total_area(m) - 96.0).abs() < 1e-3, "{}", total_area(m));
        let want = tilt([0.0, 0.0, 1.0], 30.0);
        for t in 0..triangle_count(m) {
            assert!(!in_square(untilt(centroid(m, t), 30.0), &HOLE_A));
            let n = normal(m, t);
            let facing = n[0] * want[0] + n[1] * want[1] + n[2] * want[2];
            assert!(facing > 0.999, "winding flipped: {facing}");
        }
        assert!(!has_hole_warning(&result));
    }

    #[test]
    fn two_holes_in_one_face_are_both_cut() {
        let mut pts = SQUARE.to_vec();
        pts.extend(HOLE_A);
        pts.extend(HOLE_B);
        let src = face_src(
            &pts,
            &[
                (true, &[0, 1, 2, 3]),
                (false, &[4, 5, 6, 7]),
                (false, &[8, 9, 10, 11]),
            ],
        );
        let result = parse_to_meshes(&src).expect("parse");
        let m = &result.meshes[0];
        assert!(
            (total_area(m) - (100.0 - 8.0)).abs() < 1e-4,
            "{}",
            total_area(m)
        );
        for t in 0..triangle_count(m) {
            let c = centroid(m, t);
            assert!(!in_square(c, &HOLE_A) && !in_square(c, &HOLE_B));
        }
        assert!(!has_hole_warning(&result));
    }

    /// An ADVANCED_FACE whose outline is a 10 x 10 square of LINE edges and
    /// whose one hole is an EDGE_LOOP written by `hole_entities` (entities
    /// and the id of the EDGE_LOOP, `#50`). Everything goes through
    /// EDGE_LOOP / ORIENTED_EDGE / EDGE_CURVE, as real exporters write it.
    fn edge_face_src(hole_entities: &str) -> String {
        let mut src = String::from("DATA;\n");
        let corners = [(0, 0), (10, 0), (10, 10), (0, 10)];
        for (i, (x, y)) in corners.iter().enumerate() {
            let n = i + 1;
            src += &format!("#{n} = CARTESIAN_POINT('',({x}.,{y}.,0.));\n");
            src += &format!("#{} = VERTEX_POINT('',#{n});\n", 10 + n);
        }
        src += "#9 = LINE('',#1,#8);\n";
        for i in 0..4 {
            let (a, b) = (11 + i, 11 + (i + 1) % 4);
            src += &format!("#{} = EDGE_CURVE('',#{a},#{b},#9,.T.);\n", 20 + i);
            src += &format!("#{} = ORIENTED_EDGE('',*,*,#{},.T.);\n", 30 + i, 20 + i);
        }
        src += "#40 = EDGE_LOOP('',(#30,#31,#32,#33));\n";
        src += "#41 = FACE_OUTER_BOUND('',#40,.T.);\n";
        src += hole_entities;
        src += "#51 = FACE_BOUND('',#50,.T.);\n";
        src += "#300 = ADVANCED_FACE('',(#41,#51),#999,.T.);\nENDSEC;";
        src
    }

    #[test]
    fn edge_loop_square_hole_is_cut() {
        // Four LINE edges around (4,4)-(6,6).
        let mut hole = String::new();
        let pts = [(4, 4), (6, 4), (6, 6), (4, 6)];
        for (i, (x, y)) in pts.iter().enumerate() {
            hole += &format!("#{} = CARTESIAN_POINT('',({x}.,{y}.,0.));\n", 60 + i);
            hole += &format!("#{} = VERTEX_POINT('',#{});\n", 70 + i, 60 + i);
        }
        hole += "#79 = LINE('',#60,#8);\n";
        for i in 0..4 {
            let (a, b) = (70 + i, 70 + (i + 1) % 4);
            hole += &format!("#{} = EDGE_CURVE('',#{a},#{b},#79,.T.);\n", 80 + i);
            hole += &format!("#{} = ORIENTED_EDGE('',*,*,#{},.T.);\n", 90 + i, 80 + i);
        }
        hole += "#50 = EDGE_LOOP('',(#90,#91,#92,#93));\n";
        let result = parse_to_meshes(&edge_face_src(&hole)).expect("parse");
        let m = &result.meshes[0];
        assert!((total_area(m) - 96.0).abs() < 1e-4, "{}", total_area(m));
        assert!(
            !has_hole_warning(&result),
            "{:?}",
            result.unsupported_entities
        );
    }

    #[test]
    fn edge_loop_circle_hole_is_warned_not_silently_filled() {
        // A circle as two half-circle EDGE_CURVEs a->b and b->a over one
        // CIRCLE: the loop's end vertices are [a, b, a], zero area.
        let hole = "#60 = CARTESIAN_POINT('',(4.,5.,0.));\n\
             #61 = CARTESIAN_POINT('',(6.,5.,0.));\n\
             #62 = VERTEX_POINT('',#60);\n#63 = VERTEX_POINT('',#61);\n\
             #64 = CIRCLE('',#999,1.);\n\
             #65 = EDGE_CURVE('',#62,#63,#64,.T.);\n\
             #66 = EDGE_CURVE('',#63,#62,#64,.T.);\n\
             #67 = ORIENTED_EDGE('',*,*,#65,.T.);\n\
             #68 = ORIENTED_EDGE('',*,*,#66,.T.);\n\
             #50 = EDGE_LOOP('',(#67,#68));\n";
        let result = parse_to_meshes(&edge_face_src(hole)).expect("parse");
        // The outline fill stays, and the lost hole is reported.
        assert!((total_area(&result.meshes[0]) - 100.0).abs() < 1e-4);
        assert!(
            result
                .unsupported_entities
                .iter()
                .any(|e| e.contains("hole not cut: hole has curved edges")),
            "{:?}",
            result.unsupported_entities
        );
    }

    #[test]
    fn non_planar_hole_falls_back_to_the_outline_with_a_warning() {
        let mut pts = SQUARE.to_vec();
        pts.extend(HOLE_A);
        pts[5][2] = 3.0; // a hole corner far off the face plane
        let src = face_src(&pts, &[(true, &[0, 1, 2, 3]), (false, &[4, 5, 6, 7])]);
        let result = parse_to_meshes(&src).expect("parse");
        let m = &result.meshes[0];
        assert!((total_area(m) - 100.0).abs() < 1e-4, "outline fill kept");
        assert!(
            result
                .unsupported_entities
                .iter()
                .any(|e| e.contains("hole not cut: non-planar loop"))
        );
    }

    const TRIANGLES: &str = "#1 = CARTESIAN_POINT('',(0.,0.,0.));
#2 = CARTESIAN_POINT('',(4.,0.,0.));
#3 = CARTESIAN_POINT('',(0.,4.,0.));
#4 = CARTESIAN_POINT('',(1.,1.,0.));
#5 = CARTESIAN_POINT('',(2.,1.,0.));
#6 = CARTESIAN_POINT('',(1.,2.,0.));
#20 = POLY_LOOP('',(#1,#2,#3));
#21 = POLY_LOOP('',(#4,#5,#6));";

    #[test]
    fn all_face_bound_face_keeps_the_largest_loop_as_outline() {
        // No FACE_OUTER_BOUND, hole listed first.
        let src = format!(
            "DATA;\n{TRIANGLES}\n#22 = FACE_BOUND('',#21,.T.);\n\
             #23 = FACE_BOUND('',#20,.T.);\n#30 = ADVANCED_FACE('',(#22,#23),#999,.T.);\nENDSEC;"
        );
        let result = parse_to_meshes(&src).expect("parse");
        assert_eq!(result.meshes.len(), 1);
        // Outline 8 minus the 0.5 hole, not the hole's 0.5.
        assert!((total_area(&result.meshes[0]) - 7.5).abs() < 1e-4);
    }

    #[test]
    fn hole_warning_survives_a_full_unsupported_entity_list() {
        let noise: String = (100..140)
            .map(|i| format!("#{i} = FANCY_THING_{i}('');\n"))
            .collect();
        // Non-planar hole, so the warning is still emitted.
        let triangles = TRIANGLES.replace("(1.,1.,0.)", "(1.,1.,5.)");
        let src = format!(
            "DATA;\n{triangles}\n{noise}#22 = FACE_BOUND('',#21,.T.);\n\
             #23 = FACE_OUTER_BOUND('',#20,.T.);\n#30 = ADVANCED_FACE('',(#22,#23),#999,.T.);\nENDSEC;"
        );
        let result = parse_to_meshes(&src).expect("parse");
        assert!(
            result
                .unsupported_entities
                .iter()
                .any(|e| e.contains("hole"))
        );
    }

    #[test]
    fn parse_missing_data_section_fails() {
        let src = "ISO-10303-21;\nHEADER;\nENDSEC;\n";
        let err = parse_to_meshes(src).expect_err("must fail");
        assert!(matches!(err, ParseError::DataSectionMissing));
    }
}
