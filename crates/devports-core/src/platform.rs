//! Operating-system adapters.
//!
//! Each submodule implements the core's source traits for one platform. Only
//! macOS exists in v1 (ADR-016); porting means writing one more module here.

#[cfg(target_os = "macos")]
pub mod macos;
