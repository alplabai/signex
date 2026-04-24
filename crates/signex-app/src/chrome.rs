//! OS-specific chrome polish for the borderless main window.
//!
//! The borderless main window (see `bootstrap.rs` — `decorations: false`)
//! opts out of every native adornment the OS would otherwise provide: no
//! title-bar, no system menu, and on Windows 11 no rounded corners or
//! drop-shadow either. This module re-adds the corner rounding in the
//! cleanest per-OS way:
//!
//! - **Windows 11** (build 22000+): `DwmSetWindowAttribute` with
//!   `DWMWA_WINDOW_CORNER_PREFERENCE = DWMWCP_ROUND`. DWM handles the
//!   mask, anti-aliasing, and the small system drop-shadow — matches how
//!   VS Code's borderless frame looks. Silently no-ops on Windows 10.
//! - **macOS / Linux**: not yet wired. macOS already rounds top-level
//!   windows on its own; Linux is WM-dependent. A transparent-window +
//!   rounded-container fallback can be layered on later without touching
//!   the Windows path.

use iced::Task;
use iced::window::Id;

/// Fire-and-forget task that applies OS-native rounded corners to the
/// given window id. Returns a `Task<M>` for any `M` so callers can
/// batch this into whatever message flow they already have.
pub fn apply_rounded_corners<M: 'static + Send>(id: Id) -> Task<M> {
    #[cfg(windows)]
    {
        iced::window::run(id, |w| windows_impl::set_rounded(w)).discard()
    }
    #[cfg(not(windows))]
    {
        let _ = id;
        Task::none()
    }
}

#[cfg(windows)]
mod windows_impl {
    use iced::window::raw_window_handle::RawWindowHandle;
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::Graphics::Dwm::{
        DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DwmSetWindowAttribute,
    };

    pub(super) fn set_rounded(w: &dyn iced::window::Window) {
        let Ok(handle) = w.window_handle() else {
            return;
        };
        let RawWindowHandle::Win32(win32) = handle.as_raw() else {
            return;
        };
        // NonZeroIsize -> HWND (*mut c_void). Safe cast on 32- and 64-bit
        // Windows where isize and pointer widths match.
        let hwnd: HWND = win32.hwnd.get() as HWND;
        let pref: u32 = DWMWCP_ROUND as u32;
        // Errors are ignored on purpose: Windows 10 returns E_INVALIDARG
        // because the attribute doesn't exist pre-22000, and we just want
        // to no-op in that case.
        unsafe {
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE as u32,
                &pref as *const u32 as *const _,
                std::mem::size_of::<u32>() as u32,
            );
        }
    }
}
