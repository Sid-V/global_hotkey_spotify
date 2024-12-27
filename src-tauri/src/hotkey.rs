use global_hotkey::{
    hotkey::{Code, HotKey, Modifiers},
    GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState,
};
use keyboard_types::Key;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::State;

use crate::AppState;
use crate::api::AuthResult;

lazy_static! {
    pub static ref REGISTERED_HOTKEYS: Mutex<HashMap<String, HotKey>> = Mutex::new(HashMap::new());
}

#[derive(Debug, Serialize, Deserialize)]
struct HotkeyConfig {
    hotkeys: HashMap<String, HotkeyDefinition>,
}

#[derive(Debug, Serialize, Deserialize)]
struct HotkeyDefinition {
    modifiers: Vec<String>,
    key: String,
}

// TODO - need to unregister all hotkeys before registering new ones

impl HotkeyDefinition {
    fn to_hotkey(&self) -> Result<HotKey, String> {
        println!("Converting hotkey definition to hotkey: {:?}", self);

        // Convert modifier strings to Modifiers
        let modifiers = self
            .modifiers
            .iter()
            .try_fold(Modifiers::empty(), |acc, modifier| {
                match modifier.to_uppercase().as_str() {
                    "CONTROL" => Ok(acc | Modifiers::CONTROL),
                    "ALT" => Ok(acc | Modifiers::ALT),
                    "SHIFT" => Ok(acc | Modifiers::SHIFT),
                    "META" | "COMMAND" => Ok(acc | Modifiers::META),
                    _ => Err(format!("Unknown modifier: {}", modifier)),
                }
            })?;

        // Convert key string to Code using FromStr trait
        let key = Code::from_str(&self.key.to_uppercase())
            .map_err(|_| format!("Unknown key: {}", self.key))?;
        println!("Hotkey created: {:?}", HotKey::new(Some(modifiers), key));
        Ok(HotKey::new(Some(modifiers), key))
    }
}

pub fn init_hotkeys() -> HotkeyManager {
    println!("Initializing hotkeys");

    HotkeyManager::new()
}

pub struct HotkeyManager {
    pub manager: GlobalHotKeyManager,
    pub hotkeys: HashMap<String, HotKey>,
}

unsafe impl Send for HotkeyManager {}
unsafe impl Sync for HotkeyManager {}

impl HotkeyManager {
    pub fn new() -> Self {
        let manager = Self {
            manager: GlobalHotKeyManager::new().expect("Failed to initialize hotkey manager"),
            hotkeys: HashMap::new(),
        };
        
        manager
    }

    pub fn register(&mut self, name: String, hotkey: HotKey) -> Result<(), String> {
        // If we already have a hotkey with this name, unregister it first
        if let Some(old_hotkey) = self.hotkeys.get(&name) {
            println!("Unregistering old hotkey: {:?}", old_hotkey);
            let _ = self.manager.unregister(*old_hotkey);
        }

        // Register the new hotkey
        match self.manager.register(hotkey) {
            Ok(_) => {
                self.hotkeys.insert(name, hotkey);
                Ok(())
            }
            Err(e) => Err(format!("Failed to register hotkey: {:?}", e))
        }
    }

    pub fn clear(&mut self) {
        //self.manager = GlobalHotKeyManager::new().expect("Failed to initialize hotkey manager");
        // Loop through all hotkeys and unregister them
        for (_, hotkey) in self.hotkeys.iter() {
            let _ = self.manager.unregister(*hotkey);
        }
        self.hotkeys.clear();
    }
}

#[tauri::command]
pub async fn set_hotkeys(
    state: State<'_, AppState>,
    playPauseHotkey: String,
    nextTrackHotkey: String,
    prevTrackHotkey: String,
) -> Result<AuthResult, String> {
    let new_hotkeys = get_hotkeys(playPauseHotkey, nextTrackHotkey, prevTrackHotkey)?;
    let mut manager = state.hotkey_manager.lock().map_err(|e| e.to_string())?;
    
    println!("Current hotkeys: {:?}", manager.hotkeys);
    manager.clear();

    //unregister all hotkeys
    for (name, hotkey) in new_hotkeys.iter() {
        let _ = manager.manager.unregister(*hotkey);
    }
    
    for (name, hotkey) in new_hotkeys {
        if let Err(e) = manager.register(name.clone(), hotkey) {
            println!("Failed to register {}: ID:{} | {}", name, hotkey.id, e);
        } else {
            println!("Successfully registered {}: ID:{}", name, hotkey.id);
        }
    }

    println!("Final hotkeys: {:?}", manager.hotkeys);
    
    Ok(AuthResult::Success {
        ok: "ok".to_string(),
    })
}

pub fn get_hotkeys(
    playPauseHotkey: String,
    nextTrackHotkey: String,
    prevTrackHotkey: String,
) -> Result<HashMap<String, HotKey>, String> {
    let mut hotkeys = HashMap::new();

    // Helper function to parse hotkey string

    // Parse each hotkey string
    if let Ok(hotkey) = parse_hotkey(&playPauseHotkey) {
        hotkeys.insert("play_pause".to_string(), hotkey);
    }

    if let Ok(hotkey) = parse_hotkey(&nextTrackHotkey) {
        hotkeys.insert("next_track".to_string(), hotkey);
    }

    if let Ok(hotkey) = parse_hotkey(&prevTrackHotkey) {
        hotkeys.insert("prev_track".to_string(), hotkey);
    }

    Ok(hotkeys)
}

fn parse_hotkey(hotkey_str: &str) -> Result<HotKey, String> {
    println!("Parsing hotkey: {}", hotkey_str);
    let parts: Vec<&str> = hotkey_str.split(" + ").map(|s| s.trim()).collect();
    let mut hotkey_def = HotkeyDefinition {
        modifiers: Vec::new(),
        key: String::new(),
    };

    for part in parts {
        match part {
            "CTRL" => hotkey_def.modifiers.push("CONTROL".to_string()),
            "ALT" => hotkey_def.modifiers.push("ALT".to_string()),
            "SHIFT" => hotkey_def.modifiers.push("SHIFT".to_string()),
            "META" => hotkey_def.modifiers.push("META".to_string()),
            "COMMAND" => hotkey_def.modifiers.push("COMMAND".to_string()),
            "CAPSLOCK" => hotkey_def.modifiers.push("CAPS_LOCK".to_string()),
            k => {
                hotkey_def.key = k.to_string();
            }
        }
    }

    println!(
        "Parsed hotkey definition - Modifiers: {:?}, Key: {}",
        hotkey_def.modifiers, hotkey_def.key
    );

    hotkey_def.to_hotkey()
}

// Extension trait to parse Code from string
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
            _ => Err(format!("Unsupported key: {}", s)),
        }
    }
}
