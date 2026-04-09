//! Safe Rust wrapper around libngspice shared library via `libloading`.
//!
//! Loads libngspice at runtime, resolves function symbols, and provides
//! a safe API with callback bridging to Rust channels.

use libloading::{Library, Symbol};
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex, Once};

// --- C types mirroring sharedspice.h ---

#[repr(C)]
pub struct NgComplex {
    pub cx_real: f64,
    pub cx_imag: f64,
}

#[repr(C)]
pub struct VectorInfo {
    pub v_name: *mut c_char,
    pub v_type: c_int,
    pub v_flags: i16,
    pub v_realdata: *mut f64,
    pub v_compdata: *mut NgComplex,
    pub v_length: c_int,
}

#[repr(C)]
pub struct VecValues {
    pub name: *mut c_char,
    pub creal: f64,
    pub cimag: f64,
    pub is_scale: bool,
    pub is_complex: bool,
}

#[repr(C)]
pub struct VecValuesAll {
    pub veccount: c_int,
    pub vecindex: c_int,
    pub vecsa: *mut *mut VecValues,
}

#[repr(C)]
pub struct VecInfo {
    pub number: c_int,
    pub vecname: *mut c_char,
    pub is_real: bool,
    pub pdvec: *mut c_void,
    pub pdvecscale: *mut c_void,
}

#[repr(C)]
pub struct VecInfoAll {
    pub name: *mut c_char,
    pub title: *mut c_char,
    pub date: *mut c_char,
    pub r#type: *mut c_char,
    pub veccount: c_int,
    pub vecs: *mut *mut VecInfo,
}

// --- Callback type aliases ---

type SendCharFn = extern "C" fn(*mut c_char, c_int, *mut c_void) -> c_int;
type SendStatFn = extern "C" fn(*mut c_char, c_int, *mut c_void) -> c_int;
type ControlledExitFn = extern "C" fn(c_int, bool, bool, c_int, *mut c_void) -> c_int;
type SendDataFn =
    extern "C" fn(*mut VecValuesAll, c_int, c_int, *mut c_void) -> c_int;
type SendInitDataFn = extern "C" fn(*mut VecInfoAll, c_int, *mut c_void) -> c_int;
type BGThreadRunningFn = extern "C" fn(bool, c_int, *mut c_void) -> c_int;

// --- ngSpice function signatures ---

type NgSpiceInitFn = unsafe extern "C" fn(
    SendCharFn,
    SendStatFn,
    ControlledExitFn,
    SendDataFn,
    SendInitDataFn,
    BGThreadRunningFn,
    *mut c_void,
) -> c_int;

type NgSpiceCommandFn = unsafe extern "C" fn(*mut c_char) -> c_int;
type NgGetVecInfoFn = unsafe extern "C" fn(*mut c_char) -> *mut VectorInfo;
type NgSpiceCurPlotFn = unsafe extern "C" fn() -> *mut c_char;
type NgSpiceAllPlotsFn = unsafe extern "C" fn() -> *mut *mut c_char;
type NgSpiceAllVecsFn = unsafe extern "C" fn(*mut c_char) -> *mut *mut c_char;
type NgSpiceRunningFn = unsafe extern "C" fn() -> bool;
type NgSpiceCircFn = unsafe extern "C" fn(*mut *mut c_char) -> c_int;
type NgSpiceResetFn = unsafe extern "C" fn() -> c_int;

// --- Callback messages ---

#[derive(Debug, Clone)]
pub enum NgspiceMessage {
    /// Text output from ngspice (stdout/stderr)
    Output(String),
    /// Status update (percent complete, message)
    Status(String),
    /// Simulation data point received
    DataReady,
    /// Background thread state changed
    ThreadState(bool),
    /// Controlled exit requested
    Exit(i32),
}

// --- Global callback channel ---
// ngspice callbacks are global (C function pointers), so we use a global sender.
static INIT: Once = Once::new();
static mut CALLBACK_TX: Option<Mutex<mpsc::Sender<NgspiceMessage>>> = None;

fn set_callback_sender(tx: mpsc::Sender<NgspiceMessage>) {
    unsafe {
        INIT.call_once(|| {
            CALLBACK_TX = Some(Mutex::new(tx.clone()));
        });
        // If already initialized, update the sender
        if let Some(ref mtx) = CALLBACK_TX {
            if let Ok(mut guard) = mtx.lock() {
                *guard = tx;
            }
        }
    }
}

fn send_callback(msg: NgspiceMessage) {
    unsafe {
        if let Some(ref mtx) = CALLBACK_TX {
            if let Ok(guard) = mtx.lock() {
                let _ = guard.send(msg);
            }
        }
    }
}

// --- C callback implementations ---

extern "C" fn cb_send_char(msg: *mut c_char, _id: c_int, _user: *mut c_void) -> c_int {
    if !msg.is_null() {
        let s = unsafe { CStr::from_ptr(msg) }
            .to_string_lossy()
            .into_owned();
        send_callback(NgspiceMessage::Output(s));
    }
    0
}

extern "C" fn cb_send_stat(msg: *mut c_char, _id: c_int, _user: *mut c_void) -> c_int {
    if !msg.is_null() {
        let s = unsafe { CStr::from_ptr(msg) }
            .to_string_lossy()
            .into_owned();
        send_callback(NgspiceMessage::Status(s));
    }
    0
}

extern "C" fn cb_controlled_exit(
    status: c_int,
    _immediate: bool,
    _quit: bool,
    _id: c_int,
    _user: *mut c_void,
) -> c_int {
    send_callback(NgspiceMessage::Exit(status));
    0
}

extern "C" fn cb_send_data(
    _data: *mut VecValuesAll,
    _count: c_int,
    _id: c_int,
    _user: *mut c_void,
) -> c_int {
    send_callback(NgspiceMessage::DataReady);
    0
}

extern "C" fn cb_send_init_data(
    _data: *mut VecInfoAll,
    _id: c_int,
    _user: *mut c_void,
) -> c_int {
    0
}

extern "C" fn cb_bg_thread_running(running: bool, _id: c_int, _user: *mut c_void) -> c_int {
    send_callback(NgspiceMessage::ThreadState(running));
    0
}

// --- Public API ---

/// Detect ngspice shared library location.
pub fn detect_ngspice() -> Option<PathBuf> {
    // Platform-specific library names
    #[cfg(target_os = "windows")]
    let names = &["ngspice.dll", "libngspice.dll"];
    #[cfg(target_os = "linux")]
    let names = &["libngspice.so", "libngspice.so.0"];
    #[cfg(target_os = "macos")]
    let names = &["libngspice.dylib", "libngspice.0.dylib"];

    // Try loading directly (searches PATH / LD_LIBRARY_PATH / DYLD_LIBRARY_PATH)
    for name in names {
        if unsafe { Library::new(name) }.is_ok() {
            return Some(PathBuf::from(name));
        }
    }

    // Check common install locations
    #[cfg(target_os = "windows")]
    let search_dirs = &[
        r"C:\Spice64\bin",
        r"C:\Program Files\ngspice\bin",
        r"C:\Program Files (x86)\ngspice\bin",
    ];
    #[cfg(target_os = "linux")]
    let search_dirs = &["/usr/lib", "/usr/local/lib", "/usr/lib/x86_64-linux-gnu"];
    #[cfg(target_os = "macos")]
    let search_dirs = &["/usr/local/lib", "/opt/homebrew/lib"];

    for dir in search_dirs {
        for name in names {
            let path = Path::new(dir).join(name);
            if path.exists() {
                return Some(path);
            }
        }
    }

    None
}

/// A loaded ngspice instance.
pub struct NgspiceInstance {
    _lib: Library,
    // Core functions
    fn_command: unsafe extern "C" fn(*mut c_char) -> c_int,
    fn_get_vec_info: unsafe extern "C" fn(*mut c_char) -> *mut VectorInfo,
    fn_cur_plot: unsafe extern "C" fn() -> *mut c_char,
    fn_all_plots: unsafe extern "C" fn() -> *mut *mut c_char,
    fn_all_vecs: unsafe extern "C" fn(*mut c_char) -> *mut *mut c_char,
    fn_running: unsafe extern "C" fn() -> bool,
    fn_circ: unsafe extern "C" fn(*mut *mut c_char) -> c_int,
    fn_reset: unsafe extern "C" fn() -> c_int,
    /// Receiver for callback messages
    pub rx: mpsc::Receiver<NgspiceMessage>,
}

impl NgspiceInstance {
    /// Load and initialize ngspice from the given library path.
    pub fn new(lib_path: &Path) -> Result<Self, String> {
        let lib = unsafe { Library::new(lib_path) }
            .map_err(|e| format!("Failed to load ngspice library: {}", e))?;

        // Resolve function symbols
        let fn_init: NgSpiceInitFn = unsafe {
            *lib.get::<NgSpiceInitFn>(b"ngSpice_Init")
                .map_err(|e| format!("ngSpice_Init not found: {}", e))?
        };
        let fn_command: NgSpiceCommandFn = unsafe {
            *lib.get::<NgSpiceCommandFn>(b"ngSpice_Command")
                .map_err(|e| format!("ngSpice_Command not found: {}", e))?
        };
        let fn_get_vec_info: NgGetVecInfoFn = unsafe {
            *lib.get::<NgGetVecInfoFn>(b"ngGet_Vec_Info")
                .map_err(|e| format!("ngGet_Vec_Info not found: {}", e))?
        };
        let fn_cur_plot: NgSpiceCurPlotFn = unsafe {
            *lib.get::<NgSpiceCurPlotFn>(b"ngSpice_CurPlot")
                .map_err(|e| format!("ngSpice_CurPlot not found: {}", e))?
        };
        let fn_all_plots: NgSpiceAllPlotsFn = unsafe {
            *lib.get::<NgSpiceAllPlotsFn>(b"ngSpice_AllPlots")
                .map_err(|e| format!("ngSpice_AllPlots not found: {}", e))?
        };
        let fn_all_vecs: NgSpiceAllVecsFn = unsafe {
            *lib.get::<NgSpiceAllVecsFn>(b"ngSpice_AllVecs")
                .map_err(|e| format!("ngSpice_AllVecs not found: {}", e))?
        };
        let fn_running: NgSpiceRunningFn = unsafe {
            *lib.get::<NgSpiceRunningFn>(b"ngSpice_running")
                .map_err(|e| format!("ngSpice_running not found: {}", e))?
        };
        let fn_circ: NgSpiceCircFn = unsafe {
            *lib.get::<NgSpiceCircFn>(b"ngSpice_Circ")
                .map_err(|e| format!("ngSpice_Circ not found: {}", e))?
        };
        let fn_reset: NgSpiceResetFn = unsafe {
            *lib.get::<NgSpiceResetFn>(b"ngSpice_Reset")
                .map_err(|e| format!("ngSpice_Reset not found: {}", e))?
        };

        // Set up callback channel
        let (tx, rx) = mpsc::channel();
        set_callback_sender(tx);

        // Initialize ngspice
        let ret = unsafe {
            fn_init(
                cb_send_char,
                cb_send_stat,
                cb_controlled_exit,
                cb_send_data,
                cb_send_init_data,
                cb_bg_thread_running,
                std::ptr::null_mut(),
            )
        };
        if ret != 0 {
            return Err(format!("ngSpice_Init failed with code {}", ret));
        }

        Ok(Self {
            _lib: lib,
            fn_command,
            fn_get_vec_info,
            fn_cur_plot,
            fn_all_plots,
            fn_all_vecs,
            fn_running,
            fn_circ,
            fn_reset,
            rx,
        })
    }

    /// Send a command to ngspice (e.g., "source file.cir", "run", "quit").
    pub fn command(&self, cmd: &str) -> Result<(), String> {
        let c_cmd = CString::new(cmd).map_err(|e| format!("Invalid command string: {}", e))?;
        let ret = unsafe { (self.fn_command)(c_cmd.into_raw()) };
        if ret != 0 {
            return Err(format!("ngSpice_Command('{}') failed with code {}", cmd, ret));
        }
        Ok(())
    }

    /// Send a circuit (array of lines) to ngspice.
    pub fn load_circuit(&self, lines: &[&str]) -> Result<(), String> {
        let c_lines: Vec<CString> = lines
            .iter()
            .map(|l| CString::new(*l).unwrap())
            .collect();
        let mut ptrs: Vec<*mut c_char> = c_lines
            .iter()
            .map(|cs| cs.as_ptr() as *mut c_char)
            .collect();
        ptrs.push(std::ptr::null_mut()); // NULL terminator

        let ret = unsafe { (self.fn_circ)(ptrs.as_mut_ptr()) };
        if ret != 0 {
            return Err(format!("ngSpice_Circ failed with code {}", ret));
        }
        Ok(())
    }

    /// Get vector data by name. Returns (real_data, optional_imag_data).
    pub fn get_vector(&self, name: &str) -> Result<(Vec<f64>, Option<Vec<f64>>), String> {
        let c_name = CString::new(name).map_err(|e| format!("Invalid vector name: {}", e))?;
        let info = unsafe { (self.fn_get_vec_info)(c_name.into_raw()) };

        if info.is_null() {
            return Err(format!("Vector '{}' not found", name));
        }

        let vi = unsafe { &*info };
        let len = vi.v_length as usize;

        if !vi.v_realdata.is_null() {
            let real = unsafe { std::slice::from_raw_parts(vi.v_realdata, len) }.to_vec();
            Ok((real, None))
        } else if !vi.v_compdata.is_null() {
            let comp = unsafe { std::slice::from_raw_parts(vi.v_compdata, len) };
            let real: Vec<f64> = comp.iter().map(|c| c.cx_real).collect();
            let imag: Vec<f64> = comp.iter().map(|c| c.cx_imag).collect();
            Ok((real, Some(imag)))
        } else {
            Err(format!("Vector '{}' has no data", name))
        }
    }

    /// Get the name of the current plot.
    pub fn current_plot(&self) -> Option<String> {
        let ptr = unsafe { (self.fn_cur_plot)() };
        if ptr.is_null() {
            return None;
        }
        Some(unsafe { CStr::from_ptr(ptr) }.to_string_lossy().into_owned())
    }

    /// Get all plot names.
    pub fn all_plots(&self) -> Vec<String> {
        let ptr = unsafe { (self.fn_all_plots)() };
        if ptr.is_null() {
            return vec![];
        }
        let mut result = Vec::new();
        let mut i = 0;
        loop {
            let p = unsafe { *ptr.add(i) };
            if p.is_null() {
                break;
            }
            result.push(unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned());
            i += 1;
        }
        result
    }

    /// Get all vector names in a plot.
    pub fn all_vectors(&self, plot_name: &str) -> Vec<String> {
        let c_name = CString::new(plot_name).unwrap();
        let ptr = unsafe { (self.fn_all_vecs)(c_name.into_raw()) };
        if ptr.is_null() {
            return vec![];
        }
        let mut result = Vec::new();
        let mut i = 0;
        loop {
            let p = unsafe { *ptr.add(i) };
            if p.is_null() {
                break;
            }
            result.push(unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned());
            i += 1;
        }
        result
    }

    /// Check if a simulation is currently running.
    pub fn is_running(&self) -> bool {
        unsafe { (self.fn_running)() }
    }

    /// Reset ngspice state.
    pub fn reset(&self) -> Result<(), String> {
        let ret = unsafe { (self.fn_reset)() };
        if ret != 0 {
            return Err(format!("ngSpice_Reset failed with code {}", ret));
        }
        Ok(())
    }
}

// NgspiceInstance is not Send/Sync by default due to raw pointers.
// ngspice is single-threaded internally — all calls must come from the init thread.
// We mark it Send so it can be held in a Mutex on the Tokio runtime,
// but all method calls must be serialized.
unsafe impl Send for NgspiceInstance {}
