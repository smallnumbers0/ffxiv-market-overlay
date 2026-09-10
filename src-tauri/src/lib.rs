//! FFXIV Market Overlay - app wiring.
//!
//! A standalone companion window. It does not read the game's memory, inject
//! into the game process, or touch game files; it only talks to XIVAPI and
//! Universalis over HTTPS.

pub mod cache;
pub mod commands;
pub mod config;
pub mod db;
pub mod error;
pub mod hotkey;
pub mod search;
pub mod state;
pub mod universalis;
pub mod xivapi_sync;

use std::path::PathBuf;

use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Manager, WindowEvent};

use crate::error::{AppError, AppResult};
use crate::state::AppState;

/// Label of the overlay window, as declared in `tauri.conf.json`.
pub const MAIN_WINDOW: &str = "main";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            commands::search_items,
            commands::get_price,
            commands::get_settings,
            commands::ensure_market_scope,
            commands::set_market_scope,
            commands::set_hotkey,
            commands::list_market_scopes,
            commands::record_recent_item,
            commands::get_recent_items,
            commands::clear_recent_items,
            commands::refresh_catalog,
            commands::hide_overlay,
            commands::get_catalog_path,
        ])
        .setup(|app| {
            let handle = app.handle();
            let state = AppState::new(&config_path(handle)?, &prepare_catalog(handle)?)?;

            // A hotkey another app already owns must not stop the app from
            // starting - the overlay is still usable from the tray, and the
            // settings panel explains what happened.
            let accelerator = state.config.get().hotkey;
            if let Err(error) = hotkey::register(handle, &accelerator) {
                eprintln!("global hotkey not registered: {error}");
                state.set_hotkey_error(Some(error.to_string()));
            }

            app.manage(state);
            match build_tray(handle) {
                Ok(()) => eprintln!("tray icon created"),
                // A tray icon the OS refuses is a degraded experience, not a
                // dead app - the hotkey still works.
                Err(error) => eprintln!("tray icon not created: {error}"),
            }

            // The window is declared hidden in tauri.conf.json so it can be
            // positioned before its first paint; show it now. Launching an app
            // and seeing nothing at all appear is indistinguishable from a
            // failed launch - the hotkey takes over from here.
            if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
                restore_geometry(&window);
                let _ = window.show();
                let _ = window.set_focus();
            }
            Ok(())
        })
        .on_window_event(|window, event| match event {
            // Closing the overlay hides it; the app keeps running for the next
            // hotkey press. Quit is on the tray menu.
            WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                let _ = window.hide();
                save_geometry(window);
            }
            // Persist geometry when the user is done moving things, rather
            // than on every pixel of a drag.
            WindowEvent::Focused(false) => save_geometry(window),
            _ => {}
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Where the user's settings live.
fn config_path<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> AppResult<PathBuf> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| AppError::Config(format!("no config directory: {e}")))?;
    Ok(dir.join("config.json"))
}

/// Return the writable catalog path, seeding it from the bundled resource on
/// first run.
///
/// The app always reads the app-data copy: the bundled one is read-only (and
/// inside a signed bundle on some platforms), so "refresh item database" needs
/// somewhere else to write.
fn prepare_catalog<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> AppResult<PathBuf> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Internal(format!("no app data directory: {e}")))?;
    std::fs::create_dir_all(&data_dir)?;
    let catalog_path = data_dir.join("items.db");

    if db::is_usable_catalog(&catalog_path) {
        return Ok(catalog_path);
    }

    let bundled = app
        .path()
        .resolve("resources/items.db", tauri::path::BaseDirectory::Resource)
        .ok()
        .filter(|path| db::is_usable_catalog(path));

    match bundled {
        Some(bundled) => {
            std::fs::copy(&bundled, &catalog_path)?;
            eprintln!("seeded catalog from {}", bundled.display());
        }
        // No bundled catalog either. Not fatal: the app opens on the setup
        // screen, which offers to sync one.
        None => eprintln!("no bundled catalog found; the app will prompt for a sync"),
    }
    Ok(catalog_path)
}

/// Put the overlay back where the user last left it.
pub(crate) fn restore_geometry<R: tauri::Runtime>(window: &tauri::WebviewWindow<R>) {
    let Some(state) = window.try_state::<AppState>() else {
        return;
    };
    let Some(saved) = state.config.get().window else {
        return;
    };
    let _ = window.set_position(tauri::PhysicalPosition::new(saved.x, saved.y));
    let _ = window.set_size(tauri::PhysicalSize::new(saved.width, saved.height));
}

/// Remember where and how big the overlay is, so the next show restores it.
pub(crate) fn save_geometry<R: tauri::Runtime>(window: &tauri::Window<R>) {
    let Some(state) = window.try_state::<AppState>() else {
        return;
    };
    let (Ok(position), Ok(size)) = (window.outer_position(), window.inner_size()) else {
        return;
    };
    let _ = state.config.set_window(config::WindowState {
        x: position.x,
        y: position.y,
        width: size.width,
        height: size.height,
    });
}

/// Tray icon. With `skipTaskbar` and no window decorations, this is the only
/// way to reach a hidden overlay if the hotkey is unavailable - and the only
/// way to quit.
fn build_tray<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> tauri::Result<()> {
    let toggle = MenuItem::with_id(app, "toggle", "Show / hide overlay", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&toggle, &quit])?;

    TrayIconBuilder::with_id("main-tray")
        .icon(
            app.default_window_icon()
                .cloned()
                .ok_or_else(|| tauri::Error::AssetNotFound("default window icon".into()))?,
        )
        .tooltip("FFXIV Market Overlay")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "toggle" => hotkey::toggle_overlay(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;
    Ok(())
}
