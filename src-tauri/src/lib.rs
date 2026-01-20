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
            save_icon,
            reset_config,
            list_icons,
            get_preset_commands,
            clear_page_buttons,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
