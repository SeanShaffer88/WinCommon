# win-common — agent notes

- Zero dependencies is a design rule (house standard across the whole DLL collection): std-only; a new dependency needs a written justification.
- This crate holds ONLY the code the collection's DLLs genuinely share: FFI last-error/C-string plumbing, JSON writing, civil calendar math, UTF-16 helpers. Per-DLL Win32 bindings stay hand-declared in each DLL's own src/platform.rs with explicit #[link(name = "...")] on every extern block — do not move bindings here.
- Extracted code must match the donor projects' behavior byte-for-byte (escapes, error messages, date math). Where donors had variants, the module docs name the chosen variant and the difference; changing extracted behavior is a breaking change for every adopter at once.
- Error codes 0–6 in src/error.rs are the collection convention (WinRecovery/FastSearch/ArchiveKit/VaultKit). CrashKit and TimeKit deviate on purpose (documented in error.rs) — do not "fix" their tables from here.
- Gate: WinDevKit tools/gate.sh (cargo fmt --all, cargo clippy --all-targets -- -D warnings, cargo clippy --target x86_64-pc-windows-msvc --all-targets -- -D warnings — host clippy never sees cfg(windows) code, cargo test with the FULL suite, cargo check --target x86_64-pc-windows-msvc --all-targets). Check/clippy do not link — CI's real build is the link gate.
- Never claim Windows runtime behavior is verified until it has run on a real Windows machine; compile-verification is not runtime verification.
- Adoption of win-common by the six DLLs is a separate, per-DLL step: each DLL keeps its own C ABI (its `xx_*` exports become thin wrappers), and its header does not change.
