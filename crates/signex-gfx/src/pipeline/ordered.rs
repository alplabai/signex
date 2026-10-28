//! Draws a paint-ordered drawings bucket as per-kind runs (#1579).
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: IPC-2612-1, IEEE 315, IEC 60617, wgpu/WGSL public docs.
//!
//! Owns one buffer per primitive kind, filled in list order, and reuses the
//! existing line/circle/arc/polygon render pipelines: a run is an instance
//! range (`first..first + count`) or a vertex range into one of those buffers.
//! The shaders never read `instance_index`, so no shader changed.

use super::arc::ArcPipeline;
use super::circle::CirclePipeline;
use super::growth::{GrowthParams, ensure_capacity};
use super::line::LinePipeline;
use super::polygon::{PolygonPipeline, PolygonVertex};
use crate::primitive::arc::Arc;
use crate::primitive::circle::Circle;
use crate::primitive::line::LineSegment;
use crate::primitive::ordered::{DrawKind, DrawRun, OrderedBatches, OrderedItem};

/// The four pipelines a run can be drawn with.
pub struct KindPipelines<'a> {
    pub line: &'a LinePipeline,
    pub circle: &'a CirclePipeline,
    pub arc: &'a ArcPipeline,
    pub polygon: &'a PolygonPipeline,
}

/// Two triangles per instanced quad.
const QUAD_VERTICES: u32 = 6;

const BUFFER_USAGE: wgpu::BufferUsages =
    wgpu::BufferUsages::VERTEX.union(wgpu::BufferUsages::COPY_DST);

/// One kind's buffer: grown on demand, with the element count last written.
struct KindBuffer {
    buffer: wgpu::Buffer,
    capacity: usize,
    written: u32,
    label: &'static str,
    elem_size: usize,
}

impl KindBuffer {
    fn new<T>(device: &wgpu::Device, label: &'static str) -> Self {
        let elem_size = std::mem::size_of::<T>();
        Self {
            buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: elem_size as wgpu::BufferAddress,
                usage: BUFFER_USAGE,
                mapped_at_creation: false,
            }),
            capacity: 1,
            written: 0,
            label,
            elem_size,
        }
    }

    fn upload<T: bytemuck::Pod>(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        items: &[T],
    ) {
        if items.is_empty() {
            self.written = 0;
            return;
        }
        let writable = ensure_capacity(
            device,
            &mut self.buffer,
            &mut self.capacity,
            items.len(),
            &GrowthParams {
                elem_size: self.elem_size,
                label: self.label,
                usage: BUFFER_USAGE,
                max_buffer_size: device.limits().max_buffer_size,
            },
        );
        self.written = writable as u32;
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&items[..writable]));
    }

    /// `run`'s range clamped to what was written, or `None` when nothing of
    /// it survives (a device-limit clamp can truncate an upload).
    fn clamp(&self, run: &DrawRun) -> Option<std::ops::Range<u32>> {
        let end = run.first.saturating_add(run.count).min(self.written);
        (run.first < end).then_some(run.first..end)
    }
}

/// Per-kind buffers plus the run plan that interleaves them.
pub struct OrderedPipeline {
    lines: KindBuffer,
    circles: KindBuffer,
    arcs: KindBuffer,
    polygons: KindBuffer,
    runs: Vec<DrawRun>,
}

impl OrderedPipeline {
    pub fn new(device: &wgpu::Device) -> Self {
        Self {
            lines: KindBuffer::new::<LineSegment>(device, "signex_gfx_ordered_lines"),
            circles: KindBuffer::new::<Circle>(device, "signex_gfx_ordered_circles"),
            arcs: KindBuffer::new::<Arc>(device, "signex_gfx_ordered_arcs"),
            polygons: KindBuffer::new::<PolygonVertex>(device, "signex_gfx_ordered_polygons"),
            runs: Vec::new(),
        }
    }

    /// Replace the resident drawings with `items`, keeping their order.
    pub fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, items: &[OrderedItem]) {
        let batches = OrderedBatches::build(items);
        self.lines.upload(device, queue, &batches.lines);
        self.circles.upload(device, queue, &batches.circles);
        self.arcs.upload(device, queue, &batches.arcs);
        self.polygons
            .upload(device, queue, &batches.polygon_vertices);
        self.runs = batches.runs;
    }

    /// Draw every run, in order. Instanced kinds draw an instance range of the
    /// shared 6-vertex quad; polygons draw a vertex range.
    pub fn draw_runs(
        &self,
        render_pass: &mut wgpu::RenderPass<'_>,
        camera_bind_group: &wgpu::BindGroup,
        pipelines: &KindPipelines<'_>,
    ) {
        for run in &self.runs {
            let (buffer, pipeline) = match run.kind {
                DrawKind::Line => (&self.lines, pipelines.line.render_pipeline()),
                DrawKind::Circle => (&self.circles, pipelines.circle.render_pipeline()),
                DrawKind::Arc => (&self.arcs, pipelines.arc.render_pipeline()),
                DrawKind::Polygon => (&self.polygons, pipelines.polygon.render_pipeline()),
            };
            let Some(range) = buffer.clamp(run) else {
                continue;
            };
            render_pass.set_pipeline(pipeline);
            render_pass.set_bind_group(0, camera_bind_group, &[]);
            render_pass.set_vertex_buffer(0, buffer.buffer.slice(..));
            match run.kind {
                DrawKind::Polygon => render_pass.draw(range, 0..1),
                _ => render_pass.draw(0..QUAD_VERTICES, range),
            }
        }
    }
}
