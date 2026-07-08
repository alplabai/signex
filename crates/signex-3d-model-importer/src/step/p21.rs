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
    let mut face_bounds: HashMap<u32, u32> = HashMap::new();
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
                    face_bounds.insert(id, loop_ref);
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
                if unsupported_entities.len() < 24 {
                    unsupported_entities.push(other.to_owned());
                }
            }
        }
    }

    let mut meshes = Vec::<VrmlMesh>::new();

    for bound_refs in advanced_faces.values() {
        for bound_ref in bound_refs {
            let Some(loop_id) = face_bounds.get(bound_ref).copied() else {
                continue;
            };

            let loop_points = if let Some(vertex_refs) = poly_loops.get(&loop_id) {
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
            };

            if loop_points.len() >= 3 {
                let mesh = triangulate_polygon(&loop_points);
                if !mesh.indices.is_empty() {
                    meshes.push(mesh);
                }
            }
        }
    }

    if meshes.is_empty() {
        return Err(ParseError::EmptyGeometry);
    }

    Ok(StepMeshResult {
        meshes,
        unsupported_entities,
    })
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
    let mut cursor = 0usize;

    for statement in data.split_inclusive(';') {
        let trimmed = statement.trim();
        let line = data[..cursor].bytes().filter(|b| *b == b'\n').count() + 1;
        cursor += statement.len();

        if !trimmed.starts_with('#') {
            continue;
        }

        let entity = parse_entity_statement(trimmed, line)?;
        out.insert(entity.0, entity.1);
    }

    Ok(out)
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

fn parse_numbers(params: &str, line: usize) -> Result<Vec<f32>, ParseError> {
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
            if start < i {
                if let Ok(id) = params[start..i].parse::<u32>() {
                    refs.push(id);
                }
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
    fn parse_missing_data_section_fails() {
        let src = "ISO-10303-21;\nHEADER;\nENDSEC;\n";
        let err = parse_to_meshes(src).expect_err("must fail");
        assert!(matches!(err, ParseError::DataSectionMissing));
    }
}
