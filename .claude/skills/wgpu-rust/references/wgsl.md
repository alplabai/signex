# WGSL — WebGPU Shading Language

> wgpu supports WGSL (default), SPIR-V (feature: `spirv`), and GLSL (feature: `glsl`).
> WGSL is the primary language — always supported, cross-platform.

---

## Module structure

```wgsl
// 1. Uniform / storage declarations
// 2. Struct definitions  
// 3. Helper functions
// 4. Entry points (@vertex, @fragment, @compute)
```

---

## Scalar types

| WGSL | Description | Rust equivalent |
|------|-------------|----------------|
| `f32` | 32-bit float | `f32` |
| `f16` | 16-bit float (feature) | `half::f16` |
| `i32` | 32-bit signed int | `i32` |
| `u32` | 32-bit unsigned int | `u32` |
| `bool` | boolean (not in buffers) | `bool` |

---

## Vector types

```wgsl
let a: vec2<f32> = vec2(1.0, 2.0);
let b: vec3<f32> = vec3(1.0, 2.0, 3.0);
let c: vec4<f32> = vec4(0.0, 0.5, 1.0, 1.0);

// Component access:
let x: f32 = a.x;      // or a.r or a[0]
let xy: vec2<f32> = c.xy;
let zw: vec2<f32> = c.zw;
let bgr: vec3<f32> = c.bgr;   // swizzle

// Constructors:
let v3 = vec3(a, 0.5);        // vec2 + scalar -> vec3
let v4 = vec4(v3, 1.0);       // vec3 + scalar -> vec4

// Shorthand aliases:
// vec2f == vec2<f32>, vec3f, vec4f
// vec2u == vec2<u32>, vec2i == vec2<i32>
```

---

## Matrix types

```wgsl
let m: mat4x4<f32> = mat4x4<f32>(
    vec4(1.0, 0.0, 0.0, 0.0),  // column 0
    vec4(0.0, 1.0, 0.0, 0.0),  // column 1
    vec4(0.0, 0.0, 1.0, 0.0),  // column 2
    vec4(0.0, 0.0, 0.0, 1.0),  // column 3
);

// Multiply matrix by vector (column-major, right-multiply):
let transformed: vec4<f32> = m * vec4(pos, 1.0);

// Access column:
let col0: vec4<f32> = m[0];
// Access element [row][col]:
let elem: f32 = m[2][1];
```

**Memory layout** (matches Rust `[[f32;4];4]`): matrices are stored **column-major**.

---

## Array types

```wgsl
// Fixed-size array:
var arr: array<f32, 4> = array(1.0, 2.0, 3.0, 4.0);

// Runtime-sized array (only in storage buffers):
var<storage, read> data: array<f32>;

// Length of a runtime array:
let n: u32 = arrayLength(&data);
```

---

## Struct definition

```wgsl
struct Vertex {
    @location(0) position: vec3<f32>,
    @location(1) uv:       vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0)       uv:       vec2<f32>,
    @location(1)       color:    vec4<f32>,
}
```

---

## Address spaces

```wgsl
var<uniform>              u:   Uniforms;       // read-only, small (256 bytes typ.)
var<storage, read>        sr:  array<f32>;     // read-only storage buffer
var<storage, read_write>  srw: array<f32>;     // read-write storage buffer
var<workgroup>            sh:  array<f32, 64>; // shared within workgroup
var<private>              p:   f32;            // thread-private
var<function>             f:   f32;            // function-local (default for let/var)
```

---

## Vertex shader

```wgsl
struct CameraUniforms {
    view_proj: mat4x4<f32>,
}

struct InstanceData {
    @location(2) model_0: vec4<f32>,
    @location(3) model_1: vec4<f32>,
    @location(4) model_2: vec4<f32>,
    @location(5) model_3: vec4<f32>,
    @location(6) color:   vec4<f32>,
}

@group(0) @binding(0) var<uniform> camera: CameraUniforms;

@vertex
fn vs_main(
    @location(0) position: vec3<f32>,
    @location(1) uv:       vec2<f32>,
    inst: InstanceData,
) -> VertexOutput {
    let model = mat4x4<f32>(
        inst.model_0, inst.model_1, inst.model_2, inst.model_3,
    );

    var out: VertexOutput;
    out.clip_pos = camera.view_proj * model * vec4(position, 1.0);
    out.uv       = uv;
    out.color    = inst.color;
    return out;
}
```

---

## Fragment shader

```wgsl
@group(1) @binding(0) var t_diffuse: texture_2d<f32>;
@group(1) @binding(1) var s_diffuse: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(t_diffuse, s_diffuse, in.uv);
    return tex_color * in.color;
}
```

### Multiple render targets

```wgsl
struct FragOutput {
    @location(0) color:  vec4<f32>,
    @location(1) normal: vec4<f32>,
}

@fragment
fn fs_main(in: VertexOutput) -> FragOutput {
    var out: FragOutput;
    out.color  = vec4(1.0, 0.0, 0.0, 1.0);
    out.normal = vec4(in.world_normal, 0.0);
    return out;
}
```

---

## Compute shader

```wgsl
@group(0) @binding(0) var<storage, read>        input:  array<f32>;
@group(0) @binding(1) var<storage, read_write>   output: array<f32>;

@compute @workgroup_size(64, 1, 1)
fn cs_main(
    @builtin(global_invocation_id)   gid: vec3<u32>,
    @builtin(local_invocation_id)    lid: vec3<u32>,
    @builtin(workgroup_id)           wgid: vec3<u32>,
    @builtin(num_workgroups)         nwg:  vec3<u32>,
) {
    let idx = gid.x;
    if idx >= arrayLength(&input) { return; }
    output[idx] = input[idx] * 2.0;
}
```

---

## Built-in functions

### Math

```wgsl
abs(x)           // absolute value
clamp(x, lo, hi) // clamp to range
min(a, b)
max(a, b)
floor(x), ceil(x), round(x), fract(x)
sqrt(x), pow(x, e), exp(x), log(x)
sin(x), cos(x), tan(x), asin(x), acos(x), atan2(y, x)
sign(x)          // -1, 0, or 1
saturate(x)      // clamp(x, 0.0, 1.0)
```

### Vector math

```wgsl
dot(a, b)          // dot product
cross(a, b)        // cross product (vec3 only)
length(v)          // Euclidean length
normalize(v)       // unit vector
reflect(i, n)      // reflection
distance(a, b)     // Euclidean distance
mix(a, b, t)       // linear interpolation: a + t*(b-a)
smoothstep(lo, hi, x)  // smooth Hermite interpolation
step(edge, x)      // 0 if x < edge, else 1
```

### Texture sampling

```wgsl
textureSample(t, s, uv)              // standard 2D sample
textureSampleLevel(t, s, uv, lod)   // explicit mip level
textureDimensions(t)                 // returns texture size as vec2<u32>
textureLoad(t, coords, mip)          // load without sampler (integer coords)
textureStore(t, coords, value)       // write to storage texture
```

---

## Control flow

```wgsl
// if / else:
if x > 0.0 {
    // ...
} else if x < 0.0 {
    // ...
} else {
    // ...
}

// for loop:
for (var i: u32 = 0u; i < 10u; i++) {
    // ...
}

// while:
var i: u32 = 0u;
while i < 10u {
    i++;
}

// loop with break:
loop {
    if condition { break; }
    continuing { i++; }
}

// switch (on integer):
switch my_int {
    case 0u: { /* ... */ }
    case 1u, 2u: { /* ... */ }
    default: { /* ... */ }
}
```

---

## WGSL alignment rules (std140 / std430)

WGSL storage uses explicit alignment. Mismatched Rust structs cause wrong data.

| Type | Align | Size |
|------|-------|------|
| `f32` / `u32` / `i32` | 4 | 4 |
| `vec2<f32>` | 8 | 8 |
| `vec3<f32>` | 16 | 12 |
| `vec4<f32>` | 16 | 16 |
| `mat4x4<f32>` | 16 | 64 |
| `array<f32, N>` | 16 | 16*N (each element padded to 16!) |

**Uniform buffers** use std140: `array` elements are padded to 16 bytes each.
**Storage buffers** use std430: `array<f32>` elements are packed at 4 bytes each.

```rust
// Correct Rust uniform struct for this WGSL:
// struct Uniforms { time: f32, _pad: [f32; 3], view_proj: mat4x4 }
#[repr(C)]
#[derive(Pod, Zeroable)]
struct Uniforms {
    time:     f32,
    _pad:     [f32; 3],          // pad to 16 bytes before mat4
    view_proj: [[f32; 4]; 4],
}
```

---

## Push constants in WGSL

```wgsl
struct PushConstants {
    model: mat4x4<f32>,
    tint:  vec4<f32>,
}

var<push_constant> pc: PushConstants;
```
