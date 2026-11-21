//! Picks the GPU backend before iced starts.
//!
//! On Windows the default backend choice can land on Vulkan, whose swapchain
//! stalls the UI thread for about two seconds on every maximize and restore on
//! some integrated GPUs. DX12 resizes without that stall, so it is preferred
//! when a hardware DX12 adapter exists. `WGPU_BACKEND` set by the user wins.

/// The variable iced reads for its backend.
pub const BACKEND_ENV: &str = "WGPU_BACKEND";

/// The backend to request, if any: DX12 when the user chose none and the
/// machine has a hardware DX12 adapter.
pub fn preferred_backend(user_chose: bool, has_dx12_hardware: bool) -> Option<&'static str> {
    (!user_chose && has_dx12_hardware).then_some("dx12")
}

/// Call in `main` before iced starts, so iced reads the choice.
#[cfg(windows)]
pub fn prefer_dx12_on_windows() {
    let user_chose = std::env::var_os(BACKEND_ENV).is_some();
    if user_chose {
        return;
    }
    if let Some(backend) = preferred_backend(user_chose, has_dx12_hardware_adapter()) {
        // SAFETY: this function only exists on Windows, where the standard
        // library documents `set_var` as always sound, multi-threaded
        // included. That matters here: the value depends on the adapter
        // probe above, so it cannot be set before wgpu has run, and the probe
        // may have left driver threads behind.
        unsafe { std::env::set_var(BACKEND_ENV, backend) };
    }
}

/// Other platforms keep the default backend choice.
#[cfg(not(windows))]
pub fn prefer_dx12_on_windows() {}

#[cfg(windows)]
fn has_dx12_hardware_adapter() -> bool {
    use signex_gfx::wgpu;

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::DX12,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    iced::futures::executor::block_on(instance.enumerate_adapters(wgpu::Backends::DX12))
        .iter()
        .any(|adapter| adapter.get_info().device_type != wgpu::DeviceType::Cpu)
}

#[cfg(test)]
mod tests {
    use super::preferred_backend;

    #[test]
    fn dx12_is_preferred_only_with_hardware_and_no_user_choice() {
        assert_eq!(preferred_backend(false, true), Some("dx12"));
        assert_eq!(preferred_backend(false, false), None);
        assert_eq!(preferred_backend(true, true), None);
        assert_eq!(preferred_backend(true, false), None);
    }
}
