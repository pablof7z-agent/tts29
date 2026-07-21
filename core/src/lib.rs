mod action;
mod clock;
mod identity;
mod model;
mod projection;
mod runtime;
mod session;

#[cfg(test)]
mod projection_tests;
#[cfg(test)]
mod session_tests;

use std::ffi::{c_char, c_void, CStr, CString};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use nmp::ObservationCancel;

use crate::action::AppAction;
use crate::clock::SystemClock;
use crate::model::{KernelConfiguration, QueueSnapshot};
use crate::runtime::RuntimeEvent;

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
    sender: Sender<RuntimeEvent>,
}

impl Control {
    fn new(sender: Sender<RuntimeEvent>) -> Self {
        Self {
            stopping: AtomicBool::new(false),
            cancellation: Mutex::new(None),
            sender,
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

    pub(crate) fn is_stopping(&self) -> bool {
        self.stopping.load(Ordering::Acquire)
    }

    pub(crate) fn cancel_observation(&self) {
        let stored = self
            .cancellation
            .lock()
            .unwrap_or_else(|value| value.into_inner());
        if let Some(cancellation) = stored.as_ref() {
            cancellation.cancel();
        }
    }

    fn send(&self, event: RuntimeEvent) {
        if !self.is_stopping() {
            let _ = self.sender.send(event);
        }
    }

    fn stop(&self) {
        if self.stopping.swap(true, Ordering::AcqRel) {
            return;
        }
        self.cancel_observation();
        let _ = self.sender.send(RuntimeEvent::Stop);
    }
}

struct KernelHandle {
    control: Arc<Control>,
    thread: Option<JoinHandle<()>>,
}

#[no_mangle]
/// Starts the kernel with one bounded JSON configuration and snapshot callback.
///
/// # Safety
/// The configuration must be a live NUL-terminated UTF-8 string. The callback
/// context must remain valid until this handle is passed exactly once to
/// [`tts29_stop`].
pub unsafe extern "C" fn tts29_start(
    configuration_json: *const c_char,
    callback: Option<SnapshotCallback>,
    context: *mut c_void,
) -> *mut c_void {
    let Some(callback) = callback else {
        return std::ptr::null_mut();
    };
    let Some(configuration) = parse_c_string(configuration_json)
        .and_then(|value| serde_json::from_str::<KernelConfiguration>(&value).ok())
    else {
        return std::ptr::null_mut();
    };
    let (sender, receiver) = std::sync::mpsc::channel();
    let control = Arc::new(Control::new(sender.clone()));
    let thread_control = Arc::clone(&control);
    let emitter = Emitter {
        callback,
        context: context as usize,
    };
    let thread = std::thread::Builder::new()
        .name("tts29-kernel".into())
        .spawn(move || {
            runtime::run(
                configuration,
                emitter,
                thread_control,
                receiver,
                sender,
                Arc::new(SystemClock),
            )
        });
    let Ok(thread) = thread else {
        return std::ptr::null_mut();
    };
    Box::into_raw(Box::new(KernelHandle {
        control,
        thread: Some(thread),
    })) as *mut c_void
}

#[no_mangle]
/// Submits a user-entered secret through the dedicated login boundary.
///
/// # Safety
/// `handle` must be live and owned by the caller; `secret` must be a live
/// NUL-terminated UTF-8 string for the duration of this call.
pub unsafe extern "C" fn tts29_login(handle: *mut c_void, secret: *const c_char) {
    send_secret(handle, secret, true);
}

#[no_mangle]
/// Restores a Keychain-loaded secret without requesting another save.
///
/// # Safety
/// `handle` must be live and `secret` must be a live NUL-terminated UTF-8
/// string for the duration of this call.
pub unsafe extern "C" fn tts29_restore_login(handle: *mut c_void, secret: *const c_char) {
    send_secret(handle, secret, false);
}

#[no_mangle]
/// Reports a raw Keychain load failure to the kernel.
///
/// # Safety
/// `handle` must be live and `error` must be a live NUL-terminated UTF-8
/// string for the duration of this call.
pub unsafe extern "C" fn tts29_credential_load_failed(handle: *mut c_void, error: *const c_char) {
    let Some(handle) = kernel(handle) else { return };
    let Some(error) = parse_c_string(error) else {
        return;
    };
    handle
        .control
        .send(RuntimeEvent::CredentialLoadFailed(error));
}

#[no_mangle]
/// Dispatches one non-secret semantic action encoded as JSON.
///
/// # Safety
/// `handle` must be live and `action` must be a live NUL-terminated UTF-8
/// string for the duration of this call.
pub unsafe extern "C" fn tts29_dispatch(handle: *mut c_void, action: *const c_char) {
    let Some(handle) = kernel(handle) else { return };
    let event = parse_c_string(action)
        .and_then(|value| serde_json::from_str::<AppAction>(&value).ok())
        .map(RuntimeEvent::Action)
        .unwrap_or_else(|| RuntimeEvent::ActionError("The app action was invalid.".into()));
    handle.control.send(event);
}

#[no_mangle]
/// Reports the raw result of a kernel-requested Keychain operation.
///
/// # Safety
/// `handle` must be live. A non-null `error` must point to a live
/// NUL-terminated UTF-8 string for the duration of this call.
pub unsafe extern "C" fn tts29_credential_result(
    handle: *mut c_void,
    request_id: u64,
    succeeded: bool,
    error: *const c_char,
) {
    let Some(handle) = kernel(handle) else { return };
    handle.control.send(RuntimeEvent::CredentialResult {
        request_id,
        succeeded,
        error: parse_c_string(error),
    });
}

#[no_mangle]
/// Stops and releases one exact kernel handle.
///
/// # Safety
/// A non-null `handle` must have been returned by [`tts29_start`] and must be
/// passed to this function no more than once.
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

unsafe fn send_secret(handle: *mut c_void, secret: *const c_char, persist: bool) {
    let Some(handle) = kernel(handle) else { return };
    let Some(secret) = parse_c_string(secret) else {
        return;
    };
    handle.control.send(RuntimeEvent::Login { secret, persist });
}

unsafe fn kernel<'a>(handle: *mut c_void) -> Option<&'a KernelHandle> {
    (handle as *const KernelHandle).as_ref()
}

unsafe fn parse_c_string(value: *const c_char) -> Option<String> {
    if value.is_null() {
        return None;
    }
    CStr::from_ptr(value).to_str().ok().map(str::to_string)
}
