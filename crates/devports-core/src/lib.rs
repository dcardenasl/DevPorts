//! DevPorts core.
//!
//! Every rule of the product lives in this crate: project discovery, process and
//! container observation, service identity, instance leases and the IPC server.
//! It deliberately does not depend on `tauri`, so it can run and be tested with
//! no interface at all; the menu-bar app is just one host for it.

pub mod manifest;
pub mod platform;
pub mod probe;
pub mod process;

/// Protocol version the core's IPC server speaks.
pub const SUPPORTED_PROTOCOL: u32 = devports_proto::PROTOCOL_VERSION;
