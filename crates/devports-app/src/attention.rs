//! Spike stand-in for the attention counter (duplicates plus leases about to
//! expire). The real value comes from the core in task T2.1.

#![allow(
    clippy::needless_pass_by_value,
    reason = "Tauri extracts command arguments by value; the signatures are fixed"
)]

use std::sync::atomic::{AtomicU32, Ordering};

use tauri::{AppHandle, State};

/// The number of things waiting for a decision.
#[derive(Debug, Default)]
pub struct Attention(AtomicU32);

impl Attention {
    /// Sets the counter and returns the new value.
    pub fn set(&self, value: u32) -> u32 {
        self.0.store(value, Ordering::Relaxed);
        value
    }

    /// Adds one and returns the new value.
    pub fn bump(&self) -> u32 {
        self.0.fetch_add(1, Ordering::Relaxed).saturating_add(1)
    }

    /// The current value.
    pub fn get(&self) -> u32 {
        self.0.load(Ordering::Relaxed)
    }
}

#[tauri::command]
pub fn get_counter(attention: State<'_, Attention>) -> u32 {
    attention.get()
}

#[tauri::command]
pub fn bump_counter(app: AppHandle, attention: State<'_, Attention>) -> u32 {
    let value = attention.bump();
    crate::tray::show_count(&app, value);
    value
}

#[tauri::command]
pub fn reset_counter(app: AppHandle, attention: State<'_, Attention>) -> u32 {
    let value = attention.set(0);
    crate::tray::show_count(&app, value);
    value
}
