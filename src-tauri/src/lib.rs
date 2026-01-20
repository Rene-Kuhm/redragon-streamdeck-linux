use rusb::{Context, DeviceHandle, UsbContext};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Cursor;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use std::thread;
use tauri::{Manager, State};
use image::{DynamicImage, ImageBuffer, Rgb, RgbImage, imageops};
use imageproc::drawing::{draw_text_mut, text_size};
use ab_glyph::{FontRef, PxScale};

// USB IDs for Redragon SS-550
const VENDOR_ID: u16 = 0x0200;
const PRODUCT_ID: u16 = 0x1000;

// Global flag to signal refresh needed
static REFRESH_NEEDED: AtomicBool = AtomicBool::new(false);

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

// Command prefix: "CRT\0\0"
const CMD_PREFIX: [u8; 5] = [0x43, 0x52, 0x54, 0x00, 0x00];
// Brightness command: "LIG\0\0" + value
const CMD_LIG: [u8; 5] = [0x4c, 0x49, 0x47, 0x00, 0x00];
// Clear screen command: "CLE\0\0\0" + target
const CMD_CLE: [u8; 6] = [0x43, 0x4c, 0x45, 0x00, 0x00, 0x00];
// Wake/Display command: "DIS\0\0"
const CMD_DIS: [u8; 5] = [0x44, 0x49, 0x53, 0x00, 0x00];
// Refresh command: "STP\0\0"
const CMD_STP: [u8; 5] = [0x53, 0x54, 0x50, 0x00, 0x00];
// Batch/Image command: "BAT" + size(4 bytes) + keyId
const CMD_BAT: [u8; 3] = [0x42, 0x41, 0x54];

const PACKET_SIZE: usize = 512;
const BUTTON_SIZE: u32 = 100;

// Key mapping: physical position -> logical key ID (1-15)
// Used when receiving key presses from the device
fn map_physical_to_logical(physical: u8) -> u8 {
    match physical {
        0x0b => 1,
        0x0c => 2,
        0x0d => 3,
        0x0e => 4,
        0x0f => 5,
        0x06 => 6,
        0x07 => 7,
        0x08 => 8,
        0x09 => 9,
        0x0a => 10,
        0x01 => 11,
        0x02 => 12,
        0x03 => 13,
        0x04 => 14,
        0x05 => 15,
        _ => physical,
    }
}

fn find_device() -> Option<DeviceHandle<Context>> {
    let context = Context::new().ok()?;

    for device in context.devices().ok()?.iter() {
        let desc = device.device_descriptor().ok()?;
        if desc.vendor_id() == VENDOR_ID && desc.product_id() == PRODUCT_ID {
            eprintln!("DEBUG: Found device VID={:04x} PID={:04x}", desc.vendor_id(), desc.product_id());

            #[allow(unused_mut)]
            let mut handle = match device.open() {
                Ok(h) => {
                    eprintln!("DEBUG: Device opened successfully");
                    h
                }
                Err(e) => {
                    eprintln!("DEBUG: Failed to open device: {:?}", e);
                    return None;
                }
            };

            // Set configuration (required for some devices)
            match handle.set_active_configuration(1) {
                Ok(_) => eprintln!("DEBUG: Configuration 1 set"),
                Err(e) => eprintln!("DEBUG: Could not set configuration (may already be set): {:?}", e),
            }

            // Detach kernel driver if attached (Linux)
            #[cfg(target_os = "linux")]
            {
                match handle.kernel_driver_active(0) {
                    Ok(true) => {
                        eprintln!("DEBUG: Kernel driver active, detaching...");
                        match handle.detach_kernel_driver(0) {
                            Ok(_) => eprintln!("DEBUG: Kernel driver detached"),
                            Err(e) => eprintln!("DEBUG: Failed to detach kernel driver: {:?}", e),
                        }
                    }
                    Ok(false) => eprintln!("DEBUG: No kernel driver active"),
                    Err(e) => eprintln!("DEBUG: Error checking kernel driver: {:?}", e),
                }
            }

            // Claim the interface
            match handle.claim_interface(0) {
                Ok(_) => eprintln!("DEBUG: Interface 0 claimed successfully"),
                Err(e) => {
                    eprintln!("DEBUG: Failed to claim interface 0: {:?}", e);
                    return None;
                }
            }

            return Some(handle);
        }
    }
    eprintln!("DEBUG: Device not found");
    None
}

fn send_to_device(handle: &DeviceHandle<Context>, data: &[u8], use_prefix: bool) -> Result<(), String> {
    // Build the full packet: prefix (5 bytes) + data (padded to 512 bytes)
    let mut packet = Vec::with_capacity(CMD_PREFIX.len() + PACKET_SIZE);

    if use_prefix {
        packet.extend_from_slice(&CMD_PREFIX);
    }

    packet.extend_from_slice(data);

    // Pad to full packet size
    let total_size = if use_prefix { CMD_PREFIX.len() + PACKET_SIZE } else { PACKET_SIZE };
    while packet.len() < total_size {
        packet.push(0x00);
    }

    eprintln!("DEBUG: Sending {} bytes to endpoint 0x01", packet.len());
    eprintln!("DEBUG: First 20 bytes: {:02x?}", &packet[..20.min(packet.len())]);

    // Endpoint 0x01 is the OUT endpoint for this device
    match handle.write_interrupt(0x01, &packet, Duration::from_millis(1000)) {
        Ok(bytes_written) => {
            eprintln!("DEBUG: Successfully wrote {} bytes", bytes_written);
            Ok(())
        }
        Err(e) => {
            eprintln!("DEBUG: USB write error: {:?}", e);
            Err(format!("USB write error: {}", e))
        }
    }
}

fn set_device_brightness(handle: &DeviceHandle<Context>, brightness: u8) -> Result<(), String> {
    // Convert 0-100 to 0-64 range
    let level = (brightness as f32 * 0.64) as u8;

    // Command: LIG\0\0 + brightness_value
    let mut cmd_data = Vec::with_capacity(CMD_LIG.len() + 1);
    cmd_data.extend_from_slice(&CMD_LIG);
    cmd_data.push(level);

    send_to_device(handle, &cmd_data, true)
}

fn clear_screen(handle: &DeviceHandle<Context>) -> Result<(), String> {
    // Command: CLE\0\0\0 + 0xFF (clear all)
    let mut cmd_data = Vec::with_capacity(CMD_CLE.len() + 1);
    cmd_data.extend_from_slice(&CMD_CLE);
    cmd_data.push(0xFF);

    send_to_device(handle, &cmd_data, true)
}

fn wake_screen(handle: &DeviceHandle<Context>) -> Result<(), String> {
    // Command: DIS\0\0
    send_to_device(handle, &CMD_DIS, true)
}

fn refresh_screen(handle: &DeviceHandle<Context>) -> Result<(), String> {
    // Command: STP\0\0
    send_to_device(handle, &CMD_STP, true)
}

// Send raw bytes in 512-byte chunks (without prefix)
fn send_bytes(handle: &DeviceHandle<Context>, data: &[u8]) -> Result<(), String> {
    let mut offset = 0;
    while offset < data.len() {
        let end = std::cmp::min(offset + PACKET_SIZE, data.len());
        let chunk = &data[offset..end];
        send_to_device(handle, chunk, false)?;
        offset += PACKET_SIZE;
    }
    Ok(())
}

// Convert size to 4 hex bytes (big endian)
fn size_to_bytes(size: usize) -> [u8; 4] {
    [
        ((size >> 24) & 0xFF) as u8,
        ((size >> 16) & 0xFF) as u8,
        ((size >> 8) & 0xFF) as u8,
        (size & 0xFF) as u8,
    ]
}

// Parse hex color string to RGB
fn parse_hex_color(color: &str) -> (u8, u8, u8) {
    let hex = color.trim_start_matches('#');
    if hex.len() >= 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(26);
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(26);
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(46);
        (r, g, b)
    } else {
        (26, 26, 46) // Default dark color
    }
}

// Generate a button image from config
fn generate_button_image(button: &ButtonConfig, icons_path: &PathBuf) -> Result<Vec<u8>, String> {
    let (r, g, b) = parse_hex_color(&button.color);

    // Try to load icon if specified
    let mut img: RgbImage = if !button.icon.is_empty() {
        let icon_path = icons_path.join(&button.icon);
        if icon_path.exists() {
            match image::open(&icon_path) {
                Ok(icon) => {
                    let resized = icon.resize_exact(BUTTON_SIZE, BUTTON_SIZE, imageops::FilterType::Lanczos3);
                    resized.to_rgb8()
                }
                Err(_) => {
                    // Create solid color background if icon fails to load
                    ImageBuffer::from_pixel(BUTTON_SIZE, BUTTON_SIZE, Rgb([r, g, b]))
                }
            }
        } else {
            ImageBuffer::from_pixel(BUTTON_SIZE, BUTTON_SIZE, Rgb([r, g, b]))
        }
    } else {
        // No icon, create solid color background
        ImageBuffer::from_pixel(BUTTON_SIZE, BUTTON_SIZE, Rgb([r, g, b]))
    };

    // Draw label text if specified and no icon
    if !button.label.is_empty() && button.icon.is_empty() {
        let font_data = include_bytes!("/usr/share/fonts/TTF/DejaVuSans.ttf");
        if let Ok(font) = FontRef::try_from_slice(font_data) {
            let scale = if button.label.len() > 8 {
                PxScale::from(16.0)
            } else if button.label.len() > 5 {
                PxScale::from(20.0)
            } else {
                PxScale::from(28.0)
            };

            let (text_width, text_height) = text_size(scale, &font, &button.label);
            let x = ((BUTTON_SIZE as i32 - text_width as i32) / 2).max(2);
            let y = ((BUTTON_SIZE as i32 - text_height as i32) / 2).max(2);

            draw_text_mut(&mut img, Rgb([255, 255, 255]), x, y, scale, &font, &button.label);
        }
    }

    // Rotate 180 degrees (required by the device)
    let rotated = imageops::rotate180(&img);

    // Convert to JPEG
    let mut jpeg_data = Vec::new();
    let mut cursor = Cursor::new(&mut jpeg_data);

    let dynamic_img = DynamicImage::ImageRgb8(rotated);
    dynamic_img.write_to(&mut cursor, image::ImageFormat::Jpeg)
        .map_err(|e| format!("Failed to encode JPEG: {}", e))?;

    eprintln!("DEBUG: Generated button image, {} bytes JPEG", jpeg_data.len());
    Ok(jpeg_data)
}

// Set image for a specific key
fn set_key_image(handle: &DeviceHandle<Context>, key_id: u8, jpeg_data: &[u8]) -> Result<(), String> {
    let size_bytes = size_to_bytes(jpeg_data.len());

    // Build BAT command: BAT + size(4 bytes) + keyId
    // Note: key_id is sent directly (1-15), no mapping needed for sending images
    let mut cmd_data = Vec::with_capacity(CMD_BAT.len() + 5);
    cmd_data.extend_from_slice(&CMD_BAT);
    cmd_data.extend_from_slice(&size_bytes);
    cmd_data.push(key_id);

    eprintln!("DEBUG: Setting key {} with {} bytes image", key_id, jpeg_data.len());

    // Send BAT command
    send_to_device(handle, &cmd_data, true)?;

    // Send image data in chunks
    send_bytes(handle, jpeg_data)?;

    // Refresh to display
    refresh_screen(handle)?;

    Ok(())
}

// Load all buttons for a page to the device
fn load_page_to_device(handle: &DeviceHandle<Context>, page: &Page, brightness: u8, icons_path: &PathBuf) -> Result<(), String> {
    eprintln!("DEBUG: Loading page '{}' to device", page.name);

    // Wake and clear screen first
    wake_screen(handle)?;
    clear_screen(handle)?;
    set_device_brightness(handle, brightness)?;

    // Send each button image
    for (key_id_str, button) in &page.buttons {
        if let Ok(key_id) = key_id_str.parse::<u8>() {
            if key_id >= 1 && key_id <= 15 {
                // Only send if button has content
                if !button.label.is_empty() || !button.icon.is_empty() || button.color != "#1a1a2e" {
                    match generate_button_image(button, icons_path) {
                        Ok(jpeg_data) => {
                            if let Err(e) = set_key_image(handle, key_id, &jpeg_data) {
                                eprintln!("DEBUG: Failed to set key {}: {}", key_id, e);
                            }
                        }
                        Err(e) => {
                            eprintln!("DEBUG: Failed to generate image for key {}: {}", key_id, e);
                        }
                    }
                }
            }
        }
    }

    eprintln!("DEBUG: Page loaded successfully");
    Ok(())
}

// ============================================================================
// Button Listener Functions
// ============================================================================

// Read a key press from the device
// Returns (key_id, state) where state=1 means pressed, state=0 means released
fn read_key_press(handle: &DeviceHandle<Context>) -> Result<(u8, u8), String> {
    let mut buf = [0u8; 512];

    // Read from endpoint 0x82 (IN endpoint)
    match handle.read_interrupt(0x82, &mut buf, Duration::from_millis(100)) {
        Ok(len) => {
            if len >= 11 {
                let physical_key = buf[9];
                let state = buf[10];
                let logical_key = map_physical_to_logical(physical_key);
                Ok((logical_key, state))
            } else {
                Err("Invalid data length".to_string())
            }
        }
        Err(rusb::Error::Timeout) => {
            Err("timeout".to_string())
        }
        Err(e) => {
            Err(format!("USB read error: {}", e))
        }
    }
}

// Handle a button press - execute the associated command
fn handle_button_press(key_id: u8, config_path: &PathBuf, icons_path: &PathBuf) {
    // Read current config from file
    let config: Config = match fs::read_to_string(config_path) {
        Ok(content) => match serde_json::from_str(&content) {
            Ok(c) => c,
            Err(_) => return,
        },
        Err(_) => return,
    };

    let page = match config.pages.get(config.current_page) {
        Some(p) => p,
        None => return,
    };

    let button = match page.buttons.get(&key_id.to_string()) {
        Some(b) => b,
        None => return,
    };

    if button.command.is_empty() {
        return;
    }

    let cmd = &button.command;
    eprintln!("DEBUG: Button {} pressed, command: {}", key_id, cmd);

    // Handle special page navigation commands
    if cmd == "__NEXT_PAGE__" {
        let next_page = (config.current_page + 1) % config.pages.len();
        change_page(next_page, config_path, icons_path);
        return;
    }

    if cmd == "__PREV_PAGE__" {
        let prev_page = if config.current_page == 0 {
            config.pages.len() - 1
        } else {
            config.current_page - 1
        };
        change_page(prev_page, config_path, icons_path);
        return;
    }

    // Check for __PAGE_N__ pattern
    if cmd.starts_with("__PAGE_") && cmd.ends_with("__") {
        let page_str = &cmd[7..cmd.len()-2];
        if let Ok(target_page) = page_str.parse::<usize>() {
            if target_page < config.pages.len() {
                change_page(target_page, config_path, icons_path);
            }
        }
        return;
    }

    // Execute normal command
    eprintln!("DEBUG: Executing command: {}", cmd);
    let cmd_clone = cmd.clone();
    thread::spawn(move || {
        Command::new("sh")
            .arg("-c")
            .arg(&cmd_clone)
            .spawn()
            .ok();
    });
}

// Change to a different page and update the device
fn change_page(page_index: usize, config_path: &PathBuf, icons_path: &PathBuf) {
    // Read and update config
    let mut config: Config = match fs::read_to_string(config_path) {
        Ok(content) => match serde_json::from_str(&content) {
            Ok(c) => c,
            Err(_) => return,
        },
        Err(_) => return,
    };

    if page_index >= config.pages.len() {
        return;
    }

    config.current_page = page_index;

    // Save updated config
    if let Ok(content) = serde_json::to_string_pretty(&config) {
        fs::write(config_path, content).ok();
    }

    // Load the new page to device
    if let Some(handle) = find_device() {
        let page = &config.pages[page_index];
        if let Err(e) = load_page_to_device(&handle, page, config.brightness, icons_path) {
            eprintln!("DEBUG: Failed to load page: {}", e);
        }
    }
}

// Start the button listener in a background thread
fn start_button_listener(config_path: PathBuf, icons_path: PathBuf) {
    thread::spawn(move || {
        eprintln!("DEBUG: Button listener started");

        loop {
            // Try to find and open device
            let handle = match find_device() {
                Some(h) => h,
                None => {
                    // Device not found, wait and retry
                    thread::sleep(Duration::from_secs(2));
                    continue;
                }
            };

            eprintln!("DEBUG: Button listener connected to device");

            // Load initial page on connect
            load_current_page_internal(&handle, &config_path, &icons_path);

            // Listen for button presses
            loop {
                // Check if refresh is requested
                if REFRESH_NEEDED.swap(false, Ordering::SeqCst) {
                    eprintln!("DEBUG: Refresh requested, reloading page");
                    load_current_page_internal(&handle, &config_path, &icons_path);
                }

                match read_key_press(&handle) {
                    Ok((key_id, state)) => {
                        if state == 1 {
                            // Key pressed
                            handle_button_press(key_id, &config_path, &icons_path);
                        }
                    }
                    Err(e) => {
                        if e != "timeout" {
                            eprintln!("DEBUG: Button listener error: {}", e);
                            break; // Reconnect
                        }
                    }
                }
            }

            // Wait before reconnecting
            thread::sleep(Duration::from_secs(1));
        }
    });
}

// Internal function to load current page (used by button listener)
fn load_current_page_internal(handle: &DeviceHandle<Context>, config_path: &PathBuf, icons_path: &PathBuf) {
    let config: Config = match fs::read_to_string(config_path) {
        Ok(content) => match serde_json::from_str(&content) {
            Ok(c) => c,
            Err(_) => return,
        },
        Err(_) => return,
    };

    if config.current_page < config.pages.len() {
        let page = &config.pages[config.current_page];
        if let Err(e) = load_page_to_device(handle, page, config.brightness, icons_path) {
            eprintln!("DEBUG: Failed to load page: {}", e);
        }
    }
}

// Signal that a refresh is needed (called from UI)
fn request_refresh() {
    REFRESH_NEEDED.store(true, Ordering::SeqCst);
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

    // Try to set brightness on device
    match find_device() {
        Some(handle) => {
            if let Err(e) = set_device_brightness(&handle, brightness) {
                eprintln!("Warning: Could not set brightness: {}", e);
            }
        }
        None => {
            eprintln!("Warning: Device not found for brightness change");
        }
    }

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

#[tauri::command]
fn save_icon(state: State<AppState>, source_path: String, icon_name: String) -> Result<String, String> {
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

fn chrono_lite() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
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
    use base64::{Engine as _, engine::general_purpose::STANDARD};

    let icon_path = state.icons_path.join(&filename);
    if !icon_path.exists() {
        return Err(format!("Icon not found: {}", filename));
    }

    let data = fs::read(&icon_path)
        .map_err(|e| format!("Failed to read icon: {}", e))?;

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
        ("Vol +".to_string(), "wpctl set-volume @DEFAULT_AUDIO_SINK@ 5%+".to_string(), "Subir volumen".to_string()),
        ("Vol -".to_string(), "wpctl set-volume @DEFAULT_AUDIO_SINK@ 5%-".to_string(), "Bajar volumen".to_string()),
        ("Mute".to_string(), "wpctl set-mute @DEFAULT_AUDIO_SINK@ toggle".to_string(), "Silenciar/Activar audio".to_string()),
        ("Play/Pause".to_string(), "playerctl play-pause".to_string(), "Reproducir/Pausar media".to_string()),
        ("Next".to_string(), "playerctl next".to_string(), "Siguiente pista".to_string()),
        ("Prev".to_string(), "playerctl previous".to_string(), "Pista anterior".to_string()),

        // Apps comunes
        ("Firefox".to_string(), "firefox".to_string(), "Navegador Firefox".to_string()),
        ("Chrome".to_string(), "google-chrome-stable || chromium".to_string(), "Navegador Chrome/Chromium".to_string()),
        ("Terminal".to_string(), "kitty || alacritty || gnome-terminal".to_string(), "Terminal".to_string()),
        ("Files".to_string(), "thunar || nautilus || dolphin".to_string(), "Administrador de archivos".to_string()),
        ("VS Code".to_string(), "code || codium".to_string(), "Visual Studio Code".to_string()),
        ("Discord".to_string(), "discord".to_string(), "Discord".to_string()),
        ("Spotify".to_string(), "spotify".to_string(), "Spotify".to_string()),
        ("Steam".to_string(), "steam".to_string(), "Steam".to_string()),
        ("OBS".to_string(), "obs".to_string(), "OBS Studio".to_string()),

        // Hyprland/Sway workspaces
        ("WS 1".to_string(), "hyprctl dispatch workspace 1".to_string(), "Ir a workspace 1".to_string()),
        ("WS 2".to_string(), "hyprctl dispatch workspace 2".to_string(), "Ir a workspace 2".to_string()),
        ("WS 3".to_string(), "hyprctl dispatch workspace 3".to_string(), "Ir a workspace 3".to_string()),
        ("WS 4".to_string(), "hyprctl dispatch workspace 4".to_string(), "Ir a workspace 4".to_string()),
        ("WS 5".to_string(), "hyprctl dispatch workspace 5".to_string(), "Ir a workspace 5".to_string()),

        // Sistema
        ("Screenshot".to_string(), "grim -g \"$(slurp)\" - | wl-copy".to_string(), "Captura de pantalla".to_string()),
        ("Lock".to_string(), "swaylock || i3lock".to_string(), "Bloquear pantalla".to_string()),
        ("Suspend".to_string(), "systemctl suspend".to_string(), "Suspender sistema".to_string()),

        // Navegación de páginas
        (">> Next".to_string(), "__NEXT_PAGE__".to_string(), "Siguiente página".to_string()),
        ("<< Prev".to_string(), "__PREV_PAGE__".to_string(), "Página anterior".to_string()),
        ("Home".to_string(), "__PAGE_0__".to_string(), "Ir a página principal".to_string()),
    ]
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
            let app_dir = app.path().app_data_dir().unwrap_or_else(|_| PathBuf::from("."));
            fs::create_dir_all(&app_dir).ok();

            let state = AppState::new(app_dir.clone());

            // Start the button listener in background
            let config_path = app_dir.join("config.json");
            let icons_path = app_dir.join("icons");
            start_button_listener(config_path, icons_path);

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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
