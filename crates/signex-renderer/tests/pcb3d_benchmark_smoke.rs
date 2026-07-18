//! Milestone C benchmark smoke gates for the PCB 3D runtime.
//!
//! These tests verify that the full pipeline — ingest → mesh staging →
//! opaque pass → projection pass — produces stable primitive counts for
//! two canonical fixture tiers (small and medium).
//!
//! Fixture tiers
//! ─────────────
//! Tier S  (small)  :  1 scene, 1 node, 1 mesh,  3 primitives
//! Tier M  (medium) :  2 scenes, 4 nodes, 3 meshes, 7 primitives across nodes

use signex_gfx::scene::Scene;
use signex_renderer::pcb3d::{
    GlbSource, ModelTransform, OpaquePassLayout, ProjectionPassConfig, RuntimeGlbIngestRequest,
    RuntimeMaterialPolicy, check_projection_alignment, emit_opaque_pass_preview,
    emit_projection_pass, ingest_runtime_glb,
};
use signex_renderer::theme::ResolvedTheme;

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

fn request(model_id: &str, json: &str) -> RuntimeGlbIngestRequest {
    RuntimeGlbIngestRequest {
        model_id: model_id.to_string(),
        glb_source: GlbSource::Bytes(make_glb_bytes(json)),
        transform: ModelTransform::default(),
        material_policy: RuntimeMaterialPolicy::PreserveEmbedded,
    }
}

fn make_glb_bytes(json: &str) -> Vec<u8> {
    let mut json_chunk = json.as_bytes().to_vec();
    while json_chunk.len() % 4 != 0 {
        json_chunk.push(b' ');
    }
    let total_len = 12 + 8 + json_chunk.len();
    let mut bytes = Vec::with_capacity(total_len);
    bytes.extend_from_slice(&0x46546C67_u32.to_le_bytes()); // magic
    bytes.extend_from_slice(&2_u32.to_le_bytes()); // version
    bytes.extend_from_slice(&(total_len as u32).to_le_bytes());
    bytes.extend_from_slice(&(json_chunk.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&0x4E4F534A_u32.to_le_bytes()); // JSON chunk type
    bytes.extend_from_slice(&json_chunk);
    bytes
}

// ---------------------------------------------------------------------------
// Tier S fixture  —  1 scene, 1 node, 1 mesh, 3 primitives
// ---------------------------------------------------------------------------

fn tier_s_json() -> &'static str {
    r#"{
      "asset": { "version": "2.0" },
      "scenes": [{ "nodes": [0] }],
      "nodes": [{ "mesh": 0 }],
      "meshes": [{
        "primitives": [
          { "attributes": {} },
          { "attributes": {} },
          { "attributes": {} }
        ]
      }]
    }"#
}

#[test]
fn benchmark_smoke_tier_s_ingest_metadata_counts() {
    let model =
        ingest_runtime_glb(request("tier-s", tier_s_json())).expect("tier-S ingest must succeed");

    assert_eq!(model.metadata.scene_count, 1);
    assert_eq!(model.metadata.node_count, 1);
    assert_eq!(model.metadata.mesh_count, 1);
    assert_eq!(model.metadata.mesh_primitive_count, 3);
    assert_eq!(model.metadata.opaque_instance_count, 3);
    assert_eq!(model.mesh_staging.opaque_primitives.len(), 3);
}

#[test]
fn benchmark_smoke_tier_s_opaque_pass_emits_expected_polygon_count() {
    let model =
        ingest_runtime_glb(request("tier-s", tier_s_json())).expect("tier-S ingest must succeed");
    let mut scene = Scene::default();
    let theme = ResolvedTheme::builtin_default();

    emit_opaque_pass_preview(&model, &theme, &mut scene, OpaquePassLayout::default());

    assert_eq!(scene.polygons.len(), 3);
    assert!(
        scene.overlay_polygons.is_empty(),
        "opaque pass must not touch overlay_polygons"
    );
}

#[test]
fn benchmark_smoke_tier_s_projection_pass_emits_expected_overlay_count() {
    let model =
        ingest_runtime_glb(request("tier-s", tier_s_json())).expect("tier-S ingest must succeed");
    let mut scene = Scene::default();
    let theme = ResolvedTheme::builtin_default();

    let config = ProjectionPassConfig::default();
    assert!(check_projection_alignment("tier-s", &config).is_ok());

    emit_projection_pass(&model, &theme, &mut scene, config)
        .expect("tier-S projection must succeed");

    assert_eq!(scene.overlay_polygons.len(), 3);
    assert!(
        scene.polygons.is_empty(),
        "projection pass must not touch base polygons"
    );
}

#[test]
fn benchmark_smoke_tier_s_full_pipeline_pass_separation_holds() {
    let model = ingest_runtime_glb(request("tier-s-full", tier_s_json())).expect("tier-S ingest");
    let mut scene = Scene::default();
    let theme = ResolvedTheme::builtin_default();

    emit_opaque_pass_preview(&model, &theme, &mut scene, OpaquePassLayout::default());
    emit_projection_pass(&model, &theme, &mut scene, ProjectionPassConfig::default())
        .expect("tier-S projection");

    assert_eq!(
        scene.polygons.len(),
        3,
        "opaque pass count must be stable after projection pass"
    );
    assert_eq!(scene.overlay_polygons.len(), 3, "projection pass count");
}

// ---------------------------------------------------------------------------
// Tier M fixture  —  2 scenes, 4 nodes, 3 meshes, 7 primitives across nodes
// ---------------------------------------------------------------------------
//
// Scene 0: nodes [0, 1]
//   node 0 → mesh 0 (2 primitives)
//   node 1 → mesh 1 (3 primitives)
// Scene 1: nodes [2, 3]
//   node 2 → mesh 2 (2 primitives)
//   node 3 → (no mesh)
// Total staged primitives: 2 + 3 + 2 = 7

fn tier_m_json() -> &'static str {
    r#"{
      "asset": { "version": "2.0" },
      "scenes": [
        { "nodes": [0, 1] },
        { "nodes": [2, 3] }
      ],
      "nodes": [
        { "mesh": 0 },
        { "mesh": 1 },
        { "mesh": 2 },
        {}
      ],
      "meshes": [
        { "primitives": [{ "attributes": {} }, { "attributes": {} }] },
        { "primitives": [{ "attributes": {} }, { "attributes": {} }, { "attributes": {} }] },
        { "primitives": [{ "attributes": {} }, { "attributes": {} }] }
      ]
    }"#
}

#[test]
fn benchmark_smoke_tier_m_ingest_metadata_counts() {
    let model =
        ingest_runtime_glb(request("tier-m", tier_m_json())).expect("tier-M ingest must succeed");

    assert_eq!(model.metadata.scene_count, 2);
    assert_eq!(model.metadata.node_count, 4);
    assert_eq!(model.metadata.mesh_count, 3);
    assert_eq!(model.metadata.mesh_primitive_count, 7);
    assert_eq!(model.metadata.opaque_instance_count, 7);
    assert_eq!(model.mesh_staging.opaque_primitives.len(), 7);
}

#[test]
fn benchmark_smoke_tier_m_opaque_pass_emits_expected_polygon_count() {
    let model =
        ingest_runtime_glb(request("tier-m", tier_m_json())).expect("tier-M ingest must succeed");
    let mut scene = Scene::default();
    let theme = ResolvedTheme::builtin_default();

    emit_opaque_pass_preview(&model, &theme, &mut scene, OpaquePassLayout::default());

    assert_eq!(scene.polygons.len(), 7);
}

#[test]
fn benchmark_smoke_tier_m_projection_pass_emits_expected_overlay_count() {
    let model =
        ingest_runtime_glb(request("tier-m", tier_m_json())).expect("tier-M ingest must succeed");
    let mut scene = Scene::default();
    let theme = ResolvedTheme::builtin_default();

    emit_projection_pass(&model, &theme, &mut scene, ProjectionPassConfig::default())
        .expect("tier-M projection must succeed");

    assert_eq!(scene.overlay_polygons.len(), 7);
}

#[test]
fn benchmark_smoke_tier_m_full_pipeline_pass_separation_holds() {
    let model = ingest_runtime_glb(request("tier-m-full", tier_m_json())).expect("tier-M ingest");
    let mut scene = Scene::default();
    let theme = ResolvedTheme::builtin_default();

    emit_opaque_pass_preview(&model, &theme, &mut scene, OpaquePassLayout::default());
    emit_projection_pass(&model, &theme, &mut scene, ProjectionPassConfig::default())
        .expect("tier-M projection");

    assert_eq!(
        scene.polygons.len(),
        7,
        "opaque pass count stable after projection pass"
    );
    assert_eq!(scene.overlay_polygons.len(), 7, "projection overlay count");
}
