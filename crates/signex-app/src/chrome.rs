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
use iced::window::{Direction, Id};

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

/// Begin an OS-level move on the given window. Uses iced's
/// `window::drag` (winit's `drag_window`) on all platforms.
///
/// Historical note: an earlier `WM_SYSCOMMAND SC_MOVE` Win32 detour
/// was added (`c682d7ca`) on the assumption that winit's
/// `PostMessageW(WM_NCLBUTTONDOWN)` silently no-ops on borderless
/// Windows. Verified empirically: iced's `window::drag` actually
/// works on `decorations: false` windows on the current pinned
/// winit version. Reverted (2026-05-02) — the SC_MOVE/SC_SIZE
/// path entered Windows' modal sizing loop, starving iced's
/// runtime and producing the resize-stretch regression bisected
/// to that commit (see `docs/internal/TEST_CHECKLIST_v0.10_v0.11.md`
/// F9).
pub fn start_window_drag<M: 'static + Send>(id: Id) -> Task<M> {
    iced::window::drag(id)
}

/// Begin an OS-level resize on the given window in the direction the
/// user grabbed (one of the eight cardinal / corner edges). Uses
/// iced's `window::drag_resize` (winit's `drag_resize_window`) on
/// all platforms. See `start_window_drag` for the rationale behind
/// dropping the Win32 SC_SIZE detour.
pub fn start_window_resize<M: 'static + Send>(id: Id, direction: Direction) -> Task<M> {
    iced::window::drag_resize(id, direction)
}

#[cfg(windows)]
mod windows_impl {
    use iced::window::raw_window_handle::RawWindowHandle;
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::Graphics::Dwm::{
        DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DwmSetWindowAttribute,
    };

    pub(super) fn set_rounded(w: &dyn iced::window::Window) {
        let Some(hwnd) = hwnd_for(w) else {
            return;
        };
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

    fn hwnd_for(w: &dyn iced::window::Window) -> Option<HWND> {
        let handle = w.window_handle().ok()?;
        let RawWindowHandle::Win32(win32) = handle.as_raw() else {
            return None;
        };
        Some(win32.hwnd.get() as HWND)
    }
}
