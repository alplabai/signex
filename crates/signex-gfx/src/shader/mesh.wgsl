// Lit triangle meshes for 3D views (#2390).
//
// CLEAN ROOM DECLARATION
// This module was written without reference to GPL-licensed software.
// Sources: wgpu/WGSL public docs.

// Mirrors `MeshUniform` in camera3d.rs: 96 bytes.
struct Camera {
    view_proj: mat4x4<f32>,
    light_dir: vec4<f32>,
    highlight: vec4<f32>,
};

@group(0) @binding(0) var<uniform> camera: Camera;

struct VertexIn {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
};

struct VertexOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(input: VertexIn) -> VertexOut {
    var out: VertexOut;
    out.clip = camera.view_proj * vec4<f32>(input.position, 1.0);
    out.world = input.position;
    out.color = input.color;
    return out;
}

// Flat shading from the face's own slope, so meshes need no normals; both
// faces are lit alike, so winding never darkens a face.
@fragment
fn fs_main(input: VertexOut) -> @location(0) vec4<f32> {
    let edge = cross(dpdx(input.world), dpdy(input.world));
    let len = length(edge);
    let normal = select(vec3<f32>(0.0, 0.0, 1.0), edge / len, len > 1e-12);
    let diffuse = abs(dot(normal, camera.light_dir.xyz));
    let shade = 0.35 + 0.65 * diffuse;
    return vec4<f32>(input.color.rgb * shade, input.color.a);
}

// The selection wash, drawn over a part already in the depth buffer.
@fragment
fn fs_highlight(_input: VertexOut) -> @location(0) vec4<f32> {
    return camera.highlight;
}
