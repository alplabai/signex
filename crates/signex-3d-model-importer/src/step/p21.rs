use std::collections::HashMap;

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

    let mut holes_skipped = 0usize;
    for bound_refs in advanced_faces.values() {
        // The face outline is its FACE_OUTER_BOUND, or, when none is marked
        // (legal STEP), the loop with the largest area. Every other bound is a
        // hole. Holes are not cut: that needs a real polygon-with-holes
        // triangulation, and the outline's fan still covers the opening. They
        // are only left out of the mesh as extra solid faces, and counted (#708).
        let mut loops: Vec<(Vec<[f32; 3]>, bool)> = bound_refs
            .iter()
            .filter_map(|r| face_bounds.get(r))
            .map(|&(loop_id, is_outer)| (loop_points_of(loop_id), is_outer))
            .collect();
        holes_skipped += loops.len().saturating_sub(1);
        let outline = match loops.iter().position(|l| l.1) {
            Some(i) => Some(i),
            None => (0..loops.len())
                .max_by(|&i, &j| loop_area(&loops[i].0).total_cmp(&loop_area(&loops[j].0))),
        };
        if let Some(i) = outline {
            let loop_points = loops.swap_remove(i).0;
            if loop_points.len() >= 3 {
                let mesh = triangulate_polygon(&loop_points);
                if !mesh.indices.is_empty() {
                    meshes.push(mesh);
                }
            }
        }
    }
    if holes_skipped > 0 {
        // Outside the shared 24-entry cap, which real files fill first.
        unsupported_entities.push(format!("FACE_BOUND (hole not cut) x{holes_skipped}"));
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

    fn best_parse_time(data: &str, expected: usize) -> std::time::Duration {
        (0..3)
            .map(|_| {
                let started = std::time::Instant::now();
                let entities = parse_entities(data).expect("parse");
                assert_eq!(entities.len(), expected);
                started.elapsed()
            })
            .min()
            .unwrap()
    }

    #[test]
    fn many_entities_parse_in_linear_time() {
        // The old per-statement newline recount was O(n^2). Compare a 20k and
        // an 80k run instead of an absolute budget: 4x the input costs ~4x
        // when linear and ~16x when quadratic, so a 10x ceiling separates
        // them with slack for scheduler noise (best of three per size).
        let small = best_parse_time(&synthetic_data(20_000), 20_000);
        let large = best_parse_time(&synthetic_data(80_000), 80_000);
        let floor = std::time::Duration::from_millis(1);
        let ratio = large.as_secs_f64() / small.max(floor).as_secs_f64();
        assert!(ratio < 10.0, "parse scaled {ratio:.1}x for 4x input");
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

    #[test]
    fn face_bound_hole_is_not_fan_filled() {
        let src = r#"
DATA;
#1 = CARTESIAN_POINT('',(0.,0.,0.));
#2 = CARTESIAN_POINT('',(4.,0.,0.));
#3 = CARTESIAN_POINT('',(0.,4.,0.));
#4 = CARTESIAN_POINT('',(1.,1.,0.));
#5 = CARTESIAN_POINT('',(2.,1.,0.));
#6 = CARTESIAN_POINT('',(1.,2.,0.));
#20 = POLY_LOOP('',(#1,#2,#3));
#21 = POLY_LOOP('',(#4,#5,#6));
#22 = FACE_BOUND('',#21,.T.);
#23 = FACE_OUTER_BOUND('',#20,.T.);
#30 = ADVANCED_FACE('',(#22,#23),#999,.T.);
ENDSEC;
"#;
        let result = parse_to_meshes(src).expect("parse");
        assert_eq!(result.meshes.len(), 1, "only the outer loop is filled");
        assert_eq!(result.meshes[0].positions[4], 0.0);
        assert_eq!(result.meshes[0].positions[3], 4.0);
        assert!(
            result
                .unsupported_entities
                .iter()
                .any(|e| e.contains("hole"))
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
        assert_eq!(
            result.meshes[0].positions[3], 4.0,
            "outer loop, not the hole"
        );
    }

    #[test]
    fn hole_warning_survives_a_full_unsupported_entity_list() {
        let noise: String = (100..140)
            .map(|i| format!("#{i} = FANCY_THING_{i}('');\n"))
            .collect();
        let src = format!(
            "DATA;\n{TRIANGLES}\n{noise}#22 = FACE_BOUND('',#21,.T.);\n\
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
