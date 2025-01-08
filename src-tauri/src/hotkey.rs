use global_hotkey::{
    hotkey::{Code, HotKey, Modifiers},
    GlobalHotKeyManager,
    GlobalHotKeyEvent, 
    HotKeyState
};
use std::{collections::HashMap, str::FromStr, cell::RefCell};
use tauri::State;
use crate::{api::AuthResult, play_pause};
use crate::AppState;
use crossbeam_channel::{unbounded, TryRecvError};
use std::time::Duration;
use tokio::time::sleep;
use std::sync::Arc;
use tokio::sync::Mutex;



thread_local! {
    pub static HOTKEY_MANAGER: RefCell<Option<GlobalHotKeyManager>> = RefCell::new(None);
    pub static HOTKEY_HASHMAP: RefCell<HashMap<String, HotKey>> = RefCell::new(HashMap::new());
}

// pub struct HotkeyManager {
//     pub manager: GlobalHotKeyManager,
//     pub hotkeys: HashMap<String, HotKey>,
// }

// unsafe impl Send for HotkeyManager {}
// unsafe impl Sync for HotkeyManager {}


// impl HotkeyManager {
//     pub fn new() -> Self {
//         Self {
//             manager: GlobalHotKeyManager::new().expect("Failed to initialize hotkey manager"),
//             hotkeys: HashMap::new(),
//         }
//     }

//     pub fn register_hotkey(&mut self, name: String, mut hotkey: HotKey) -> Result<(), String> {
//         // If there's an existing hotkey with this name, unregister it
//         if let Some(old_hotkey) = self.hotkeys.get(&name) {
//             if let Err(e) = self.manager.unregister(*old_hotkey) {
//                 println!("Failed to unregister old hotkey: {:?}", e);
//             }
//         }

//         // Register the new hotkey
//         match self.manager.register(hotkey) {
//             Ok(_) => {
//                 self.hotkeys.insert(name, hotkey);
//                 Ok(())
//             }
//             Err(e) => {
//                 Err(format!("Failed to register hotkey: {:?}", e))
//             }
//         }
//     }
// }

#[tauri::command]
pub async fn set_hotkeys(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
    playPauseHotkey: String,
    nextTrackHotkey: String,
    prevTrackHotkey: String,
) -> Result<AuthResult, String> {
    let new_hotkeys = get_hotkeys(playPauseHotkey, nextTrackHotkey, prevTrackHotkey)?;
    //let mut manager = state.hotkey_manager.lock().unwrap();

    println!("new hotkeys received: {:?}", new_hotkeys);
    
    //let app_handle_clone = app_handle.clone();

    let _ = app_handle.run_on_main_thread( move || {
        println!("Thread ID in set_hotkeys: {:?}", std::thread::current().id());
        HOTKEY_MANAGER.with(|manager| {
            if let Some(testmanager) = manager.borrow().as_ref() {
                // Unregister and remove existing hotkeys
                HOTKEY_HASHMAP.with(|hotkey_map| {
                    let mut hotkey_map = hotkey_map.borrow_mut();

                    for (name, hotkey) in hotkey_map.drain() {
                        if let Err(e) = testmanager.unregister(hotkey) {
                            println!("Failed to unregister hotkey '{}': {}", name, e);
                        } else {
                            println!("Unregistered hotkey '{}'", name);
                        }
                    }
                });

                // Register new hotkeys and update the hashmap
                HOTKEY_HASHMAP.with(|hotkey_map| {
                    let mut hotkey_map = hotkey_map.borrow_mut();

                    for (name, hotkey) in new_hotkeys {
                        if let Err(e) = testmanager.register(hotkey) {
                            println!("Failed to register hotkey '{}': {}", name, e);
                        } else {
                            println!("Registered hotkey '{}', {:?}", name, hotkey);
                            hotkey_map.insert(name, hotkey);
                        }
                    }
                });
            } else {
                println!("HOTKEY_MANAGER is not initialized!");
            }
        });
        
    });

    // Update the hotkeys map
    // manager.hotkeys.clear();
    // for (name, hotkey) in new_hotkeys {
    //     manager.hotkeys.insert(name, hotkey);
    // }

    //app_handle.run_on_main_thread(move || {
        // for hotkey in manager.hotkeys.values() {
        //     let _ = manager.manager.unregister(*hotkey);
        //     // HOTKEY_MANAGER.with(|testmanager| {
        //     //     if let Some(testmanager) = testmanager.borrow().as_ref() {
        //     //         testmanager.unregister(*hotkey);
        //     //     }
        //     // }
        // }
    
        // // Register new hotkeys synchronously while holding the lock
        // for (name, hotkey) in &new_hotkeys {
        //     if let Err(e) = manager.manager.register(*hotkey) {
        //         println!("Failed to register hotkey: {}", e);
        //     }

        //     // HOTKEY_MANAGER.with(|testmanager| {
        //     //     if let Some(testmanager) = testmanager.borrow().as_ref() {
        //     //         testmanager.register(*hotkey);
        //     //     }
        //     // }
        // }
    
        // // Clear and update the hotkeys map after the main thread operations
        // manager.hotkeys.clear();
        // for (name, hotkey) in new_hotkeys {
        //     manager.hotkeys.insert(name, hotkey);
        // }


    //});
    // Clear existing hotkeys synchronously while holding the lock

    Ok(AuthResult::Success { ok: "ok".to_string() })
}

pub fn get_hotkeys(
    play_pause_hotkey: String,
    next_track_hotkey: String,
    prev_track_hotkey: String,
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
    println!("Hotkey created: {:?}", hotkey);

    //hotkey.id = rand::random::<u32>();
    Ok(hotkey)
}

pub async fn init_hotkeys(state: State<'_, AppState>, app_handle: tauri::AppHandle) {
    println!("Initializing hotkey manager...");
    println!("Thread ID in init_hotkeys: {:?}", std::thread::current().id());
    let testmanager = GlobalHotKeyManager::new().expect("Failed to initialize new test manager");
    HOTKEY_MANAGER.with(|m| {
        println!("Accessing HOTKEY_MANAGER in thread: {:?}", std::thread::current().id());
        *m.borrow_mut() = Some(testmanager)
    });

    let state_clone = Arc::new(Mutex::new(state.clone()));
    let app_handle_for_hotkey = app_handle.clone();
    // Start new thread to listen to hotkey events
    tauri::async_runtime::spawn(async move {
        println!("Starting hotkey event listener thread");

        let global_hotkey_receiver = GlobalHotKeyEvent::receiver();

        loop {
            match global_hotkey_receiver.try_recv() {
                Ok(event) => {
                    if event.state == HotKeyState::Released {
                        println!("Hotkey released: {}", event.id);
                        // Lock the state before calling handle_hotkey_event
                        let state_locked = state_clone.lock().await;

                        let app_state = state_locked.clone();
                        // Handle the hotkey event with the locked state
                        handle_hotkey_event(app_state, &app_handle_for_hotkey, event.id).await;
                    }
                }
                Err(e) => match e {
                    TryRecvError::Empty => {
                        // No events, sleep briefly to avoid busy-waiting
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                    TryRecvError::Disconnected => {
                        eprintln!("Hotkey receiver disconnected");
                        break;
                    }
                },
            }
        }

        println!("Hotkey event listener thread ended");
    });
}


pub async fn handle_hotkey_event(state: State<'_, AppState>, app_handle: &tauri::AppHandle, hotkey_id: u32) {

    let _ = app_handle.run_on_main_thread( move || {
        // Use block_on to block the main thread until async code is complete
        let _ = tokio::runtime::Runtime::new().unwrap().block_on(async {
            HOTKEY_HASHMAP.with(|hotkey_map| {
                let hotkey_map = hotkey_map.borrow();
    
                println!("all hotkeys in map: {:?}", hotkey_map);
    
                // Find the hotkey associated with the triggered event
                if let Some((name, _)) = hotkey_map.iter().find(|(_, hotkey)| hotkey.id() == hotkey_id) {
                    println!("Hotkey '{}' triggered", name);
    
                    // Perform the action associated with this hotkey
                    match name.as_str() {
                        "play_pause" => {
                            println!("Toggling Play/Pause");
                            // Call play_pause function here
                            match play_pause(state).await {
                                Ok(_) => println!("Successfully toggled play/pause."),
                                Err(e) => println!("Error in play/pause action: {}", e),
                            }
                        }
                        "next_track" => {
                            println!("Skipping to Next Track");
                            // Perform next track action here
                        }
                        "prev_track" => {
                            println!("Rewinding to Previous Track");
                            // Perform previous track action here
                        }
                        _ => {
                            println!("Unknown hotkey action for '{}'", name);
                        }
                    }
                } else {
                    println!("Hotkey ID '{}' not found in HOTKEY_HASHMAP", hotkey_id);
                }
            });
        });
    });
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
            _ => Err(format!("Unsupported key: {}", s)),
        }
    }
}