//! Interfaz grafica del Redragon Stream Deck.
//!
//! Solo la capa de Tauri: los comandos que expone el frontend y el arranque de
//! la aplicacion. Toda la logica del dispositivo vive en `redragon-core`, que
//! no depende de Tauri y es lo que usa `redragon-daemon` para funcionar sin
//! entorno grafico.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::Ordering;
use std::sync::RwLock;
use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{Manager, State};

// El core exporta todo lo que la GUI necesita: tipos de configuracion, acceso
// al dispositivo, widgets y las integraciones con OBS y Twitch.
use redragon_core::*;

use rdev::{listen, Event, EventType, Key};
use std::sync::atomic::AtomicBool;

// ============================================================================
// Global Hotkey System
// ============================================================================

// Registered hotkeys: maps key combination string to (page, button_id)
lazy_static::lazy_static! {
    static ref REGISTERED_HOTKEYS: RwLock<HashMap<String, (usize, u8)>> = RwLock::new(HashMap::new());
    static ref CURRENT_KEYS: RwLock<Vec<Key>> = RwLock::new(Vec::new());
    static ref HOTKEY_RECORDING: AtomicBool = AtomicBool::new(false);
    static ref RECORDED_HOTKEY: RwLock<Vec<Key>> = RwLock::new(Vec::new());
    static ref GLOBAL_CONFIG_PATH: RwLock<Option<PathBuf>> = RwLock::new(None);
    static ref GLOBAL_ICONS_PATH: RwLock<Option<PathBuf>> = RwLock::new(None);
}

// Convert rdev::Key to a readable string
pub fn key_to_string(key: &Key) -> String {
    match key {
        Key::Alt => "Alt".to_string(),
        Key::AltGr => "AltGr".to_string(),
        Key::Backspace => "Backspace".to_string(),
        Key::CapsLock => "CapsLock".to_string(),
        Key::ControlLeft => "Ctrl".to_string(),
        Key::ControlRight => "RCtrl".to_string(),
        Key::Delete => "Delete".to_string(),
        Key::DownArrow => "Down".to_string(),
        Key::End => "End".to_string(),
        Key::Escape => "Esc".to_string(),
        Key::F1 => "F1".to_string(),
        Key::F2 => "F2".to_string(),
        Key::F3 => "F3".to_string(),
        Key::F4 => "F4".to_string(),
        Key::F5 => "F5".to_string(),
        Key::F6 => "F6".to_string(),
        Key::F7 => "F7".to_string(),
        Key::F8 => "F8".to_string(),
        Key::F9 => "F9".to_string(),
        Key::F10 => "F10".to_string(),
        Key::F11 => "F11".to_string(),
        Key::F12 => "F12".to_string(),
        Key::Home => "Home".to_string(),
        Key::LeftArrow => "Left".to_string(),
        Key::MetaLeft => "Super".to_string(),
        Key::MetaRight => "RSuper".to_string(),
        Key::PageDown => "PageDown".to_string(),
        Key::PageUp => "PageUp".to_string(),
        Key::Return => "Enter".to_string(),
        Key::RightArrow => "Right".to_string(),
        Key::ShiftLeft => "Shift".to_string(),
        Key::ShiftRight => "RShift".to_string(),
        Key::Space => "Space".to_string(),
        Key::Tab => "Tab".to_string(),
        Key::UpArrow => "Up".to_string(),
        Key::PrintScreen => "PrintScreen".to_string(),
        Key::ScrollLock => "ScrollLock".to_string(),
        Key::Pause => "Pause".to_string(),
        Key::NumLock => "NumLock".to_string(),
        Key::Insert => "Insert".to_string(),
        Key::KeyA => "A".to_string(),
        Key::KeyB => "B".to_string(),
        Key::KeyC => "C".to_string(),
        Key::KeyD => "D".to_string(),
        Key::KeyE => "E".to_string(),
        Key::KeyF => "F".to_string(),
        Key::KeyG => "G".to_string(),
        Key::KeyH => "H".to_string(),
        Key::KeyI => "I".to_string(),
        Key::KeyJ => "J".to_string(),
        Key::KeyK => "K".to_string(),
        Key::KeyL => "L".to_string(),
        Key::KeyM => "M".to_string(),
        Key::KeyN => "N".to_string(),
        Key::KeyO => "O".to_string(),
        Key::KeyP => "P".to_string(),
        Key::KeyQ => "Q".to_string(),
        Key::KeyR => "R".to_string(),
        Key::KeyS => "S".to_string(),
        Key::KeyT => "T".to_string(),
        Key::KeyU => "U".to_string(),
        Key::KeyV => "V".to_string(),
        Key::KeyW => "W".to_string(),
        Key::KeyX => "X".to_string(),
        Key::KeyY => "Y".to_string(),
        Key::KeyZ => "Z".to_string(),
        Key::Num0 => "0".to_string(),
        Key::Num1 => "1".to_string(),
        Key::Num2 => "2".to_string(),
        Key::Num3 => "3".to_string(),
        Key::Num4 => "4".to_string(),
        Key::Num5 => "5".to_string(),
        Key::Num6 => "6".to_string(),
        Key::Num7 => "7".to_string(),
        Key::Num8 => "8".to_string(),
        Key::Num9 => "9".to_string(),
        Key::Kp0 => "KP0".to_string(),
        Key::Kp1 => "KP1".to_string(),
        Key::Kp2 => "KP2".to_string(),
        Key::Kp3 => "KP3".to_string(),
        Key::Kp4 => "KP4".to_string(),
        Key::Kp5 => "KP5".to_string(),
        Key::Kp6 => "KP6".to_string(),
        Key::Kp7 => "KP7".to_string(),
        Key::Kp8 => "KP8".to_string(),
        Key::Kp9 => "KP9".to_string(),
        Key::KpMinus => "KP-".to_string(),
        Key::KpPlus => "KP+".to_string(),
        Key::KpMultiply => "KP*".to_string(),
        Key::KpDivide => "KP/".to_string(),
        Key::KpDelete => "KP.".to_string(),
        Key::KpReturn => "KPEnter".to_string(),
        Key::Minus => "-".to_string(),
        Key::Equal => "=".to_string(),
        Key::LeftBracket => "[".to_string(),
        Key::RightBracket => "]".to_string(),
        Key::SemiColon => ";".to_string(),
        Key::Quote => "'".to_string(),
        Key::BackQuote => "`".to_string(),
        Key::BackSlash => "\\".to_string(),
        Key::Comma => ",".to_string(),
        Key::Dot => ".".to_string(),
        Key::Slash => "/".to_string(),
        Key::Unknown(code) => format!("Key{}", code),
        _ => format!("{:?}", key),
    }
}

// Check if a key is a modifier
pub fn is_modifier(key: &Key) -> bool {
    matches!(
        key,
        Key::Alt
            | Key::AltGr
            | Key::ControlLeft
            | Key::ControlRight
            | Key::ShiftLeft
            | Key::ShiftRight
            | Key::MetaLeft
            | Key::MetaRight
    )
}

// Convert current pressed keys to a normalized hotkey string
pub fn keys_to_hotkey_string(keys: &[Key]) -> String {
    let mut modifiers: Vec<&str> = Vec::new();
    let mut regular_keys: Vec<String> = Vec::new();

    for key in keys {
        if is_modifier(key) {
            let mod_name = match key {
                Key::ControlLeft | Key::ControlRight => "Ctrl",
                Key::ShiftLeft | Key::ShiftRight => "Shift",
                Key::Alt | Key::AltGr => "Alt",
                Key::MetaLeft | Key::MetaRight => "Super",
                _ => continue,
            };
            if !modifiers.contains(&mod_name) {
                modifiers.push(mod_name);
            }
        } else {
            regular_keys.push(key_to_string(key));
        }
    }

    // Sort modifiers in consistent order: Ctrl+Shift+Alt+Super
    let order = ["Ctrl", "Shift", "Alt", "Super"];
    modifiers.sort_by_key(|m| order.iter().position(|o| o == m).unwrap_or(99));

    let mut result: Vec<String> = modifiers.iter().map(|s| s.to_string()).collect();
    result.extend(regular_keys);
    result.join("+")
}

// Start the global keyboard listener
pub fn start_keyboard_listener(config_path: PathBuf, icons_path: PathBuf) {
    // Store paths globally for use in the callback
    if let Ok(mut path) = GLOBAL_CONFIG_PATH.write() {
        *path = Some(config_path.clone());
    }
    if let Ok(mut path) = GLOBAL_ICONS_PATH.write() {
        *path = Some(icons_path.clone());
    }

    thread::spawn(move || {
        debug_log!("Global keyboard listener started");

        if let Err(e) = listen(move |event: Event| {
            match event.event_type {
                EventType::KeyPress(key) => {
                    // Add key to current pressed keys
                    if let Ok(mut keys) = CURRENT_KEYS.write() {
                        if !keys.contains(&key) {
                            keys.push(key);
                        }

                        // If recording, update recorded keys
                        if HOTKEY_RECORDING.load(Ordering::Relaxed) {
                            if let Ok(mut recorded) = RECORDED_HOTKEY.write() {
                                if !recorded.contains(&key) {
                                    recorded.push(key);
                                }
                            }
                        } else {
                            // Check if current combination matches a registered hotkey
                            let hotkey_str = keys_to_hotkey_string(&keys);
                            if !hotkey_str.is_empty() {
                                if let Ok(hotkeys) = REGISTERED_HOTKEYS.read() {
                                    if let Some((page, button_id)) = hotkeys.get(&hotkey_str) {
                                        debug_log!(
                                            "Hotkey triggered: {} -> page {}, button {}",
                                            hotkey_str,
                                            page,
                                            button_id
                                        );
                                        // Execute the button action
                                        if let Ok(cfg_path) = GLOBAL_CONFIG_PATH.read() {
                                            if let Ok(icn_path) = GLOBAL_ICONS_PATH.read() {
                                                if let (Some(cp), Some(ip)) =
                                                    (cfg_path.as_ref(), icn_path.as_ref())
                                                {
                                                    trigger_hotkey_action(
                                                        *page, *button_id, cp, ip,
                                                    );
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                EventType::KeyRelease(key) => {
                    // Remove key from current pressed keys
                    if let Ok(mut keys) = CURRENT_KEYS.write() {
                        keys.retain(|k| k != &key);
                    }
                }
                _ => {}
            }
        }) {
            eprintln!("ERROR: Keyboard listener failed: {:?}", e);
        }
    });
}

// Trigger action for a hotkey-activated button
pub fn trigger_hotkey_action(page: usize, button_id: u8, config_path: &PathBuf, icons_path: &PathBuf) {
    // Read config to get the button command
    let config: Config = match fs::read_to_string(config_path) {
        Ok(content) => match serde_json::from_str(&content) {
            Ok(c) => c,
            Err(_) => return,
        },
        Err(_) => return,
    };

    // Get the specific page and button
    if let Some(target_page) = config.pages.get(page) {
        if let Some(button) = target_page.buttons.get(&button_id.to_string()) {
            if !button.command.is_empty() {
                // Extract the actual command (remove __HOTKEY_ prefix if present)
                let cmd = if button.command.starts_with("__HOTKEY_") {
                    // Find the command after the hotkey definition
                    // Format: __HOTKEY_Ctrl+F1__command_here or just __HOTKEY_Ctrl+F1__
                    if let Some(idx) = button.command[9..].find("__") {
                        let after_hotkey = &button.command[9 + idx + 2..];
                        if after_hotkey.is_empty() {
                            return; // No command after hotkey
                        }
                        after_hotkey.to_string()
                    } else {
                        return; // Malformed hotkey command
                    }
                } else {
                    button.command.clone()
                };

                debug_log!("Executing hotkey command: {}", cmd);

                // Execute the command in a new thread
                let config_path_clone = config_path.clone();
                let icons_path_clone = icons_path.clone();
                thread::spawn(move || {
                    execute_hotkey_command(&cmd, &config_path_clone, &icons_path_clone);
                });
            }
        }
    }
}

// Execute a command from hotkey (reuses existing command logic)
pub fn execute_hotkey_command(cmd: &str, config_path: &PathBuf, icons_path: &PathBuf) {
    // Handle __URL_ command
    if cmd.starts_with("__URL_") {
        let url = &cmd[6..];
        Command::new("xdg-open").arg(url).spawn().ok();
        return;
    }

    // Handle __TYPE_ command
    if cmd.starts_with("__TYPE_") {
        let text = &cmd[7..];
        ydotool_command().args(["type", text]).spawn().ok();
        return;
    }

    // Handle __KEY_ command
    if cmd.starts_with("__KEY_") {
        let keys = &cmd[6..];
        execute_hotkey(keys);
        return;
    }

    // Handle page navigation
    if cmd == "__NEXT_PAGE__" || cmd == "__PREV_PAGE__" || cmd.starts_with("__PAGE_") {
        // Read config to get page count
        if let Ok(content) = fs::read_to_string(config_path) {
            if let Ok(config) = serde_json::from_str::<Config>(&content) {
                let new_page = if cmd == "__NEXT_PAGE__" {
                    (config.current_page + 1) % config.pages.len()
                } else if cmd == "__PREV_PAGE__" {
                    if config.current_page == 0 {
                        config.pages.len() - 1
                    } else {
                        config.current_page - 1
                    }
                } else if cmd.starts_with("__PAGE_") && cmd.ends_with("__") {
                    cmd[7..cmd.len() - 2]
                        .parse::<usize>()
                        .unwrap_or(config.current_page)
                } else {
                    return;
                };
                change_page(new_page, config_path, icons_path);
            }
        }
        return;
    }

    // Normal shell command
    Command::new("sh").arg("-c").arg(cmd).spawn().ok();
}

// Load registered hotkeys from config
pub fn load_hotkeys_from_config(config_path: &PathBuf) {
    let config: Config = match fs::read_to_string(config_path) {
        Ok(content) => match serde_json::from_str(&content) {
            Ok(c) => c,
            Err(_) => return,
        },
        Err(_) => return,
    };

    if let Ok(mut hotkeys) = REGISTERED_HOTKEYS.write() {
        hotkeys.clear();

        for (page_idx, page) in config.pages.iter().enumerate() {
            for (button_id_str, button) in &page.buttons {
                if button.command.starts_with("__HOTKEY_") {
                    // Extract hotkey combination: __HOTKEY_Ctrl+F1__...
                    let hotkey_part = &button.command[9..];
                    if let Some(end_idx) = hotkey_part.find("__") {
                        let hotkey_str = &hotkey_part[..end_idx];
                        if let Ok(button_id) = button_id_str.parse::<u8>() {
                            debug_log!(
                                "Registered hotkey '{}' for page {} button {}",
                                hotkey_str,
                                page_idx,
                                button_id
                            );
                            hotkeys.insert(hotkey_str.to_string(), (page_idx, button_id));
                        }
                    }
                }
            }
        }
    }
}


// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
fn get_config(state: State<AppState>) -> Result<Config, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    Ok(config.clone())
}

#[tauri::command]
fn save_full_config(state: State<AppState>, config: Config) -> Result<(), String> {
    let mut current = state.config.lock().map_err(|e| e.to_string())?;
    *current = config;
    drop(current);
    state.save_config();
    Ok(())
}

#[tauri::command]
fn get_app_profiles(state: State<AppState>) -> Result<std::collections::HashMap<String, usize>, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    Ok(config.app_profiles.clone())
}

#[tauri::command]
fn save_app_profiles(state: State<AppState>, app_profiles: std::collections::HashMap<String, usize>) -> Result<(), String> {
    let mut config = state.config.lock().map_err(|e| e.to_string())?;
    config.app_profiles = app_profiles;
    drop(config);
    state.save_config();
    Ok(())
}

#[tauri::command]
fn get_status(state: State<AppState>) -> StatusResponse {
    let connected = state.device_connected.lock().map(|c| *c).unwrap_or(false);
    StatusResponse { connected }
}

#[tauri::command]
fn connect_device(state: State<AppState>) -> Result<bool, String> {
    // Try to find and connect to the device
    let context = match Context::new() {
        Ok(c) => c,
        Err(e) => return Err(format!("USB context error: {}", e)),
    };

    let devices = match context.devices() {
        Ok(d) => d,
        Err(e) => return Err(format!("Could not list USB devices: {}", e)),
    };

    for device in devices.iter() {
        let desc = match device.device_descriptor() {
            Ok(d) => d,
            Err(_) => continue,
        };

        if desc.vendor_id() == VENDOR_ID && desc.product_id() == PRODUCT_ID {
            // Found the device!
            if let Ok(mut dev_state) = state.device_connected.lock() {
                *dev_state = true;
            }
            return Ok(true);
        }
    }

    // Device not found
    if let Ok(mut dev_state) = state.device_connected.lock() {
        *dev_state = false;
    }
    Ok(false)
}

#[tauri::command]
fn set_page(state: State<AppState>, index: usize) -> Result<(), String> {
    let mut config = state.config.lock().map_err(|e| e.to_string())?;
    if index < config.pages.len() {
        config.current_page = index;
    }
    drop(config);
    state.save_config();
    Ok(())
}

#[tauri::command]
fn add_page(state: State<AppState>, name: String) -> Result<usize, String> {
    let mut config = state.config.lock().map_err(|e| e.to_string())?;

    let mut buttons = HashMap::new();
    for i in 1..=15 {
        buttons.insert(
            i.to_string(),
            ButtonConfig {
                label: String::new(),
                command: String::new(),
                color: "#1a1a2e".to_string(),
                icon: String::new(),
            },
        );
    }

    config.pages.push(Page { name, buttons });
    let new_index = config.pages.len() - 1;
    drop(config);
    state.save_config();

    Ok(new_index)
}

#[tauri::command]
fn delete_page(state: State<AppState>, index: usize) -> Result<(), String> {
    let mut config = state.config.lock().map_err(|e| e.to_string())?;

    if config.pages.len() <= 1 {
        return Err("Cannot delete the last page".to_string());
    }

    if index < config.pages.len() {
        config.pages.remove(index);
        if config.current_page >= config.pages.len() {
            config.current_page = config.pages.len() - 1;
        }
    }
    drop(config);
    state.save_config();

    Ok(())
}

#[tauri::command]
fn update_page_name(state: State<AppState>, index: usize, name: String) -> Result<(), String> {
    let mut config = state.config.lock().map_err(|e| e.to_string())?;

    if index < config.pages.len() {
        config.pages[index].name = name;
    }
    drop(config);
    state.save_config();

    Ok(())
}

#[tauri::command]
fn update_button(
    state: State<AppState>,
    page_index: usize,
    button_id: String,
    button_config: ButtonConfig,
) -> Result<(), String> {
    let mut config = state.config.lock().map_err(|e| e.to_string())?;

    if page_index < config.pages.len() {
        config.pages[page_index]
            .buttons
            .insert(button_id, button_config);
    }
    drop(config);
    state.save_config();

    Ok(())
}

#[tauri::command]
fn set_brightness_level(state: State<AppState>, brightness: u8) -> Result<(), String> {
    let mut config = state.config.lock().map_err(|e| e.to_string())?;
    config.brightness = brightness;
    drop(config);
    state.save_config();

    // Signal the button listener to refresh (which will apply new brightness)
    request_refresh();
    debug_log!("Brightness set to {}, refresh requested", brightness);

    Ok(())
}

#[tauri::command]
fn clear_page_buttons(state: State<AppState>, page_index: usize) -> Result<(), String> {
    let mut config = state.config.lock().map_err(|e| e.to_string())?;

    if page_index >= config.pages.len() {
        return Err("Invalid page index".to_string());
    }

    // Reset all buttons on the page to default
    for i in 1..=15 {
        config.pages[page_index].buttons.insert(
            i.to_string(),
            ButtonConfig {
                label: String::new(),
                command: String::new(),
                color: "#1a1a2e".to_string(),
                icon: String::new(),
            },
        );
    }

    drop(config);
    state.save_config();

    Ok(())
}

#[tauri::command]
fn run_command(command: String) -> Result<(), String> {
    if command.is_empty() {
        return Ok(());
    }

    // Handle special commands (same logic as handle_button_press)
    let cmd = command.clone();

    // Handle __URL_ command
    if cmd.starts_with("__URL_") {
        let url = cmd[6..].to_string();
        std::thread::spawn(move || {
            Command::new("xdg-open").arg(&url).spawn().ok();
        });
        return Ok(());
    }

    // Handle __TYPE_ command
    if cmd.starts_with("__TYPE_") {
        let text = cmd[7..].to_string();
        std::thread::spawn(move || {
            ydotool_command().args(["type", &text]).spawn().ok();
        });
        return Ok(());
    }

    // Handle __KEY_ command
    if cmd.starts_with("__KEY_") {
        let keys = cmd[6..].to_string();
        std::thread::spawn(move || {
            execute_hotkey_sync(&keys);
        });
        return Ok(());
    }

    // Handle __MULTI_ command
    if cmd.starts_with("__MULTI_") {
        let commands = cmd[8..].to_string();
        std::thread::spawn(move || {
            for single_cmd in commands.split(";;") {
                let trimmed = single_cmd.trim();
                if !trimmed.is_empty() {
                    if trimmed.starts_with("__URL_") {
                        let url = &trimmed[6..];
                        Command::new("xdg-open").arg(url).spawn().ok();
                    } else if trimmed.starts_with("__TYPE_") {
                        let text = &trimmed[7..];
                        ydotool_command().args(["type", text]).status().ok();
                    } else if trimmed.starts_with("__KEY_") {
                        let keys = &trimmed[6..];
                        execute_hotkey_sync(keys);
                    } else if trimmed.starts_with("__DELAY_") {
                        if let Ok(ms) = trimmed[8..].parse::<u64>() {
                            std::thread::sleep(Duration::from_millis(ms));
                        }
                    } else {
                        Command::new("sh").arg("-c").arg(trimmed).status().ok();
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
        });
        return Ok(());
    }

    // Handle Spotify control commands
    if cmd == "__SPOTIFY_PLAY__" {
        spotify_control("play");
        return Ok(());
    }
    if cmd == "__SPOTIFY_PAUSE__" {
        spotify_control("pause");
        return Ok(());
    }
    if cmd == "__SPOTIFY_TOGGLE__" {
        spotify_control("toggle");
        return Ok(());
    }
    if cmd == "__SPOTIFY_NEXT__" {
        spotify_control("next");
        return Ok(());
    }
    if cmd == "__SPOTIFY_PREV__" {
        spotify_control("prev");
        return Ok(());
    }

    // Handle __PROFILE_ command (manual profile switch)
    if cmd.starts_with("__PROFILE_") {
        let profile_name = cmd[10..].to_string();
        handle_profile_switch(&profile_name);
        return Ok(());
    }

    // Execute normal shell command
    std::thread::spawn(move || {
        Command::new("sh").arg("-c").arg(&command).spawn().ok();
    });

    Ok(())
}

#[tauri::command]
fn refresh_device(_state: State<AppState>) -> Result<(), String> {
    // Signal the button listener to refresh the page
    request_refresh();
    Ok(())
}

#[tauri::command]
fn load_current_page(_state: State<AppState>) -> Result<(), String> {
    // Signal the button listener to refresh the page
    request_refresh();
    Ok(())
}

#[tauri::command]
fn get_icons_path(state: State<AppState>) -> String {
    state.icons_path.to_string_lossy().to_string()
}

/// Where install.sh puts the rule, plus the locations older versions used.
///
/// The 60- prefix matters: uaccess is applied by 73-seat-late.rules, so a rule
/// numbered 99- is evaluated afterwards and the tag never takes effect. The old
/// 99- rules only appeared to work because they also set MODE="0666".
const UDEV_RULE_PATH: &str = "/etc/udev/rules.d/60-redragon-streamdeck.rules";
const LEGACY_UDEV_RULE_PATHS: &[&str] = &[
    "/etc/udev/rules.d/99-redragon-streamdeck.rules",
    "/etc/udev/rules.d/99-redragon.rules",
];

#[tauri::command]
fn setup_udev_rules() -> Result<bool, String> {
    // TAG+="uaccess" grants the active local session access, which is all this
    // needs. MODE="0666" — what previous versions wrote — additionally made the
    // device writable by every user on the machine, for no benefit.
    let rules_content = format!(
        r#"SUBSYSTEM=="usb", ATTR{{idVendor}}=="{:04x}", ATTR{{idProduct}}=="{:04x}", TAG+="uaccess""#,
        VENDOR_ID, PRODUCT_ID
    );

    if check_udev_rules() {
        return Ok(true);
    }

    // Installing this is install.sh's job; this command is the fallback for
    // people who grabbed a prebuilt binary and never ran it.
    let result = Command::new("pkexec")
        .args([
            "bash",
            "-c",
            &format!(
                "printf '%s\\n' '{}' > {} && udevadm control --reload-rules && udevadm trigger",
                rules_content, UDEV_RULE_PATH
            ),
        ])
        .status();

    match result {
        Ok(status) => Ok(status.success()),
        Err(e) => Err(format!("Failed to setup udev rules: {}", e)),
    }
}

#[tauri::command]
fn check_udev_rules() -> bool {
    std::path::Path::new(UDEV_RULE_PATH).exists()
        || LEGACY_UDEV_RULE_PATHS
            .iter()
            .any(|path| std::path::Path::new(path).exists())
}

#[tauri::command]
fn save_icon(
    state: State<AppState>,
    source_path: String,
    icon_name: String,
) -> Result<String, String> {
    let source = PathBuf::from(&source_path);
    if !source.exists() {
        return Err("Source file does not exist".to_string());
    }

    // Create icons directory if it doesn't exist
    fs::create_dir_all(&state.icons_path).ok();

    // Generate unique icon name if needed
    let final_name = if icon_name.is_empty() {
        format!("custom_{}.png", chrono_lite())
    } else {
        icon_name
    };

    let dest = state.icons_path.join(&final_name);
    fs::copy(&source, &dest).map_err(|e| format!("Failed to copy icon: {}", e))?;

    Ok(final_name)
}


#[tauri::command]
fn reset_config(state: State<AppState>) -> Result<(), String> {
    // Reset to default config
    let default_config = AppState::default_config();

    let mut config = state.config.lock().map_err(|e| e.to_string())?;
    *config = default_config;
    drop(config);

    state.save_config();

    // Clear icons folder
    if state.icons_path.exists() {
        fs::remove_dir_all(&state.icons_path).ok();
        fs::create_dir_all(&state.icons_path).ok();
    }

    Ok(())
}

#[tauri::command]
fn list_icons(state: State<AppState>) -> Vec<String> {
    let mut icons = Vec::new();
    if let Ok(entries) = fs::read_dir(&state.icons_path) {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if name.ends_with(".png") || name.ends_with(".jpg") || name.ends_with(".jpeg") {
                    icons.push(name.to_string());
                }
            }
        }
    }
    icons.sort();
    icons
}

#[tauri::command]
fn get_icon_data(state: State<AppState>, filename: String) -> Result<String, String> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};

    let icon_path = state.icons_path.join(&filename);
    if !icon_path.exists() {
        return Err(format!("Icon not found: {}", filename));
    }

    let data = fs::read(&icon_path).map_err(|e| format!("Failed to read icon: {}", e))?;

    let mime = if filename.ends_with(".png") {
        "image/png"
    } else if filename.ends_with(".jpg") || filename.ends_with(".jpeg") {
        "image/jpeg"
    } else if filename.ends_with(".gif") {
        "image/gif"
    } else if filename.ends_with(".webp") {
        "image/webp"
    } else {
        "application/octet-stream"
    };

    let base64_data = STANDARD.encode(&data);
    Ok(format!("data:{};base64,{}", mime, base64_data))
}

#[tauri::command]
fn get_preset_commands() -> Vec<(String, String, String)> {
    vec![
        // Multimedia
        (
            "Vol +".to_string(),
            "wpctl set-volume @DEFAULT_AUDIO_SINK@ 5%+".to_string(),
            "Subir volumen".to_string(),
        ),
        (
            "Vol -".to_string(),
            "wpctl set-volume @DEFAULT_AUDIO_SINK@ 5%-".to_string(),
            "Bajar volumen".to_string(),
        ),
        (
            "Mute".to_string(),
            "wpctl set-mute @DEFAULT_AUDIO_SINK@ toggle".to_string(),
            "Silenciar/Activar audio".to_string(),
        ),
        (
            "Play/Pause".to_string(),
            "playerctl play-pause".to_string(),
            "Reproducir/Pausar media".to_string(),
        ),
        (
            "Next".to_string(),
            "playerctl next".to_string(),
            "Siguiente pista".to_string(),
        ),
        (
            "Prev".to_string(),
            "playerctl previous".to_string(),
            "Pista anterior".to_string(),
        ),
        // Apps comunes
        (
            "Firefox".to_string(),
            "firefox".to_string(),
            "Navegador Firefox".to_string(),
        ),
        (
            "Chrome".to_string(),
            "google-chrome-stable || chromium".to_string(),
            "Navegador Chrome/Chromium".to_string(),
        ),
        (
            "Terminal".to_string(),
            "kitty || alacritty || gnome-terminal".to_string(),
            "Terminal".to_string(),
        ),
        (
            "Files".to_string(),
            "thunar || nautilus || dolphin".to_string(),
            "Administrador de archivos".to_string(),
        ),
        (
            "VS Code".to_string(),
            "code || codium".to_string(),
            "Visual Studio Code".to_string(),
        ),
        (
            "Discord".to_string(),
            "discord".to_string(),
            "Discord".to_string(),
        ),
        (
            "Spotify".to_string(),
            "spotify".to_string(),
            "Spotify".to_string(),
        ),
        (
            "Steam".to_string(),
            "steam".to_string(),
            "Steam".to_string(),
        ),
        (
            "OBS".to_string(),
            "obs".to_string(),
            "OBS Studio".to_string(),
        ),
        // URLs - Abrir páginas web
        (
            "YouTube".to_string(),
            "__URL_https://youtube.com".to_string(),
            "Abrir YouTube".to_string(),
        ),
        (
            "Twitch".to_string(),
            "__URL_https://twitch.tv".to_string(),
            "Abrir Twitch".to_string(),
        ),
        (
            "GitHub".to_string(),
            "__URL_https://github.com".to_string(),
            "Abrir GitHub".to_string(),
        ),
        (
            "Twitter/X".to_string(),
            "__URL_https://x.com".to_string(),
            "Abrir Twitter/X".to_string(),
        ),
        (
            "ChatGPT".to_string(),
            "__URL_https://chat.openai.com".to_string(),
            "Abrir ChatGPT".to_string(),
        ),
        (
            "Claude".to_string(),
            "__URL_https://claude.ai".to_string(),
            "Abrir Claude AI".to_string(),
        ),
        // Hotkeys - Atajos de teclado
        (
            "Copiar".to_string(),
            "__KEY_ctrl+c".to_string(),
            "Ctrl+C - Copiar".to_string(),
        ),
        (
            "Pegar".to_string(),
            "__KEY_ctrl+v".to_string(),
            "Ctrl+V - Pegar".to_string(),
        ),
        (
            "Cortar".to_string(),
            "__KEY_ctrl+x".to_string(),
            "Ctrl+X - Cortar".to_string(),
        ),
        (
            "Deshacer".to_string(),
            "__KEY_ctrl+z".to_string(),
            "Ctrl+Z - Deshacer".to_string(),
        ),
        (
            "Rehacer".to_string(),
            "__KEY_ctrl+shift+z".to_string(),
            "Ctrl+Shift+Z - Rehacer".to_string(),
        ),
        (
            "Guardar".to_string(),
            "__KEY_ctrl+s".to_string(),
            "Ctrl+S - Guardar".to_string(),
        ),
        (
            "Buscar".to_string(),
            "__KEY_ctrl+f".to_string(),
            "Ctrl+F - Buscar".to_string(),
        ),
        (
            "Seleccionar todo".to_string(),
            "__KEY_ctrl+a".to_string(),
            "Ctrl+A - Seleccionar todo".to_string(),
        ),
        (
            "Cerrar ventana".to_string(),
            "__KEY_alt+f4".to_string(),
            "Alt+F4 - Cerrar ventana".to_string(),
        ),
        (
            "Cambiar ventana".to_string(),
            "__KEY_alt+tab".to_string(),
            "Alt+Tab - Cambiar ventana".to_string(),
        ),
        (
            "Pantalla completa".to_string(),
            "__KEY_f11".to_string(),
            "F11 - Pantalla completa".to_string(),
        ),
        (
            "Emoji picker".to_string(),
            "__KEY_super+period".to_string(),
            "Super+. - Selector de emojis".to_string(),
        ),
        // Texto predefinido
        (
            "Email".to_string(),
            "__TYPE_tucorreo@ejemplo.com".to_string(),
            "Escribir email (editar)".to_string(),
        ),
        (
            "Saludo".to_string(),
            "__TYPE_¡Hola! ¿Cómo estás?".to_string(),
            "Escribir saludo".to_string(),
        ),
        (
            "Firma".to_string(),
            "__TYPE_Saludos cordiales".to_string(),
            "Escribir firma".to_string(),
        ),
        // Multi-acciones
        (
            "Abrir+Escribir".to_string(),
            "__MULTI_firefox;;__DELAY_2000;;__TYPE_https://google.com".to_string(),
            "Abrir Firefox y escribir URL".to_string(),
        ),
        (
            "Copy+Paste".to_string(),
            "__MULTI___KEY_ctrl+c;;__DELAY_500;;__KEY_ctrl+v".to_string(),
            "Copiar y pegar".to_string(),
        ),
        // Widgets - Fecha/Hora
        (
            "Reloj".to_string(),
            "__CLOCK__".to_string(),
            "Muestra hora actual (HH:MM)".to_string(),
        ),
        (
            "Reloj+Seg".to_string(),
            "__CLOCK_S__".to_string(),
            "Muestra hora con segundos".to_string(),
        ),
        (
            "Fecha".to_string(),
            "__DATE__".to_string(),
            "Muestra fecha (DD/MM)".to_string(),
        ),
        (
            "Fecha completa".to_string(),
            "__DATE_FULL__".to_string(),
            "Muestra fecha completa".to_string(),
        ),
        (
            "Día semana".to_string(),
            "__WEEKDAY__".to_string(),
            "Muestra día de la semana".to_string(),
        ),
        // Widgets - Sistema
        (
            "CPU %".to_string(),
            "__CPU__".to_string(),
            "Muestra uso de CPU".to_string(),
        ),
        (
            "RAM %".to_string(),
            "__RAM__".to_string(),
            "Muestra uso de RAM".to_string(),
        ),
        (
            "Temp CPU".to_string(),
            "__TEMP__".to_string(),
            "Muestra temperatura CPU".to_string(),
        ),
        (
            "Clima".to_string(),
            "__WEATHER__".to_string(),
            "Widget: clima actual (configura ciudad con --set-weather)".to_string(),
        ),
        // Widgets - Timer
        (
            "Timer 1m".to_string(),
            "__TIMER_1__".to_string(),
            "Temporizador 1 minuto".to_string(),
        ),
        (
            "Timer 5m".to_string(),
            "__TIMER_5__".to_string(),
            "Temporizador 5 minutos".to_string(),
        ),
        (
            "Timer 10m".to_string(),
            "__TIMER_10__".to_string(),
            "Temporizador 10 minutos".to_string(),
        ),
        (
            "Timer 15m".to_string(),
            "__TIMER_15__".to_string(),
            "Temporizador 15 minutos".to_string(),
        ),
        (
            "Timer 30m".to_string(),
            "__TIMER_30__".to_string(),
            "Temporizador 30 minutos".to_string(),
        ),
        // OBS Studio - WebSocket Control
        (
            "OBS Stream".to_string(),
            "__OBS_STREAM__".to_string(),
            "Iniciar/Detener streaming".to_string(),
        ),
        (
            "OBS Record".to_string(),
            "__OBS_RECORD__".to_string(),
            "Iniciar/Detener grabación".to_string(),
        ),
        (
            "OBS Mute".to_string(),
            "__OBS_MUTE__".to_string(),
            "Mutear/Desmutear micrófono".to_string(),
        ),
        (
            "OBS Status".to_string(),
            "__OBS_STATUS__".to_string(),
            "Widget: muestra LIVE/REC".to_string(),
        ),
        (
            "Escena 1".to_string(),
            "__OBS_SCENE_Scene".to_string(),
            "Cambiar a escena (editar nombre)".to_string(),
        ),
        (
            "Escena Gaming".to_string(),
            "__OBS_SCENE_Gaming".to_string(),
            "Cambiar a escena Gaming".to_string(),
        ),
        (
            "Escena Webcam".to_string(),
            "__OBS_SCENE_Webcam".to_string(),
            "Cambiar a escena Webcam".to_string(),
        ),
        (
            "Escena BRB".to_string(),
            "__OBS_SCENE_BRB".to_string(),
            "Cambiar a escena BRB".to_string(),
        ),
        // Twitch Integration
        (
            "Twitch Viewers".to_string(),
            "__TWITCH_VIEWERS__".to_string(),
            "Widget: muestra viewers actuales".to_string(),
        ),
        (
            "Twitch Followers".to_string(),
            "__TWITCH_FOLLOWERS__".to_string(),
            "Widget: muestra total followers".to_string(),
        ),
        (
            "Twitch Clip".to_string(),
            "__TWITCH_CLIP__".to_string(),
            "Crear clip del stream".to_string(),
        ),
        (
            "Ad 30s".to_string(),
            "__TWITCH_AD_30__".to_string(),
            "Comercial de 30 segundos".to_string(),
        ),
        (
            "Ad 60s".to_string(),
            "__TWITCH_AD_60__".to_string(),
            "Comercial de 60 segundos".to_string(),
        ),
        (
            "Ad 90s".to_string(),
            "__TWITCH_AD_90__".to_string(),
            "Comercial de 90 segundos".to_string(),
        ),
        (
            "Chat Hola".to_string(),
            "__TWITCH_CHAT_¡Hola chat!".to_string(),
            "Enviar mensaje al chat".to_string(),
        ),
        (
            "Chat BRB".to_string(),
            "__TWITCH_CHAT_BRB - Vuelvo en un momento".to_string(),
            "Enviar BRB al chat".to_string(),
        ),
        // Hyprland/Sway workspaces
        (
            "WS 1".to_string(),
            "hyprctl dispatch workspace 1".to_string(),
            "Ir a workspace 1".to_string(),
        ),
        (
            "WS 2".to_string(),
            "hyprctl dispatch workspace 2".to_string(),
            "Ir a workspace 2".to_string(),
        ),
        (
            "WS 3".to_string(),
            "hyprctl dispatch workspace 3".to_string(),
            "Ir a workspace 3".to_string(),
        ),
        (
            "WS 4".to_string(),
            "hyprctl dispatch workspace 4".to_string(),
            "Ir a workspace 4".to_string(),
        ),
        (
            "WS 5".to_string(),
            "hyprctl dispatch workspace 5".to_string(),
            "Ir a workspace 5".to_string(),
        ),
        // Sistema
        (
            "Screenshot".to_string(),
            "grim -g \"$(slurp)\" - | wl-copy".to_string(),
            "Captura de pantalla".to_string(),
        ),
        (
            "Lock".to_string(),
            "swaylock || i3lock".to_string(),
            "Bloquear pantalla".to_string(),
        ),
        (
            "Suspend".to_string(),
            "systemctl suspend".to_string(),
            "Suspender sistema".to_string(),
        ),
        // Navegación de páginas
        (
            ">> Next".to_string(),
            "__NEXT_PAGE__".to_string(),
            "Siguiente página".to_string(),
        ),
        (
            "<< Prev".to_string(),
            "__PREV_PAGE__".to_string(),
            "Página anterior".to_string(),
        ),
        (
            "Home".to_string(),
            "__PAGE_0__".to_string(),
            "Ir a página principal".to_string(),
        ),
        // Spotify Integration
        (
            "Spotify".to_string(),
            "__SPOTIFY__".to_string(),
            "Widget: muestra canción actual".to_string(),
        ),
        (
            "Spotify Play".to_string(),
            "__SPOTIFY_PLAY__".to_string(),
            "Reproducir Spotify".to_string(),
        ),
        (
            "Spotify Pause".to_string(),
            "__SPOTIFY_PAUSE__".to_string(),
            "Pausar Spotify".to_string(),
        ),
        (
            "Spotify Toggle".to_string(),
            "__SPOTIFY_TOGGLE__".to_string(),
            "Play/Pause Spotify".to_string(),
        ),
        (
            "Spotify Next".to_string(),
            "__SPOTIFY_NEXT__".to_string(),
            "Siguiente pista".to_string(),
        ),
        (
            "Spotify Prev".to_string(),
            "__SPOTIFY_PREV__".to_string(),
            "Pista anterior".to_string(),
        ),
        // App Profile Commands
        (
            "Profile OBS".to_string(),
            "__PROFILE_obs".to_string(),
            "Perfil para OBS Studio".to_string(),
        ),
        (
            "Profile Firefox".to_string(),
            "__PROFILE_firefox".to_string(),
            "Perfil para Firefox".to_string(),
        ),
        (
            "Profile Discord".to_string(),
            "__PROFILE_discord".to_string(),
            "Perfil para Discord".to_string(),
        ),
        (
            "Profile Terminal".to_string(),
            "__PROFILE_terminal".to_string(),
            "Perfil para terminal".to_string(),
        ),
        (
            "Profile Default".to_string(),
            "__PROFILE_DEFAULT".to_string(),
            "Volver al perfil principal".to_string(),
        ),
        // Global Hotkeys
        (
            "Hotkey F1".to_string(),
            "__HOTKEY_F1__".to_string(),
            "Activar con tecla F1".to_string(),
        ),
        (
            "Hotkey Ctrl+F1".to_string(),
            "__HOTKEY_Ctrl+F1__".to_string(),
            "Activar con Ctrl+F1".to_string(),
        ),
        (
            "Hotkey Ctrl+Shift+1".to_string(),
            "__HOTKEY_Ctrl+Shift+1__".to_string(),
            "Activar con Ctrl+Shift+1".to_string(),
        ),
    ]
}

// ============================================================================
// Hotkey Recording Commands
// ============================================================================

#[tauri::command]
fn start_hotkey_recording() -> Result<(), String> {
    debug_log!("Starting hotkey recording");
    // Clear previous recorded keys
    if let Ok(mut recorded) = RECORDED_HOTKEY.write() {
        recorded.clear();
    }
    // Start recording
    HOTKEY_RECORDING.store(true, Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
fn stop_hotkey_recording() -> Result<String, String> {
    debug_log!("Stopping hotkey recording");
    HOTKEY_RECORDING.store(false, Ordering::Relaxed);

    // Get the recorded keys
    let hotkey_str = if let Ok(recorded) = RECORDED_HOTKEY.read() {
        keys_to_hotkey_string(&recorded)
    } else {
        String::new()
    };

    // Clear recorded keys
    if let Ok(mut recorded) = RECORDED_HOTKEY.write() {
        recorded.clear();
    }

    // Also clear current keys
    if let Ok(mut current) = CURRENT_KEYS.write() {
        current.clear();
    }

    debug_log!("Recorded hotkey: {}", hotkey_str);
    Ok(hotkey_str)
}

#[tauri::command]
fn get_current_recording() -> Result<String, String> {
    if let Ok(recorded) = RECORDED_HOTKEY.read() {
        Ok(keys_to_hotkey_string(&recorded))
    } else {
        Ok(String::new())
    }
}

#[tauri::command]
fn register_hotkey(hotkey: String, page: usize, button_id: u8) -> Result<(), String> {
    debug_log!(
        "Registering hotkey '{}' for page {} button {}",
        hotkey,
        page,
        button_id
    );
    if let Ok(mut hotkeys) = REGISTERED_HOTKEYS.write() {
        hotkeys.insert(hotkey, (page, button_id));
        Ok(())
    } else {
        Err("Failed to register hotkey".to_string())
    }
}

#[tauri::command]
fn unregister_hotkey(hotkey: String) -> Result<(), String> {
    debug_log!("Unregistering hotkey '{}'", hotkey);
    if let Ok(mut hotkeys) = REGISTERED_HOTKEYS.write() {
        hotkeys.remove(&hotkey);
        Ok(())
    } else {
        Err("Failed to unregister hotkey".to_string())
    }
}

#[tauri::command]
fn set_weather_location(city: String) -> Result<(), String> {
    if let Ok(mut loc) = WEATHER_LOCATION.write() {
        *loc = city.clone();
    }
    // Invalidate cache so next refresh fetches immediately
    if let Ok(mut cache) = WEATHER_CACHE.write() {
        cache.0 = String::new();
        cache.1 = 0;
    }
    debug_log!("Weather location set to: {}", city);
    Ok(())
}

#[tauri::command]
fn get_weather_location() -> String {
    WEATHER_LOCATION.read().ok().map(|l| l.clone()).unwrap_or_default()
}

#[tauri::command]
fn get_registered_hotkeys() -> Result<Vec<(String, usize, u8)>, String> {
    if let Ok(hotkeys) = REGISTERED_HOTKEYS.read() {
        Ok(hotkeys
            .iter()
            .map(|(k, (p, b))| (k.clone(), *p, *b))
            .collect())
    } else {
        Err("Failed to get hotkeys".to_string())
    }
}

#[tauri::command]
fn reload_hotkeys(state: State<AppState>) -> Result<(), String> {
    load_hotkeys_from_config(&state.config_path);
    Ok(())
}

// ============================================================================
// Auto-Update System
// ============================================================================

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const GITHUB_REPO: &str = "Rene-Kuhm/redragon-streamdeck-linux-";
const CURRENT_COMMIT: Option<&str> = option_env!("REDRAGON_CURRENT_COMMIT");

fn short_commit(commit: &str) -> String {
    commit[..7.min(commit.len())].to_string()
}

/// Convierte "v2.1.0" o "2.1.0-tauri" en [2, 1, 0] para poder comparar.
///
/// Se descarta la `v` inicial y todo lo que venga despues de `-`, que es
/// donde suelen ir los sufijos de prelanzamiento. Devuelve `None` si no hay
/// ningun numero, para no tratar una etiqueta rara como si fuera la version 0.
fn parse_version(tag: &str) -> Option<Vec<u64>> {
    let core = tag.trim().trim_start_matches(['v', 'V']);
    let core = core.split(['-', '+']).next()?;

    let parts: Vec<u64> = core
        .split('.')
        .map(|p| p.parse::<u64>())
        .collect::<Result<_, _>>()
        .ok()?;

    if parts.is_empty() {
        None
    } else {
        Some(parts)
    }
}

/// `true` si `candidate` es posterior a `current`.
///
/// Las listas de distinta longitud se comparan rellenando con ceros, asi
/// "2.1" y "2.1.0" resultan iguales en vez de que la mas corta pierda.
fn is_newer(candidate: &str, current: &str) -> bool {
    match (parse_version(candidate), parse_version(current)) {
        (Some(a), Some(b)) => {
            let len = a.len().max(b.len());
            for i in 0..len {
                let x = a.get(i).copied().unwrap_or(0);
                let y = b.get(i).copied().unwrap_or(0);
                if x != y {
                    return x > y;
                }
            }
            false
        }
        // Sin version comparable es preferible no avisar: un cartel de
        // actualizacion que no se puede resolver es peor que no avisar.
        _ => false,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub available: bool,
    pub current_version: String,
    pub current_commit: String,
    pub latest_commit: String,
    pub latest_commit_short: String,
    pub changes: Vec<CommitInfo>,
    pub update_date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitInfo {
    pub sha: String,
    pub message: String,
    pub author: String,
    pub date: String,
}

#[tauri::command]
async fn check_for_updates() -> Result<UpdateInfo, String> {
    debug_log!("Checking for updates...");

    let client = reqwest::blocking::Client::builder()
        .user_agent("RedragonStreamDeck/2.0")
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let current_commit = CURRENT_COMMIT
        .map(short_commit)
        .unwrap_or_else(|| "unknown".to_string());

    // Se compara contra el ultimo *release*, no contra el ultimo commit de la
    // rama principal. Comparar commits hacia que cualquier cambio de
    // documentacion o de CI disparara el aviso de actualizacion.
    let url = format!(
        "https://api.github.com/repos/{}/releases/latest",
        GITHUB_REPO
    );

    let response = client
        .get(&url)
        .send()
        .map_err(|e| format!("Failed to fetch updates: {}", e))?;

    // Un proyecto sin releases publicados no es un error: simplemente todavia
    // no hay nada a lo que actualizarse.
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        debug_log!("The repository has no published releases");
        return Ok(UpdateInfo {
            available: false,
            current_version: CURRENT_VERSION.to_string(),
            current_commit,
            latest_commit: String::new(),
            latest_commit_short: String::new(),
            changes: vec![],
            update_date: String::new(),
        });
    }

    if !response.status().is_success() {
        return Err(format!("GitHub API error: {}", response.status()));
    }

    let release: serde_json::Value = response
        .json()
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    let tag = release["tag_name"].as_str().unwrap_or("").to_string();
    let published = release["published_at"].as_str().unwrap_or("");
    let update_date = if published.len() >= 10 {
        published[..10].to_string()
    } else {
        published.to_string()
    };

    let available = is_newer(&tag, CURRENT_VERSION);

    // Las notas del release son el registro de cambios. Se listan las lineas
    // con contenido, quitando las viñetas para no duplicarlas en la interfaz.
    let author = release["author"]["login"].as_str().unwrap_or("");
    let changes: Vec<CommitInfo> = release["body"]
        .as_str()
        .unwrap_or("")
        .lines()
        .map(|line| line.trim().trim_start_matches(['-', '*', '•']).trim())
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .take(20)
        .map(|line| CommitInfo {
            sha: tag.clone(),
            message: line.to_string(),
            author: author.to_string(),
            date: update_date.clone(),
        })
        .collect();

    debug_log!(
        "Latest release: {} (current {}), available: {}",
        tag,
        CURRENT_VERSION,
        available
    );

    Ok(UpdateInfo {
        available,
        current_version: CURRENT_VERSION.to_string(),
        current_commit,
        latest_commit: tag.clone(),
        latest_commit_short: tag,
        changes,
        update_date,
    })
}

#[tauri::command]
async fn install_update() -> Result<String, String> {
    debug_log!("Starting update installation...");

    // Se reinstala sobre el mismo ejecutable que esta corriendo. Antes esto
    // copiaba a /usr/local/bin con sudo, que en un script sin terminal se
    // queda esperando la contraseña, y ademas dejaba el binario en un sitio
    // distinto del que arranca el servicio.
    let install_path =
        std::env::current_exe().map_err(|e| format!("Failed to get executable path: {}", e))?;

    let update_script = format!(
        r#"#!/bin/bash
set -euo pipefail

REPO_URL="https://github.com/{repo}.git"
INSTALL_PATH="{install_path}"
SERVICE="redragon-streamdeck.service"

TEMP_DIR=$(mktemp -d)
trap 'rm -rf "$TEMP_DIR"' EXIT

echo "=== Actualizando Redragon Stream Deck ==="
echo ""

echo "[1/5] Descargando ultima version..."
git clone --depth 1 "$REPO_URL" "$TEMP_DIR/repo"

cd "$TEMP_DIR/repo"

echo "[2/5] Compilando..."
cargo build --release -p redragon-streamdeck

# El proyecto es un workspace: cargo deja el binario en el target de la raiz,
# no en src-tauri/target, aunque se compile apuntando a ese paquete.
NEW_BIN="$TEMP_DIR/repo/target/release/redragon-streamdeck"
if [ ! -x "$NEW_BIN" ]; then
    echo "Error: no se genero el binario en $NEW_BIN"
    exit 1
fi

echo "[3/5] Deteniendo la aplicacion..."
MANAGED=0
if systemctl --user is-active --quiet "$SERVICE" || systemctl --user is-enabled --quiet "$SERVICE" 2>/dev/null; then
    MANAGED=1
    systemctl --user stop "$SERVICE" || true
else
    # -x compara contra el nombre del proceso, que systemd trunca a 15
    # caracteres. Con -f el patron aparece en la propia linea de comandos de
    # este script y se mataria a si mismo.
    pkill -x redragon-stream || true
fi
sleep 1

echo "[4/5] Instalando en $INSTALL_PATH..."
cp -f "$INSTALL_PATH" "$INSTALL_PATH.bak" 2>/dev/null || true
install -m 755 "$NEW_BIN" "$INSTALL_PATH"

echo "[5/5] Arrancando..."
if [ "$MANAGED" = 1 ]; then
    # El unit trae StartLimitBurst para cortar bucles de fallos; sin este
    # reset, arrancar tras varios reinicios seguidos falla con start-limit-hit
    # y deja el dispositivo sin responder.
    systemctl --user reset-failed "$SERVICE" || true
    systemctl --user start "$SERVICE"
else
    nohup "$INSTALL_PATH" >/dev/null 2>&1 &
fi

echo ""
echo "=== Actualizacion completada ==="
echo "Copia de seguridad de la version anterior: $INSTALL_PATH.bak"
"#,
        repo = GITHUB_REPO,
        install_path = install_path.display()
    );

    let script_path = std::env::temp_dir().join("redragon-update.sh");
    fs::write(&script_path, &update_script)
        .map_err(|e| format!("Failed to write update script: {}", e))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&script_path, fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("Failed to set script permissions: {}", e))?;
    }

    // Conviene abrirlo en una terminal: compilar tarda y sin salida visible
    // parece que la actualizacion se colgo.
    let terminals = [
        "kitty",
        "konsole",
        "foot",
        "alacritty",
        "gnome-terminal",
        "xterm",
    ];
    let script = script_path.to_string_lossy().to_string();
    let mut launched = false;

    for terminal in &terminals {
        let result = if *terminal == "gnome-terminal" || *terminal == "konsole" {
            Command::new(terminal)
                .args(["--", "bash", &script])
                .spawn()
        } else {
            Command::new(terminal).args(["-e", "bash", &script]).spawn()
        };

        if result.is_ok() {
            launched = true;
            debug_log!("Update started in {}", terminal);
            break;
        }
    }

    if !launched {
        Command::new("bash")
            .arg(&script_path)
            .spawn()
            .map_err(|e| format!("Failed to start update: {}", e))?;
    }

    Ok(format!(
        "Actualizacion iniciada. El binario se reemplazara en {}",
        install_path.display()
    ))
}

#[tauri::command]
fn get_current_version() -> (String, String) {
    (
        CURRENT_VERSION.to_string(),
        CURRENT_COMMIT
            .map(short_commit)
            .unwrap_or_else(|| "unknown".to_string()),
    )
}

// ============================================================================
// App Profile Switcher (per-application button pages)
// ============================================================================

fn get_active_window_app() -> Option<String> {
    // Try hyprctl first (Hyprland)
    if let Ok(output) = Command::new("hyprctl").args(["activewindow", "-j"]).output() {
        if output.status.success() {
            if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&output.stdout) {
                if let Some(class) = json.get("class").and_then(|v| v.as_str()) {
                    return Some(class.to_lowercase());
                }
            }
        }
    }
    // Fallback to xdotool (X11)
    if let Ok(output) = Command::new("xdotool").args(["getactivewindow", "getwindowpid", "xdotool", "getwindowclassname"]).output() {
        if output.status.success() {
            let class = String::from_utf8_lossy(&output.stdout).trim().to_lowercase();
            if !class.is_empty() {
                return Some(class);
            }
        }
    }
    None
}

fn handle_profile_switch(profile_name: &str) {
    let config_path = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("redragon-streamdeck")
        .join("config.json");

    if !config_path.exists() {
        return;
    }

    let content = fs::read_to_string(&config_path).unwrap_or_default();
    let mut config: Config = match serde_json::from_str(&content) {
        Ok(c) => c,
        Err(_) => return,
    };

    if profile_name == "DEFAULT" {
        // Reset to page 0
        config.current_page = 0;
        if let Ok(json) = serde_json::to_string_pretty(&config) {
            fs::write(&config_path, json).ok();
        }
        request_refresh();
        return;
    }

    // Look up profile: exact match first, then case-insensitive contains
    let lookup = profile_name.to_lowercase();
    if let Some(&page_index) = config.app_profiles.get(&lookup) {
        if page_index < config.pages.len() {
            config.current_page = page_index;
            if let Ok(json) = serde_json::to_string_pretty(&config) {
                fs::write(&config_path, json).ok();
            }
            request_refresh();
        }
    }
}

fn start_app_profile_watcher(config_path: PathBuf, _icons_path: PathBuf) {
    std::thread::spawn(move || {
        let mut last_app: Option<String> = None;
        loop {
            std::thread::sleep(Duration::from_secs(2));

            if let Some(current_app) = get_active_window_app() {
                if last_app.as_ref() != Some(&current_app) {
                    last_app = Some(current_app.clone());

                    if let Ok(content) = fs::read_to_string(&config_path) {
                        if let Ok(config) = serde_json::from_str::<Config>(&content) {
                            if let Some(&page_index) = config.app_profiles.get(&current_app) {
                                if page_index < config.pages.len() && page_index != config.current_page {
                                    // Update config with the new page
                                    let mut new_config = config.clone();
                                    new_config.current_page = page_index;
                                    if let Ok(json) = serde_json::to_string_pretty(&new_config) {
                                        fs::write(&config_path, json).ok();
                                    }
                                    request_refresh();
                                }
                            }
                        }
                    }
                }
            }
        }
    });
}

// ============================================================================
// Tauri App Entry Point
// ============================================================================

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            let app_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| PathBuf::from("."));
            fs::create_dir_all(&app_dir).ok();

            let state = AppState::new(app_dir.clone());

            // Start the button listener in background
            let config_path = app_dir.join("config.json");
            let icons_path = app_dir.join("icons");
            start_button_listener(config_path.clone(), icons_path.clone());

            // Start global keyboard listener for hotkeys
            start_keyboard_listener(config_path.clone(), icons_path.clone());

            // Load registered hotkeys from config
            load_hotkeys_from_config(&config_path);

            // Start app profile auto-switcher
            start_app_profile_watcher(config_path.clone(), icons_path.clone());

            app.manage(state);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_config,
            save_full_config,
            get_status,
            connect_device,
            set_page,
            add_page,
            delete_page,
            update_page_name,
            update_button,
            set_brightness_level,
            run_command,
            refresh_device,
            load_current_page,
            get_icons_path,
            setup_udev_rules,
            check_udev_rules,
            save_icon,
            reset_config,
            list_icons,
            get_icon_data,
            get_preset_commands,
            clear_page_buttons,
            // Hotkey commands
            start_hotkey_recording,
            stop_hotkey_recording,
            get_current_recording,
            register_hotkey,
            unregister_hotkey,
            get_registered_hotkeys,
            reload_hotkeys,
            // Update commands
            check_for_updates,
            install_update,
            get_current_version,
            // App profile commands
            get_app_profiles,
            save_app_profiles,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tags_with_and_without_prefix() {
        assert_eq!(parse_version("v2.1.0"), Some(vec![2, 1, 0]));
        assert_eq!(parse_version("2.1.0"), Some(vec![2, 1, 0]));
        // El repositorio ya tiene etiquetas con sufijo, como v2.0.0-tauri.
        assert_eq!(parse_version("v2.0.0-tauri"), Some(vec![2, 0, 0]));
        assert_eq!(parse_version("v2.0"), Some(vec![2, 0]));
    }

    #[test]
    fn rejects_tags_without_a_version() {
        assert_eq!(parse_version("latest"), None);
        assert_eq!(parse_version(""), None);
    }

    #[test]
    fn detects_newer_releases() {
        assert!(is_newer("v2.1.0", "2.0.0"));
        assert!(is_newer("v2.0.1", "2.0.0"));
        assert!(is_newer("v3.0.0", "2.9.9"));
    }

    #[test]
    fn ignores_equal_or_older_releases() {
        // El caso que importa: recien compilado desde la ultima version
        // publicada, no debe aparecer ningun aviso.
        assert!(!is_newer("v2.0.0", "2.0.0"));
        assert!(!is_newer("v2.0.0-tauri", "2.0.0"));
        assert!(!is_newer("v1.9.0", "2.0.0"));
    }

    #[test]
    fn treats_missing_components_as_zero() {
        assert!(!is_newer("v2.0", "2.0.0"));
        assert!(is_newer("v2.0.1", "2.0"));
    }

    #[test]
    fn stays_quiet_when_the_tag_is_not_a_version() {
        // Preferible callar antes que anunciar una actualizacion que el
        // usuario no puede resolver.
        assert!(!is_newer("nightly", "2.0.0"));
    }
}
