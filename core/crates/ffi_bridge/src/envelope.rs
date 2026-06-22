use std::ffi::{CStr, CString, c_char};

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

#[derive(Debug, Serialize)]
pub struct FfiEnvelope<T> {
    pub ok: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

/// Free a C string returned by this library.
///
/// # Safety
/// `ptr` must be a pointer previously returned by this crate from one of the `*_json` APIs.
/// It must not be freed more than once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn infomatrix_core_free_string(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    let _ = unsafe { CString::from_raw(ptr) };
}

pub fn with_input<I, O>(
    input: *const c_char,
    operation: impl FnOnce(I) -> Result<O, String>,
) -> Result<*mut c_char, *mut c_char>
where
    I: DeserializeOwned,
    O: Serialize,
{
    let parsed = match parse_input::<I>(input) {
        Ok(parsed) => parsed,
        Err(error) => return Err(respond_error(error)),
    };

    match operation(parsed) {
        Ok(output) => Ok(respond_ok(output)),
        Err(error) => Err(respond_error(error)),
    }
}

fn parse_input<T: DeserializeOwned>(input: *const c_char) -> Result<T, String> {
    if input.is_null() {
        return Err("input pointer is null".to_owned());
    }

    let payload = unsafe { CStr::from_ptr(input) }
        .to_str()
        .map_err(|err| format!("input is not valid utf-8: {err}"))?;

    serde_json::from_str(payload).map_err(|err| format!("invalid input json: {err}"))
}

pub fn respond_ok<T: Serialize>(data: T) -> *mut c_char {
    let envelope = FfiEnvelope { ok: true, data: Some(data), error: None };
    encode_envelope(&envelope)
}

pub fn respond_error(error: String) -> *mut c_char {
    let envelope = FfiEnvelope::<Value> { ok: false, data: None, error: Some(error) };
    encode_envelope(&envelope)
}

fn encode_envelope<T: Serialize>(envelope: &FfiEnvelope<T>) -> *mut c_char {
    let json = serde_json::to_string(envelope).unwrap_or_else(|err| {
        format!(
            r#"{{"ok":false,"data":null,"error":"failed to encode json: {}"}}"#,
            sanitize_error(&err.to_string())
        )
    });
    CString::new(json)
        .unwrap_or_else(|_| {
            CString::new(r#"{"ok":false,"data":null,"error":"response contained null byte"}"#)
                .expect("fallback CString")
        })
        .into_raw()
}

fn sanitize_error(message: &str) -> String {
    message.replace('"', "'")
}
