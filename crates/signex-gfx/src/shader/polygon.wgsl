// CLEAN ROOM DECLARATION
// This shader was written without reference to GPL-licensed software.
// Sources: IPC-2612-1, IEEE 315, IEC 60617, wgpu/WGSL public docs.

struct Camera {
    view_proj: mat4x4<f32>,
    viewport: vec2<f32>,
    mm_per_px: f32,
    // Screen-space legibility floors, in pixels; 0.0 disables. Mirrors
    // `CameraUniform` — the two layouts must agree byte for byte.
    min_stroke_px: f32,
    min_radius_px: f32,
    // Three scalars, not a vec3: a `vec3<f32>` carries 16-byte alignment in
    // WGSL, which would push the struct to 112 bytes and mismatch the
    // 96-byte `CameraUniform`. wgpu rejects the bind group outright.
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
}

@group(0) @binding(0)
var<uniform> camera: Camera;

struct PolygonVertex {
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
};

struct VertexOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(input: PolygonVertex) -> VertexOut {
    var out: VertexOut;
    out.clip_pos = camera.view_proj * vec4<f32>(input.position, 0.0, 1.0);
    out.color = input.color;
    return out;
}

@fragment
fn fs_main(input: VertexOut) -> @location(0) vec4<f32> {
    return input.color;
}
