//! Native window styling that Tauri does not expose (macOS).
//!
//! The panel has no title bar, so its window is a plain rectangle. The design
//! calls for 14 px rounded corners (`UI-REFERENCIA.md`). Tauri can make a window
//! transparent only through a private WebKit API (`macOSPrivateApi`), which can
//! break with any macOS release. Rounding the content view's layer is entirely
//! public AppKit and Core Animation API and needs no transparent web view: the
//! panel is opaque, the window is made non-opaque with a clear background so the
//! corners show what is behind it, and the layer clips the content.
//!
//! This is the only module of the app allowed to use `unsafe` (ADR-025): Tauri
//! hands out the `NSWindow` as a raw pointer.

#![allow(
    unsafe_code,
    reason = "Tauri exposes the NSWindow only as a raw pointer; one guarded cast"
)]

use objc2_app_kit::{NSColor, NSWindow};
use tauri::WebviewWindow;

/// Clips the window's content to rounded corners of `radius` points and sets its
/// opacity (`0.0` invisible, `1.0` fully visible).
///
/// A window that is not on screen is not painted: WebKit suspends rendering and
/// `requestAnimationFrame` never fires for it. So the panel is put on screen
/// fully transparent first, which lets the page paint, and made opaque when it
/// reports that it is ready.
pub fn style(window: &WebviewWindow, radius: f64, opacity: f64) -> tauri::Result<()> {
    window.with_webview(move |webview| {
        // SAFETY: `with_webview` runs this closure on the main thread, and
        // `ns_window` is the window's `NSWindow`, which outlives the call.
        let Some(ns_window) = (unsafe { webview.ns_window().cast::<NSWindow>().as_ref() }) else {
            return;
        };
        apply(ns_window, radius, opacity);
    })
}

/// Changes only the opacity of the window.
pub fn set_opacity(window: &WebviewWindow, opacity: f64) -> tauri::Result<()> {
    window.with_webview(move |webview| {
        // SAFETY: as in `style`.
        if let Some(ns_window) = unsafe { webview.ns_window().cast::<NSWindow>().as_ref() } {
            ns_window.setAlphaValue(opacity);
            ns_window.invalidateShadow();
        }
    })
}

fn apply(window: &NSWindow, radius: f64, opacity: f64) {
    window.setOpaque(false);
    window.setAlphaValue(opacity);
    window.setBackgroundColor(Some(&NSColor::clearColor()));
    window.setHasShadow(true);
    if let Some(view) = window.contentView() {
        view.setWantsLayer(true);
        if let Some(layer) = view.layer() {
            layer.setCornerRadius(radius);
            layer.setMasksToBounds(true);
        }
    }
    // The shadow is computed from the window's shape: recompute it now that the
    // corners are round.
    window.invalidateShadow();
}
