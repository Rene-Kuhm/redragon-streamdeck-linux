# Redragon Stream Deck Linux

Driver y panel de control open source para **Redragon SS-550 Stream Deck** en Linux.

![License](https://img.shields.io/badge/license-MIT-green)
![Platform](https://img.shields.io/badge/platform-Linux-blue)
![Tauri](https://img.shields.io/badge/Tauri-2.x-blue)
![Rust](https://img.shields.io/badge/Rust-1.70+-orange)

## Características

### Funciones Básicas
- Interfaz gráfica nativa (Tauri/GTK)
- Soporte para múltiples páginas de botones
- Iconos personalizados (100x100)
- Ejecución de comandos del sistema
- Control de brillo
- Navegación entre páginas con botones físicos
- Compatible con Wayland (Hyprland, Sway, GNOME) y X11

### Funciones Avanzadas
- **URLs**: Abrir páginas web con un botón
- **Texto**: Escribir texto automáticamente (ydotool)
- **Hotkeys**: Simular atajos de teclado (Ctrl+C, Alt+Tab, etc.)
- **Multi-acción**: Secuencias de comandos con delays

### Widgets Dinámicos (actualización automática)
- **Reloj**: Hora actual (con/sin segundos)
- **Fecha**: Día, mes, año, día de la semana
- **Sistema**: CPU%, RAM%, Temperatura
- **Temporizador**: Cuenta regresiva configurable

### Integraciones de Streaming
- **OBS Studio** (WebSocket 5.x):
  - Iniciar/detener streaming y grabación
  - Cambiar escenas
  - Mutear/desmutear micrófono
  - Widget de estado en tiempo real
- **Twitch API**:
  - Mostrar viewers y followers en botones
  - Crear clips con un clic
  - Correr comerciales
  - Enviar mensajes al chat

## Instalación

```bash
git clone https://github.com/Rene-Kuhm/redragon-streamdeck-linux-.git
cd redragon-streamdeck-linux-
./install.sh
```

El instalador detecta la distribución leyendo `/etc/os-release`, instala las
dependencias, compila, configura la regla udev y ydotool, y deja la aplicación
lista para arrancar sola al iniciar sesión.

**Distribuciones soportadas:** Fedora/RHEL, Debian/Ubuntu, Arch y openSUSE.
Los derivados (Linux Mint, Pop!_OS, CachyOS, EndeavourOS, Nobara...) funcionan
automáticamente a través de `ID_LIKE`, sin necesidad de una entrada propia.

En una distribución no listada, instalá las dependencias a mano y usá
`./install.sh --skip-deps`.

### Opciones

| Opción | Efecto |
|---|---|
| `--build-only` | Solo compila, no toca el sistema |
| `--skip-deps` | No instala dependencias del sistema |
| `--no-autostart` | No configura el arranque automático |
| `--daemon-only` | Solo el daemon, sin interfaz gráfica |

### Uso sin entorno gráfico

El proyecto se divide en un daemon que maneja el dispositivo y una interfaz
gráfica opcional para configurarlo. El daemon no depende de Tauri, GTK ni
webkit, así que funciona en un equipo sin escritorio, por SSH o en un servidor:

```bash
./install.sh --daemon-only
```

Ambos comparten el mismo `config.json`, de modo que podés configurar los botones
con la aplicación en un equipo y después dejar corriendo solo el daemon. No
pueden usar el dispositivo a la vez: los units de systemd lo declaran con
`Conflicts=`.

```bash
systemctl --user disable --now redragon-streamdeck   # apagar la interfaz
systemctl --user enable  --now redragon-daemon       # usar solo el daemon
```

## Uso

### Ejecutar la aplicación

```bash
redragon-streamdeck
```

O busca "Redragon Stream Deck" en el menú de aplicaciones.

### Auto-inicio

Para iniciar la app automáticamente al iniciar sesión:

```bash
./install.sh --enable-autostart
```

Comandos útiles:

```bash
./install.sh --autostart-status
./install.sh --disable-autostart
```

### Configurar un botón

1. Haz clic en cualquier botón en la interfaz
2. Configura:
   - **Etiqueta**: Texto que se muestra
   - **Comando**: Acción a ejecutar
   - **Color**: Color de fondo
   - **Icono**: Imagen personalizada

### Comandos Especiales

| Categoría | Comando | Descripción |
|-----------|---------|-------------|
| **Navegación** | `__NEXT_PAGE__` | Página siguiente |
| | `__PREV_PAGE__` | Página anterior |
| | `__PAGE_0__` | Ir a página específica |
| **URLs** | `__URL_https://youtube.com` | Abrir URL |
| **Texto** | `__TYPE_Hola mundo` | Escribir texto |
| **Hotkeys** | `__KEY_ctrl+shift+s` | Simular teclas |
| **Multi** | `__MULTI_cmd1;;cmd2` | Secuencia de comandos |
| **Widgets** | `__CLOCK__` | Reloj HH:MM |
| | `__CPU__` | Uso de CPU |
| | `__RAM__` | Uso de RAM |
| | `__TIMER_5__` | Timer 5 minutos |
| **OBS** | `__OBS_STREAM__` | Toggle streaming |
| | `__OBS_RECORD__` | Toggle grabación |
| | `__OBS_SCENE_Gaming` | Cambiar escena |
| **Twitch** | `__TWITCH_VIEWERS__` | Mostrar viewers |
| | `__TWITCH_CLIP__` | Crear clip |

Ver [CLAUDE.md](CLAUDE.md) para la lista completa de comandos.

## Configurar Integraciones

### OBS Studio

1. En OBS: **Tools > WebSocket Server Settings**
2. Habilitar "Enable WebSocket server"
3. (Opcional) Configurar password

```bash
# Ejecutar con password de OBS
OBS_WEBSOCKET_PASSWORD="tu_password" redragon-streamdeck
```

### Twitch

1. Crear app en https://dev.twitch.tv/console
2. Configurar variables de entorno:

```bash
export TWITCH_CLIENT_ID="tu_client_id"
export TWITCH_ACCESS_TOKEN="tu_token"
export TWITCH_CHANNEL="tu_canal"
```

## Distribución de Botones

```
┌────┬────┬────┬────┬────┐
│ 11 │ 12 │ 13 │ 14 │ 15 │  ← Fila superior
├────┼────┼────┼────┼────┤
│  6 │  7 │  8 │  9 │ 10 │  ← Fila media
├────┼────┼────┼────┼────┤
│  1 │  2 │  3 │  4 │  5 │  ← Fila inferior
└────┴────┴────┴────┴────┘
```

## Estructura del Proyecto

```
redragon-streamdeck-linux/
├── src-tauri/
│   ├── src/lib.rs     # Backend Rust (USB, OBS, Twitch)
│   └── Cargo.toml     # Dependencias Rust
├── public/
│   ├── index.html     # Interfaz gráfica
│   ├── app-tauri.js   # JavaScript frontend
│   └── style.css      # Estilos
├── install.sh         # Instalador (detecta la distribución)
├── redragon-streamdeck.service # Servicio systemd de usuario para auto-inicio
├── uninstall.sh       # Desinstalador
└── CLAUDE.md          # Documentación de comandos
```

## Solución de Problemas

### El Stream Deck no se detecta

```bash
# Verificar conexión
lsusb | grep "0200:1000"

# Verificar reglas udev
cat /etc/udev/rules.d/99-redragon-streamdeck.rules

# Desconectar y reconectar el dispositivo
```

### Los hotkeys no funcionan

```bash
# Verificar ydotoold
systemctl status ydotoold.service

# Verificar grupo input
groups | grep input

# Verificar que el socket sea accesible por el grupo input
stat -c '%a %U:%G %n' /tmp/.ydotool_socket

# Si no estás en el grupo, agrégarte y reiniciar sesión
sudo usermod -aG input $USER
```

El socket de `ydotoold` debe verse así:

```text
660 root:input /tmp/.ydotool_socket
```

Si aparece como `root:root`, reinstala o reinicia el servicio generado por `install.sh`:

```bash
sudo cp ydotoold.service /etc/systemd/system/ydotoold.service
sudo systemctl daemon-reload
sudo systemctl restart ydotoold.service
```

También puedes probar el acceso directamente:

```bash
YDOTOOL_SOCKET=/tmp/.ydotool_socket ydotool type ""
YDOTOOL_SOCKET=/tmp/.ydotool_socket ydotool key 29:1 29:0
```

### Error "Interface Busy"

```bash
# Buscar procesos usando el dispositivo
pkill -f redragon

# Reiniciar la aplicación
redragon-streamdeck
```

## Desinstalar

```bash
./uninstall.sh
```

## Dispositivos Compatibles

- Redragon SS-550 (USB ID: 0200:1000)
- Posiblemente otros dispositivos basados en StreamDock/Mirabox

## Contribuir

¡Las contribuciones son bienvenidas!

1. Fork el repositorio
2. Crea una rama: `git checkout -b mi-feature`
3. Haz commit: `git commit -m 'Agregar feature'`
4. Push: `git push origin mi-feature`
5. Abre un Pull Request

## Créditos

- **Tecnodespegue** - Desarrollo y mantenimiento
- Basado en el protocolo de [mirabox-streamdock-node](https://github.com/nicross/mirabox-streamdock-node)
- Desarrollado con ayuda de Claude AI

## Licencia

Este proyecto está bajo la licencia MIT. Ver [LICENSE](LICENSE) para más detalles.

---

⭐ Si este proyecto te fue útil, ¡dale una estrella en GitHub!
