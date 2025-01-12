#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    LogicalPosition, LogicalSize, Manager, Position, Size, WindowEvent,
};

use tokio::sync::Mutex;
use global_hotkey::hotkey::HotKey;
use rspotify::AuthCodeSpotify;
use tauri_plugin_log::{Target, TargetKind};
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
}

// Implement Default for AppState
impl Default for AppState {
    fn default() -> Self {
        Self {
            spotify: Mutex::new(Some(init_spotify())),
            hotkey_hashmap: Mutex::new(Some(HashMap::new())),
        }
    }
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::new().targets([
            Target::new(TargetKind::Stdout),
            Target::new(TargetKind::LogDir { file_name: Some(LOGS_FILENAME.to_string())})
        ])        
        .build())
        .setup(|app| {
            log::info!("Setting up Tauri...");
            let app_cache_dir = app.path().app_cache_dir().unwrap();
            log::info!("App cache dir: {:?}", app_cache_dir);
            fs::create_dir_all(&app_cache_dir).expect("Failed to create app cache directory");
            APP_CACHE_DIR.set(app_cache_dir.clone()).expect("Failed to set APP_CACHE_DIR");            
            // Setup autostart on desktop
            #[cfg(desktop)]
            {
                use tauri_plugin_autostart::MacosLauncher;
                use tauri_plugin_autostart::ManagerExt;

                let _ = app.handle().plugin(tauri_plugin_autostart::init(
                    MacosLauncher::LaunchAgent,
                    Some(vec!["--flag1", "--flag2"]),
                ));

                // Get the autostart manager
                let autostart_manager = app.autolaunch();
                // Enable autostart
                let _ = autostart_manager.enable();
                // Check enable state
                log::info!("registered for autostart? {}", autostart_manager.is_enabled().unwrap());
                // Disable autostart
                let _ = autostart_manager.disable();
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

            let window = app.get_webview_window("main").unwrap();
            let window_hider = window.clone();
            window.on_window_event(move |event| match event {
                WindowEvent::Focused(false) => {
                    window_hider.hide().unwrap();
                }
                _ => {}
            });

            // Tray icon events
            let _ = TrayIconBuilder::new()
                .tooltip("Spotify Hotkey")
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menuitems)
                .on_menu_event(move |app, event| match event.id().as_ref() {
                    "quit" => app.exit(0),
                    "show" => {
                        let window = app.get_webview_window("main").unwrap();
                        window.show().unwrap();
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
                        let _ = window.show().unwrap();
                        let logical_size = LogicalSize::<f64> {
                            width: 700.00, // TODO - figure out variable sizing?
                            height: 700.00,
                        };
                        let logical_s = Size::Logical(logical_size);
                        let _ = window.set_size(logical_s);
                        let logical_position = LogicalPosition::<f64> {
                            x: position.x - logical_size.width,
                            y: position.y - logical_size.height - 30.,
                        };
                        let logical_pos: Position = Position::Logical(logical_position);
                        let _ = window.set_position(logical_pos);
                        let _ = window.set_focus();
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
            set_hotkeys,
            return_loaded_hotkeys
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}