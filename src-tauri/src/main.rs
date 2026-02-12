#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder,}, tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent}, 
    Manager, WebviewWindow, WindowEvent
};

use global_hotkey::hotkey::HotKey;
use rspotify::AuthCodePkceSpotify;
use tauri_plugin_log::{Target, TargetKind};
use log::LevelFilter;
use std::{collections::HashMap, path::PathBuf, fs, sync::OnceLock};

use crate::api::*;
use crate::hotkey::*;

pub mod api;
pub mod hotkey;

pub const HOTKEY_CACHE: &str = ".hotkey_cache.json";
pub const LOGS_FILENAME: &str = "global-hotkey-spotify-logs";
const MAX_LOG_FILE_SIZE: u128 = 100_000;
pub static APP_CACHE_DIR: OnceLock<PathBuf> = OnceLock::new();

// Main state of the app
pub struct AppState {
    pub spotify: tokio::sync::Mutex<Option<AuthCodePkceSpotify>>,
    pub hotkey_hashmap: tokio::sync::Mutex<Option<HashMap<String, HotKey>>>,
    pub volume: tokio::sync::Mutex<u8>,
}

// Implement Default for AppState
impl Default for AppState {
    fn default() -> Self {
        Self {
            spotify: tokio::sync::Mutex::new(None),
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
        .level_for("rspotify_http::reqwest", LevelFilter::Off)
        .max_file_size(MAX_LOG_FILE_SIZE)
        .build())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            log::info!("Setting up Tauri...");
            let app_cache_dir = app.path().app_cache_dir()
                .map_err(|e| format!("Failed to get app cache directory: {}", e))?;
            log::info!("App cache dir: {:?}", app_cache_dir);
            fs::create_dir_all(&app_cache_dir)
                .map_err(|e| format!("Failed to create app cache directory: {}", e))?;
            APP_CACHE_DIR.set(app_cache_dir.clone())
                .map_err(|_| "APP_CACHE_DIR already set")?;

            // Initialize Spotify client with persistent token cache in app data dir
            let app_data_dir = app.path().app_data_dir()
                .map_err(|e| format!("Failed to get app data directory: {}", e))?;
            fs::create_dir_all(&app_data_dir)
                .map_err(|e| format!("Failed to create app data directory: {}", e))?;
            log::info!("App data dir (spotify token cache): {:?}", app_data_dir);
            let spotify_client = init_spotify(app_data_dir);
            let app_state = app.state::<AppState>();
            *app_state.spotify.blocking_lock() = Some(spotify_client);

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
                log::info!("registered for autostart? {}", autostart_manager.is_enabled().unwrap_or(false));
            }
            
            // Setup hotkeys manager
            let app_handle_for_hotkey = app.app_handle().clone();
            init_hotkeys(app_handle_for_hotkey);

            // System Tray setup
            let quit = MenuItemBuilder::new("Quit").id("quit").build(app)
                .map_err(|e| format!("Failed to build quit menu item: {}", e))?;
            let show = MenuItemBuilder::new("Show").id("show").build(app)
                .map_err(|e| format!("Failed to build show menu item: {}", e))?;
            let menuitems = MenuBuilder::new(app)
                .items(&[&quit, &show])
                .build()
                .map_err(|e| format!("Failed to build menu: {}", e))?;

            let main_window = app.get_webview_window("main")
                .ok_or_else(|| "Failed to get main window".to_string())?;
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
            let tray_icon = app.default_window_icon()
                .ok_or_else(|| "Failed to get default window icon".to_string())?
                .clone();
            let _ = TrayIconBuilder::new()
                .tooltip("Global Hotkey Spotify")
                .icon(tray_icon)
                .menu(&menuitems)
                .on_menu_event(move |app, event| match event.id().as_ref() {
                    "quit" => {
                        log::info!("Quitting application - performing graceful shutdown...");
                        // Signal callback server to stop
                        shutdown_callback_server();
                        log::info!("Callback server shutdown signaled");
                        app.exit(0)
                    }
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            reveal_window(&window);
                        } else {
                            log::error!("Failed to get main window for show action");
                        }
                    }
                    _ => {
                        log::error!("Menu item event: menu item was not handled");
                    }
                })
                .on_tray_icon_event(|tray_icon, event| match event {
                    TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } => {
                        // LEFT CLICK BEHAVIOR
                        if let Some(window) = tray_icon.app_handle().get_webview_window("main") {
                            reveal_window(&window);
                        } else {
                            log::error!("Failed to get main window for tray click");
                        }
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