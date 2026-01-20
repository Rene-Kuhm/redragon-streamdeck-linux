use rusb::{Context, DeviceHandle, UsbContext};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{Manager, State};

// USB IDs for Redragon SS-550
const VENDOR_ID: u16 = 0x0200;
const PRODUCT_ID: u16 = 0x1000;

// ============================================================================
// Data Structures
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ButtonConfig {
    pub label: String,
    pub command: String,
    pub color: String,
    pub icon: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page {
    pub name: String,
    pub buttons: HashMap<String, ButtonConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub brightness: u8,
    #[serde(rename = "currentPage")]
    pub current_page: usize,
    pub pages: Vec<Page>,
}

#[derive(Debug, Serialize)]
pub struct StatusResponse {
    pub connected: bool,
}

// ============================================================================
// App State
// ============================================================================

pub struct AppState {
    pub config: Mutex<Config>,
    pub device_connected: Mutex<bool>,
    pub config_path: PathBuf,
    pub icons_path: PathBuf,
}

impl AppState {
    pub fn new(app_dir: PathBuf) -> Self {
        let config_path = app_dir.join("config.json");
        let icons_path = app_dir.join("icons");

        fs::create_dir_all(&icons_path).ok();

        let config = if config_path.exists() {
            let content = fs::read_to_string(&config_path).unwrap_or_default();
            serde_json::from_str(&content).unwrap_or_else(|_| Self::default_config())
        } else {
            let config = Self::default_config();
            if let Ok(content) = serde_json::to_string_pretty(&config) {
                fs::write(&config_path, content).ok();
            }
            config
        };

        Self {
            config: Mutex::new(config),
            device_connected: Mutex::new(false),
            config_path,
            icons_path,
        }
    }

    fn default_config() -> Config {
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
        buttons.insert(
            "5".to_string(),
            ButtonConfig {
                label: ">>".to_string(),
                command: "__NEXT_PAGE__".to_string(),
                color: "#e94560".to_string(),
                icon: String::new(),
            },
        );

        Config {
            brightness: 50,
            current_page: 0,
            pages: vec![Page {
                name: "Principal".to_string(),
                buttons,
            }],
        }
    }

    pub fn save_config(&self) {
        if let Ok(config) = self.config.lock() {
            if let Ok(content) = serde_json::to_string_pretty(&*config) {
                fs::write(&self.config_path, content).ok();
            }
        }
    }
}

// ============================================================================
// USB Stream Deck Functions
// ============================================================================

fn find_device() -> Option<DeviceHandle<Context>> {
    let context = Context::new().ok()?;

    for device in context.devices().ok()?.iter() {
        let desc = device.device_descriptor().ok()?;
        if desc.vendor_id() == VENDOR_ID && desc.product_id() == PRODUCT_ID {
            #[allow(unused_mut)]
            let mut handle = device.open().ok()?;

            // Detach kernel driver if attached (Linux)
            #[cfg(target_os = "linux")]
            {
                if handle.kernel_driver_active(0).unwrap_or(false) {
                    let _ = handle.detach_kernel_driver(0);
                }
            }

            // Claim the interface
            let _ = handle.claim_interface(0);

            return Some(handle);
        }
    }
    None
}

fn send_to_device(handle: &DeviceHandle<Context>, data: &[u8]) -> Result<(), String> {
    handle
        .write_interrupt(0x02, data, Duration::from_millis(1000))
        .map_err(|e| format!("USB write error: {}", e))?;
    Ok(())
}

fn set_device_brightness(handle: &DeviceHandle<Context>, brightness: u8) -> Result<(), String> {
    let level = (brightness as f32 * 0.64) as u8;
    let mut data = vec![0x00; 32];
    data[0] = 0x03;
    data[1] = 0x6d;
    data[2] = level;
    send_to_device(handle, &data)
}

fn clear_screen(handle: &DeviceHandle<Context>) -> Result<(), String> {
    let mut data = vec![0x00; 32];
    data[0] = 0x03;
    data[1] = 0x63;
    send_to_device(handle, &data)
}

fn wake_screen(handle: &DeviceHandle<Context>) -> Result<(), String> {
    let mut data = vec![0x00; 32];
    data[0] = 0x03;
    data[1] = 0x77;
    send_to_device(handle, &data)
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
        config.pages[page_index].buttons.insert(button_id, button_config);
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

    if let Some(handle) = find_device() {
        set_device_brightness(&handle, brightness)?;
    }

    Ok(())
}

#[tauri::command]
fn run_command(command: String) -> Result<(), String> {
    if command.is_empty() {
        return Ok(());
    }

    std::thread::spawn(move || {
        Command::new("sh")
            .arg("-c")
            .arg(&command)
            .spawn()
            .ok();
    });

    Ok(())
}

#[tauri::command]
fn refresh_device(state: State<AppState>) -> Result<(), String> {
    let handle = match find_device() {
        Some(h) => h,
        None => return Ok(()),
    };

    let config = state.config.lock().map_err(|e| e.to_string())?;

    wake_screen(&handle)?;
    clear_screen(&handle)?;
    set_device_brightness(&handle, config.brightness)?;

    Ok(())
}

#[tauri::command]
fn get_icons_path(state: State<AppState>) -> String {
    state.icons_path.to_string_lossy().to_string()
}

#[tauri::command]
fn setup_udev_rules() -> Result<bool, String> {
    let rules_path = "/etc/udev/rules.d/99-redragon.rules";
    let rules_content = r#"SUBSYSTEM=="usb", ATTR{idVendor}=="0200", ATTR{idProduct}=="1000", MODE="0666""#;

    // Check if rules already exist
    if std::path::Path::new(rules_path).exists() {
        return Ok(true);
    }

    // Try to create rules using pkexec
    let result = Command::new("pkexec")
        .args(["bash", "-c", &format!(
            "echo '{}' > {} && udevadm control --reload-rules && udevadm trigger",
            rules_content,
            rules_path
        )])
        .status();

    match result {
        Ok(status) => Ok(status.success()),
        Err(e) => Err(format!("Failed to setup udev rules: {}", e)),
    }
}

#[tauri::command]
fn check_udev_rules() -> bool {
    std::path::Path::new("/etc/udev/rules.d/99-redragon.rules").exists()
}

// ============================================================================
// Tauri App Entry Point
// ============================================================================

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let app_dir = app.path().app_data_dir().unwrap_or_else(|_| PathBuf::from("."));
            fs::create_dir_all(&app_dir).ok();

            let state = AppState::new(app_dir);
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
            get_icons_path,
            setup_udev_rules,
            check_udev_rules,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
