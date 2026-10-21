//! Hand-rolled JSON writing — the collection has zero dependencies by
//! design, so every DLL writes its JSON output with these few helpers
//! instead of a serialization crate.
//!
//! [`escape`] is byte-for-byte the `json_escape` that WinRecovery,
//! FastSearch, ArchiveKit, VaultKit, and CrashKit each carry a private
//! copy of (TimeKit produces no JSON). Escaping rules: `"` and `\` are
//! backslash-escaped; newline, carriage return, and tab use their short
//! escapes; any other control character below U+0020 becomes `\u00xx`
//! (lowercase hex, zero-padded to 4 digits); everything else — including
//! all non-ASCII — passes through unchanged (the output is UTF-8 JSON).

use std::fmt::Write as _;

/// Escape a string for inclusion inside JSON double quotes.
///
/// The result does NOT include the surrounding quotes; see
/// [`JsonObject::str`] / [`quoted`] for forms that add them.
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    escape_into(&mut out, s);
    out
}

/// [`escape`], appending into an existing buffer.
pub fn escape_into(out: &mut String, s: &str) {
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
}

/// `s` as a complete JSON string literal, quotes included.
pub fn quoted(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 10);
    out.push('"');
    escape_into(&mut out, s);
    out.push('"');
    out
}

/// Join pre-rendered JSON values into a JSON array string.
///
/// Extracted from WinRecovery's `json_array`; the identical join loop
/// also appears inline in VaultKit's and ArchiveKit's `list_json`.
pub fn array(items: &[String]) -> String {
    let mut s = String::from("[");
    for (i, it) in items.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(it);
    }
    s.push(']');
    s
}

/// A minimal JSON object builder.
///
/// Generalizes WinRecovery's private `js`/`jn`/`jb` field writers (the
/// only field-writer set in the collection): fields are emitted in call
/// order, keys and string values are escaped, and no trailing comma is
/// ever produced. [`JsonObject::raw`] covers the pre-rendered values
/// CrashKit hand-formats (hex strings, `null`, nested objects).
#[derive(Debug)]
pub struct JsonObject {
    buf: String,
    first: bool,
}

impl Default for JsonObject {
    fn default() -> Self {
        Self::new()
    }
}

impl JsonObject {
    pub fn new() -> Self {
        JsonObject {
            buf: String::from("{"),
            first: true,
        }
    }

    fn field_key(&mut self, key: &str) {
        if !self.first {
            self.buf.push(',');
        }
        self.first = false;
        self.buf.push('"');
        escape_into(&mut self.buf, key);
        self.buf.push_str("\":");
    }

    /// Add a `"key":"value"` string field (both escaped).
    pub fn str(&mut self, key: &str, val: &str) -> &mut Self {
        self.field_key(key);
        self.buf.push('"');
        escape_into(&mut self.buf, val);
        self.buf.push('"');
        self
    }

    /// Add a `"key":value` unsigned-integer field.
    pub fn u64(&mut self, key: &str, val: u64) -> &mut Self {
        self.field_key(key);
        let _ = write!(self.buf, "{val}");
        self
    }

    /// Add a `"key":value` signed-integer field.
    pub fn i64(&mut self, key: &str, val: i64) -> &mut Self {
        self.field_key(key);
        let _ = write!(self.buf, "{val}");
        self
    }

    /// Add a `"key":true|false` field.
    pub fn bool(&mut self, key: &str, val: bool) -> &mut Self {
        self.field_key(key);
        let _ = write!(self.buf, "{val}");
        self
    }

    /// Add a field whose value is already-rendered JSON (a number,
    /// `null`, a nested object/array string, …). The value is inserted
    /// verbatim — the caller is responsible for its validity.
    pub fn raw(&mut self, key: &str, raw_json: &str) -> &mut Self {
        self.field_key(key);
        self.buf.push_str(raw_json);
        self
    }

    /// Close the object and return the JSON string.
    pub fn finish(mut self) -> String {
        self.buf.push('}');
        self.buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_matches_donor_behavior() {
        // Ported from WinRecovery's lib.rs tests.
        assert_eq!(escape("a\"b\\c\nd"), "a\\\"b\\\\c\\nd");
        assert_eq!(escape("plain"), "plain");
        // Short escapes and \u forms for the other control characters.
        assert_eq!(escape("\r\t"), "\\r\\t");
        assert_eq!(escape("\u{0}\u{1f}"), "\\u0000\\u001f");
        // Non-ASCII passes through untouched (output is UTF-8).
        assert_eq!(escape("café — ☃"), "café — ☃");
    }

    #[test]
    fn quoted_wraps() {
        assert_eq!(quoted("a\"b"), "\"a\\\"b\"");
        assert_eq!(quoted(""), "\"\"");
    }

    #[test]
    fn array_joins() {
        assert_eq!(array(&[]), "[]");
        assert_eq!(
            array(&["{\"a\":1}".to_string(), "2".to_string()]),
            "[{\"a\":1},2]"
        );
    }

    #[test]
    fn object_builder() {
        let mut o = JsonObject::new();
        o.str("name", "a\"b")
            .u64("count", 3)
            .i64("delta", -2)
            .bool("ok", true)
            .raw("maybe", "null");
        assert_eq!(
            o.finish(),
            "{\"name\":\"a\\\"b\",\"count\":3,\"delta\":-2,\"ok\":true,\"maybe\":null}"
        );
        assert_eq!(JsonObject::new().finish(), "{}");
        assert_eq!(JsonObject::default().finish(), "{}");
        let mut default = JsonObject::default();
        default.str("key", "value");
        assert_eq!(default.finish(), "{\"key\":\"value\"}");
    }
}
