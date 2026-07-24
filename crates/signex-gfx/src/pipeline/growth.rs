//! Shared instance/vertex buffer growth for the GPU pipelines.
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: wgpu/WGSL public docs.

/// Grow `buffer` (and its element `capacity`) so it can hold `required`
/// elements of `elem_size` bytes, clamped so the allocation never exceeds the
/// device's `max_buffer_size`.
///
/// Capacity only ever grows and is rounded up to a power of two to amortise
/// reallocation. When the requested element count would push the buffer past
/// `max_buffer_size` — a pathological board, e.g. a fully poured plane fanned
/// into millions of triangles — the capacity is clamped to the largest count
/// that fits and the caller must upload only that many elements. A clamped,
/// partially drawn scene is a graceful degrade; letting `create_buffer` run
/// with an oversize descriptor trips wgpu validation and panics the render
/// thread.
///
/// Returns the number of elements the caller may safely write (`== required`
/// unless clamped), so the caller can truncate its upload slice and set its
/// draw count to the returned value. Logs once per process the first time a
/// clamp happens.
pub(crate) fn ensure_capacity(
    device: &wgpu::Device,
    buffer: &mut wgpu::Buffer,
    capacity: &mut usize,
    required: usize,
    elem_size: usize,
    label: &'static str,
    usage: wgpu::BufferUsages,
    max_buffer_size: u64,
) -> usize {
    let max_elems = (max_buffer_size / elem_size.max(1) as u64).max(1) as usize;
    let writable = required.min(max_elems);
    if writable < required {
        warn_clamp_once(label, required, writable);
    }

    if writable > *capacity {
        // Power-of-two headroom, but never past the element count the device
        // limit permits (a clamped capacity need not be a power of two).
        *capacity = writable.next_power_of_two().min(max_elems);
        *buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: *capacity as u64 * elem_size as u64,
            usage,
            mapped_at_creation: false,
        });
    }

    writable
}

fn warn_clamp_once(label: &'static str, required: usize, writable: usize) {
    static WARNED: std::sync::Once = std::sync::Once::new();
    WARNED.call_once(|| {
        log::warn!(
            "signex_gfx: {label} needs {required} elements but the device \
             max_buffer_size fits only {writable}; drawing a truncated scene"
        );
    });
}

#[cfg(test)]
mod tests {
    // The growth helper needs a real `wgpu::Device` to exercise buffer
    // recreation, so its allocation path is covered by the headless smoke
    // passes in `debug_pass`. These pure tests pin the clamp arithmetic — the
    // part that guards the render thread from an oversize `create_buffer`.

    /// Mirror of `ensure_capacity`'s pure clamp math, so the arithmetic that
    /// decides how many elements fit under `max_buffer_size` is testable
    /// without a GPU device.
    fn writable_and_capacity(
        required: usize,
        current: usize,
        elem_size: usize,
        max_buffer_size: u64,
    ) -> (usize, usize) {
        let max_elems = (max_buffer_size / elem_size.max(1) as u64).max(1) as usize;
        let writable = required.min(max_elems);
        let capacity = if writable > current {
            writable.next_power_of_two().min(max_elems)
        } else {
            current
        };
        (writable, capacity)
    }

    #[test]
    fn writes_everything_when_it_fits_and_grows_by_power_of_two() {
        // 10 elements of 24 bytes fit easily under 256 MiB.
        let (writable, capacity) = writable_and_capacity(10, 1, 24, 256 << 20);
        assert_eq!(writable, 10);
        assert_eq!(capacity, 16);
    }

    #[test]
    fn keeps_capacity_when_required_fits_current() {
        let (writable, capacity) = writable_and_capacity(5, 16, 24, 256 << 20);
        assert_eq!(writable, 5);
        assert_eq!(capacity, 16);
    }

    #[test]
    fn clamps_when_required_exceeds_the_device_limit() {
        // max_buffer_size = 240 bytes, 24 bytes/elem => at most 10 elements.
        let (writable, capacity) = writable_and_capacity(1_000, 1, 24, 240);
        assert_eq!(writable, 10);
        assert_eq!(capacity, 10);
    }

    #[test]
    fn clamp_never_yields_a_power_of_two_past_the_limit() {
        // 12 elements fit (12 * 20 = 240 <= 250) but next_power_of_two(12) = 16
        // would overflow the limit, so capacity is clamped back to 12.
        let (writable, capacity) = writable_and_capacity(12, 1, 20, 250);
        assert_eq!(writable, 12);
        assert_eq!(capacity, 12);
    }
}
