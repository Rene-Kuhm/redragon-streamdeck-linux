//! Daemon sin interfaz para el Redragon Stream Deck.
//!
//! Habla con el dispositivo y ejecuta los comandos de los botones, y nada mas.
//! No depende de Tauri, GTK ni webkit, asi que compila y corre en cualquier
//! Linux — incluido un equipo sin entorno grafico, por SSH o en un servidor.
//!
//! La configuracion es el mismo `config.json` que usa la GUI, de modo que se
//! puede configurar con la aplicacion y despues dejar corriendo solo el daemon.

use std::path::PathBuf;
use std::process::ExitCode;

use redragon_core::{start_button_listener, AppState, PRODUCT_ID, VENDOR_ID};

const USAGE: &str = "\
redragon-daemon - driver del Redragon Stream Deck, sin interfaz grafica

USO:
    redragon-daemon [OPCIONES]

OPCIONES:
    -c, --config-dir <RUTA>   Directorio de configuracion
                              (por defecto: ~/.local/share/com.tecnodespegue.redragon-streamdeck)
    -v, --verbose             Muestra los mensajes de diagnostico
                              (equivale a REDRAGON_STREAMDECK_DEBUG=1)
    -h, --help                Esta ayuda

ENTORNO:
    REDRAGON_STREAMDECK_DEBUG   Si esta definida, activa el diagnostico
    YDOTOOL_SOCKET              Socket de ydotoold para las acciones de teclado
";

fn default_config_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("com.tecnodespegue.redragon-streamdeck"))
}

fn parse_args() -> Result<PathBuf, ExitCode> {
    let mut args = std::env::args().skip(1);
    let mut config_dir = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                print!("{USAGE}");
                return Err(ExitCode::SUCCESS);
            }
            "-v" | "--verbose" => {
                // La macro debug_log! del core lee esta variable.
                std::env::set_var("REDRAGON_STREAMDECK_DEBUG", "1");
            }
            "-c" | "--config-dir" => match args.next() {
                Some(path) => config_dir = Some(PathBuf::from(path)),
                None => {
                    eprintln!("error: {arg} necesita una ruta");
                    return Err(ExitCode::FAILURE);
                }
            },
            other => {
                eprintln!("error: opcion desconocida: {other}");
                eprintln!("Proba con --help");
                return Err(ExitCode::FAILURE);
            }
        }
    }

    match config_dir.or_else(default_config_dir) {
        Some(dir) => Ok(dir),
        None => {
            eprintln!("error: no se pudo determinar el directorio de configuracion");
            eprintln!("Indicalo con --config-dir");
            Err(ExitCode::FAILURE)
        }
    }
}

fn main() -> ExitCode {
    let config_dir = match parse_args() {
        Ok(dir) => dir,
        Err(code) => return code,
    };

    if let Err(e) = std::fs::create_dir_all(&config_dir) {
        eprintln!("error: no se pudo crear {}: {e}", config_dir.display());
        return ExitCode::FAILURE;
    }

    // AppState escribe un config.json por defecto si todavia no existe.
    let state = AppState::new(config_dir.clone());
    let config_path = state.config_path.clone();
    let icons_path = state.icons_path.clone();

    eprintln!(
        "redragon-daemon: dispositivo {VENDOR_ID:04x}:{PRODUCT_ID:04x}, configuracion en {}",
        config_path.display()
    );

    // El listener reintenta la conexion solo, asi que arrancar sin el
    // dispositivo enchufado es valido: lo toma cuando aparece.
    start_button_listener(config_path, icons_path);

    // start_button_listener corre en su propio hilo; este se queda dormido
    // para no terminar el proceso.
    loop {
        std::thread::park();
    }
}
