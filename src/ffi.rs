//! FFI plumbing shared by every DLL in the collection: the thread-local
//! last error, C string ownership, input decoding, and panic guards.
//!
//! Each adopting DLL keeps its own C exports (`xx_last_error`,
//! `xx_last_error_code`, `xx_free_string`, …) as thin wrappers over the
//! functions here, so its public C ABI does not change. Because
//! win-common is linked statically into each DLL, each DLL binary gets
//! its own copy of the thread-local below — the same behavior the DLLs
//! have today with their private copies.
//!
//! Caller contract (the house standard, stated in every DLL header):
//! - String inputs are `const char*` UTF-8, NUL-terminated, or null
//!   where the function documents null as meaningful.
//! - `*mut c_char` results are caller-owned and must be released with
//!   the producing library's `*_free_string` export.
//! - The last-error message pointer is borrowed: do NOT free it; it is
//!   valid until the next win-common call on the same thread.
//! - Nothing ever unwinds across the FFI boundary: entry points run
//!   their bodies through one of the guards in this module.

use std::cell::RefCell;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::panic::{catch_unwind, AssertUnwindSafe};

use crate::error::{Error, INTERNAL, INVALID_ARG};

thread_local! {
    static LAST_ERROR: RefCell<(i32, CString)> =
        RefCell::new((crate::error::OK, CString::new("").unwrap()));
}

/// Record `code` + `msg` as this thread's last error.
///
/// Uses the VaultKit/CrashKit/TimeKit variant, which strips NUL
/// characters from the message first: WinRecovery/FastSearch/ArchiveKit
/// pass the message to `CString::new` directly and silently degrade to
/// the literal `"error"` if it ever contains a NUL.
pub fn set_error(code: i32, msg: &str) {
    let clean: String = msg.chars().filter(|&c| c != '\0').collect();
    LAST_ERROR.with(|e| {
        *e.borrow_mut() = (
            code,
            CString::new(clean).unwrap_or_else(|_| CString::new("error").unwrap()),
        );
    });
}

/// Reset the last error to `(OK, "")`.
pub fn clear_error() {
    set_error(crate::error::OK, "");
}

/// The current last-error code for this thread.
pub fn last_error_code() -> i32 {
    LAST_ERROR.with(|e| e.borrow().0)
}

/// The current last-error message for this thread (Rust-side accessor;
/// C callers go through [`last_error_ptr`]).
pub fn last_error_message() -> String {
    LAST_ERROR.with(|e| e.borrow().1.to_string_lossy().into_owned())
}

/// Borrowed pointer to the last-error message, for a DLL's
/// `xx_last_error` export. Do NOT free; valid until the next
/// [`set_error`] / [`clear_error`] on this thread.
pub fn last_error_ptr() -> *const c_char {
    LAST_ERROR.with(|e| e.borrow().1.as_ptr())
}

/// Hand a Rust `String` to a C caller as an owned `*mut c_char`
/// (release with [`free_c_string`], via the DLL's `*_free_string`).
///
/// On an interior NUL byte there is nothing valid to hand over: sets
/// the last error to INTERNAL and returns null — the exact behavior of
/// WinRecovery/FastSearch/ArchiveKit's `into_c_string`. (VaultKit's
/// `string_out` additionally clears the error on success; this function
/// leaves the error untouched on success, matching the majority.)
pub fn into_raw_c_string(s: String) -> *mut c_char {
    match CString::new(s) {
        Ok(cs) => cs.into_raw(),
        Err(_) => {
            set_error(INTERNAL, "result contained an interior NUL byte");
            std::ptr::null_mut()
        }
    }
}

/// Release a string produced by [`into_raw_c_string`]. Null is a no-op.
///
/// # Caller contract
/// `p` must be null or a pointer returned by [`into_raw_c_string`] that
/// has not been freed yet — the standard C ownership rule.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn free_c_string(p: *mut c_char) {
    if !p.is_null() {
        // SAFETY: caller contract above.
        unsafe {
            drop(CString::from_raw(p));
        }
    }
}

/// Decode a required UTF-8 input argument into an owned `String`.
///
/// Errors are `(INVALID_ARG, "<what> must not be null")` for a null
/// pointer and `(INVALID_ARG, "<what> is not valid UTF-8")` for bad
/// bytes — the messages FastSearch/ArchiveKit's `require_str` and
/// VaultKit/CrashKit's `input` agree on.
///
/// # Caller contract
/// `p` must be a valid NUL-terminated UTF-8 string or null.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn require_str(p: *const c_char, what: &str) -> Result<String, Error> {
    if p.is_null() {
        return crate::error::err(INVALID_ARG, format!("{what} must not be null"));
    }
    // SAFETY: caller contract above.
    let s = unsafe { CStr::from_ptr(p) };
    s.to_str()
        .map(|v| v.to_string())
        .map_err(|_| (INVALID_ARG, format!("{what} is not valid UTF-8")))
}

/// Decode an optional UTF-8 input argument: null → `None`, invalid
/// UTF-8 → `None`. Extracted from WinRecovery's `opt_c_str`, the only
/// DLL that takes nullable string inputs today.
///
/// # Caller contract
/// `p` must be a valid NUL-terminated UTF-8 string or null. The returned
/// reference borrows from the caller's buffer.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn opt_c_str<'a>(p: *const c_char) -> Option<&'a str> {
    if p.is_null() {
        None
    } else {
        // SAFETY: caller contract above.
        unsafe { CStr::from_ptr(p) }.to_str().ok()
    }
}

/// Run `f`, converting a panic into
/// `(INTERNAL, "internal panic caught at the FFI boundary")`.
///
/// Extracted from FastSearch/ArchiveKit's `guard` — the variant for
/// bodies that already return the collection's `Result` shape.
pub fn guard<T>(f: impl FnOnce() -> Result<T, Error>) -> Result<T, Error> {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(r) => r,
        Err(_) => crate::error::err(INTERNAL, "internal panic caught at the FFI boundary"),
    }
}

/// Run `f` under `catch_unwind`, passing the raw `thread::Result`
/// through. Extracted from VaultKit/CrashKit/TimeKit's `guard`, whose
/// callers map the panic case themselves (typically to an INTERNAL
/// status via a `panic_code` helper).
pub fn guard_value<T, F: FnOnce() -> T>(f: F) -> std::thread::Result<T> {
    catch_unwind(AssertUnwindSafe(f))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cstr(s: &str) -> CString {
        CString::new(s).unwrap()
    }

    #[test]
    fn error_roundtrip() {
        clear_error();
        assert_eq!(last_error_code(), crate::error::OK);
        assert_eq!(last_error_message(), "");
        set_error(INVALID_ARG, "bad thing");
        assert_eq!(last_error_code(), INVALID_ARG);
        assert_eq!(last_error_message(), "bad thing");
        // SAFETY: borrowed thread-local message pointer.
        let s = unsafe { CStr::from_ptr(last_error_ptr()) }
            .to_str()
            .unwrap();
        assert_eq!(s, "bad thing");
        clear_error();
        assert_eq!(last_error_code(), crate::error::OK);
    }

    #[test]
    fn set_error_strips_nuls() {
        // The VaultKit/CrashKit/TimeKit variant: an embedded NUL must
        // not degrade the whole message to the "error" fallback.
        set_error(INTERNAL, "be\0fore");
        assert_eq!(last_error_message(), "before");
        clear_error();
    }

    #[test]
    fn raw_string_roundtrip_and_free_null() {
        let p = into_raw_c_string("hello".to_string());
        assert!(!p.is_null());
        // SAFETY: p came from into_raw_c_string just above.
        let s = unsafe { CStr::from_ptr(p) }.to_str().unwrap().to_string();
        assert_eq!(s, "hello");
        free_c_string(p);
        free_c_string(std::ptr::null_mut()); // no-op, no crash
    }

    #[test]
    fn interior_nul_is_internal_error() {
        let p = into_raw_c_string("a\0b".to_string());
        assert!(p.is_null());
        assert_eq!(last_error_code(), INTERNAL);
        clear_error();
    }

    #[test]
    fn require_str_contract() {
        assert_eq!(
            require_str(std::ptr::null(), "path").unwrap_err(),
            (INVALID_ARG, "path must not be null".to_string())
        );
        let bad = CString::new(vec![0xFFu8]).unwrap();
        assert_eq!(
            require_str(bad.as_ptr(), "path").unwrap_err(),
            (INVALID_ARG, "path is not valid UTF-8".to_string())
        );
        let good = cstr("fine");
        assert_eq!(require_str(good.as_ptr(), "path").unwrap(), "fine");
    }

    #[test]
    fn opt_c_str_contract() {
        assert_eq!(opt_c_str(std::ptr::null()), None);
        let good = cstr("x");
        assert_eq!(opt_c_str(good.as_ptr()), Some("x"));
        let bad = CString::new(vec![0xFFu8]).unwrap();
        assert_eq!(opt_c_str(bad.as_ptr()), None);
    }

    #[test]
    fn guard_maps_panic_to_internal() {
        let r: Result<i32, Error> = guard(|| panic!("boom"));
        assert_eq!(
            r.unwrap_err(),
            (
                INTERNAL,
                "internal panic caught at the FFI boundary".to_string()
            )
        );
        let ok: Result<i32, Error> = guard(|| Ok(7));
        assert_eq!(ok.unwrap(), 7);
    }

    #[test]
    fn guard_value_passes_result_through() {
        assert_eq!(guard_value(|| 3).unwrap(), 3);
        assert!(guard_value(|| -> i32 { panic!("boom") }).is_err());
    }
}
