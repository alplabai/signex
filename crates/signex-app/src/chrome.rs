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

/// Begin an OS-level move on the given window. On Windows this posts
/// `WM_SYSCOMMAND SC_MOVE | HTCAPTION` to the window directly, which
/// works on borderless windows even when the original mouse-down has
/// already been released. On other platforms it falls back to iced's
/// `window::drag` (winit `drag_window`).
///
/// Why not just `iced::window::drag`? winit's Windows backend defers
/// the `PostMessageW(WM_NCLBUTTONDOWN)` via a thread executor, so by
/// the time the message reaches the OS the user's button-down has
/// already been released. The OS silently no-ops, then winit's own
/// `dragging` flag stays stuck at `true` and every subsequent attempt
/// short-circuits in `handle_os_dragging`. `WM_SYSCOMMAND` triggers
/// the system-level move loop and doesn't depend on a current
/// button-down state.
pub fn start_window_drag<M: 'static + Send>(id: Id) -> Task<M> {
    #[cfg(windows)]
    {
        iced::window::run(id, |w| windows_impl::start_drag(w)).discard()
    }
    #[cfg(not(windows))]
    {
        iced::window::drag(id)
    }
}

/// Begin an OS-level resize on the given window in the direction the
/// user grabbed (one of the eight cardinal / corner edges). Same
/// rationale as `start_window_drag` — the Win32 path bypasses winit's
/// stuck-flag bug on borderless windows.
pub fn start_window_resize<M: 'static + Send>(id: Id, direction: Direction) -> Task<M> {
    #[cfg(windows)]
    {
        let edge = windows_impl::resize_edge_for(direction);
        iced::window::run(id, move |w| windows_impl::start_resize(w, edge)).discard()
    }
    #[cfg(not(windows))]
    {
        iced::window::drag_resize(id, direction)
    }
}

#[cfg(windows)]
mod windows_impl {
    use iced::window::Direction;
    use iced::window::raw_window_handle::RawWindowHandle;
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::Graphics::Dwm::{
        DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DwmSetWindowAttribute,
    };
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SC_MOVE, SC_SIZE, SendMessageW, WM_SYSCOMMAND,
    };

    // wparam values for SC_SIZE — see WMSZ_* in MSDN. winit's
    // `winuser.h` definitions are integer constants 1..=8 OR'd into
    // SC_SIZE to tell the OS which edge the user grabbed.
    const WMSZ_LEFT: usize = 1;
    const WMSZ_RIGHT: usize = 2;
    const WMSZ_TOP: usize = 3;
    const WMSZ_TOPLEFT: usize = 4;
    const WMSZ_TOPRIGHT: usize = 5;
    const WMSZ_BOTTOM: usize = 6;
    const WMSZ_BOTTOMLEFT: usize = 7;
    const WMSZ_BOTTOMRIGHT: usize = 8;

    /// Map iced's `Direction` to the Win32 `WMSZ_*` edge code.
    pub(super) fn resize_edge_for(direction: Direction) -> usize {
        match direction {
            Direction::West => WMSZ_LEFT,
            Direction::East => WMSZ_RIGHT,
            Direction::North => WMSZ_TOP,
            Direction::South => WMSZ_BOTTOM,
            Direction::NorthWest => WMSZ_TOPLEFT,
            Direction::NorthEast => WMSZ_TOPRIGHT,
            Direction::SouthWest => WMSZ_BOTTOMLEFT,
            Direction::SouthEast => WMSZ_BOTTOMRIGHT,
        }
    }

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

    pub(super) fn start_drag(w: &dyn iced::window::Window) {
        let Some(hwnd) = hwnd_for(w) else {
            return;
        };
        // ReleaseCapture mirrors what winit's own `handle_os_dragging`
        // does and is required so the OS can begin its modal move
        // loop. SendMessageW (sync) is preferred over PostMessageW
        // (async) here because we want the OS to enter SC_MOVE while
        // we're still on the main thread — the deferred PostMessageW
        // is exactly what was failing in winit's path.
        unsafe {
            let _ = ReleaseCapture();
            // SC_MOVE | HTCAPTION = 0xF012. Tells the OS "begin the
            // system move command, treating this as if the user
            // pressed the title bar". Works on borderless windows.
            const HTCAPTION: usize = 2;
            let _ = SendMessageW(hwnd, WM_SYSCOMMAND, SC_MOVE as usize | HTCAPTION, 0);
        }
    }

    pub(super) fn start_resize(w: &dyn iced::window::Window, edge: usize) {
        let Some(hwnd) = hwnd_for(w) else {
            return;
        };
        unsafe {
            let _ = ReleaseCapture();
            // SC_SIZE | WMSZ_<edge>. The OS enters its modal sizing
            // loop and waits for the next mouse motion to begin
            // tracking the resize.
            let _ = SendMessageW(hwnd, WM_SYSCOMMAND, SC_SIZE as usize | edge, 0);
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
