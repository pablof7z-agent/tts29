mod model;
mod projection;
mod runtime;

#[cfg(test)]
mod projection_tests;

use std::ffi::{c_char, c_void, CStr, CString};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use nmp::ObservationCancel;

use crate::model::{KernelConfiguration, QueueSnapshot};

type SnapshotCallback = extern "C" fn(*const c_char, *mut c_void);

#[derive(Clone, Copy)]
pub(crate) struct Emitter {
    callback: SnapshotCallback,
    context: usize,
}

impl Emitter {
    pub(crate) fn emit(&self, snapshot: &QueueSnapshot) {
        let Ok(json) = serde_json::to_string(snapshot) else {
            return;
        };
        let Ok(json) = CString::new(json) else {
            return;
        };
        (self.callback)(json.as_ptr(), self.context as *mut c_void);
    }
}

pub(crate) struct Control {
    stopping: AtomicBool,
    cancellation: Mutex<Option<ObservationCancel>>,
}

impl Control {
    fn new() -> Self {
        Self {
            stopping: AtomicBool::new(false),
            cancellation: Mutex::new(None),
        }
    }

    pub(crate) fn install(&self, cancellation: ObservationCancel) -> bool {
        if self.stopping.load(Ordering::Acquire) {
            cancellation.cancel();
            return false;
        }
        let mut stored = self
            .cancellation
            .lock()
            .unwrap_or_else(|value| value.into_inner());
        if self.stopping.load(Ordering::Acquire) {
            cancellation.cancel();
            return false;
        }
        *stored = Some(cancellation);
        true
    }

    fn stop(&self) {
        self.stopping.store(true, Ordering::Release);
        let stored = self
            .cancellation
            .lock()
            .unwrap_or_else(|value| value.into_inner());
        if let Some(cancellation) = stored.as_ref() {
            cancellation.cancel();
        }
    }
}

struct KernelHandle {
    control: Arc<Control>,
    thread: Option<JoinHandle<()>>,
}

#[no_mangle]
/// Starts one event-driven TTS29 kernel and returns its opaque owner handle.
///
/// # Safety
///
/// `configuration_json` must point to a valid NUL-terminated string for this
/// call. `context` must remain valid for callbacks until the returned handle is
/// passed exactly once to [`tts29_stop`].
pub unsafe extern "C" fn tts29_start(
    configuration_json: *const c_char,
    callback: Option<SnapshotCallback>,
    context: *mut c_void,
) -> *mut c_void {
    let Some(callback) = callback else {
        return std::ptr::null_mut();
    };
    if configuration_json.is_null() {
        return std::ptr::null_mut();
    }
    let configuration = match CStr::from_ptr(configuration_json).to_str() {
        Ok(value) => match serde_json::from_str::<KernelConfiguration>(value) {
            Ok(configuration) => configuration,
            Err(_) => return std::ptr::null_mut(),
        },
        Err(_) => return std::ptr::null_mut(),
    };

    let control = Arc::new(Control::new());
    let thread_control = Arc::clone(&control);
    let emitter = Emitter {
        callback,
        context: context as usize,
    };
    let thread = std::thread::Builder::new()
        .name("tts29-kernel".into())
        .spawn(move || runtime::run(configuration, emitter, thread_control));
    let Ok(thread) = thread else {
        return std::ptr::null_mut();
    };
    Box::into_raw(Box::new(KernelHandle {
        control,
        thread: Some(thread),
    })) as *mut c_void
}

#[no_mangle]
/// Cancels, joins, and releases a handle returned by [`tts29_start`].
///
/// # Safety
///
/// `handle` must be null or a live handle returned by [`tts29_start`], and a
/// non-null handle must be passed to this function no more than once.
pub unsafe extern "C" fn tts29_stop(handle: *mut c_void) {
    if handle.is_null() {
        return;
    }
    let mut handle = Box::from_raw(handle as *mut KernelHandle);
    handle.control.stop();
    if let Some(thread) = handle.thread.take() {
        let _ = thread.join();
    }
}
