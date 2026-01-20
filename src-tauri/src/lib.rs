use rusb::{Context, DeviceHandle, UsbContext};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Manager, State};

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

        // Create icons directory if it doesn't exist
        fs::create_dir_all(&icons_path).ok();

        // Load or create config
        let config = if config_path.exists() {
            let content = fs::read_to_string(&config_path).unwrap_or_default();
            serde_json::from_str(&content).unwrap_or_else(|_| Self::default_config())
        } else {
            let config = Self::default_config();
            let content = serde_json::to_string_pretty(&config).unwrap();
            fs::write(&config_path, content).ok();
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
        // Add navigation button
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
            let mut handle = device.open().ok()?;

            // Detach kernel driver if active
            if handle.kernel_driver_active(0).unwrap_or(false) {
                handle.detach_kernel_driver(0).ok()?;
            }

            handle.claim_interface(0).ok()?;
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

fn set_brightness(handle: &DeviceHandle<Context>, brightness: u8) -> Result<(), String> {
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

fn send_image_to_key(handle: &DeviceHandle<Context>, key_id: u8, image_data: &[u8]) -> Result<(), String> {
    // Protocol for sending images to StreamDock
    let chunk_size = 1016;
    let total_chunks = (image_data.len() + chunk_size - 1) / chunk_size;

    for (i, chunk) in image_data.chunks(chunk_size).enumerate() {
        let mut packet = vec![0x00; 1024];
        packet[0] = 0x02; // Image data command
        packet[1] = 0x01;
        packet[2] = (i & 0xFF) as u8;
        packet[3] = ((i >> 8) & 0xFF) as u8;
        packet[4] = if i == total_chunks - 1 { 0x01 } else { 0x00 }; // Last chunk flag
        packet[5] = key_id;
        packet[6..6 + chunk.len()].copy_from_slice(chunk);

        send_to_device(handle, &packet)?;
    }
    Ok(())
}

// ============================================================================
// Image Generation
// ============================================================================

fn generate_button_image(color: &str, label: &str, icon_path: Option<&PathBuf>) -> Result<Vec<u8>, String> {
    use image::{Rgb, RgbImage, imageops};

    let size = 256u32;

    // Parse color
    let color_hex = color.trim_start_matches('#');
    let r = u8::from_str_radix(&color_hex[0..2], 16).unwrap_or(26);
    let g = u8::from_str_radix(&color_hex[2..4], 16).unwrap_or(26);
    let b = u8::from_str_radix(&color_hex[4..6], 16).unwrap_or(46);

    let mut img = RgbImage::from_pixel(size, size, Rgb([r, g, b]));

    // If icon exists, overlay it
    if let Some(icon_path) = icon_path {
        if icon_path.exists() {
            if let Ok(icon) = image::open(icon_path) {
                let icon = icon.resize_exact(size, size, imageops::FilterType::Lanczos3);
                let icon_rgb = icon.to_rgb8();
                img = icon_rgb;
            }
        }
    }

    // Convert to JPEG bytes for the device
    let mut buffer = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut buffer);
    img.write_to(&mut cursor, image::ImageFormat::Jpeg)
        .map_err(|e| format!("Image encoding error: {}", e))?;

    Ok(buffer)
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub fn get_config(state: State<AppState>) -> Result<Config, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    Ok(config.clone())
}

#[tauri::command]
pub fn save_config(state: State<AppState>, config: Config) -> Result<(), String> {
    let mut current = state.config.lock().map_err(|e| e.to_string())?;
    *current = config;
    drop(current);
    state.save_config();
    Ok(())
}

#[tauri::command]
pub fn get_status(state: State<AppState>) -> StatusResponse {
    let connected = state.device_connected.lock().map(|c| *c).unwrap_or(false);
    StatusResponse { connected }
}

#[tauri::command]
pub fn connect_device(state: State<AppState>) -> Result<bool, String> {
    let connected = find_device().is_some();
    if let Ok(mut dev_state) = state.device_connected.lock() {
        *dev_state = connected;
    }
    Ok(connected)
}

#[tauri::command]
pub fn set_page(state: State<AppState>, index: usize) -> Result<(), String> {
    let mut config = state.config.lock().map_err(|e| e.to_string())?;
    if index < config.pages.len() {
        config.current_page = index;
    }
    drop(config);
    state.save_config();

    // Reload page on device
    load_current_page_to_device(&state)?;
    Ok(())
}

#[tauri::command]
pub fn add_page(state: State<AppState>, name: String) -> Result<usize, String> {
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
pub fn delete_page(state: State<AppState>, index: usize) -> Result<(), String> {
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
pub fn update_page_name(state: State<AppState>, index: usize, name: String) -> Result<(), String> {
    let mut config = state.config.lock().map_err(|e| e.to_string())?;

    if index < config.pages.len() {
        config.pages[index].name = name;
    }
    drop(config);
    state.save_config();

    Ok(())
}

#[tauri::command]
pub fn update_button(
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

    // Refresh device if on current page
    load_current_page_to_device(&state)?;

    Ok(())
}

#[tauri::command]
pub fn set_brightness_cmd(state: State<AppState>, brightness: u8) -> Result<(), String> {
    let mut config = state.config.lock().map_err(|e| e.to_string())?;
    config.brightness = brightness;
    drop(config);
    state.save_config();

    // Apply to device
    if let Some(handle) = find_device() {
        set_brightness(&handle, brightness)?;
    }

    Ok(())
}

#[tauri::command]
pub fn execute_command(command: String) -> Result<(), String> {
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
pub fn handle_button_press(state: State<AppState>, key_id: u8) -> Result<Option<String>, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    let page = &config.pages[config.current_page];

    if let Some(button) = page.buttons.get(&key_id.to_string()) {
        let cmd = &button.command;

        // Special navigation commands
        if cmd == "__NEXT_PAGE__" {
            let next = (config.current_page + 1) % config.pages.len();
            drop(config);
            set_page(state, next)?;
            return Ok(Some("page_changed".to_string()));
        }

        if cmd == "__PREV_PAGE__" {
            let prev = if config.current_page == 0 {
                config.pages.len() - 1
            } else {
                config.current_page - 1
            };
            drop(config);
            set_page(state, prev)?;
            return Ok(Some("page_changed".to_string()));
        }

        // Check for __PAGE_N__ pattern
        if cmd.starts_with("__PAGE_") && cmd.ends_with("__") {
            let num_str = &cmd[7..cmd.len() - 2];
            if let Ok(target_page) = num_str.parse::<usize>() {
                if target_page < config.pages.len() {
                    drop(config);
                    set_page(state, target_page)?;
                    return Ok(Some("page_changed".to_string()));
                }
            }
        }

        // Regular command
        if !cmd.is_empty() {
            execute_command(cmd.clone())?;
            return Ok(Some(button.label.clone()));
        }
    }

    Ok(None)
}

fn load_current_page_to_device(state: &State<AppState>) -> Result<(), String> {
    let handle = match find_device() {
        Some(h) => h,
        None => return Ok(()), // Device not connected, skip
    };

    let config = state.config.lock().map_err(|e| e.to_string())?;
    let page = &config.pages[config.current_page];

    wake_screen(&handle)?;
    clear_screen(&handle)?;
    set_brightness(&handle, config.brightness)?;

    for (key_id_str, button) in &page.buttons {
        let key_id: u8 = key_id_str.parse().unwrap_or(0);

        let icon_path = if !button.icon.is_empty() {
            Some(state.icons_path.join(&button.icon))
        } else {
            None
        };

        let image_data = generate_button_image(&button.color, &button.label, icon_path.as_ref())?;
        send_image_to_key(&handle, key_id, &image_data)?;
    }

    Ok(())
}

#[tauri::command]
pub fn refresh_device(state: State<AppState>) -> Result<(), String> {
    load_current_page_to_device(&state)
}

#[tauri::command]
pub fn get_icons_path(state: State<AppState>) -> String {
    state.icons_path.to_string_lossy().to_string()
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
            save_config,
            get_status,
            connect_device,
            set_page,
            add_page,
            delete_page,
            update_page_name,
            update_button,
            set_brightness_cmd,
            execute_command,
            handle_button_press,
            refresh_device,
            get_icons_path,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
