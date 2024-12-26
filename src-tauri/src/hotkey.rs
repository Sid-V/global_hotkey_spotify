use tauri::State;
use std::collections::HashMap;
use global_hotkey::{
    hotkey::{Code, HotKey, Modifiers},
    GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState,
};
use keyboard_types::Key;
use serde::{Serialize, Deserialize};
use crate::AppState;


#[derive(Debug, Serialize, Deserialize)]
struct HotkeyConfig {
    hotkeys: HashMap<String, HotkeyDefinition>
}

#[derive(Debug, Serialize, Deserialize)]
struct HotkeyDefinition {
    modifiers: Vec<String>,
    key: String
}

impl HotkeyDefinition {
    fn to_hotkey(&self) -> Result<HotKey, String> {
        // Convert modifier strings to Modifiers
        let modifiers = self.modifiers.iter()
            .try_fold(Modifiers::empty(), |acc, modifier| {
                match modifier.to_uppercase().as_str() {
                    "CONTROL" => Ok(acc | Modifiers::CONTROL),
                    "ALT" => Ok(acc | Modifiers::ALT),
                    "SHIFT" => Ok(acc | Modifiers::SHIFT),
                    "META" | "COMMAND" => Ok(acc | Modifiers::META),
                    _ => Err(format!("Unknown modifier: {}", modifier))
                }
            })?;
        
            // Convert key string to Code using FromStr trait
            let key = Code::from_str(&self.key.to_uppercase())
                .map_err(|_| format!("Unknown key: {}", self.key))?;
            
            Ok(HotKey::new(Some(modifiers), key))
        
    }
}

pub fn init_hotkeys() -> GlobalHotKeyManager {
    println!("Initializing hotkeys");
    
    GlobalHotKeyManager::new().unwrap()
}

#[tauri::command]
pub async fn set_hotkeys(state: State<'_, AppState>, playPauseHotkey: String, nextTrackHotkey: String, prevTrackHotkey: String) -> Result<(), String> {
    let hotkeys = get_hotkeys(playPauseHotkey, nextTrackHotkey, prevTrackHotkey)?;
    let guard = state.hotkey_manager.lock().await;
    let hotkey_manager = guard.as_ref().unwrap().inner();
    
    for (name, hotkey) in &hotkeys {
        match hotkey_manager.unregister(*hotkey) {
            Ok(_) => println!("Hotkey {}: {:?} success unregistered", name, hotkey),
            Err(e) => println!("Hotkey {}: {:?} already unregistered", name, hotkey),   
        }
    }

    for (name, hotkey) in &hotkeys {
        match hotkey_manager.register(*hotkey) {
            Ok(_) => println!("Hotkey {}: {:?} success registered", name, hotkey),
            Err(e) => println!("Hotkey {}: {:?} already registered", name, hotkey),   
        }
    }

    Ok(())
}

pub fn get_hotkeys(playPauseHotkey: String, nextTrackHotkey: String, prevTrackHotkey: String) -> Result<HashMap<String, HotKey>, String> {


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
    let parts: Vec<&str> = hotkey_str.split(" + ").map(|s| s.trim()).collect();
    let mut hotkey_def = HotkeyDefinition {
        modifiers: Vec::new(),
        key: String::new()
    };

    for part in parts {
        match part.to_lowercase().as_str() {
            "ctrl" => hotkey_def.modifiers.push("CONTROL".to_string()),
            "alt" => hotkey_def.modifiers.push("ALT".to_string()),
            "shift" => hotkey_def.modifiers.push("SHIFT".to_string()),
            k => {
                // Convert the key string to a HotKey
                if let Ok(code) = Code::from_str(k) {
                    hotkey_def.key = code.to_string();
                } else {
                    return Err(format!("Invalid key: {}", k));
                }
            }
        }
    }
    println!("Parsed hotkey definition - Modifiers: {:?}, Key: {}", hotkey_def.modifiers, hotkey_def.key);

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
            "DIGIT0" => Ok(Code::Digit0),
            "DIGIT1" => Ok(Code::Digit1),
            "DIGIT2" => Ok(Code::Digit2),
            "DIGIT3" => Ok(Code::Digit3),
            "DIGIT4" => Ok(Code::Digit4),
            "DIGIT5" => Ok(Code::Digit5),
            "DIGIT6" => Ok(Code::Digit6),
            "DIGIT7" => Ok(Code::Digit7),
            "DIGIT8" => Ok(Code::Digit8),
            "DIGIT9" => Ok(Code::Digit9),

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
            " " => Ok(Code::Space),
            _ => Err(format!("Unsupported key: {}", s))
        }
    }
}