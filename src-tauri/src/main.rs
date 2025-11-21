#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder,}, tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent}, 
    Manager, WebviewWindow, WindowEvent
};

use global_hotkey::hotkey::HotKey;
use rspotify::AuthCodeSpotify;
use tauri_plugin_log::{Target, TargetKind};
use log::LevelFilter;
use std::{collections::HashMap, path::PathBuf, fs};
use once_cell::sync::OnceCell;

use crate::api::*;
use crate::hotkey::*;

pub mod api;
pub mod hotkey;

pub const HOTKEY_CACHE: &str = ".hotkey_cache.json";
pub const LOGS_FILENAME: &str = "global-hotkey-spotify-logs";
pub static APP_CACHE_DIR: OnceCell<PathBuf> = OnceCell::new();

// Main state of the app
pub struct AppState {
    pub spotify: tokio::sync::Mutex<Option<AuthCodeSpotify>>,
    pub hotkey_hashmap: tokio::sync::Mutex<Option<HashMap<String, HotKey>>>,
    pub volume: tokio::sync::Mutex<u8>,
}

// Implement Default for AppState
impl Default for AppState {
    fn default() -> Self {
        Self {
            spotify: tokio::sync::Mutex::new(Some(init_spotify())),
            hotkey_hashmap: tokio::sync::Mutex::new(Some(HashMap::new())),
            volume: tokio::sync::Mutex::new(50),
        }
    }
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::new().targets([
            Target::new(TargetKind::Stdout),
            Target::new(TargetKind::LogDir { file_name: Some(LOGS_FILENAME.to_string())})
        ])
        .level_for("rspotify_http::reqwest", LevelFilter::Off) // Don't need these large logs to be written to file
        .max_file_size(100000) // 100kb max file size
        .build())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            log::info!("Setting up Tauri...");
            let app_cache_dir = app.path().app_cache_dir().unwrap();
            log::info!("App cache dir: {:?}", app_cache_dir);
            fs::create_dir_all(&app_cache_dir).expect("Failed to create app cache directory");
            APP_CACHE_DIR.set(app_cache_dir.clone()).expect("Failed to set APP_CACHE_DIR");            
            if let Err(e) = ensure_hotkey_cache_file_exists(&app_cache_dir) {
                log::warn!("Failed to initialize hotkey cache file: {}", e);
            }
            
            // Setup autostart on desktop
            #[cfg(desktop)]
            {
                use tauri_plugin_autostart::MacosLauncher;
                use tauri_plugin_autostart::ManagerExt;

                let _ = app.handle().plugin(tauri_plugin_autostart::init(
                    MacosLauncher::LaunchAgent,
                    Some(vec!["--flag1", "--flag2"]),
                ));

                let autostart_manager = app.autolaunch();
                let _ = autostart_manager.enable();
                log::info!("registered for autostart? {}", autostart_manager.is_enabled().unwrap());
            }
            
            // Setup hotkeys manager
            let app_handle_for_hotkey = app.app_handle().clone();
            init_hotkeys(app_handle_for_hotkey);

            // System Tray setup
            let quit = MenuItemBuilder::new("Quit").id("quit").build(app).unwrap();
            let show = MenuItemBuilder::new("Show").id("show").build(app).unwrap();
            let menuitems = MenuBuilder::new(app)
                .items(&[&quit, &show])
                .build()
                .unwrap();

            let main_window = app.get_webview_window("main").unwrap();
            // Don't show taskbar icon
            if let Err(err) = main_window.set_skip_taskbar(true) {
                log::debug!("Failed to mark window as skip_taskbar: {err:?}");
            }

            // If closed, move to systray
            let window_for_events = main_window.clone();
            main_window.on_window_event(move |event| match event {
                WindowEvent::CloseRequested { api, .. } => {
                    api.prevent_close();
                    if let Err(err) = window_for_events.hide() {
                        log::debug!("Failed to hide window on close request: {err:?}");
                    }
                }
                WindowEvent::Focused(false) | WindowEvent::Resized(_) => {
                    if matches!(window_for_events.is_minimized(), Ok(true)) {
                        if let Err(err) = window_for_events.hide() {
                            log::debug!("Failed to hide window after minimize: {err:?}");
                        }
                    }
                }
                _ => {}
            });
                  

            // Tray icon events
            let _ = TrayIconBuilder::new()
                .tooltip("Global Hotkey Spotify")
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menuitems)
                .on_menu_event(move |app, event| match event.id().as_ref() {
                    "quit" => {
                        log::info!("Quitting application through tray exit...");
                        app.exit(0)
                    }
                    "show" => {
                        let window = app.get_webview_window("main").unwrap();
                        reveal_window(&window);
                    }
                    _ => {
                        log::error!("Menu item event: menu item was not handled");
                    }
                })
                .on_tray_icon_event(|tray_icon, event| match event {
                    TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        position,
                        ..
                    } => {
                        // LEFT CLICK BEHAVIOR
                        let window = tray_icon.app_handle().get_webview_window("main").unwrap();
                        reveal_window(&window);
                    }
                    TrayIconEvent::Click {
                        button: MouseButton::Right,
                        button_state: MouseButtonState::Up,
                        ..
                    } => {
                        // RIGHT CLICK BEHAVIOR
                    }
                    TrayIconEvent::DoubleClick {
                        id: _,
                        position: _,
                        rect: _,
                        button: _,
                    } => {
                        // DOUBLE CLICK BEHAVIOR
                    }
                    _ => {}
                })
                .build(app);
            log::info!("Tauri setup complete!");
            Ok(())
        })
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            init_auth,
            handle_callback,
            check_auth_status,            
            play_pause,
            next_track,
            prev_track,
            volume_control_up,
            volume_control_down,
            set_hotkeys,
            return_loaded_hotkeys
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// Helper to show window to screen
fn reveal_window(window: &WebviewWindow) {
    if let Err(err) = window.unminimize() {
        log::debug!("Failed to unminimize window: {err:?}");
    }
    if let Err(err) = window.show() {
        log::debug!("Failed to show window: {err:?}");
    }
    if let Err(err) = window.set_focus() {
        log::debug!("Failed to focus window: {err:?}");
    }
}