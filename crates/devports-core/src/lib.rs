//! DevPorts core.
//!
//! Every rule of the product lives in this crate: project discovery, process and
//! container observation, service identity, instance leases and the IPC server.
//! It deliberately does not depend on `tauri`, so it can run and be tested with
//! no interface at all; the menu-bar app is just one host for it.

pub mod adopt;
pub mod catalog;
pub mod classify;
pub mod clock;
pub mod config;
pub mod diagnostic;
pub mod discovery;
pub mod docker;
pub mod document;
pub mod engine;
pub mod fingerprint;
pub mod gitinfo;
pub mod hash;
pub mod ids;
pub mod ignore;
pub mod ipc;
pub mod manifest;
pub mod messages;
pub mod platform;
pub mod ports;
pub mod process;
pub mod reasons;
pub mod runtime;
pub mod settings;
mod shell;
pub mod store;
pub mod uses;
pub mod yaml_keys;

/// Protocol version the core's IPC server speaks.
pub const SUPPORTED_PROTOCOL: u32 = devports_proto::PROTOCOL_VERSION;
