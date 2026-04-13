# Compute Pipeline

---

## Compute pipeline creation

```rust
let cs_module = device.create_shader_module(wgpu::include_wgsl!("compute.wgsl"));

let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
    label:       Some("My Compute Pipeline"),
    layout:      Some(&pipeline_layout),
    module:      &cs_module,
    entry_point: Some("cs_main"),
    compilation_options: wgpu::PipelineCompilationOptions::default(),
    cache: None,
});
```

---

## Dispatch

```rust
let mut encoder = device.create_command_encoder(&Default::default());

{
    let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
        label:            Some("Compute Pass"),
        timestamp_writes: None,
    });

    cpass.set_pipeline(&compute_pipeline);
    cpass.set_bind_group(0, &data_bg, &[]);

    // Dispatch (work_groups_x, work_groups_y, work_groups_z):
    // Total threads = work_groups * workgroup_size (defined in WGSL @compute)
    let num_elements: u32 = data.len() as u32;
    let workgroup_size: u32 = 64;
    let num_groups = (num_elements + workgroup_size - 1) / workgroup_size;
    cpass.dispatch_workgroups(num_groups, 1, 1);
}

queue.submit(std::iter::once(encoder.finish()));
```

---

## Storage buffer for compute

```rust
// Input buffer (read-only in shader):
let input_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
    label:    Some("Compute Input"),
    contents: bytemuck::cast_slice(&input_data),
    usage:    wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
});

// Output buffer (read-write in shader):
let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
    label:              Some("Compute Output"),
    size:               output_size_bytes,
    usage:              wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
    mapped_at_creation: false,
});

// Bind group:
let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
    layout:  &bgl,
    entries: &[
        wgpu::BindGroupEntry { binding: 0, resource: input_buffer.as_entire_binding() },
        wgpu::BindGroupEntry { binding: 1, resource: output_buffer.as_entire_binding() },
    ],
    label:   Some("Compute BG"),
});
```

---

## Minimal compute WGSL

```wgsl
// compute.wgsl
struct Data {
    values: array<f32>,
}

@group(0) @binding(0) var<storage, read>       input:  Data;
@group(0) @binding(1) var<storage, read_write>  output: Data;

@compute @workgroup_size(64, 1, 1)
fn cs_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    // Guard against out-of-bounds (last workgroup may be partially filled):
    if idx >= arrayLength(&input.values) { return; }
    output.values[idx] = input.values[idx] * 2.0;
}
```

---

## Reading results back to CPU

```rust
// Readback buffer (MAP_READ, must be a separate buffer):
let readback = device.create_buffer(&wgpu::BufferDescriptor {
    label:              Some("Readback"),
    size:               output_size_bytes,
    usage:              wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
    mapped_at_creation: false,
});

// Copy output -> readback:
let mut encoder = device.create_command_encoder(&Default::default());
encoder.copy_buffer_to_buffer(&output_buffer, 0, &readback, 0, output_size_bytes);
queue.submit(std::iter::once(encoder.finish()));

// Map and read:
let slice = readback.slice(..);
let (tx, rx) = std::sync::mpsc::channel();
slice.map_async(wgpu::MapMode::Read, move |r| { tx.send(r).unwrap(); });
device.poll(wgpu::Maintain::Wait);
rx.recv().unwrap().unwrap();

let raw = slice.get_mapped_range();
let results: &[f32] = bytemuck::cast_slice(&raw);
println!("first result: {}", results[0]);
drop(raw);
readback.unmap();
```

---

## Workgroup shared memory

Shared memory is fast L1 cache local to the workgroup — use for inter-thread communication:

```wgsl
var<workgroup> shared_data: array<f32, 64>;

@compute @workgroup_size(64)
fn cs_main(
    @builtin(global_invocation_id)  gid: vec3<u32>,
    @builtin(local_invocation_index) lid: u32,
) {
    shared_data[lid] = input.values[gid.x];
    workgroupBarrier();  // sync all threads in workgroup before reading

    // Now any thread can read any index of shared_data safely.
    let neighbour = shared_data[(lid + 1u) % 64u];
    output.values[gid.x] = neighbour;
}
```

---

## Compute for PCB rendering (DRC / connectivity)

```
Use case: run DRC net checks on 100K+ pads without GPU-CPU roundtrips.

1. Upload pad positions and net IDs to a storage buffer.
2. Dispatch a compute shader that builds a spatial hash or BVH.
3. A second dispatch checks clearance violations.
4. Copy the error list back to CPU for display.

This avoids stalling the GPU->CPU pipeline by batching all validation in one submission.
```
