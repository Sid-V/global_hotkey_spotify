use global_hotkey::{
    hotkey::{Code, HotKey, Modifiers},
    GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState,
};
use crossbeam_channel::TryRecvError;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use std::{cell::RefCell, collections::HashMap, fs, path::PathBuf};
use tauri::{Manager, State};

use crate::{AppState, APP_CACHE_DIR};
use crate::HOTKEY_CACHE;
use crate::{api::AuthResult, next_track, play_pause, prev_track, volume_control_up, volume_control_down};

// Hotkey manager needs to be declared in the same thread as the registration of hotkeys
// So reinforcing that fact that making it thread_local and registering only in the main thread
thread_local! {
    pub static HOTKEY_MANAGER: RefCell<Option<GlobalHotKeyManager>> = RefCell::new(None);
}

// Persist Hotkeys
#[derive(Serialize, Deserialize, Debug)]
struct HotkeyCache {
    string_hotkeys: HashMap<String, String>,
}

impl HotkeyCache {
    fn save_to_file(&self, path: PathBuf) -> Result<(), String> {
        let data = serde_json::to_string(self).map_err(|e| e.to_string())?;
        fs::write(path, data).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn load_from_file(path: PathBuf) -> Result<Self, String> {
        let data = fs::read_to_string(path).map_err(|e| e.to_string())?;
        serde_json::from_str(&data).map_err(|e| e.to_string())
    }
}

// Function to save reversed hotkeys
pub fn save_hotkeys_to_cache(
    string_hotkeys: HashMap<String, String>,
    cache_file_path: PathBuf,
) -> Result<(), String> {
    let cache = HotkeyCache { string_hotkeys };
    cache.save_to_file(cache_file_path)
}

// Function to load reversed hotkeys on app boot
pub fn load_hotkeys_from_cache(cache_file_path: PathBuf) -> HashMap<String, HotKey> {
    if let Ok(cache) = HotkeyCache::load_from_file(cache_file_path) {
        let mut hotkey_map: HashMap<String, HotKey> = HashMap::new();
        let string_hotkeys = cache.string_hotkeys;
        for (name, hotkey_str) in string_hotkeys {
            match parse_hotkey(&hotkey_str) {
                Ok(hotkey) => {
                    hotkey_map.insert(name, hotkey);
                }
                Err(e) => {
                    log::error!("LOAD_HOTKEY_FROM_CACHE: Failed to parse hotkey '{}': {}", hotkey_str, e);
                }
            }
        }
        hotkey_map
    } else {
        HashMap::new() // Return an empty map if loading fails
    }
}

#[tauri::command]
pub async fn return_loaded_hotkeys(
) -> Result<HashMap<String, String>, String> {
    match HotkeyCache::load_from_file(APP_CACHE_DIR.get().expect("hotkey: APP_CACHE_DIR not initialized").join(HOTKEY_CACHE)) {
        Ok(cache) => Ok(cache.string_hotkeys),
        _ => {
            log::error!("Return_Loaded_Hotkeys: Cannot find APP_CACHE_DIR hotkeys");
            Err(format!("Failed to load hotkeys"))
        }
        //Err(e) => Err(format!("Failed to load hotkeys: {}", e)),
    }
}

#[tauri::command]
pub async fn set_hotkeys(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
    play_pause_hotkey: String,
    next_track_hotkey: String,
    prev_track_hotkey: String,
    volume_up_hotkey: String,
    volume_down_hotkey: String,
) -> Result<AuthResult, String> {

    // TODO - need to check if they are empty and skip otherwise
    
    let mut save_hotkeys = HashMap::new();
    save_hotkeys.insert("play_pause".to_string(), play_pause_hotkey.clone());
    save_hotkeys.insert("next_track".to_string(), next_track_hotkey.clone());
    save_hotkeys.insert("prev_track".to_string(), prev_track_hotkey.clone());
    save_hotkeys.insert("volume_up".to_string(), volume_up_hotkey.clone());
    save_hotkeys.insert("volume_down".to_string(), volume_down_hotkey.clone());

    let _ = save_hotkeys_to_cache(save_hotkeys, APP_CACHE_DIR.get().expect("hotkey: APP_CACHE_DIR not initialized").join(HOTKEY_CACHE));
    
    let new_hotkeys: HashMap<String, HotKey> =
        get_hotkeys(play_pause_hotkey, next_track_hotkey, prev_track_hotkey, volume_up_hotkey, volume_down_hotkey)?;
    let cloned_new_hotkeys = new_hotkeys.clone();

    // Lock the hotkey_hashmap and extract data before entering the main thread logic
    let mut hotkey_map_guard = state.hotkey_hashmap.lock().await;

    // Get a mutable reference to the hashmap inside the Mutex
    if let Some(hotkey_map) = hotkey_map_guard.as_mut() {
        // Prepare to unregister the existing hotkeys
        let old_hotkeys: Vec<(String, HotKey)> = hotkey_map.drain().collect();

        let _ = app_handle.run_on_main_thread(move || {
            HOTKEY_MANAGER.with(|manager| {
                if let Some(hotkey_manager) = manager.borrow().as_ref() {
                    // Unregister old hotkeys
                    for (name, hotkey) in old_hotkeys {
                        if let Err(e) = hotkey_manager.unregister(hotkey) {
                            log::error!("Set_Hotkeys: Failed to unregister hotkey '{}': {}", name, e);
                        } else {
                            log::debug!("Set_Hotkeys: Unregistered hotkey '{}'", name);
                        }
                    }

                    // Register new hotkeys
                    for (name, hotkey) in cloned_new_hotkeys {
                        if let Err(e) = hotkey_manager.register(hotkey) {
                            log::error!("Set_Hotkeys: Failed to register hotkey '{}': {}", name, e);
                        } else {
                            log::debug!("Set_Hotkeys: Registered hotkey '{}', {:?}", name, hotkey);
                        }
                    }
                } else {
                    log::error!("Set_Hotkeys: HOTKEY_MANAGER is not initialized!");
                }
            });
        });

        // Update the hashmap with the new hotkeys
        hotkey_map.extend(new_hotkeys);
    } else {
        log::error!("Set_Hotkeys: Hotkey hashmap is not initialized!");
    }

    Ok(AuthResult::Success {
        ok: "ok".to_string(),
    })
}

pub fn get_hotkeys(
    play_pause_hotkey: String,
    next_track_hotkey: String,
    prev_track_hotkey: String,
    volume_up_hotkey: String,
    volume_down_hotkey: String,
) -> Result<HashMap<String, HotKey>, String> {
    let mut hotkeys = HashMap::new();

    if !play_pause_hotkey.is_empty() {
        if let Ok(hotkey) = parse_hotkey(&play_pause_hotkey) {
            hotkeys.insert("play_pause".to_string(), hotkey);
        }
    }

    if !next_track_hotkey.is_empty() {
        if let Ok(hotkey) = parse_hotkey(&next_track_hotkey) {
            hotkeys.insert("next_track".to_string(), hotkey);
        }
    }

    if !prev_track_hotkey.is_empty() {
        if let Ok(hotkey) = parse_hotkey(&prev_track_hotkey) {
            hotkeys.insert("prev_track".to_string(), hotkey);
        }
    }

    if !volume_up_hotkey.is_empty() {
        if let Ok(hotkey) = parse_hotkey(&volume_up_hotkey) {
            hotkeys.insert("volume_up".to_string(), hotkey);
        }
    }

    if !volume_down_hotkey.is_empty() {
        if let Ok(hotkey) = parse_hotkey(&volume_down_hotkey) {
            hotkeys.insert("volume_down".to_string(), hotkey);
        }
    }

    Ok(hotkeys)
}

fn parse_hotkey(hotkey_str: &str) -> Result<HotKey, String> {
    let parts: Vec<&str> = hotkey_str.split(" + ").map(|s| s.trim()).collect();
    let mut modifiers = Modifiers::empty();
    let mut key = String::new();

    for part in parts {
        match part.to_uppercase().as_str() {
            "CTRL" | "CONTROL" => modifiers |= Modifiers::CONTROL,
            "ALT" => modifiers |= Modifiers::ALT,
            "SHIFT" => modifiers |= Modifiers::SHIFT,
            "META" | "COMMAND" => modifiers |= Modifiers::META,
            k => key = k.to_string(),
        }
    }

    let code = <Code as CodeExt>::from_str(&key.to_uppercase()).unwrap();
    let hotkey = HotKey::new(Some(modifiers), code);
    //hotkey.id = rand::random::<u32>();
    Ok(hotkey)
}

pub fn init_hotkeys(app_handle: tauri::AppHandle) {
    let manager = GlobalHotKeyManager::new().expect("Failed to initialize new test manager");
    HOTKEY_MANAGER.with(|m| {
        *m.borrow_mut() = Some(manager)
    });

    let loaded_hotkey_app_handle = app_handle.clone();
    // Load hotkeys from cache, if it exists
    let loaded_hotkeys = load_hotkeys_from_cache(APP_CACHE_DIR.get().expect("hotkey: APP_CACHE_DIR not initialized").join(HOTKEY_CACHE));
    let cloned_loaded_hotkeys = loaded_hotkeys.clone();
    if !loaded_hotkeys.is_empty() {
        // Lock the hotkey_hashmap and extract data before entering the main thread logic

        let _ = app_handle.run_on_main_thread(move || {
            HOTKEY_MANAGER.with(|manager| {
                if let Some(hotkey_manager) = manager.borrow().as_ref() {
                    // Register hotkeys
                    for (name, hotkey) in loaded_hotkeys {
                        if let Err(e) = hotkey_manager.register(hotkey) {
                            log::error!("Init_Hotkeys: Failed to register hotkey '{}': {}", name, e);
                        } else {
                            log::debug!("Init_Hotkeys: Registered hotkey '{}', {:?}", name, hotkey);
                        }
                    }
                } else {
                    log::error!("Init_Hotkeys: HOTKEY_MANAGER is not initialized!");
                }
            });
        });

        tauri::async_runtime::spawn(async move {
            log::debug!("Init_Hotkeys: Adding previously persited hotkeys to Hotkey_map");
            let app_state = loaded_hotkey_app_handle.state::<AppState>();
            let mut hotkey_map_guard = app_state.hotkey_hashmap.lock().await;
            if let Some(hotkey_map) = hotkey_map_guard.as_mut() {
                hotkey_map.extend(cloned_loaded_hotkeys);
            }
        });
    }

    let app_handle_for_hotkey = app_handle.clone();
    // Start new thread to listen to hotkey events
    tauri::async_runtime::spawn(async move {
        log::info!("Init_Hotkeys: Starting hotkey event listener thread");

        let global_hotkey_receiver = GlobalHotKeyEvent::receiver();

        loop {
            match global_hotkey_receiver.try_recv() {
                Ok(event) => {
                    if event.state == HotKeyState::Released {
                        handle_hotkey_event(app_handle_for_hotkey.state(), event.id).await;
                    }
                }
                Err(e) => match e {
                    TryRecvError::Empty => {
                        // No events, sleep briefly to avoid busy-waiting
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                    TryRecvError::Disconnected => {
                        log::error!("Init_Hotkeys: Hotkey receiver disconnected");
                        break;
                    }
                },
            }
        }

        log::info!("Init_Hotkeys: Hotkey event listener thread ended");
    });
}

pub async fn handle_hotkey_event(state: State<'_, AppState>, hotkey_id: u32) {
    // Awaiting the lock to access HOTKEY_HASHMAP
    // let hotkey_map = HOTKEY_HASHMAP.lock().await;

    let hotkey_map_guard = state.hotkey_hashmap.lock().await;
    if let Some(hotkey_map) = hotkey_map_guard.as_ref() {
        // Find the hotkey associated with the triggered event
        if let Some((name, _)) = hotkey_map
            .iter()
            .find(|(_, hotkey)| hotkey.id() == hotkey_id)
        {
            // Perform the action associated with this hotkey
            match name.as_str() {
                "play_pause" => {
                    drop(hotkey_map_guard);
                    if let Err(e) = play_pause(state).await {
                        log::error!("Error in play/pause action: {}", e);
                    }
                }
                "next_track" => {
                    drop(hotkey_map_guard);
                    if let Err(e) = next_track(state).await {
                        log::error!("Error in play/pause action: {}", e);
                    }
                }
                "prev_track" => {
                    drop(hotkey_map_guard);
                    if let Err(e) = prev_track(state).await {
                        log::error!("Error in play/pause action: {}", e);
                    }
                }
                "volume_up" => {
                    drop(hotkey_map_guard);
                    if let Err(e) = volume_control_up(state).await {
                        log::error!("Error in volume up action: {}", e);
                    }
                }
                "volume_down" => {
                    drop(hotkey_map_guard);
                    if let Err(e) = volume_control_down(state).await {
                        log::error!("Error in volume down action: {}", e);
                    }
                }
                _ => {
                    log::error!("Unknown hotkey action for '{}'", name);
                }
            }
        } else {
            log::error!("Hotkey ID '{}' not found in HOTKEY_HASHMAP", hotkey_id);
        }
    }
}

//Extension trait to parse Code from string
trait CodeExt {
    fn from_str(s: &str) -> Result<Code, String>;
}

impl CodeExt for Code {
    fn from_str(s: &str) -> Result<Code, String> {
        match s {
            // Digits
            "0" => Ok(Code::Digit0),
            "1" => Ok(Code::Digit1),
            "2" => Ok(Code::Digit2),
            "3" => Ok(Code::Digit3),
            "4" => Ok(Code::Digit4),
            "5" => Ok(Code::Digit5),
            "6" => Ok(Code::Digit6),
            "7" => Ok(Code::Digit7),
            "8" => Ok(Code::Digit8),
            "9" => Ok(Code::Digit9),

            // Letters
            "A" => Ok(Code::KeyA),
            "B" => Ok(Code::KeyB),
            "C" => Ok(Code::KeyC),
            "D" => Ok(Code::KeyD),
            "E" => Ok(Code::KeyE),
            "F" => Ok(Code::KeyF),
            "G" => Ok(Code::KeyG),
            "H" => Ok(Code::KeyH),
            "I" => Ok(Code::KeyI),
            "J" => Ok(Code::KeyJ),
            "K" => Ok(Code::KeyK),
            "L" => Ok(Code::KeyL),
            "M" => Ok(Code::KeyM),
            "N" => Ok(Code::KeyN),
            "O" => Ok(Code::KeyO),
            "P" => Ok(Code::KeyP),
            "Q" => Ok(Code::KeyQ),
            "R" => Ok(Code::KeyR),
            "S" => Ok(Code::KeyS),
            "T" => Ok(Code::KeyT),
            "U" => Ok(Code::KeyU),
            "V" => Ok(Code::KeyV),
            "W" => Ok(Code::KeyW),
            "X" => Ok(Code::KeyX),
            "Y" => Ok(Code::KeyY),
            "Z" => Ok(Code::KeyZ),

            // Special keys
            "SPACE" => Ok(Code::Space),
            "ENTER" => Ok(Code::Enter),
            "-" => Ok(Code::Minus),
            "=" => Ok(Code::Equal),
            "/" => Ok(Code::Slash),
            "\\" => Ok(Code::Backslash),
            ";" => Ok(Code::Semicolon),
            "'" => Ok(Code::Quote),
            "," => Ok(Code::Comma),
            "." => Ok(Code::Period),
            "[" => Ok(Code::BracketLeft),
            "]" => Ok(Code::BracketRight),
            "`" => Ok(Code::Backquote),
            "Home" => Ok(Code::Home),
            "End" => Ok(Code::End),
            "PageUp" => Ok(Code::PageUp),
            "PageDown" => Ok(Code::PageDown),
            "Delete" => Ok(Code::Delete),
            "Backspace" => Ok(Code::Backspace),
            "Escape" => Ok(Code::Escape),
            "Tab" => Ok(Code::Tab),
            "PrintScreen" => Ok(Code::PrintScreen),
            "ScrollLock" => Ok(Code::ScrollLock),
            "Pause" => Ok(Code::Pause),
            "Insert" => Ok(Code::Insert),
            "NumLock" => Ok(Code::NumLock),
            "F1" => Ok(Code::F1),
            "F2" => Ok(Code::F2),
            "F3" => Ok(Code::F3),
            "F4" => Ok(Code::F4),
            "F5" => Ok(Code::F5),
            "F6" => Ok(Code::F6),
            "F7" => Ok(Code::F7),
            "F8" => Ok(Code::F8),
            "F9" => Ok(Code::F9),
            "F10" => Ok(Code::F10),
            "F11" => Ok(Code::F11),
            "F12" => Ok(Code::F12),
            "F13" => Ok(Code::F13),
            "F14" => Ok(Code::F14),
            "F15" => Ok(Code::F15),
            "F16" => Ok(Code::F16),
            "F17" => Ok(Code::F17),
            "F18" => Ok(Code::F18),
            "F19" => Ok(Code::F19),
            "F20" => Ok(Code::F20),
            _ => {
                log::error!("Unsupported key '{}' was pressed. Please create a new issue in Github.", s);
                Err(format!("The key '{}' is not supported as a hotkey. Please create a new issue in Github.", s))
            }
        }
    }
}
