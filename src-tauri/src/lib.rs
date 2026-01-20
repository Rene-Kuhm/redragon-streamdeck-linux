use rusb::{Context, DeviceHandle, UsbContext};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{Cursor, Read as IoRead, Write as IoWrite};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::thread;
use tauri::{Manager, State};
use image::{DynamicImage, ImageBuffer, Rgb, RgbImage, imageops};
use imageproc::drawing::{draw_text_mut, text_size};
use ab_glyph::{FontRef, PxScale};
use chrono::{Local, Datelike};
use sysinfo::System;
use tungstenite::{connect, Message};
use sha2::{Sha256, Digest};
use base64::{Engine as _, engine::general_purpose::STANDARD};

// USB IDs for Redragon SS-550
const VENDOR_ID: u16 = 0x0200;
const PRODUCT_ID: u16 = 0x1000;

// Global flag to signal refresh needed
static REFRESH_NEEDED: AtomicBool = AtomicBool::new(false);

// Global timer state (timestamp when timer started, 0 = not running)
static TIMER_START: AtomicU64 = AtomicU64::new(0);
static TIMER_DURATION: AtomicU64 = AtomicU64::new(0); // Duration in seconds

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

    // Determine text to display
    // If command is a widget, show dynamic text; otherwise show label
    let display_text = if is_widget_command(&button.command) {
        get_widget_text(&button.command).unwrap_or_else(|| button.label.clone())
    } else {
        button.label.clone()
    };

    // Draw text if specified
    if !display_text.is_empty() {
        let font_data = include_bytes!("/usr/share/fonts/TTF/DejaVuSans.ttf");
        if let Ok(font) = FontRef::try_from_slice(font_data) {
            let scale = if display_text.len() > 8 {
                PxScale::from(16.0)
            } else if display_text.len() > 5 {
                PxScale::from(20.0)
            } else {
                PxScale::from(28.0)
            };

            let (text_width, text_height) = text_size(scale, &font, &display_text);
            let x = ((BUTTON_SIZE as i32 - text_width as i32) / 2).max(2);
            let y = ((BUTTON_SIZE as i32 - text_height as i32) / 2).max(2);

            // For widgets, draw on top of icon if present (with semi-transparent background)
            if is_widget_command(&button.command) && !button.icon.is_empty() {
                // Draw semi-transparent background for readability
                for py in y.max(0) as u32..(y as u32 + text_height).min(BUTTON_SIZE) {
                    for px in 0..BUTTON_SIZE {
                        let pixel = img.get_pixel_mut(px, py);
                        pixel[0] = (pixel[0] as u16 * 40 / 100) as u8;
                        pixel[1] = (pixel[1] as u16 * 40 / 100) as u8;
                        pixel[2] = (pixel[2] as u16 * 40 / 100) as u8;
                    }
                }
            }

            draw_text_mut(&mut img, Rgb([255, 255, 255]), x, y, scale, &font, &display_text);
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
// Hotkey Functions (ydotool for Wayland)
// ============================================================================

// Map key names to ydotool key codes
fn key_name_to_code(key: &str) -> Option<&'static str> {
    match key.to_lowercase().as_str() {
        // Modifiers
        "ctrl" | "control" => Some("29"),      // KEY_LEFTCTRL
        "shift" => Some("42"),                  // KEY_LEFTSHIFT
        "alt" => Some("56"),                    // KEY_LEFTALT
        "super" | "win" | "meta" => Some("125"), // KEY_LEFTMETA

        // Function keys
        "f1" => Some("59"),
        "f2" => Some("60"),
        "f3" => Some("61"),
        "f4" => Some("62"),
        "f5" => Some("63"),
        "f6" => Some("64"),
        "f7" => Some("65"),
        "f8" => Some("66"),
        "f9" => Some("67"),
        "f10" => Some("68"),
        "f11" => Some("87"),
        "f12" => Some("88"),

        // Special keys
        "esc" | "escape" => Some("1"),
        "tab" => Some("15"),
        "enter" | "return" => Some("28"),
        "space" => Some("57"),
        "backspace" => Some("14"),
        "delete" | "del" => Some("111"),
        "insert" | "ins" => Some("110"),
        "home" => Some("102"),
        "end" => Some("107"),
        "pageup" | "pgup" => Some("104"),
        "pagedown" | "pgdn" => Some("109"),
        "up" => Some("103"),
        "down" => Some("108"),
        "left" => Some("105"),
        "right" => Some("106"),
        "printscreen" | "prtsc" => Some("99"),
        "pause" => Some("119"),
        "capslock" => Some("58"),
        "numlock" => Some("69"),
        "scrolllock" => Some("70"),

        // Letters
        "a" => Some("30"),
        "b" => Some("48"),
        "c" => Some("46"),
        "d" => Some("32"),
        "e" => Some("18"),
        "f" => Some("33"),
        "g" => Some("34"),
        "h" => Some("35"),
        "i" => Some("23"),
        "j" => Some("36"),
        "k" => Some("37"),
        "l" => Some("38"),
        "m" => Some("50"),
        "n" => Some("49"),
        "o" => Some("24"),
        "p" => Some("25"),
        "q" => Some("16"),
        "r" => Some("19"),
        "s" => Some("31"),
        "t" => Some("20"),
        "u" => Some("22"),
        "v" => Some("47"),
        "w" => Some("17"),
        "x" => Some("45"),
        "y" => Some("21"),
        "z" => Some("44"),

        // Numbers
        "0" => Some("11"),
        "1" => Some("2"),
        "2" => Some("3"),
        "3" => Some("4"),
        "4" => Some("5"),
        "5" => Some("6"),
        "6" => Some("7"),
        "7" => Some("8"),
        "8" => Some("9"),
        "9" => Some("10"),

        // Symbols
        "-" | "minus" => Some("12"),
        "=" | "equal" => Some("13"),
        "[" | "leftbracket" => Some("26"),
        "]" | "rightbracket" => Some("27"),
        "\\" | "backslash" => Some("43"),
        ";" | "semicolon" => Some("39"),
        "'" | "apostrophe" => Some("40"),
        "`" | "grave" => Some("41"),
        "," | "comma" => Some("51"),
        "." | "period" => Some("52"),
        "/" | "slash" => Some("53"),

        // Media keys
        "volumeup" | "volup" => Some("115"),
        "volumedown" | "voldown" => Some("114"),
        "mute" => Some("113"),
        "playpause" | "play" => Some("164"),
        "stop" => Some("166"),
        "next" | "nextsong" => Some("163"),
        "prev" | "previoussong" => Some("165"),

        // Numpad
        "kp0" | "numpad0" => Some("82"),
        "kp1" | "numpad1" => Some("79"),
        "kp2" | "numpad2" => Some("80"),
        "kp3" | "numpad3" => Some("81"),
        "kp4" | "numpad4" => Some("75"),
        "kp5" | "numpad5" => Some("76"),
        "kp6" | "numpad6" => Some("77"),
        "kp7" | "numpad7" => Some("71"),
        "kp8" | "numpad8" => Some("72"),
        "kp9" | "numpad9" => Some("73"),
        "kpenter" | "numpadenter" => Some("96"),
        "kpplus" | "numpadplus" => Some("78"),
        "kpminus" | "numpadminus" => Some("74"),
        "kpmultiply" | "numpadmultiply" | "kpasterisk" => Some("55"),
        "kpdivide" | "numpaddivide" | "kpslash" => Some("98"),
        "kpdot" | "numpaddot" | "kpperiod" => Some("83"),

        // Additional useful keys
        "menu" | "contextmenu" => Some("127"),  // KEY_COMPOSE / context menu
        "rctrl" | "rightctrl" => Some("97"),
        "rshift" | "rightshift" => Some("54"),
        "ralt" | "rightalt" | "altgr" => Some("100"),
        "rsuper" | "rightsuper" | "rwin" => Some("126"),

        _ => None,
    }
}

// Execute hotkey asynchronously
fn execute_hotkey(keys: &str) {
    let keys_clone = keys.to_string();
    thread::spawn(move || {
        execute_hotkey_sync(&keys_clone);
    });
}

// Execute hotkey synchronously
fn execute_hotkey_sync(keys: &str) {
    // Parse keys like "ctrl+shift+a" or "alt+tab"
    let key_parts: Vec<&str> = keys.split('+').collect();

    // Build ydotool key sequence
    // Format: key codes with :1 for press, :0 for release
    let mut key_codes: Vec<String> = Vec::new();

    for key in &key_parts {
        if let Some(code) = key_name_to_code(key.trim()) {
            key_codes.push(format!("{}:1", code)); // Press
        }
    }

    // Release in reverse order
    for key in key_parts.iter().rev() {
        if let Some(code) = key_name_to_code(key.trim()) {
            key_codes.push(format!("{}:0", code)); // Release
        }
    }

    if !key_codes.is_empty() {
        let key_sequence = key_codes.join(" ");
        eprintln!("DEBUG: ydotool key {}", key_sequence);

        Command::new("ydotool")
            .arg("key")
            .args(key_codes)
            .status()
            .ok();
    }
}

// ============================================================================
// Widget Functions (Dynamic Content)
// ============================================================================

// Get current time as string
fn get_widget_clock() -> String {
    Local::now().format("%H:%M").to_string()
}

// Get current time with seconds
fn get_widget_clock_seconds() -> String {
    Local::now().format("%H:%M:%S").to_string()
}

// Get current date as string
fn get_widget_date() -> String {
    Local::now().format("%d/%m").to_string()
}

// Get current date with year
fn get_widget_date_full() -> String {
    Local::now().format("%d/%m/%Y").to_string()
}

// Get day of week
fn get_widget_weekday() -> String {
    let weekdays = ["Dom", "Lun", "Mar", "Mié", "Jue", "Vie", "Sáb"];
    let day = Local::now().weekday().num_days_from_sunday() as usize;
    weekdays[day].to_string()
}

// Get CPU usage percentage
fn get_widget_cpu() -> String {
    let mut sys = System::new();
    sys.refresh_cpu_usage();
    thread::sleep(Duration::from_millis(200));
    sys.refresh_cpu_usage();
    let cpu_usage = sys.global_cpu_usage();
    format!("{:.0}%", cpu_usage)
}

// Get RAM usage percentage
fn get_widget_ram() -> String {
    let mut sys = System::new_all();
    sys.refresh_memory();
    let used = sys.used_memory() as f64;
    let total = sys.total_memory() as f64;
    let percent = (used / total) * 100.0;
    format!("{:.0}%", percent)
}

// Get CPU temperature (Linux-specific)
fn get_widget_temp() -> String {
    // Try to read from thermal zone
    if let Ok(temp) = fs::read_to_string("/sys/class/thermal/thermal_zone0/temp") {
        if let Ok(millidegrees) = temp.trim().parse::<i32>() {
            return format!("{}°C", millidegrees / 1000);
        }
    }
    // Fallback: try hwmon
    for i in 0..10 {
        let path = format!("/sys/class/hwmon/hwmon{}/temp1_input", i);
        if let Ok(temp) = fs::read_to_string(&path) {
            if let Ok(millidegrees) = temp.trim().parse::<i32>() {
                return format!("{}°C", millidegrees / 1000);
            }
        }
    }
    "N/A".to_string()
}

// Get timer remaining time
fn get_widget_timer() -> String {
    let start = TIMER_START.load(Ordering::Relaxed);
    let duration = TIMER_DURATION.load(Ordering::Relaxed);

    if start == 0 || duration == 0 {
        return "00:00".to_string();
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let elapsed = now.saturating_sub(start);
    let remaining = duration.saturating_sub(elapsed);

    if remaining == 0 {
        // Timer finished
        TIMER_START.store(0, Ordering::Relaxed);
        TIMER_DURATION.store(0, Ordering::Relaxed);
        return "DONE!".to_string();
    }

    let mins = remaining / 60;
    let secs = remaining % 60;
    format!("{:02}:{:02}", mins, secs)
}

// Start a timer with given duration in seconds
fn start_timer(duration_secs: u64) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    TIMER_START.store(now, Ordering::Relaxed);
    TIMER_DURATION.store(duration_secs, Ordering::Relaxed);
}

// Stop/reset timer
fn stop_timer() {
    TIMER_START.store(0, Ordering::Relaxed);
    TIMER_DURATION.store(0, Ordering::Relaxed);
}

// Check if a command is a widget that needs dynamic updates
fn is_widget_command(cmd: &str) -> bool {
    cmd.starts_with("__CLOCK") ||
    cmd.starts_with("__DATE") ||
    cmd.starts_with("__WEEKDAY") ||
    cmd.starts_with("__CPU") ||
    cmd.starts_with("__RAM") ||
    cmd.starts_with("__TEMP") ||
    cmd.starts_with("__TIMER") ||
    cmd == "__OBS_STATUS__" ||
    cmd == "__TWITCH_VIEWERS__" ||
    cmd == "__TWITCH_FOLLOWERS__"
}

// Get the display text for a widget command
fn get_widget_text(cmd: &str) -> Option<String> {
    if cmd == "__CLOCK__" {
        Some(get_widget_clock())
    } else if cmd == "__CLOCK_S__" {
        Some(get_widget_clock_seconds())
    } else if cmd == "__DATE__" {
        Some(get_widget_date())
    } else if cmd == "__DATE_FULL__" {
        Some(get_widget_date_full())
    } else if cmd == "__WEEKDAY__" {
        Some(get_widget_weekday())
    } else if cmd == "__CPU__" {
        Some(get_widget_cpu())
    } else if cmd == "__RAM__" {
        Some(get_widget_ram())
    } else if cmd == "__TEMP__" {
        Some(get_widget_temp())
    } else if cmd.starts_with("__TIMER_") && cmd.ends_with("__") {
        // __TIMER_5__ means 5 minute timer, show remaining time
        Some(get_widget_timer())
    } else if cmd == "__OBS_STATUS__" {
        Some(get_obs_status_text())
    } else if cmd == "__TWITCH_VIEWERS__" {
        Some(get_twitch_viewers_text())
    } else if cmd == "__TWITCH_FOLLOWERS__" {
        Some(get_twitch_followers_text())
    } else {
        None
    }
}

// ============================================================================
// OBS WebSocket Integration (obs-websocket 5.x)
// ============================================================================

use std::sync::RwLock;

// OBS connection state
lazy_static::lazy_static! {
    static ref OBS_STATE: RwLock<ObsState> = RwLock::new(ObsState::default());
}

#[derive(Default, Clone)]
struct ObsState {
    connected: bool,
    streaming: bool,
    recording: bool,
    current_scene: String,
    scenes: Vec<String>,
    muted: bool,
    last_error: String,
}

// OBS WebSocket message types
#[derive(Serialize, Deserialize, Debug)]
struct ObsMessage {
    op: u8,
    d: serde_json::Value,
}

// Connect to OBS WebSocket and authenticate
fn obs_connect(host: &str, port: u16, password: Option<&str>) -> Result<(), String> {
    let url = format!("ws://{}:{}", host, port);
    eprintln!("DEBUG: OBS connecting to {}", url);

    let (mut socket, _response) = connect(&url)
        .map_err(|e| format!("OBS connection failed: {}", e))?;

    // Read Hello message (op=0)
    let hello_msg = socket.read()
        .map_err(|e| format!("Failed to read OBS Hello: {}", e))?;

    let hello: ObsMessage = match hello_msg {
        Message::Text(text) => serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse Hello: {}", e))?,
        _ => return Err("Unexpected message type".to_string()),
    };

    if hello.op != 0 {
        return Err(format!("Expected Hello (op=0), got op={}", hello.op));
    }

    eprintln!("DEBUG: OBS Hello received");

    // Check if authentication is required
    let auth_data = hello.d.get("authentication");

    let identify_data = if let (Some(auth), Some(pwd)) = (auth_data, password) {
        // Authentication required
        let challenge = auth.get("challenge")
            .and_then(|v| v.as_str())
            .ok_or("Missing challenge")?;
        let salt = auth.get("salt")
            .and_then(|v| v.as_str())
            .ok_or("Missing salt")?;

        // Generate authentication string
        let auth_string = generate_obs_auth(pwd, challenge, salt);

        serde_json::json!({
            "rpcVersion": 1,
            "authentication": auth_string
        })
    } else {
        // No authentication required
        serde_json::json!({
            "rpcVersion": 1
        })
    };

    // Send Identify message (op=1)
    let identify = ObsMessage {
        op: 1,
        d: identify_data,
    };
    socket.send(Message::Text(serde_json::to_string(&identify).unwrap().into()))
        .map_err(|e| format!("Failed to send Identify: {}", e))?;

    // Read Identified response (op=2)
    let identified_msg = socket.read()
        .map_err(|e| format!("Failed to read Identified: {}", e))?;

    let identified: ObsMessage = match identified_msg {
        Message::Text(text) => serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse Identified: {}", e))?,
        _ => return Err("Unexpected message type".to_string()),
    };

    if identified.op != 2 {
        return Err(format!("Authentication failed (op={})", identified.op));
    }

    eprintln!("DEBUG: OBS authenticated successfully");

    // Update state
    if let Ok(mut state) = OBS_STATE.write() {
        state.connected = true;
        state.last_error.clear();
    }

    // Get initial state
    obs_update_status_internal(&mut socket)?;

    Ok(())
}

// Generate OBS authentication string (SHA256)
fn generate_obs_auth(password: &str, challenge: &str, salt: &str) -> String {
    // base64(SHA256(base64(SHA256(password + salt)) + challenge))
    let pass_salt = format!("{}{}", password, salt);
    let hash1 = Sha256::digest(pass_salt.as_bytes());
    let base64_hash1 = STANDARD.encode(hash1);

    let hash1_challenge = format!("{}{}", base64_hash1, challenge);
    let hash2 = Sha256::digest(hash1_challenge.as_bytes());
    STANDARD.encode(hash2)
}

// Send OBS request and get response
fn obs_request(request_type: &str, request_data: Option<serde_json::Value>) -> Result<serde_json::Value, String> {
    let url = get_obs_websocket_url();
    let password = get_obs_password();

    let (mut socket, _) = connect(&url)
        .map_err(|e| format!("OBS connection failed: {}", e))?;

    // Read Hello
    let hello_msg = socket.read().map_err(|e| format!("Read error: {}", e))?;
    let hello: ObsMessage = match hello_msg {
        Message::Text(text) => serde_json::from_str(&text).map_err(|e| format!("Parse error: {}", e))?,
        _ => return Err("Unexpected message".to_string()),
    };

    // Authenticate
    let auth_data = hello.d.get("authentication");
    let identify_data = if let Some(auth) = auth_data {
        let challenge = auth.get("challenge").and_then(|v| v.as_str()).ok_or("Missing challenge")?;
        let salt = auth.get("salt").and_then(|v| v.as_str()).ok_or("Missing salt")?;
        let auth_string = generate_obs_auth(&password, challenge, salt);
        serde_json::json!({"rpcVersion": 1, "authentication": auth_string})
    } else {
        serde_json::json!({"rpcVersion": 1})
    };

    let identify = ObsMessage { op: 1, d: identify_data };
    socket.send(Message::Text(serde_json::to_string(&identify).unwrap().into())).ok();

    let _ = socket.read(); // Read Identified

    // Send request (op=6)
    let request_id = format!("req_{}", chrono_lite());
    let mut req_data = serde_json::json!({
        "requestType": request_type,
        "requestId": request_id
    });
    if let Some(data) = request_data {
        req_data["requestData"] = data;
    }

    let request = ObsMessage { op: 6, d: req_data };
    socket.send(Message::Text(serde_json::to_string(&request).unwrap().into()))
        .map_err(|e| format!("Send error: {}", e))?;

    // Read response (op=7)
    let response_msg = socket.read().map_err(|e| format!("Read error: {}", e))?;
    let response: ObsMessage = match response_msg {
        Message::Text(text) => serde_json::from_str(&text).map_err(|e| format!("Parse error: {}", e))?,
        _ => return Err("Unexpected message".to_string()),
    };

    if response.op == 7 {
        let status = response.d.get("requestStatus");
        if let Some(status) = status {
            if status.get("result").and_then(|v| v.as_bool()).unwrap_or(false) {
                return Ok(response.d.get("responseData").cloned().unwrap_or(serde_json::Value::Null));
            } else {
                let comment = status.get("comment").and_then(|v| v.as_str()).unwrap_or("Unknown error");
                return Err(comment.to_string());
            }
        }
    }

    Err("Invalid response".to_string())
}

// Update OBS status internally
fn obs_update_status_internal(socket: &mut tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<TcpStream>>) -> Result<(), String> {
    // Get streaming status
    let req_id = format!("status_{}", chrono_lite());
    let request = ObsMessage {
        op: 6,
        d: serde_json::json!({
            "requestType": "GetStreamStatus",
            "requestId": req_id
        }),
    };
    socket.send(Message::Text(serde_json::to_string(&request).unwrap().into())).ok();

    if let Ok(Message::Text(text)) = socket.read() {
        if let Ok(response) = serde_json::from_str::<ObsMessage>(&text) {
            if response.op == 7 {
                if let Some(data) = response.d.get("responseData") {
                    let streaming = data.get("outputActive").and_then(|v| v.as_bool()).unwrap_or(false);
                    if let Ok(mut state) = OBS_STATE.write() {
                        state.streaming = streaming;
                    }
                }
            }
        }
    }

    // Get recording status
    let req_id = format!("rec_{}", chrono_lite());
    let request = ObsMessage {
        op: 6,
        d: serde_json::json!({
            "requestType": "GetRecordStatus",
            "requestId": req_id
        }),
    };
    socket.send(Message::Text(serde_json::to_string(&request).unwrap().into())).ok();

    if let Ok(Message::Text(text)) = socket.read() {
        if let Ok(response) = serde_json::from_str::<ObsMessage>(&text) {
            if response.op == 7 {
                if let Some(data) = response.d.get("responseData") {
                    let recording = data.get("outputActive").and_then(|v| v.as_bool()).unwrap_or(false);
                    if let Ok(mut state) = OBS_STATE.write() {
                        state.recording = recording;
                    }
                }
            }
        }
    }

    // Get current scene
    let req_id = format!("scene_{}", chrono_lite());
    let request = ObsMessage {
        op: 6,
        d: serde_json::json!({
            "requestType": "GetCurrentProgramScene",
            "requestId": req_id
        }),
    };
    socket.send(Message::Text(serde_json::to_string(&request).unwrap().into())).ok();

    if let Ok(Message::Text(text)) = socket.read() {
        if let Ok(response) = serde_json::from_str::<ObsMessage>(&text) {
            if response.op == 7 {
                if let Some(data) = response.d.get("responseData") {
                    let scene = data.get("currentProgramSceneName")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    if let Ok(mut state) = OBS_STATE.write() {
                        state.current_scene = scene;
                    }
                }
            }
        }
    }

    Ok(())
}

// OBS config (stored in a simple way)
fn get_obs_websocket_url() -> String {
    std::env::var("OBS_WEBSOCKET_URL").unwrap_or_else(|_| "ws://localhost:4455".to_string())
}

fn get_obs_password() -> String {
    std::env::var("OBS_WEBSOCKET_PASSWORD").unwrap_or_default()
}

// OBS Commands for button presses
fn obs_toggle_stream() {
    thread::spawn(|| {
        match obs_request("ToggleStream", None) {
            Ok(_) => eprintln!("DEBUG: OBS stream toggled"),
            Err(e) => eprintln!("DEBUG: OBS toggle stream error: {}", e),
        }
    });
}

fn obs_toggle_record() {
    thread::spawn(|| {
        match obs_request("ToggleRecord", None) {
            Ok(_) => eprintln!("DEBUG: OBS record toggled"),
            Err(e) => eprintln!("DEBUG: OBS toggle record error: {}", e),
        }
    });
}

fn obs_toggle_mute() {
    thread::spawn(|| {
        // Toggle mute for default audio input
        match obs_request("ToggleInputMute", Some(serde_json::json!({"inputName": "Mic/Aux"}))) {
            Ok(_) => eprintln!("DEBUG: OBS mic mute toggled"),
            Err(e) => {
                // Try alternative input name
                match obs_request("ToggleInputMute", Some(serde_json::json!({"inputName": "Desktop Audio"}))) {
                    Ok(_) => eprintln!("DEBUG: OBS desktop audio mute toggled"),
                    Err(e2) => eprintln!("DEBUG: OBS toggle mute error: {} / {}", e, e2),
                }
            }
        }
    });
}

fn obs_set_scene(scene_name: &str) {
    let scene = scene_name.to_string();
    thread::spawn(move || {
        match obs_request("SetCurrentProgramScene", Some(serde_json::json!({"sceneName": scene}))) {
            Ok(_) => eprintln!("DEBUG: OBS scene changed to: {}", scene),
            Err(e) => eprintln!("DEBUG: OBS set scene error: {}", e),
        }
    });
}

// Get OBS status text for widget display
fn get_obs_status_text() -> String {
    // Try to update status first (non-blocking)
    let _ = thread::spawn(|| {
        if let Ok(data) = obs_request("GetStreamStatus", None) {
            let streaming = data.get("outputActive").and_then(|v| v.as_bool()).unwrap_or(false);
            if let Ok(mut state) = OBS_STATE.write() {
                state.streaming = streaming;
                state.connected = true;
            }
        }
        if let Ok(data) = obs_request("GetRecordStatus", None) {
            let recording = data.get("outputActive").and_then(|v| v.as_bool()).unwrap_or(false);
            if let Ok(mut state) = OBS_STATE.write() {
                state.recording = recording;
            }
        }
    });

    // Return current cached state
    if let Ok(state) = OBS_STATE.read() {
        if !state.connected {
            return "OBS OFF".to_string();
        }
        let s = if state.streaming { "LIVE" } else { "---" };
        let r = if state.recording { "REC" } else { "---" };
        format!("{}/{}", s, r)
    } else {
        "OBS?".to_string()
    }
}

// ============================================================================
// Twitch API Integration
// ============================================================================

lazy_static::lazy_static! {
    static ref TWITCH_STATE: RwLock<TwitchState> = RwLock::new(TwitchState::default());
}

#[derive(Default, Clone)]
struct TwitchState {
    connected: bool,
    channel_name: String,
    viewers: u32,
    followers: u32,
    is_live: bool,
    access_token: String,
    client_id: String,
    broadcaster_id: String,
    last_update: u64,
}

// Twitch config from environment
fn get_twitch_client_id() -> String {
    std::env::var("TWITCH_CLIENT_ID").unwrap_or_default()
}

fn get_twitch_access_token() -> String {
    std::env::var("TWITCH_ACCESS_TOKEN").unwrap_or_default()
}

fn get_twitch_channel() -> String {
    std::env::var("TWITCH_CHANNEL").unwrap_or_default()
}

// Initialize Twitch connection and get broadcaster ID
fn twitch_init() -> Result<(), String> {
    let client_id = get_twitch_client_id();
    let access_token = get_twitch_access_token();
    let channel = get_twitch_channel();

    if client_id.is_empty() || access_token.is_empty() || channel.is_empty() {
        return Err("Twitch credentials not configured".to_string());
    }

    // Get broadcaster ID from channel name
    let client = reqwest::blocking::Client::new();
    let resp = client
        .get(format!("https://api.twitch.tv/helix/users?login={}", channel))
        .header("Client-ID", &client_id)
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .map_err(|e| format!("Twitch API error: {}", e))?;

    let data: serde_json::Value = resp.json()
        .map_err(|e| format!("Parse error: {}", e))?;

    let broadcaster_id = data["data"][0]["id"]
        .as_str()
        .ok_or("Could not get broadcaster ID")?
        .to_string();

    if let Ok(mut state) = TWITCH_STATE.write() {
        state.connected = true;
        state.channel_name = channel;
        state.client_id = client_id;
        state.access_token = access_token;
        state.broadcaster_id = broadcaster_id;
    }

    Ok(())
}

// Get current viewers (for live streams)
fn twitch_get_viewers() -> Result<u32, String> {
    let (client_id, access_token, broadcaster_id) = {
        let state = TWITCH_STATE.read().map_err(|e| e.to_string())?;
        if !state.connected || state.broadcaster_id.is_empty() {
            return Err("Twitch not connected".to_string());
        }
        (state.client_id.clone(), state.access_token.clone(), state.broadcaster_id.clone())
    };

    let client = reqwest::blocking::Client::new();
    let resp = client
        .get(format!("https://api.twitch.tv/helix/streams?user_id={}", broadcaster_id))
        .header("Client-ID", &client_id)
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .map_err(|e| format!("API error: {}", e))?;

    let data: serde_json::Value = resp.json().map_err(|e| format!("Parse error: {}", e))?;

    let viewers = data["data"][0]["viewer_count"]
        .as_u64()
        .unwrap_or(0) as u32;

    let is_live = data["data"].as_array().map(|a| !a.is_empty()).unwrap_or(false);

    if let Ok(mut state) = TWITCH_STATE.write() {
        state.viewers = viewers;
        state.is_live = is_live;
        state.last_update = chrono_lite();
    }

    Ok(viewers)
}

// Get follower count
fn twitch_get_followers() -> Result<u32, String> {
    let (client_id, access_token, broadcaster_id) = {
        let state = TWITCH_STATE.read().map_err(|e| e.to_string())?;
        if !state.connected || state.broadcaster_id.is_empty() {
            return Err("Twitch not connected".to_string());
        }
        (state.client_id.clone(), state.access_token.clone(), state.broadcaster_id.clone())
    };

    let client = reqwest::blocking::Client::new();
    let resp = client
        .get(format!("https://api.twitch.tv/helix/channels/followers?broadcaster_id={}", broadcaster_id))
        .header("Client-ID", &client_id)
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .map_err(|e| format!("API error: {}", e))?;

    let data: serde_json::Value = resp.json().map_err(|e| format!("Parse error: {}", e))?;

    let followers = data["total"].as_u64().unwrap_or(0) as u32;

    if let Ok(mut state) = TWITCH_STATE.write() {
        state.followers = followers;
    }

    Ok(followers)
}

// Send chat message
fn twitch_send_chat(message: &str) {
    let msg = message.to_string();
    thread::spawn(move || {
        let (client_id, access_token, broadcaster_id) = {
            if let Ok(state) = TWITCH_STATE.read() {
                (state.client_id.clone(), state.access_token.clone(), state.broadcaster_id.clone())
            } else {
                return;
            }
        };

        if broadcaster_id.is_empty() {
            eprintln!("DEBUG: Twitch not connected");
            return;
        }

        let client = reqwest::blocking::Client::new();
        let _ = client
            .post("https://api.twitch.tv/helix/chat/messages")
            .header("Client-ID", &client_id)
            .header("Authorization", format!("Bearer {}", access_token))
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({
                "broadcaster_id": broadcaster_id,
                "sender_id": broadcaster_id,
                "message": msg
            }))
            .send();

        eprintln!("DEBUG: Twitch chat message sent: {}", msg);
    });
}

// Create clip
fn twitch_create_clip() {
    thread::spawn(|| {
        let (client_id, access_token, broadcaster_id) = {
            if let Ok(state) = TWITCH_STATE.read() {
                (state.client_id.clone(), state.access_token.clone(), state.broadcaster_id.clone())
            } else {
                return;
            }
        };

        if broadcaster_id.is_empty() {
            eprintln!("DEBUG: Twitch not connected");
            return;
        }

        let client = reqwest::blocking::Client::new();
        match client
            .post(format!("https://api.twitch.tv/helix/clips?broadcaster_id={}", broadcaster_id))
            .header("Client-ID", &client_id)
            .header("Authorization", format!("Bearer {}", access_token))
            .send()
        {
            Ok(resp) => {
                if let Ok(data) = resp.json::<serde_json::Value>() {
                    if let Some(clip_id) = data["data"][0]["id"].as_str() {
                        eprintln!("DEBUG: Twitch clip created: {}", clip_id);
                    }
                }
            }
            Err(e) => eprintln!("DEBUG: Twitch create clip error: {}", e),
        }
    });
}

// Run commercial (ads)
fn twitch_run_commercial(length: u32) {
    thread::spawn(move || {
        let (client_id, access_token, broadcaster_id) = {
            if let Ok(state) = TWITCH_STATE.read() {
                (state.client_id.clone(), state.access_token.clone(), state.broadcaster_id.clone())
            } else {
                return;
            }
        };

        if broadcaster_id.is_empty() {
            eprintln!("DEBUG: Twitch not connected");
            return;
        }

        let client = reqwest::blocking::Client::new();
        let _ = client
            .post("https://api.twitch.tv/helix/channels/commercial")
            .header("Client-ID", &client_id)
            .header("Authorization", format!("Bearer {}", access_token))
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({
                "broadcaster_id": broadcaster_id,
                "length": length
            }))
            .send();

        eprintln!("DEBUG: Twitch commercial started: {}s", length);
    });
}

// Widget text functions
fn get_twitch_viewers_text() -> String {
    // Try to update (cached for 30 seconds)
    let should_update = {
        if let Ok(state) = TWITCH_STATE.read() {
            let now = chrono_lite();
            !state.connected || now - state.last_update > 30
        } else {
            true
        }
    };

    if should_update {
        let _ = thread::spawn(|| {
            if let Err(_) = twitch_init() {
                return;
            }
            let _ = twitch_get_viewers();
        });
    }

    if let Ok(state) = TWITCH_STATE.read() {
        if !state.connected {
            return "TWITCH".to_string();
        }
        if state.is_live {
            format!("{}v", state.viewers)
        } else {
            "OFFLINE".to_string()
        }
    } else {
        "---".to_string()
    }
}

fn get_twitch_followers_text() -> String {
    let should_update = {
        if let Ok(state) = TWITCH_STATE.read() {
            let now = chrono_lite();
            !state.connected || now - state.last_update > 60
        } else {
            true
        }
    };

    if should_update {
        let _ = thread::spawn(|| {
            if let Err(_) = twitch_init() {
                return;
            }
            let _ = twitch_get_followers();
        });
    }

    if let Ok(state) = TWITCH_STATE.read() {
        if !state.connected {
            return "TWITCH".to_string();
        }
        format!("{}f", state.followers)
    } else {
        "---".to_string()
    }
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

    // Handle __TIMER_N__ - start/stop timer (N = minutes)
    if cmd.starts_with("__TIMER_") && cmd.ends_with("__") {
        let timer_str = &cmd[8..cmd.len()-2];
        if let Ok(minutes) = timer_str.parse::<u64>() {
            // Toggle timer: if running, stop; if stopped, start
            let current_start = TIMER_START.load(Ordering::Relaxed);
            if current_start > 0 {
                // Timer is running, stop it
                stop_timer();
                eprintln!("DEBUG: Timer stopped");
            } else {
                // Start timer with N minutes
                start_timer(minutes * 60);
                eprintln!("DEBUG: Timer started for {} minutes", minutes);
            }
            // Request refresh to update display
            request_refresh();
        }
        return;
    }

    // Handle widget display commands (they don't execute anything, just display)
    if cmd == "__CLOCK__" || cmd == "__CLOCK_S__" ||
       cmd == "__DATE__" || cmd == "__DATE_FULL__" ||
       cmd == "__WEEKDAY__" ||
       cmd == "__CPU__" || cmd == "__RAM__" || cmd == "__TEMP__" ||
       cmd == "__OBS_STATUS__" || cmd == "__TWITCH_VIEWERS__" || cmd == "__TWITCH_FOLLOWERS__" {
        // Widgets don't execute anything when pressed, they just display info
        // But we can request a refresh to show updated value
        request_refresh();
        return;
    }

    // Handle OBS commands
    if cmd == "__OBS_STREAM__" {
        eprintln!("DEBUG: OBS toggle stream");
        obs_toggle_stream();
        return;
    }
    if cmd == "__OBS_RECORD__" {
        eprintln!("DEBUG: OBS toggle record");
        obs_toggle_record();
        return;
    }
    if cmd == "__OBS_MUTE__" {
        eprintln!("DEBUG: OBS toggle mute");
        obs_toggle_mute();
        return;
    }
    // Handle __OBS_SCENE_scenename pattern
    if cmd.starts_with("__OBS_SCENE_") {
        let scene_name = &cmd[12..];
        eprintln!("DEBUG: OBS set scene: {}", scene_name);
        obs_set_scene(scene_name);
        return;
    }

    // Handle Twitch commands
    // __TWITCH_CHAT_message - send chat message
    if cmd.starts_with("__TWITCH_CHAT_") {
        let message = &cmd[14..];
        eprintln!("DEBUG: Twitch chat: {}", message);
        twitch_send_chat(message);
        return;
    }
    // __TWITCH_CLIP__ - create clip
    if cmd == "__TWITCH_CLIP__" {
        eprintln!("DEBUG: Twitch create clip");
        twitch_create_clip();
        return;
    }
    // __TWITCH_AD_30__, __TWITCH_AD_60__, etc. - run commercial
    if cmd.starts_with("__TWITCH_AD_") && cmd.ends_with("__") {
        let length_str = &cmd[12..cmd.len()-2];
        if let Ok(length) = length_str.parse::<u32>() {
            eprintln!("DEBUG: Twitch commercial: {}s", length);
            twitch_run_commercial(length);
        }
        return;
    }

    // Handle __URL_ command - open URL in default browser
    if cmd.starts_with("__URL_") {
        let url = &cmd[6..];
        eprintln!("DEBUG: Opening URL: {}", url);
        let url_clone = url.to_string();
        thread::spawn(move || {
            Command::new("xdg-open")
                .arg(&url_clone)
                .spawn()
                .ok();
        });
        return;
    }

    // Handle __TYPE_ command - type text using ydotool
    if cmd.starts_with("__TYPE_") {
        let text = &cmd[7..];
        eprintln!("DEBUG: Typing text: {}", text);
        let text_clone = text.to_string();
        thread::spawn(move || {
            Command::new("ydotool")
                .args(["type", "--clearmodifiers", &text_clone])
                .spawn()
                .ok();
        });
        return;
    }

    // Handle __KEY_ command - simulate key press using ydotool
    if cmd.starts_with("__KEY_") {
        let keys = &cmd[6..];
        eprintln!("DEBUG: Pressing keys: {}", keys);
        execute_hotkey(keys);
        return;
    }

    // Handle __MULTI_ command - execute multiple commands in sequence
    if cmd.starts_with("__MULTI_") {
        let commands = &cmd[8..];
        eprintln!("DEBUG: Executing multi-action: {}", commands);
        let commands_clone = commands.to_string();
        thread::spawn(move || {
            for single_cmd in commands_clone.split(";;") {
                let trimmed = single_cmd.trim();
                if !trimmed.is_empty() {
                    eprintln!("DEBUG: Multi-action step: {}", trimmed);
                    // Handle special commands within multi-action
                    if trimmed.starts_with("__URL_") {
                        let url = &trimmed[6..];
                        Command::new("xdg-open").arg(url).spawn().ok();
                    } else if trimmed.starts_with("__TYPE_") {
                        let text = &trimmed[7..];
                        Command::new("ydotool")
                            .args(["type", "--clearmodifiers", text])
                            .status()
                            .ok();
                    } else if trimmed.starts_with("__KEY_") {
                        let keys = &trimmed[6..];
                        execute_hotkey_sync(keys);
                    } else if trimmed.starts_with("__DELAY_") {
                        if let Ok(ms) = trimmed[8..].parse::<u64>() {
                            thread::sleep(Duration::from_millis(ms));
                        }
                    } else {
                        // Normal shell command
                        Command::new("sh")
                            .arg("-c")
                            .arg(trimmed)
                            .status()
                            .ok();
                    }
                    // Small delay between actions
                    thread::sleep(Duration::from_millis(100));
                }
            }
        });
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

            // Widget update counter (update every ~10 loop iterations = ~1 second)
            let mut widget_counter: u32 = 0;
            let widget_update_interval: u32 = 10;

            // Listen for button presses
            loop {
                // Check if refresh is requested
                if REFRESH_NEEDED.swap(false, Ordering::SeqCst) {
                    eprintln!("DEBUG: Refresh requested, reloading page");
                    load_current_page_internal(&handle, &config_path, &icons_path);
                    widget_counter = 0; // Reset counter after full refresh
                }

                // Periodically update widgets (every ~1 second)
                widget_counter += 1;
                if widget_counter >= widget_update_interval {
                    widget_counter = 0;
                    update_widget_buttons(&handle, &config_path, &icons_path);
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

// Update only buttons that have widget commands
fn update_widget_buttons(handle: &DeviceHandle<Context>, config_path: &PathBuf, icons_path: &PathBuf) {
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

    // Find buttons with widget commands and update them
    for (key_str, button) in &page.buttons {
        if is_widget_command(&button.command) {
            if let Ok(key_id) = key_str.parse::<u8>() {
                // Generate new image for this widget button
                match generate_button_image(button, icons_path) {
                    Ok(jpeg_data) => {
                        if let Err(e) = set_key_image(handle, key_id, &jpeg_data) {
                            eprintln!("DEBUG: Failed to update widget button {}: {}", key_id, e);
                        }
                    }
                    Err(e) => {
                        eprintln!("DEBUG: Failed to generate widget image for button {}: {}", key_id, e);
                    }
                }
            }
        }
    }
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

    // Signal the button listener to refresh (which will apply new brightness)
    request_refresh();
    eprintln!("DEBUG: Brightness set to {}, refresh requested", brightness);

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
            Command::new("xdg-open")
                .arg(&url)
                .spawn()
                .ok();
        });
        return Ok(());
    }

    // Handle __TYPE_ command
    if cmd.starts_with("__TYPE_") {
        let text = cmd[7..].to_string();
        std::thread::spawn(move || {
            Command::new("ydotool")
                .args(["type", "--clearmodifiers", &text])
                .spawn()
                .ok();
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
                        Command::new("ydotool")
                            .args(["type", "--clearmodifiers", text])
                            .status()
                            .ok();
                    } else if trimmed.starts_with("__KEY_") {
                        let keys = &trimmed[6..];
                        execute_hotkey_sync(keys);
                    } else if trimmed.starts_with("__DELAY_") {
                        if let Ok(ms) = trimmed[8..].parse::<u64>() {
                            std::thread::sleep(Duration::from_millis(ms));
                        }
                    } else {
                        Command::new("sh")
                            .arg("-c")
                            .arg(trimmed)
                            .status()
                            .ok();
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
        });
        return Ok(());
    }

    // Execute normal shell command
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

        // URLs - Abrir páginas web
        ("YouTube".to_string(), "__URL_https://youtube.com".to_string(), "Abrir YouTube".to_string()),
        ("Twitch".to_string(), "__URL_https://twitch.tv".to_string(), "Abrir Twitch".to_string()),
        ("GitHub".to_string(), "__URL_https://github.com".to_string(), "Abrir GitHub".to_string()),
        ("Twitter/X".to_string(), "__URL_https://x.com".to_string(), "Abrir Twitter/X".to_string()),
        ("ChatGPT".to_string(), "__URL_https://chat.openai.com".to_string(), "Abrir ChatGPT".to_string()),
        ("Claude".to_string(), "__URL_https://claude.ai".to_string(), "Abrir Claude AI".to_string()),

        // Hotkeys - Atajos de teclado
        ("Copiar".to_string(), "__KEY_ctrl+c".to_string(), "Ctrl+C - Copiar".to_string()),
        ("Pegar".to_string(), "__KEY_ctrl+v".to_string(), "Ctrl+V - Pegar".to_string()),
        ("Cortar".to_string(), "__KEY_ctrl+x".to_string(), "Ctrl+X - Cortar".to_string()),
        ("Deshacer".to_string(), "__KEY_ctrl+z".to_string(), "Ctrl+Z - Deshacer".to_string()),
        ("Rehacer".to_string(), "__KEY_ctrl+shift+z".to_string(), "Ctrl+Shift+Z - Rehacer".to_string()),
        ("Guardar".to_string(), "__KEY_ctrl+s".to_string(), "Ctrl+S - Guardar".to_string()),
        ("Buscar".to_string(), "__KEY_ctrl+f".to_string(), "Ctrl+F - Buscar".to_string()),
        ("Seleccionar todo".to_string(), "__KEY_ctrl+a".to_string(), "Ctrl+A - Seleccionar todo".to_string()),
        ("Cerrar ventana".to_string(), "__KEY_alt+f4".to_string(), "Alt+F4 - Cerrar ventana".to_string()),
        ("Cambiar ventana".to_string(), "__KEY_alt+tab".to_string(), "Alt+Tab - Cambiar ventana".to_string()),
        ("Pantalla completa".to_string(), "__KEY_f11".to_string(), "F11 - Pantalla completa".to_string()),
        ("Emoji picker".to_string(), "__KEY_super+period".to_string(), "Super+. - Selector de emojis".to_string()),

        // Texto predefinido
        ("Email".to_string(), "__TYPE_tucorreo@ejemplo.com".to_string(), "Escribir email (editar)".to_string()),
        ("Saludo".to_string(), "__TYPE_¡Hola! ¿Cómo estás?".to_string(), "Escribir saludo".to_string()),
        ("Firma".to_string(), "__TYPE_Saludos cordiales".to_string(), "Escribir firma".to_string()),

        // Multi-acciones
        ("Abrir+Escribir".to_string(), "__MULTI_firefox;;__DELAY_2000;;__TYPE_https://google.com".to_string(), "Abrir Firefox y escribir URL".to_string()),
        ("Copy+Paste".to_string(), "__MULTI___KEY_ctrl+c;;__DELAY_500;;__KEY_ctrl+v".to_string(), "Copiar y pegar".to_string()),

        // Widgets - Fecha/Hora
        ("Reloj".to_string(), "__CLOCK__".to_string(), "Muestra hora actual (HH:MM)".to_string()),
        ("Reloj+Seg".to_string(), "__CLOCK_S__".to_string(), "Muestra hora con segundos".to_string()),
        ("Fecha".to_string(), "__DATE__".to_string(), "Muestra fecha (DD/MM)".to_string()),
        ("Fecha completa".to_string(), "__DATE_FULL__".to_string(), "Muestra fecha completa".to_string()),
        ("Día semana".to_string(), "__WEEKDAY__".to_string(), "Muestra día de la semana".to_string()),

        // Widgets - Sistema
        ("CPU %".to_string(), "__CPU__".to_string(), "Muestra uso de CPU".to_string()),
        ("RAM %".to_string(), "__RAM__".to_string(), "Muestra uso de RAM".to_string()),
        ("Temp CPU".to_string(), "__TEMP__".to_string(), "Muestra temperatura CPU".to_string()),

        // Widgets - Timer
        ("Timer 1m".to_string(), "__TIMER_1__".to_string(), "Temporizador 1 minuto".to_string()),
        ("Timer 5m".to_string(), "__TIMER_5__".to_string(), "Temporizador 5 minutos".to_string()),
        ("Timer 10m".to_string(), "__TIMER_10__".to_string(), "Temporizador 10 minutos".to_string()),
        ("Timer 15m".to_string(), "__TIMER_15__".to_string(), "Temporizador 15 minutos".to_string()),
        ("Timer 30m".to_string(), "__TIMER_30__".to_string(), "Temporizador 30 minutos".to_string()),

        // OBS Studio - WebSocket Control
        ("OBS Stream".to_string(), "__OBS_STREAM__".to_string(), "Iniciar/Detener streaming".to_string()),
        ("OBS Record".to_string(), "__OBS_RECORD__".to_string(), "Iniciar/Detener grabación".to_string()),
        ("OBS Mute".to_string(), "__OBS_MUTE__".to_string(), "Mutear/Desmutear micrófono".to_string()),
        ("OBS Status".to_string(), "__OBS_STATUS__".to_string(), "Widget: muestra LIVE/REC".to_string()),
        ("Escena 1".to_string(), "__OBS_SCENE_Scene".to_string(), "Cambiar a escena (editar nombre)".to_string()),
        ("Escena Gaming".to_string(), "__OBS_SCENE_Gaming".to_string(), "Cambiar a escena Gaming".to_string()),
        ("Escena Webcam".to_string(), "__OBS_SCENE_Webcam".to_string(), "Cambiar a escena Webcam".to_string()),
        ("Escena BRB".to_string(), "__OBS_SCENE_BRB".to_string(), "Cambiar a escena BRB".to_string()),

        // Twitch Integration
        ("Twitch Viewers".to_string(), "__TWITCH_VIEWERS__".to_string(), "Widget: muestra viewers actuales".to_string()),
        ("Twitch Followers".to_string(), "__TWITCH_FOLLOWERS__".to_string(), "Widget: muestra total followers".to_string()),
        ("Twitch Clip".to_string(), "__TWITCH_CLIP__".to_string(), "Crear clip del stream".to_string()),
        ("Ad 30s".to_string(), "__TWITCH_AD_30__".to_string(), "Comercial de 30 segundos".to_string()),
        ("Ad 60s".to_string(), "__TWITCH_AD_60__".to_string(), "Comercial de 60 segundos".to_string()),
        ("Ad 90s".to_string(), "__TWITCH_AD_90__".to_string(), "Comercial de 90 segundos".to_string()),
        ("Chat Hola".to_string(), "__TWITCH_CHAT_¡Hola chat!".to_string(), "Enviar mensaje al chat".to_string()),
        ("Chat BRB".to_string(), "__TWITCH_CHAT_BRB - Vuelvo en un momento".to_string(), "Enviar BRB al chat".to_string()),

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
