//! win-common — the shared building blocks of Sean's Windows DLL
//! collection (WinRecovery, FastSearch, ArchiveKit, VaultKit, CrashKit,
//! TimeKit).
//!
//! Each of those DLLs was built zero-dependency and, in the process,
//! grew its own copy of the same skeleton: a thread-local last error, C
//! string ownership helpers, a hand-rolled JSON escaper, civil calendar
//! math, and — where needed — UTF-16 wide-string helpers. This crate is
//! those pieces, extracted once, from the copies the DLLs actually
//! carry (where copies differed, the module docs say which variant was
//! chosen and why).
//!
//! What is deliberately NOT here: any Win32 bindings or platform calls.
//! Those stay hand-declared in each DLL's own platform layer, per the
//! collection's design rules. Everything in win-common is pure,
//! portable, and unit-tested off-Windows.
//!
//! Modules:
//! - [`error`] — the shared `(code, message)` error convention and the
//!   standard code assignments 0–6.
//! - [`ffi`] — thread-local last error, C string ownership, input
//!   decoding, and panic guards for FFI entry points.
//! - [`json`] — JSON string escaping, array joining, and a minimal
//!   object builder.
//! - [`time`] — civil calendar math (Hinnant's algorithms), broken-down
//!   UTC, ISO-8601 / filename formatting, FILETIME conversions.
//! - [`wide`] — UTF-16 helpers for Win32 `W` APIs.

pub mod error;
pub mod ffi;
pub mod json;
pub mod time;
pub mod wide;
