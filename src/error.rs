//! The collection's shared error convention.
//!
//! Every DLL in the collection reports failures the same way: an `i32`
//! code plus a human-readable message (surfaced to C callers through the
//! thread-local last error, see [`crate::ffi`]). Codes 0–6 below are the
//! standard assignments, used identically by WinRecovery, FastSearch,
//! ArchiveKit, and VaultKit.
//!
//! Known per-DLL deviations (documented, not papered over):
//! - ArchiveKit adds `ERR_CORRUPT = 7` for malformed archives.
//! - CrashKit uses 3 for `NOT_INSTALLED` and has no NOT_FOUND /
//!   ACCESS_DENIED codes.
//! - TimeKit reassigns 2/3/4 for zone semantics (`ERR_UNKNOWN_ZONE`,
//!   `ERR_GAP`, `ERR_AMBIGUOUS`) but keeps 6 for INTERNAL.
//!
//! Adopters with deviating code tables keep their own constants and use
//! this module's [`Error`] / [`err`] shapes, which are code-agnostic.

/// Success.
pub const OK: i32 = 0;
/// A caller argument was null, malformed, or out of range.
pub const INVALID_ARG: i32 = 1;
/// The operation is not supported on this platform or for this input.
pub const NOT_SUPPORTED: i32 = 2;
/// The requested item does not exist.
pub const NOT_FOUND: i32 = 3;
/// The OS refused access.
pub const ACCESS_DENIED: i32 = 4;
/// The underlying platform call failed (see the message for detail).
pub const PLATFORM: i32 = 5;
/// An internal failure — in FFI code, a panic caught at the boundary.
pub const INTERNAL: i32 = 6;

/// The collection's universal error shape: `(code, message)`.
pub type Error = (i32, String);

/// The collection's universal result shape.
pub type Result<T> = std::result::Result<T, Error>;

/// Build an `Err` with the given code and message.
///
/// Extracted from ArchiveKit's `err` helper; the other DLLs construct
/// the tuple inline with the same shape.
pub fn err<T>(code: i32, msg: impl Into<String>) -> Result<T> {
    Err((code, msg.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_codes() {
        assert_eq!(OK, 0);
        assert_eq!(INVALID_ARG, 1);
        assert_eq!(NOT_SUPPORTED, 2);
        assert_eq!(NOT_FOUND, 3);
        assert_eq!(ACCESS_DENIED, 4);
        assert_eq!(PLATFORM, 5);
        assert_eq!(INTERNAL, 6);
    }

    #[test]
    fn err_helper_shape() {
        let r: Result<()> = err(INVALID_ARG, "nope");
        assert_eq!(r.unwrap_err(), (INVALID_ARG, "nope".to_string()));
    }
}
