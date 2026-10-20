# win-common

The shared building blocks of Sean's Windows DLL collection —
[WinRecovery](https://github.com/SeanShaffer88/WinRecovery),
FastSearch, ArchiveKit, VaultKit, CrashKit, and TimeKit.

Each DLL in the collection is zero-dependency by design, and each one
grew its own copy of the same skeleton: a thread-local last error, C
string ownership helpers, a hand-rolled JSON escaper, civil calendar
math, and UTF-16 wide-string helpers. win-common is those pieces
extracted once, from the copies the DLLs actually carry. It is a plain
Rust library crate (no C ABI of its own), is itself zero-dependency,
and is MIT-licensed.

**Status:** built and gated locally; not yet published, and none of
the six DLLs consume it yet. Adoption is a separate, per-DLL step (see
the notes below) — each DLL's public C ABI stays exactly as it is.

## What's inside

### `error` — the shared error convention

Every DLL reports failures as `(i32 code, String message)`. Codes 0–6
are the standard assignments used identically by WinRecovery,
FastSearch, ArchiveKit, and VaultKit:

| Code | Constant        | Meaning                                  |
|-----:|-----------------|------------------------------------------|
| 0    | `OK`            | success                                  |
| 1    | `INVALID_ARG`   | null / malformed / out-of-range argument |
| 2    | `NOT_SUPPORTED` | not supported on this platform or input  |
| 3    | `NOT_FOUND`     | the requested item does not exist        |
| 4    | `ACCESS_DENIED` | the OS refused access                    |
| 5    | `PLATFORM`      | the underlying platform call failed      |
| 6    | `INTERNAL`      | internal failure (incl. a caught panic)  |

Plus `Error` / `Result<T>` type aliases and an `err()` constructor
(from ArchiveKit). Documented deviations: ArchiveKit adds
`ERR_CORRUPT = 7`; CrashKit uses 3 for `NOT_INSTALLED`; TimeKit
reassigns 2/3/4 for zone semantics but keeps 6 for INTERNAL. Those
DLLs keep their own constants; the shared shapes are code-agnostic.

### `ffi` — FFI plumbing

- `set_error` / `clear_error` / `last_error_code` /
  `last_error_message` / `last_error_ptr` — the thread-local last
  error every DLL keeps. Uses the VaultKit/CrashKit/TimeKit variant of
  `set_error`, which strips NULs from the message instead of silently
  degrading to the literal `"error"`.
- `into_raw_c_string` / `free_c_string` — the C string ownership pair
  behind every `*_json` result and `*_free_string` export.
- `require_str` / `opt_c_str` — decoding of required / optional UTF-8
  `const char*` inputs.
- `guard` / `guard_value` — the two panic-guard variants the DLLs use
  so nothing unwinds across the FFI boundary: FastSearch/ArchiveKit's
  (returns the shared `Result` shape) and VaultKit/CrashKit/TimeKit's
  (raw `catch_unwind` passthrough).

### `json` — hand-rolled JSON writing

- `escape` / `escape_into` — the collection's `json_escape`, identical
  in five of the six DLLs: `"` and `\` escaped; `\n`, `\r`, `\t` short
  escapes; other control characters as `\u00xx`; non-ASCII passes
  through as UTF-8.
- `quoted` — a complete `"…"` string literal.
- `array` — join pre-rendered values into a JSON array (WinRecovery's
  `json_array`; the same loop sits inline in VaultKit's and ArchiveKit's
  `list_json`).
- `JsonObject` — a minimal object builder (`str` / `u64` / `i64` /
  `bool` / `raw`), generalizing WinRecovery's private `js`/`jn`/`jb`
  field writers.

### `time` — civil calendar math and UTC formatting

From TimeKit (the most complete variant) and CrashKit:

- `is_leap`, `days_in_month`, `days_from_civil`, `civil_from_days`
  (Hinnant's algorithms; the copy in CrashKit is identical),
  `weekday_from_days` (0 = Sunday).
- `Civil` — broken-down date-time with `validate()` and `wall_secs()`;
  `civil_from_unix`, `civil_from_unix_ms`.
- `format_iso8601_utc` (`YYYY-MM-DDTHH:MM:SSZ`) and
  `format_filename_utc_ms` (CrashKit's `YYYYMMDD-HHMMSS`).
- `unix_secs_from_filetime` / `unix_ms_from_filetime` — the pure
  halves of the FILETIME clock code in TimeKit's and CrashKit's
  platform layers, including their clamp-to-0 behavior.

### `wide` — UTF-16 helpers for Win32 `W` APIs

- `to_wide_z` — `&str` → NUL-terminated `Vec<u16>`. (WinRecovery's copy
  encodes via `OsStr`; VaultKit's via `str::encode_utf16`. Identical
  for `&str` inputs; this crate standardizes on the portable form.)
- `from_wide_z` — decode up to the first NUL, lossy (WinRecovery's
  `from_wide`); `from_wide_lossy` — decode the whole buffer, lossy.

## Adoption notes (per DLL)

Adoption changes no C ABI: each DLL keeps its header and every `xx_*`
export; the exports become thin wrappers over win-common. The
mechanical step is the same everywhere — add a vendored path
dependency and delete the private copies:

```toml
[dependencies]
win-common = { path = "../win-common" }
```

(Vendoring a copy of this crate into the DLL's own repo works the same
way; what matters is that the DLL's shipped tree stays self-contained.)

- **WinRecovery** — `ffi`: `LAST_ERROR`, `set_error`, `clear_error`,
  `into_c_string`, `wr_free_string` body, `require_c_str`,
  `opt_c_str`. `json`: `json_escape`, `json_array`, and the
  `js`/`jn`/`jb` writers become `JsonObject`. `wide`: platform
  `wide`/`from_wide` become `to_wide_z`/`from_wide_z`.
- **FastSearch** — `ffi`: last error, `into_c_string`, `require_str`,
  and its `guard` (the source of win-common's `guard`). `json`:
  `json_escape`. (The handle registry stays in FastSearch — no other
  DLL has one.)
- **ArchiveKit** — `ffi`: last error, `into_c_string`, `require_str`,
  `guard`. `error`: `err()` and `AResult` are the shared shapes
  already; `ERR_CORRUPT = 7` stays an ArchiveKit constant. `json`:
  `json_escape`, and the `list_json` join loop becomes `json::array`.
- **VaultKit** — `ffi`: last error (its NUL-stripping `set_error` is
  the variant win-common adopted), `guard`, `input` → `require_str`.
  Its `string_out` also clears the error on success, which
  `into_raw_c_string` deliberately does not — keep a one-line wrapper.
  `json`: `json_escape`, `list_json` loop → `json::array`. `wide`:
  platform `wide` → `to_wide_z`.
- **CrashKit** — `ffi`: last error, `guard`, `input` → `require_str`,
  and the repeated inline `CString::new` string-out blocks →
  `into_raw_c_string`. `time`: `civil_from_days`, the breakdown inside
  `Timestamp::filename_part` → `format_filename_utc_ms`, and
  `now_unix_ms`'s FILETIME math → `unix_ms_from_filetime`. (The crash
  path itself — filter, capture thread, dbghelp bindings — stays in
  CrashKit; it is not shared code. CrashKit's error codes deviate, so
  it keeps its own constants.)
- **TimeKit** — `time`: the entire civil-calendar section (`is_leap`,
  `days_in_month`, `days_from_civil`, `civil_from_days`,
  `weekday_from_days`, `Civil`, `civil_from_unix`) and `now_unix`'s
  FILETIME math → `unix_secs_from_filetime`. `ffi`: last error,
  `guard`. (The TZif/POSIX rule engine stays in TimeKit. Its zone
  error codes deviate, so it keeps its own constants.)

## Verification

Local gate (WinDevKit `tools/gate.sh`): `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings` on the host target and on
`x86_64-pc-windows-msvc`, the full `cargo test` suite, and
`cargo check --target x86_64-pc-windows-msvc --all-targets`.
Check/clippy do not link; the CI layout in `ci.yml` (meant for
`.github/workflows/ci.yml`) adds the real Windows build. Nothing here
has run on a real Windows machine — win-common is pure logic, and its
tests are the same unit tests the donor projects run.

## License

MIT — Copyright (c) 2026 Sean Shaffer. See [LICENSE](LICENSE).
