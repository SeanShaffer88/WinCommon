//! UTF-16 wide-string helpers for Win32 `W`-suffixed APIs.
//!
//! Only two of the six DLLs need these today — WinRecovery and
//! VaultKit — and their `wide()` helpers differ in mechanism only:
//! WinRecovery encodes via `OsStr::encode_wide` (Windows-only), VaultKit
//! via `str::encode_utf16` (portable). For `&str` inputs the results are
//! identical, so this module standardizes on the portable form and the
//! helpers can be unit-tested on any OS.
//!
//! Decoding is WinRecovery's `from_wide` (the only decoder in the
//! collection): truncate at the first NUL, then convert lossily —
//! unpaired surrogates become U+FFFD, the same result VaultKit gets from
//! `String::from_utf16_lossy` at its call site.

/// Encode `s` as UTF-16 with a trailing NUL terminator, ready to pass
/// as an `LPCWSTR` to a Win32 `W` function.
pub fn to_wide_z(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Decode a wide buffer to a `String`, stopping at the first NUL.
/// Invalid units are replaced with U+FFFD (lossy).
pub fn from_wide_z(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

/// Decode an entire wide buffer (embedded NULs are kept as U+0000 in
/// the result). Invalid units are replaced with U+FFFD (lossy).
pub fn from_wide_lossy(buf: &[u16]) -> String {
    String::from_utf16_lossy(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_wide_z_terminates() {
        assert_eq!(to_wide_z(""), vec![0]);
        assert_eq!(to_wide_z("Aé"), vec![0x41, 0xE9, 0]);
        // A surrogate pair (U+1F600) stays intact before the terminator.
        assert_eq!(to_wide_z("😀"), vec![0xD83D, 0xDE00, 0]);
    }

    #[test]
    fn from_wide_z_stops_at_nul() {
        assert_eq!(from_wide_z(&[0x41, 0, 0x42]), "A");
        assert_eq!(from_wide_z(&[0x41, 0x42]), "AB");
        assert_eq!(from_wide_z(&[]), "");
    }

    #[test]
    fn from_wide_lossy_keeps_everything() {
        assert_eq!(from_wide_lossy(&[0x41, 0, 0x42]), "A\0B");
        // Unpaired surrogate → replacement character.
        assert_eq!(from_wide_lossy(&[0xD83D, 0x41]), "\u{FFFD}A");
    }

    #[test]
    fn roundtrip() {
        let s = "VaultKit/<ns>/<key> — café ☃";
        assert_eq!(from_wide_z(&to_wide_z(s)), s);
    }
}
