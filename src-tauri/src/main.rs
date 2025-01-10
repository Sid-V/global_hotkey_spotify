#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use tauri::{
    menu::{ MenuBuilder, MenuItemBuilder}, 
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent}, 
    Manager, Size, Position, LogicalPosition, LogicalSize, WindowEvent
};
use tokio::sync::Mutex;
use rspotify::AuthCodeSpotify;
use global_hotkey::hotkey::HotKey;
use std::collections::HashMap;

use crate::api::*;
use crate::hotkey::*;
pub mod api;
pub mod hotkey;

// Main state of the app
pub struct AppState {
    pub spotify: tokio::sync::Mutex<Option<AuthCodeSpotify>>,
    pub hotkey_hashmap: tokio::sync::Mutex<Option<HashMap<String, HotKey>>>
}

// Implement Default for AppState
impl Default for AppState {
    fn default() -> Self {
        Self {
            spotify: Mutex::new(Some(init_spotify())),
            hotkey_hashmap: Mutex::new(Some(HashMap::new()))
        }
    }
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            
            // Setup hotkeys manager
            let app_handle_for_hotkey = app.app_handle().clone();    
            init_hotkeys(app_handle_for_hotkey);

            // system tray setup
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
                    "quit" => {
                        app.exit(0)
                    }
                    "show" => {
                        dbg!("menu item show clicked");
                        let window = app.get_webview_window("main").unwrap();
                        window.show().unwrap();
                    }
                    _ => {
                        dbg!("menu item was not handled", event.id);
                    }
                })
                .on_tray_icon_event(|tray_icon, event| match event {
                    //TrayIconEvent::Click { id, position, rect, button, button_state }
                    TrayIconEvent::Click { 
                        button: MouseButton::Left, 
                        button_state: MouseButtonState::Up,
                        position,
                        .. 
                    } => {
                        dbg!("left clicked pressed and released");
                        
                        let window = tray_icon.app_handle().get_webview_window("main").unwrap();

                        let _ = window.show().unwrap();
                        let logical_size = LogicalSize::<f64> {
                            width: 500.00, // TODO - figure out variable sizing?
                            height: 500.00,
                        };
                        let logical_s = Size::Logical(logical_size);
                        let _ = window.set_size(logical_s);
                        let logical_position = LogicalPosition::<f64> {
                            x:  position.x - logical_size.width,
                            y: position.y - logical_size.height - 30.,
                        };
                        let logical_pos: Position =
                            Position::Logical(logical_position);
                        let _ = window.set_position(logical_pos);
                        let _ = window.set_focus();            
                    },
                    TrayIconEvent::Click { 
                        button: MouseButton::Right, 
                        button_state: MouseButtonState::Up,
                        .. 
                    } => {
                        dbg!("right click pressed and released");
                    },
                    TrayIconEvent::DoubleClick { id: _, position: _, rect: _, button: _ } => {
                        dbg!("system tray received double click");
                    },
                    _ => {}
                })
                .build(app);

                Ok(())
        })
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            init_auth,
            handle_callback,
            check_auth_status,
            me,
            play_pause,
            next_track,
            prev_track,
            set_hotkeys
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}