# Redragon Stream Deck - Contexto para Claude

## Trigger de Continuación
Cuando el usuario diga **"continuamos con la aplicación Tauri"** o similar, continuar con este proyecto.

## Proyecto
- **Tipo**: Aplicación de escritorio Tauri 2.x para Linux
- **Propósito**: Driver y panel de control para Redragon SS-550 Stream Deck
- **Rama activa**: `feature/tauri-desktop-app`

## Estado Actual (Enero 2025)

### ✅ Funcionando
- Comunicación USB con el dispositivo (endpoint OUT=0x01, IN=0x82)
- Envío de imágenes a botones (JPEG 100x100, rotadas 180°)
- Listener de botones físicos en background thread
- Ejecución de comandos al presionar botones
- Navegación entre páginas de botones
- Iconos mostrados tanto en dispositivo como en UI web
- Configuración persistente (brillo, páginas, comandos)

### ✅ Fase 1 - Funciones Avanzadas (Implementadas)
- **URLs**: Abrir páginas web directamente (`__URL_https://...`)
- **Texto**: Escribir texto con ydotool (`__TYPE_texto`)
- **Hotkeys**: Simular atajos de teclado (`__KEY_ctrl+shift+s`)
- **Multi-acción**: Secuencias de comandos (`__MULTI_cmd1;;cmd2;;cmd3`)
- **Delays**: Pausas en multi-acción (`__DELAY_1000`)
- Botón "Probar comando" en la UI
- Ayuda contextual de comandos especiales

### ✅ Fase 2 - Widgets Dinámicos (Implementadas)
- **Reloj**: `__CLOCK__`, `__CLOCK_S__` (con segundos)
- **Fecha**: `__DATE__`, `__DATE_FULL__`, `__WEEKDAY__`
- **Sistema**: `__CPU__`, `__RAM__`, `__TEMP__`
- **Timer**: `__TIMER_N__` (N = minutos, toggle al presionar)
- Actualización automática cada ~1 segundo

### ✅ Fase 3 - Integraciones Streaming (Implementadas)
- **OBS Studio** (WebSocket 5.x):
  - `__OBS_STREAM__` - Iniciar/Detener streaming
  - `__OBS_RECORD__` - Iniciar/Detener grabación
  - `__OBS_MUTE__` - Mutear/Desmutear micrófono
  - `__OBS_SCENE_nombre` - Cambiar escena
  - `__OBS_STATUS__` - Widget que muestra LIVE/REC
- **Twitch API**:
  - `__TWITCH_VIEWERS__` - Widget con viewers actuales
  - `__TWITCH_FOLLOWERS__` - Widget con total de followers
  - `__TWITCH_CLIP__` - Crear clip
  - `__TWITCH_AD_N__` - Correr comercial (N = 30, 60, 90 segundos)
  - `__TWITCH_CHAT_mensaje` - Enviar mensaje al chat

### 🔧 Arquitectura
```
Frontend (public/app-tauri.js)
    ↓ invoke()
Backend Rust (src-tauri/src/lib.rs)
    ↓ rusb / WebSocket / HTTP
Dispositivo USB (VID=0x0200, PID=0x1000)
OBS Studio (ws://localhost:4455)
Twitch API (api.twitch.tv/helix)
```

### 📁 Archivos Clave
- `src-tauri/src/lib.rs` - Toda la lógica Rust
- `public/app-tauri.js` - Interfaz web
- `public/index.html` - HTML de la UI
- `public/style.css` - Estilos CSS
- `src-tauri/Cargo.toml` - Dependencias

### 🚀 Comandos
```bash
# Compilar
cargo build --release --manifest-path src-tauri/Cargo.toml

# Ejecutar
./src-tauri/target/release/redragon-streamdeck

# Ejecutar con OBS/Twitch (variables de entorno)
OBS_WEBSOCKET_PASSWORD=tupass TWITCH_CLIENT_ID=xxx TWITCH_ACCESS_TOKEN=xxx TWITCH_CHANNEL=tucanal ./src-tauri/target/release/redragon-streamdeck
```

## Comandos Especiales Disponibles

### Comandos Básicos
| Comando | Formato | Ejemplo |
|---------|---------|---------|
| **URL** | `__URL_direccion` | `__URL_https://youtube.com` |
| **Texto** | `__TYPE_texto` | `__TYPE_Hola mundo` |
| **Hotkey** | `__KEY_teclas` | `__KEY_ctrl+shift+s` |
| **Multi-acción** | `__MULTI_cmd1;;cmd2` | `__MULTI_firefox;;__DELAY_2000;;__KEY_ctrl+t` |
| **Delay** | `__DELAY_ms` | `__DELAY_1000` (solo dentro de MULTI) |
| **Página siguiente** | `__NEXT_PAGE__` | |
| **Página anterior** | `__PREV_PAGE__` | |
| **Ir a página N** | `__PAGE_N__` | `__PAGE_0__` |

### Widgets (Actualización Automática)
| Comando | Descripción |
|---------|-------------|
| `__CLOCK__` | Hora HH:MM |
| `__CLOCK_S__` | Hora HH:MM:SS |
| `__DATE__` | Fecha DD/MM |
| `__DATE_FULL__` | Fecha DD/MM/YYYY |
| `__WEEKDAY__` | Día de la semana |
| `__CPU__` | Uso de CPU % |
| `__RAM__` | Uso de RAM % |
| `__TEMP__` | Temperatura CPU |
| `__TIMER_N__` | Temporizador N minutos |
| `__OBS_STATUS__` | Estado OBS (LIVE/REC) |
| `__TWITCH_VIEWERS__` | Viewers actuales |
| `__TWITCH_FOLLOWERS__` | Total followers |

### OBS Studio
| Comando | Descripción |
|---------|-------------|
| `__OBS_STREAM__` | Toggle streaming |
| `__OBS_RECORD__` | Toggle grabación |
| `__OBS_MUTE__` | Toggle mute micrófono |
| `__OBS_SCENE_Gaming` | Cambiar a escena "Gaming" |

### Twitch
| Comando | Descripción |
|---------|-------------|
| `__TWITCH_CLIP__` | Crear clip |
| `__TWITCH_AD_30__` | Comercial 30 segundos |
| `__TWITCH_AD_60__` | Comercial 60 segundos |
| `__TWITCH_CHAT_Hola!` | Enviar "Hola!" al chat |

### Teclas Soportadas para __KEY_
- **Modificadores**: ctrl, shift, alt, super/win/meta, rctrl, rshift, ralt
- **Función**: f1-f12
- **Especiales**: esc, tab, enter, space, backspace, delete, insert, home, end, pageup, pagedown
- **Flechas**: up, down, left, right
- **Letras**: a-z
- **Números**: 0-9
- **Media**: volumeup, volumedown, mute, playpause, next, prev
- **Numpad**: kp0-kp9, kpenter, kpplus, kpminus, kpmultiply, kpdivide, kpdot

## Configuración de Integraciones

### OBS Studio
Variables de entorno:
```bash
OBS_WEBSOCKET_URL=ws://localhost:4455  # Opcional, default localhost:4455
OBS_WEBSOCKET_PASSWORD=tupassword       # Si OBS tiene password configurado
```

En OBS: Tools > WebSocket Server Settings > Enable WebSocket server

### Twitch
Variables de entorno:
```bash
TWITCH_CLIENT_ID=tu_client_id
TWITCH_ACCESS_TOKEN=tu_access_token
TWITCH_CHANNEL=tu_nombre_de_canal
```

Para obtener tokens de Twitch:
1. Crear aplicación en https://dev.twitch.tv/console
2. Obtener Client ID
3. Generar Access Token con scopes: `channel:manage:broadcast`, `clips:edit`, `chat:edit`, `channel:read:subscriptions`

## Posibles Tareas Futuras
- Clima/Tiempo actual
- Auto-inicio con systemd
- Crear instaladores (.deb, .rpm, .AppImage)
- Integración con Spotify
- Soporte para perfiles por aplicación
