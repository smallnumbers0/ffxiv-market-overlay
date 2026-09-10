//! Global show/hide hotkey.
//!
//! Registered from Rust at startup (before any webview exists) so the overlay
//! responds to the hotkey even when it has never been shown.

use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::error::{AppError, AppResult};

/// Register `accelerator` as the overlay toggle.
pub fn register<R: Runtime>(app: &AppHandle<R>, accelerator: &str) -> AppResult<()> {
    let shortcut: Shortcut = accelerator
        .parse()
        .map_err(|_| AppError::Invalid(format!("'{accelerator}' is not a valid hotkey")))?;

    app.global_shortcut()
        .on_shortcut(shortcut, |app, _shortcut, event| {
            // Fire on press only; without this the overlay toggles twice per
            // keypress (once down, once up).
            if event.state == ShortcutState::Pressed {
                toggle_overlay(app);
            }
        })
        .map_err(|e| {
            AppError::Invalid(format!(
                "'{accelerator}' could not be registered - another app may already own it ({e})"
            ))
        })
}

pub fn unregister<R: Runtime>(app: &AppHandle<R>, accelerator: &str) {
    if let Ok(shortcut) = accelerator.parse::<Shortcut>() {
        let _ = app.global_shortcut().unregister(shortcut);
    }
}

/// Show-and-focus if hidden, hide if visible.
///
/// Hiding (rather than just unfocusing) is what hands keyboard control back to
/// the game - an always-on-top window that merely loses focus still eats
/// clicks.
pub fn toggle_overlay<R: Runtime>(app: &AppHandle<R>) {
    let Some(window) = app.get_webview_window(crate::MAIN_WINDOW) else {
        return;
    };

    if window.is_visible().unwrap_or(false) {
        // Save before hiding: a window the user dragged and then toggled away
        // may never have fired a focus-lost event.
        crate::save_geometry(&window.as_ref().window());
        let _ = window.hide();
    } else {
        crate::restore_geometry(&window);
        let _ = window.show();
        let _ = window.set_focus();
    }
}
